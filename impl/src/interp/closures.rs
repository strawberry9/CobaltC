// Closures: creation, captures, and their calls.

use super::*;

impl Interp {
    pub(super) fn eval_closure(
        &mut self,
        is_move: bool,
        captures: &[String],
        params: &[Param],
        body: &std::sync::Arc<Block>,
        expr: &Expr,
        subst: &HashMap<String, Type>,
    ) -> EvalOutcome {
        let mut vals = Vec::new();
        let mut tys = Vec::new();
        for c in captures {
            let (obj, moved) = self.lookup(c).ok_or_else(|| Flow::Fault("diag.unbound-name".into()))?;
            if moved {
                return Err(Flow::Fault("diag.stale-binding".into()));
            }
            if self.capture_cells.contains(&obj) {
                // Capturing a name that is itself a borrow capture of an
                // enclosing closure: the name denotes `*self.f_i`, so this
                // is a reborrow (or a copy out) through that reference.
                let (t, tpath, ttoken, tmode, pointee) = match self.read_place(obj, &[]) {
                    Value::Ref { obj, path, token, mode, pointee, .. } => (obj, path, token, mode, pointee),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if !self.object_live(t) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if is_move {
                    if self.is_resource(&pointee) {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    if self.clash(t, &tpath, Mode::Shared, &[ttoken]) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    vals.push(self.read_place(t, &tpath));
                    tys.push(pointee);
                } else {
                    let mutated = closure_body_writes(body, c);
                    let mode = if mutated { Mode::Exclusive } else { Mode::Shared };
                    if mode == Mode::Exclusive && tmode == Mode::Shared {
                        return Err(Flow::Fault("diag.borrow-exceeds-source".into()));
                    }
                    let ancestors = vec![ttoken];
                    if self.clash(t, &tpath, mode.clone(), &ancestors) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    let token = self.mint_token(&mode);
                    let anc = self.record_token_ancestors(token, &ancestors);
                    vals.push(Value::Ref { obj: t, path: tpath, mode: mode.clone(), pointee: pointee.clone(), token, range: None, anc });
                    tys.push(Type::Ref(Box::new(pointee), mode));
                }
                continue;
            }
            if is_move {
                let ty = self.type_at(obj, &[]);
                // A `fn` value whose closure owns a resource (D-0089) moves
                // too: its type, `fn(…)`, does not say so.
                let owns = self.is_resource(&ty)
                    || (matches!(ty, Type::Closure(_)) && self.objects.get(&obj).map_or(false, |o| o.is_resource))
                    || {
                        let v = self.read_place(obj, &[]);
                        self.holds_resource_closure(&v)
                    };
                // `[Closure-Form-Move]` stores each capture as `store` does:
                // a move needs `solitary`, a copy is a read, `[Read]`'s
                // conflict check included (CHG-0301: the copy was taken with
                // no check, beside a live exclusive reference to it).
                if owns && !self.solitary(obj, &[]) {
                    return Err(Flow::Fault("diag.move-while-aliased".into()));
                }
                if !owns && self.clash(obj, &[], Mode::Shared, &[]) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                if owns && matches!(ty, Type::Closure(_)) {
                    // A closure object that owns a resource moves as a
                    // handle to itself (as a read of it does, D-0089), so
                    // it ends -- its captures destroyed -- when that handle
                    // is dropped with the closure capturing it.
                    self.invalidate_binding_pointing_at(obj);
                    vals.push(self.closure_box(obj));
                    tys.push(ty);
                } else if owns {
                    self.invalidate_binding_pointing_at(obj);
                    let v = self.remove_object(&obj).unwrap().value;
                    vals.push(v);
                    tys.push(ty);
                } else {
                    let v = self.read_place(obj, &[]);
                    vals.push(v);
                    tys.push(ty);
                }
            } else {
                let mutated = closure_body_writes(body, c);
                let mode = if mutated { Mode::Exclusive } else { Mode::Shared };
                // A captured name is a bare binding: this access has no
                // ancestors of its own.
                let ancestors: Vec<u64> = Vec::new();
                if self.clash(obj, &[], mode.clone(), &ancestors) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let pointee = self.type_at(obj, &[]);
                let token = self.mint_token(&mode);
                let anc = self.record_token_ancestors(token, &ancestors);
                vals.push(Value::Ref { obj, path: vec![], mode, pointee: pointee.clone(), token, range: None, anc });
                tys.push(Type::Ref(Box::new(pointee), if mutated { Mode::Exclusive } else { Mode::Shared }));
            }
        }
        let cid = closure_id(expr);
        let ty = Type::Closure(cid);
        let holds_owner = vals.iter().any(|v| self.holds_resource_closure(v));
        let id = self.new_object(ty, Value::Struct(vals));
        // `rule.type.is-resource`: a closure whose captures include a
        // resource is a resource by derivation, like a struct with such a
        // field. Its type tag alone cannot say so (`is_resource` answers
        // `false` for `Type::Closure`), so the object is marked here, and
        // `destroy_composite` destroys the captures.
        // So is one that captures a `fn` value whose closure owns one
        // (D-0089): its type, `fn(…)`, cannot say so either.
        let owns = tys.iter().any(|t| self.is_resource(t)) || holds_owner;
        if owns {
            if let Some(o) = self.objects.get_mut(&id) {
                o.is_resource = true;
            }
        }
        self.closure_field_types.insert(id, tys.clone());
        self.closure_cid_types.insert(cid, tys.clone());
        // remember the body+params+capture names for calling later
        self.closures.borrow_mut().insert(cid, ClosureInfo {
            captures: captures.to_vec(),
            body: body.clone(),
            subst: subst.clone(),
            is_move,
            writes_self: is_move && captures.iter().any(|c| closure_body_writes(body, c)),
        });
        self.closure_params.borrow_mut().insert(cid, params.to_vec());
        // Its result type, as the static pass found it (D-0073), unless the
        // place it is written for already gave one.
        if let Some(Type::Fn(_, r)) = self.items.closure_types.lock().unwrap().get(&(expr as *const Expr as usize)).cloned() {
            self.closure_rets.borrow_mut().entry(cid).or_insert(*r);
        }
        Ok(EvalResult::Temp(id))
    }

    // Binds each captured name inside the call frame. `[Closure-Call]`
    // rewrites the body's free `x_i` to `*self.f_i` (borrow capture) or
    // `self.f_i` (move capture) — since this interpreter does not rewrite
    // the AST, a borrow-captured name is instead bound *directly* to the
    // referent's own object identity (skipping the stored `Ref` wrapper),
    // so `x` inside the body reads/writes the original storage exactly as
    // `*self.f_i` would. The closure's own struct field keeps the `Ref`
    // value (scanned by `clash` for as long as the closure object lives),
    // so outer-scope aliasing against the still-live capture is unaffected
    // — only the call-local name resolution is simplified.
    // Binds the closure's captured names for one call and returns the
    // capture cells it created (to be removed by the caller afterwards).
    //
    // Borrow-capturing closure: every field is a `Ref` formed at closure
    // creation (`[Closure-Form-Borrow]`). The body's free occurrence of
    // `x_i` means `*self.f_i` (`[Closure-Call]`), i.e. an access *through*
    // that reference -- so the name is bound to a fresh cell object
    // holding a copy of the reference, and `eval_path` dereferences it on
    // lookup. Binding the name to the referent directly, as this used to,
    // made the body's own write to `x_i` clash with the very capture
    // reference the closure holds (a live, non-ancestor exclusive path to
    // the same cells): every valid write through an exclusive capture was
    // rejected as `diag.aliasing-conflict`.
    //
    // Move closure: every field is an owned value (`[Closure-Form-Move]`),
    // including a captured variable that is itself of reference type.
    pub(super) fn bind_closure_captures(&mut self, obj: u64, captures: &[String]) -> Vec<u64> {
        let is_move = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => self.closures.borrow().get(id).map_or(false, |i| i.is_move),
            _ => false,
        };
        let mut cells = Vec::new();
        for (i, cname) in captures.iter().enumerate() {
            let v = self.read_place(obj, &[Proj::Field(i)]);
            let fty = self.type_at(obj, &[Proj::Field(i)]);
            if !is_move {
                let id = self.new_object(fty, v);
                self.capture_cells.insert(id);
                self.bind_alias(cname, id);
                cells.push(id);
                continue;
            }
            // A fresh per-call *view* of the closure's own field
            // storage -- bound as a non-owned alias (`bind_alias`),
            // not `bind`, so this call's frame does not believe
            // itself responsible for destroying it. The real
            // owning copy is (and remains) the closure object's
            // own field; a move-captured resource called twice
            // used to have its per-call snapshot wrongly
            // destroyed at the *first* call's frame exit (running
            // its destructor early -- e.g. dropping an `Rc`'s
            // count to 0 and deallocating it), so the *second*
            // call read already-freed memory. Bug found deriving
            // `ex.e2e-closure-move-rc` (calls a move closure
            // twice), fixed here.
            let id = self.new_object(fty, v);
            self.bind_alias(cname, id);
            // Removed (not destroyed) when the call is over, like the
            // borrow cells: a view left behind kept any reference inside
            // the captured value counted as held for the rest of the run.
            // Its value goes back to the closure's field first: the body
            // wrote `self.f_i` (spec/15 [Closure-Call]), so what it
            // changed is the closure's own state for the next call.
            self.capture_writeback.insert(id, (obj, i));
            cells.push(id);
        }
        cells
    }

    pub(super) fn release_capture_cells(&mut self, cells: &[u64]) {
        for id in cells {
            self.capture_cells.remove(id);
            if let Some((obj, i)) = self.capture_writeback.remove(id) {
                if let (Some(v), true) = (self.objects.get(id).map(|o| o.value.clone()), self.objects.contains_key(&obj)) {
                    self.write_place(obj, &[Proj::Field(i)], v);
                }
            }
            self.remove_object(id);
        }
    }

    // A closure moved into a `fn(…)` value (a field, an element, a `Box`):
    // the value names the closure object, which lives on while it does, as
    // `cobc`'s callable box (`[Callable-Closure]`).
    pub(super) fn closure_box(&mut self, o: u64) -> Value {
        for t in self.stmt_temps.values_mut().flat_map(|s| s.iter_mut()) {
            t.retain(|x| *x != o);
        }
        self.temp_index.remove(&o);
        for f in self.frames_mut().iter_mut() {
            f.owned.retain(|x| *x != o);
        }
        Value::Closure(crate::value::ClosureRef::owning(o, &self.ended_closures))
    }

    pub(super) fn holds_closure(v: &Value) -> bool {
        match v {
            Value::Closure(_) => true,
            Value::Struct(fs) | Value::Array(fs) => fs.iter().any(Self::holds_closure),
            Value::Enum(_, p) => Self::holds_closure(p),
            _ => false,
        }
    }

    pub(super) fn holds_resource_closure(&self, v: &Value) -> bool {
        match v {
            Value::Closure(h) => self.objects.get(&self.closure_target(h.id())).map_or(false, |o| o.is_resource),
            Value::Struct(fs) | Value::Array(fs) => fs.iter().any(|f| self.holds_resource_closure(f)),
            Value::Enum(_, p) => self.holds_resource_closure(p),
            _ => false,
        }
    }

    // D-0089: an emptied `fn` slot, whose closure moved out. Calling it is
    // `diag.stale-binding`.
    pub(super) fn moved_fn() -> Value {
        Value::FnVal("$moved".to_string())
    }

    // D-0089: reading `v` by value out of its place: each closure that
    // owns a resource moves to the result and its slot in the place is
    // emptied; each other closure is copied. Returns (result, what the
    // place keeps).
    pub(super) fn split_closures(&mut self, v: Value) -> Result<(Value, Value), Flow> {
        match v {
            Value::Closure(h) => {
                let o = self.closure_target(h.id());
                if self.objects.get(&o).map_or(false, |ob| ob.is_resource) {
                    Ok((Value::Closure(h), Self::moved_fn()))
                } else {
                    let c = self.copy_closures(Value::Closure(h.clone()))?;
                    Ok((c, Value::Closure(h)))
                }
            }
            Value::Struct(fs) => {
                let (mut a, mut b) = (Vec::with_capacity(fs.len()), Vec::with_capacity(fs.len()));
                for f in fs {
                    let (x, y) = self.split_closures(f)?;
                    a.push(x);
                    b.push(y);
                }
                Ok((Value::Struct(a), Value::Struct(b)))
            }
            Value::Array(xs) => {
                let (mut a, mut b) = (Vec::with_capacity(xs.len()), Vec::with_capacity(xs.len()));
                for x in xs {
                    let (p, q) = self.split_closures(x)?;
                    a.push(p);
                    b.push(q);
                }
                Ok((Value::Array(a), Value::Array(b)))
            }
            Value::Enum(i, p) => {
                let (x, y) = self.split_closures(*p)?;
                Ok((Value::Enum(i, Box::new(x)), Value::Enum(i, Box::new(y))))
            }
            other => Ok((other.clone(), other)),
        }
    }

    // D-0089: a copy of a `fn` value is a copy of the closure it holds,
    // captures included (`split_closures` moves the ones that own a
    // resource instead).
    pub(super) fn copy_closures(&mut self, v: Value) -> Result<Value, Flow> {
        match v {
            Value::Closure(h) => {
                let o = self.closure_target(h.id());
                let Some(ob) = self.objects.get(&o) else { return Ok(Value::Closure(h)) };
                if ob.is_resource {
                    return Err(Flow::Fault("diag.read-of-resource".into()));
                }
                let ty = ob.ty.clone();
                let inner = ob.value.clone();
                let inner = self.copy_closures(inner)?;
                let copy = self.new_object(ty, inner);
                if let Some(t) = self.closure_field_types.get(&o).cloned() {
                    self.closure_field_types.insert(copy, t);
                }
                Ok(self.closure_box(copy))
            }
            Value::Struct(fs) => {
                let mut out = Vec::with_capacity(fs.len());
                for f in fs {
                    out.push(self.copy_closures(f)?);
                }
                Ok(Value::Struct(out))
            }
            Value::Array(xs) => {
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.copy_closures(x)?);
                }
                Ok(Value::Array(out))
            }
            Value::Enum(i, p) => Ok(Value::Enum(i, Box::new(self.copy_closures(*p)?))),
            other => Ok(other),
        }
    }

    // D-0089: destroys each closure whose last handle was dropped, and
    // what that destruction drops in turn.
    pub(super) fn end_dropped_closures(&mut self) -> Result<(), Flow> {
        if self.ending_closures || self.terminated {
            return Ok(());
        }
        self.ending_closures = true;
        let r = loop {
            let next = self.ended_closures.lock().unwrap().pop();
            let Some(id) = next else { break Ok(()) };
            if self.objects.contains_key(&id) {
                if let Err(f) = self.destroy_object(id) {
                    break Err(f);
                }
            }
        };
        self.ending_closures = false;
        r
    }

    pub(super) fn is_closure_obj(&self, o: u64) -> bool {
        matches!(self.objects.get(&o).map(|ob| &ob.ty), Some(Type::Closure(_)))
    }

    // The closure object a callable object stands for.
    // D-0239: whether calling the closure `obj` (or the one the `fn` value
    // there names) writes the closure's own storage: a `move` closure
    // whose body writes a capture.
    pub(super) fn closure_writes_self(&self, obj: u64) -> bool {
        let o = self.closure_target(obj);
        match self.objects.get(&o).map(|ob| &ob.ty) {
            Some(Type::Closure(id)) => self.closures.borrow().get(id).map_or(false, |i| i.writes_self),
            _ => false,
        }
    }

    // D-0239 `[Closure-Call]`: such a closure is borrowed exclusively for
    // the call, through the place it is called from (a binding, a field,
    // an element, `*r`), the tokens that reached it excluded.
    pub(super) fn self_write_call(&mut self, closure: u64, o: u64, path: &[Proj], excl: &[u64]) -> Result<(), Flow> {
        if !self.closure_writes_self(closure) {
            return Ok(());
        }
        if !self.object_live(o) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if self.clash(o, path, Mode::Exclusive, excl) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        Ok(())
    }

    pub(super) fn closure_target(&self, obj: u64) -> u64 {
        let mut o = obj;
        for _ in 0..64 {
            match self.objects.get(&o).map(|ob| &ob.value) {
                Some(Value::Closure(i)) if i.id() != o => o = i.id(),
                _ => break,
            }
        }
        o
    }

    pub(super) fn call_closure(&mut self, obj: u64, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        let obj = self.closure_target(obj);
        let cid = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => *id,
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let info = self.closures.borrow().get(&cid).cloned().ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
        // params come from the closure literal; we re-derive them from the
        // stored body's own param list isn't tracked, so accept args
        // positionally bound to synthetic names p0..pn is wrong; instead we
        // stashed params separately.
        let params = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
        if args.len() != params.len() {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        // Each argument at its parameter's type (`rule.type.expected`), as
        // for a function: `f(9)` for `[](u8 a)` passes a `u8`.
        let mut argvals = Vec::new();
        for (a, p) in args.iter().zip(params.iter()) {
            let pty = self.apply_subst(&p.ty, &info.subst);
            if is_bare_num_literal(a) && matches!(pty, Type::Int(_) | Type::F32 | Type::F64) {
                argvals.push({ let r0 = self.eval_expected(a, subst, &pty)?; self.arg_now(r0)? });
            } else {
                argvals.push({ let r0 = self.eval(a, subst)?; self.arg_now(r0)? });
            }
        }
        self.push_call_frame();
        for (i, p) in params.iter().enumerate() {
            self.store_binding(&p.name, argvals[i].clone(), Some(self.apply_subst(&p.ty, subst)))?;
        }
        let cells = self.bind_closure_captures(obj, &info.captures);
        let ret = self.closure_rets.borrow().get(&cid).cloned();
        // `return` in a closure's body ends the closure's call, not the
        // function it was written in (spec/15 §6).
        self.ret_stack().push(ret.clone());
        let r = match &ret {
            Some(t) => self.exec_block_body_expected(&info.body, &info.subst, Some(t)),
            None => self.exec_block_body(&info.body, &info.subst),
        };
        self.ret_stack().pop();
        let popped = self.pop_frame(result_keep(&r));
        self.release_capture_cells(&cells);
        popped?;
        match r {
            Err(Flow::Return(v)) => Ok(v),
            other => other,
        }
    }
}
