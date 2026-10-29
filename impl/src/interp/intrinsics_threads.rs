// The intrinsics: lock, spawn, cancel and join.

use super::*;

impl Interp {
    pub(super) fn intrinsic_threads(
        &mut self,
        name: &str,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        match name {
            "lock" => {
                let m = self.eval_to_value(&args[0], subst)?;
                if let Value::Ref { obj, path, pointee, token: via, .. } = m {
                    // [Lock]/[Lock-Reentrant] (spec/19 §2): held by no one
                    // -> acquire; held by *this* real CobaltC thread ->
                    // fault (reentrant, not a deadlock); held by another
                    // -> genuinely block until it releases (real OS
                    // scheduling now decides how long, not a simulation).
                    let me = self.current_thread;
                    let key = (obj, path.clone());
                    if self.shared.lock().unwrap().cobalt_locks.get(&key) == Some(&me) {
                        return Err(Flow::Fault("diag.mutex-reentrant-lock".into()));
                    }
                    if self.shared.lock().unwrap().poisoned.contains(&key) {
                        return Err(Flow::Fault("diag.mutex-poisoned".into()));
                    }
                    let k2 = key.clone();
                    self.wait_for(Wait::Lock(k2), |s| !s.cobalt_locks.contains_key(&key))?;
                    if self.shared.lock().unwrap().poisoned.contains(&key) {
                        return Err(Flow::Fault("diag.mutex-poisoned".into()));
                    }
                    self.shared.lock().unwrap().cobalt_locks.insert(key, me);
                    let mut gpath = path.clone();
                    gpath.push(Proj::Field(0));
                    let inner = if let Type::Mutex(inner) = &pointee { (**inner).clone() } else { Type::Void };
                    // D-0204: an interior that may hold an exclusive reference
                    // (a channel of them) lends its locker another's data.
                    if me != 0 && crate::typecheck::may_hold_excl_ref(&self.items, &inner) {
                        self.shared.lock().unwrap().lent_excl.insert(me);
                    }
                    let gty = Type::Guard(Box::new(inner.clone()));
                    let token = self.mint_token(&Mode::Exclusive);
                    // The lock path's base is the locking reference
                    // (`[Lock]`): it and its ancestors are the guard's.
                    let anc = self.record_token_ancestors(token, &[via]);
                    let id = self.new_object(gty, Value::Guard { obj, path: gpath, inner, token, anc });
                    // A temporary guard (`*lock(&m)`) ends with its
                    // statement, unlocking the mutex; bound (`auto g =
                    // lock(&m)`), it is its binding's.
                    self.stmt_temp(id);
                    return Ok(Some(EvalResult::Temp(id)));
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
            "Mutex::new" => {
                let hint = targs.first().map(|t| self.apply_subst(t, subst));
                return self.mutex_new(&args[0], subst, hint.as_ref()).map(Some);
            }
            // D-0249 `rule.conc.atomic`, std-only: `atomic_op(a, op, x, y)`,
            // one step under the interpreter lock. `a` is checked as a
            // shared access of the atomic; the operation forms no path into
            // it, and reads and writes its one field in place, giving the
            // value it found.
            "atomic_op" => {
                let Value::Ref { obj, path, token, anc, .. } = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let int = |v: Value| match v {
                    Value::Int(_, x) => Ok(x),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let op = int(self.eval_to_value(&args[1], subst)?)?;
                let xv = self.eval_to_value(&args[2], subst)?;
                let yv = self.eval_to_value(&args[3], subst)?;
                self.affine(token, false)?;
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                let mut chain = vec![token];
                if let Some(a) = &anc {
                    chain.extend(a.iter().copied());
                }
                if self.clash(obj, &path, Mode::Shared, &chain) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let mut cell = path.clone();
                cell.push(Proj::Field(0));
                // D-0253: an `AtomicPtr`'s cell, a raw pointer: load (0),
                // store (1), swap (2), compare-and-exchange (3) by address.
                if let Value::Rawptr(old, pt) = self.read_place(obj, &cell) {
                    let addr = |v: &Value| match v {
                        Value::Rawptr(a, _) => Ok(*a),
                        _ => Err(Flow::Fault("diag.type-mismatch".into())),
                    };
                    let new = match op {
                        0 => None,
                        1 | 2 => Some(xv.clone()),
                        3 => (old == addr(&xv)?).then(|| yv.clone()),
                        _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                    };
                    if let Some(n) = new {
                        self.write_place(obj, &cell, n);
                    }
                    return Ok(Some(EvalResult::Val(Value::Rawptr(old, pt))));
                }
                let x = int(xv)?;
                let y = int(yv)?;
                let Value::Int(t, old) = self.read_place(obj, &cell) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let new = match op {
                    0 => None,
                    1 | 2 => Some(x),
                    3 => (old == x).then_some(y),
                    4 | 5 => {
                        let r = if op == 4 { old.checked_add(x) } else { old.checked_sub(x) };
                        match r {
                            Some(r) if crate::value::in_range(t, r) => Some(r),
                            _ => return Err(Flow::Fault("diag.arith-overflow".into())),
                        }
                    }
                    6 => Some(crate::value::wrapping_op(t, old, x, '+')),
                    7 => Some(crate::value::wrapping_op(t, old, x, '-')),
                    8 => Some(old & x),
                    9 => Some(old | x),
                    10 => Some(old ^ x),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if let Some(n) = new {
                    self.write_place(obj, &cell, Value::Int(t, n));
                }
                return Ok(Some(EvalResult::Val(Value::Int(t, old))));
            }
            // D-0253, std-only: `atomic_wait_op(a, op, seen)`, the wake count
            // of `a`'s bucket (its object and path, hashed into 256): 0 reads
            // it, 1 waits until it differs from `seen` (`[Deadlock]` when no
            // thread can end the wait), 2 and 3 add one, which wakes every
            // waiter of the bucket; each checks its own value again.
            "atomic_wait_op" => {
                let Value::Ref { obj, path, token, anc, .. } = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let int = |v: Value| match v {
                    Value::Int(_, x) => Ok(x as u64),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let op = int(self.eval_to_value(&args[1], subst)?)?;
                let seen = int(self.eval_to_value(&args[2], subst)?)?;
                self.affine(token, false)?;
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                let mut chain = vec![token];
                if let Some(a) = &anc {
                    chain.extend(a.iter().copied());
                }
                if self.clash(obj, &path, Mode::Shared, &chain) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let bucket = {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    (obj, &path).hash(&mut h);
                    h.finish() % 256
                };
                let count = |s: &Shared| s.atomic_counts.get(&bucket).copied().unwrap_or(0);
                let r = match op {
                    0 => count(&self.shared.lock().unwrap()),
                    1 => {
                        self.wait_for(Wait::Atomic(bucket, seen), |s| count(s) != seen)?;
                        count(&self.shared.lock().unwrap())
                    }
                    _ => {
                        let mut sh = self.shared.lock().unwrap();
                        let c = sh.atomic_counts.entry(bucket).or_insert(0);
                        *c = c.wrapping_add(1);
                        *c
                    }
                };
                return Ok(Some(EvalResult::Val(Value::Int(IntTy::U64, r as i128))));
            }
            // D-0253, std-only: `cpu_op(op)`, a fence (0) or a spin hint (1).
            // Every step here is already sequentially consistent, and the
            // interpreter passes the turn between statements: nothing to do.
            "cpu_op" => {
                _ = self.eval_to_value(&args[0], subst)?;
                return Ok(Some(EvalResult::Val(Value::Unit)));
            }
            "spawn" => self.intrinsic_spawn(args, subst),
            "cancel" => {
                // D-0204: the thread's current or next wait faults
                // (`diag.thread-cancelled`), unless it was lent an
                // exclusive reference.
                let h = self.eval_to_value(&args[0], subst)?;
                if let Value::Ref { obj, path, .. } = h {
                    if let Value::Handle(k, _) = self.read_place(obj, &path) {
                        request_cancel(&self.shared, k);
                    }
                    return Ok(Some(EvalResult::Val(Value::Unit)));
                }
                return Err(Flow::Fault("diag.type-mismatch".into()));
            }
            "join_status" => {
                // D-0204 (`std::try_join`'s and `std::is_finished`'s
                // primitive): 0 while the thread runs, 1 when it finished
                // (its result left for `join`), its failure's id plus 2
                // when it failed. Mode 0 waits for the end and takes a
                // failure, so the handle's end raises nothing; mode 1
                // only looks.
                let h = self.eval_to_value(&args[0], subst)?;
                let mode = match self.eval_to_value(&args[1], subst)? {
                    Value::Int(_, m) => m,
                    _ => 0,
                };
                let Value::Ref { obj, path, .. } = h else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                let Value::Handle(key, _) = self.read_place(obj, &path) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                if mode == 0 {
                    self.wait_for(Wait::Join(key), |s| matches!(s.thread_status.get(&key), Some(ThreadStatus::Done(..)) | Some(ThreadStatus::Failed(..))))?;
                }
                let mut s = self.shared.lock().unwrap();
                let r = match s.thread_status.get(&key) {
                    Some(ThreadStatus::Failed(fid, _)) => {
                        let fid = *fid;
                        if mode == 0 {
                            s.thread_status.insert(key, ThreadStatus::Taken);
                        }
                        fid + 2
                    }
                    Some(ThreadStatus::Running) => 0,
                    _ => 1,
                };
                return Ok(Some(EvalResult::Val(Value::Int(IntTy::U64, r as i128))));
            }
            "join" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path) = match r {
                    EvalResult::Place(o, p, _) => (o, p),
                    EvalResult::Temp(o) => (o, vec![]),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let hv = self.read_place(obj, &path);
                let key = match hv {
                    Value::Handle(k, _) => k,
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                // [Join] (spec/19 §1): blocks (no step) until the thread
                // is done, then claims the result (`taken`) so a second
                // join/handle-destructor sees nothing left to discard.
                if matches!(self.shared.lock().unwrap().thread_status.get(&key), Some(ThreadStatus::Taken)) {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                self.wait_for(Wait::Join(key), |s| matches!(s.thread_status.get(&key), Some(ThreadStatus::Done(..)) | Some(ThreadStatus::Failed(..))))?;
                let taken = self.shared.lock().unwrap().thread_status.insert(key, ThreadStatus::Taken);
                if path.is_empty() {
                    self.invalidate_binding_pointing_at(obj);
                    self.remove_object(&obj);
                }
                let (rv, rty) = match taken {
                    Some(ThreadStatus::Done(v, t)) => (v, t),
                    // D-0204: `[Join-Failed]` raises the thread's fault
                    // again, here (`try_join` asks first: `join_status`).
                    Some(ThreadStatus::Failed(fid, _)) => {
                        let d = self.shared.lock().unwrap().failures[fid as usize].0.clone();
                        return Err(Flow::Fault(d));
                    }
                    _ => unreachable!("checked Done or Failed above"),
                };
                // A resource, or an aggregate (which `match` and field
                // access need typed), comes back as a temporary of the
                // thread's result type, ending with the joiner's statement
                // unless something takes it. `[Join]` re-keys a resource's
                // authority to the joiner: `new_object` grants it to the
                // current thread.
                if self.is_resource(&rty) || matches!(rv, Value::Struct(_) | Value::Enum(..) | Value::Array(_)) {
                    let id = self.new_object(rty, rv);
                    self.stmt_temp(id);
                    return Ok(Some(EvalResult::Temp(id)));
                }
                return Ok(Some(EvalResult::Val(rv)));
            }
            _ => self.intrinsic_runtime(name, targs, args, subst),
        }
    }

    // `spawn`'s arm of the intrinsics.
    pub(super) fn intrinsic_spawn(&mut self, args: &[Expr], subst: &HashMap<String, Type>) -> Result<Option<EvalResult>, Flow> {
        // [Spawn] (spec/19 §1): a genuine new OS thread, sharing
        // this same `Interp` behind `self_shared`'s lock. `e_f`,
        // `e1..en` are evaluated here, in the spawning thread,
        // *before* the new thread exists -- matching the rule's
        // own left-to-right evaluation in `Sigma` prior to
        // `ell'` being minted.
        let fv = self.eval(&args[0], subst)?;
        // Each argument takes its parameter's type as the expected
        // type, as for a call (`spawn(work, 1)` with `fn work(u64 n)`
        // passes a `u64`), when that type is concrete.
        let expected: Vec<Option<Type>> = match &fv {
            EvalResult::Val(Value::FnVal(name)) => match self.items.fns.get(name).cloned() {
                Some(fd) => fd.params.iter().map(|p| if mentions_type_param(&p.ty, &fd.type_params) { None } else { Some(p.ty.clone()) }).collect(),
                None => Vec::new(),
            },
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => match self.objects.get(o).map(|ob| ob.ty.clone()) {
                Some(Type::Closure(cid)) => {
                    let cparams = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
                    cparams.iter().map(|p| Some(self.apply_subst(&p.ty, subst))).collect()
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        let mut argvals = Vec::new();
        for (ai, a) in args[1..].iter().enumerate() {
            let r = match expected.get(ai) {
                Some(Some(t)) => self.eval_expected(a, subst, t)?,
                _ => self.eval(a, subst)?,
            };
            // The argument is taken now, in this thread (`[Spawn]`
            // evaluates it before the new thread exists): a plain
            // place is read, a resource moved out of its binding.
            // Left a place, it was read or moved only when the new
            // thread first ran -- by which time the spawning
            // block could have ended it (a host panic).
            let r = match r {
                EvalResult::Place(o, path, excl) => {
                    let ty = self.type_at(o, &path);
                    if !self.is_resource(&ty) {
                        EvalResult::Val(self.result_to_value(EvalResult::Place(o, path, excl))?)
                    } else {
                        if !path.is_empty() {
                            return Err(Flow::Fault("diag.move-out-of-field".into()));
                        }
                        if !excl.is_empty() || !self.solitary(o, &excl) {
                            return Err(Flow::Fault("diag.move-while-aliased".into()));
                        }
                        if self.objects.get(&o).map_or(false, |ob| ob.owner_thread != self.current_thread) {
                            return Err(Flow::Fault("diag.transfer-without-authority".into()));
                        }
                        self.invalidate_binding_pointing_at(o);
                        for f in self.frames_mut().iter_mut() {
                            f.owned.retain(|x| *x != o);
                        }
                        for t in self.stmt_temps.values_mut().flat_map(|s| s.iter_mut()) {
                            t.retain(|x| *x != o);
                        }
                        self.temp_index.remove(&o);
                        EvalResult::Temp(o)
                    }
                }
                EvalResult::Temp(o) => {
                    // Not this statement's to end: the thread adopts it.
                    for t in self.stmt_temps.values_mut().flat_map(|s| s.iter_mut()) {
                        t.retain(|x| *x != o);
                    }
                    self.temp_index.remove(&o);
                    EvalResult::Temp(o)
                }
                other => other,
            };
            argvals.push(r);
        }
        // D-0213 `[Thread-Lend]`: the exclusive references the
        // arguments carry are lent to the new thread.
        for r in argvals.iter_mut() {
            match r {
                EvalResult::Val(v) => self.lend_value(v)?,
                EvalResult::Temp(o) => self.lend_object(*o)?,
                _ => {}
            }
        }
        // Resolve the callee to its body + parameter bindings now
        // (a `fn` value or a `move` closure, spec/19's own
        // requirement) so the new thread's own dispatch loop
        // needs nothing but this already-evaluated data.
        // The callee's declared result type, when it is known here:
        // the thread's result is typed by it (`[Join]`), since a
        // struct value alone does not say which struct it is.
        let mut declared_ret: Option<Type> = None;
        // The body as the static pass saw it, never a copy: its
        // records are keyed by the address of each expression
        // (a copied body lost them, so a view formed in a thread
        // was a type mismatch).
        let (body, callee_subst, params, closure_bind): (
            SpawnBody,
            HashMap<String, Type>,
            Vec<(String, EvalResult, Option<Type>)>,
            Option<(u64, Vec<String>)>,
        ) = match &fv {
            EvalResult::Val(Value::FnVal(name)) if name == "$moved" => return Err(Flow::Fault("diag.stale-binding".into())),
            EvalResult::Val(Value::FnVal(name)) => {
                let fdecl = self
                    .items
                    .fns
                    .get(name)
                    .cloned()
                    .ok_or_else(|| Flow::Fault("diag.unbound-name".into()))?;
                let inferred = self.infer_type_params(&fdecl, &[], &argvals);
                self.ensure_checked(&fdecl, &inferred)?;
                let mut ps = Vec::new();
                for (i, p) in fdecl.params.iter().enumerate() {
                    let pty = self.apply_subst(&p.ty, &inferred);
                    ps.push((p.name.clone(), argvals[i].clone(), Some(pty)));
                }
                let ret = self.apply_subst(&fdecl.ret, &inferred);
                if !mentions_type_param(&ret, &fdecl.type_params) {
                    declared_ret = Some(ret);
                }
                (SpawnBody::Fn(fdecl), inferred, ps, None)
            }
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                let obj = *o;
                let cid = match self.objects.get(&obj).map(|ob| ob.ty.clone()) {
                    Some(Type::Closure(id)) => id,
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let info = self
                    .closures
                    .borrow()
                    .get(&cid)
                    .cloned()
                    .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                // rule.conc.spawn (spec/19 §1): the callee must be
                // "a fn value or a move closure" -- a borrowing
                // closure could let the spawned thread outlive
                // data it only holds a reference to.
                if !info.is_move {
                    return Err(Flow::Fault("diag.spawn-borrow-closure".into()));
                }
                let cparams = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
                declared_ret = self.closure_rets.borrow().get(&cid).map(|r| self.apply_subst(r, &info.subst));
                if argvals.len() != cparams.len() {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                let mut ps = Vec::new();
                for (i, p) in cparams.iter().enumerate() {
                    let pty = self.apply_subst(&p.ty, subst);
                    ps.push((p.name.clone(), argvals[i].clone(), Some(pty)));
                }
                // The new thread gets its own closure object, which
                // it ends (`run_body`): a closure is a struct
                // (spec/15), so one owning a resource moves in --
                // the spawning thread no longer owns it, and a
                // binding of it is moved from -- and any other is
                // copied. The spawner's object used to be shared,
                // and its block's exit could end it before the
                // thread ran.
                let cty = Type::Closure(cid);
                let owns_resource = self.objects.get(&obj).map_or(false, |o| o.is_resource);
                // A closure temporary (`spawn(move [r]() { … })`) is
                // the thread's to end, like a temporary argument: a
                // copy would leave the original, and the references
                // it captured, alive in this thread.
                let adopt = owns_resource || matches!(&fv, EvalResult::Temp(_));
                let obj = if adopt {
                    self.temp_index.remove(&obj);
                    self.invalidate_binding_pointing_at(obj);
                    for f in self.frames_mut().iter_mut() {
                        f.owned.retain(|o| *o != obj);
                    }
                    for t in self.stmt_temps.values_mut().flat_map(|s| s.iter_mut()) {
                        t.retain(|o| *o != obj);
                    }
                    obj
                } else {
                    let v = self.read_place(obj, &[]);
                    let copy = self.new_object(cty, v);
                    if let Some(ftys) = self.closure_field_types.get(&obj).cloned() {
                        self.closure_field_types.insert(copy, ftys);
                    }
                    copy
                };
                (SpawnBody::Closure(info.body.clone()), info.subst.clone(), ps, Some((obj, info.captures.clone())))
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };

        // D-0213: and those its closure captured.
        if let Some((o, _)) = &closure_bind {
            self.lend_object(*o)?;
        }
        // `[Spawn]` binds the parameters in the spawn step itself, so
        // a reference passed is held by the new thread from now on
        // ("until it finishes", spec/19 §1): held here, by an object
        // of the argument values that the thread ends with its body
        // (`rule.conc.guarantee`). Bound only when the thread first
        // ran, a conflicting access in between went unseen.
        let pending: Vec<Value> = params.iter().filter_map(|(_, r, _)| match r {
            EvalResult::Val(v) if Self::value_has_ref(v) => Some(v.clone()),
            _ => None,
        }).collect();
        // D-0204: a thread lent an exclusive reference changes data
        // that is not its own: it is never cancelled, and its fault
        // is not contained.
        let mut lent = pending.iter().any(value_has_excl_ref);
        if let Some((o, _)) = &closure_bind {
            lent |= self.objects.get(o).map_or(false, |ob| value_has_excl_ref(&ob.value));
        }
        let hold = if pending.is_empty() { None } else { Some(self.new_object(Type::Void, Value::Struct(pending))) };
        let ell = self.next_thread_id;
        self.next_thread_id += 1;
        if lent {
            self.shared.lock().unwrap().lent_excl.insert(ell);
        }
        self.frames_by_thread.insert(ell, Vec::new());
        self.shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Running);
        self.active_threads += 1;

        let gil = self
            .gil
            .as_ref()
            .and_then(|w| w.upgrade())
            .ok_or_else(|| Flow::Fault("spawn requires an interpreter run under a GIL (lib::run_source)".into()))?;
        let shared = self.shared.clone();
        // A program thread recurses as the main one does (main.rs),
        // so it gets the same stack: an unoptimized build's eval
        // frames are many times larger, and 512 MiB no longer held
        // the conformance suite's 20,000 calls there. The stack is
        // reserved, not committed, so a large one costs only what
        // is used.
        let stack = if cfg!(target_pointer_width = "64") {
            if cfg!(debug_assertions) { 4usize << 30 } else { 2 << 30 }
        } else {
            64 << 20
        };
        let handle_ret: Type = declared_ret.clone().unwrap_or(Type::Void);
        let builder = std::thread::Builder::new().stack_size(stack);
        gil.join_member(ell);
        let _ = builder.spawn(move || {
            set_stack_budget(stack);
            Gil::set_me(ell);
            gil.acquire();
            // Safety: this thread holds the GIL until `release`.
            let g = unsafe { gil.interp() };
            // [Fault-Unwind] (spec/18): some other thread already
            // ended the whole program -- "other threads take no
            // further steps; their frames are not unwound."
            if shared.lock().unwrap().program_fault.is_some() {
                g.active_threads -= 1;
                gil.leave_member(ell);
                gil.release();
                return;
            }
            g.current_thread = ell;
            g.set_unwind_flags((false, false, false));
            set_probe(&shared, ell);
            shared.lock().unwrap().os.insert(ell, crate::netio::os_thread());
            let outcome = g.run_body(body.block(), &callee_subst, true, &params, closure_bind.as_ref().map(|(o, c)| (*o, c.as_slice())));
            g.current_thread = ell;
            if let Some(h) = hold {
                g.remove_object(&h);
            }
            match outcome {
                Ok(res) => {
                    let rty = declared_ret.or_else(|| g.result_type_hint(&res)).unwrap_or(Type::Void);
                    match g.result_to_value_resource_aware(res) {
                        Ok(rv) => {
                            let mut s = shared.lock().unwrap();
                            s.thread_status.insert(ell, ThreadStatus::Done(rv, rty));
                            // `[Deadlock]` (b) is judged over the
                            // threads still running: one fewer now,
                            // and the rest may all wait (D-0219:
                            // found by the model; a thread ending
                            // after the last wait began left the
                            // waits hanging).
                            find_deadlock(&mut s);
                        }
                        Err(f) => {
                            let d = g.flow_to_string(f);
                            g.thread_failed(ell, d, rty);
                        }
                    }
                }
                Err(Flow::Terminated) => {}
                Err(flow) => {
                    let d = g.flow_to_string(flow);
                    let rty = declared_ret.clone().unwrap_or(Type::Void);
                    g.thread_failed(ell, d, rty);
                }
            }
            g.active_threads -= 1;
            shared.lock().unwrap().os.remove(&ell);
            gil.leave_member(ell);
            gil.release();
        }).expect("start a program thread");

        // The handle carries the thread's declared result type
        // (`handle<R>`): a generic callee infers `R` from it
        // (`try_join<R>(handle<R>)`, D-0208). `join` still reads
        // the result's own type out of `thread_status` once the
        // worker finishes; a closure with no declared type keeps
        // the `void` placeholder.
        let hty = Type::Handle(Box::new(handle_ret.clone()));
        let id = self.new_object(hty, Value::Handle(ell, handle_ret));
        return Ok(Some(EvalResult::Temp(id)));
    }
}
