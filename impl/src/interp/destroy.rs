// Destruction: scope ends, destructors, composite and box values.

use super::*;

impl Interp {
    // ---- destroy ----

    // A resource ended by [Block-Exit] or [Stmt-Exit] (spec/14) is
    // destroyed through `destroy`, whose precondition is `solitary`: a
    // live reference held elsewhere (a view kept past the statement, a
    // reference stored outward) is `[Destroy-Not-Solitary]`. A plain
    // object just ends, and its references become stale.
    // `[Fault-Unwind]` (spec/18): a fault's unwind reports that fault. A
    // destructor that faults while one is unwinding (or after a first
    // destructor fault) does not replace it: nothing more is destroyed,
    // as `cbrt` stops at that point, and the first fault goes on. During
    // `exit`'s unwind (D-0150) a destructor's fault is the outcome.
    pub(super) fn destroy_at_scope_end(&mut self, id: u64) -> Result<(), Flow> {
        let unwinding = self.fault_pending;
        let r = self.destroy_at_scope_end_inner(id);
        match r {
            Err(Flow::Fault(d)) if !d.starts_with(EXIT_MARK) => {
                if unwinding {
                    self.terminated = true;
                    return Ok(());
                }
                self.first_fault(&d);
                // During `exit`'s unwind this fault is the outcome, and,
                // as for a fault during a fault's unwind, nothing more is
                // destroyed.
                if self.exit_pending {
                    self.terminated = true;
                }
                Err(Flow::Fault(d))
            }
            other => other,
        }
    }

    pub(super) fn destroy_at_scope_end_inner(&mut self, id: u64) -> Result<(), Flow> {
        let ty = self.type_at(id, &[]);
        if self.is_resource(&ty) && !self.solitary(id, &[]) {
            return Err(Flow::Fault("diag.destroy-while-aliased".into()));
        }
        self.destroy_object(id)
    }

    pub fn destroy_object(&mut self, id: u64) -> Result<(), Flow> {
        self.destroy_object_tail(id, false)
    }

    // `destroy_object`; `tail`: nothing observable follows it in the Box
    // teardown that asked for it (so a Box its contents destroy last may
    // leave its referent to that teardown's loop).
    pub(super) fn destroy_object_tail(&mut self, id: u64, tail: bool) -> Result<(), Flow> {
        let obj = match self.objects.get(&id) {
            Some(o) => o,
            None => return Ok(()),
        };
        if obj.is_resource {
            // [Destroy-No-Authority]: only the thread holding the
            // object's destroy authority may destroy it.
            if obj.owner_thread != self.current_thread {
                return Err(Flow::Fault("diag.no-destroy-authority".into()));
            }
            let ty = obj.ty.clone();
            self.run_destructor(id, &ty)?;
            self.destroy_composite(id, tail)?;
        }
        self.remove_object(&id);
        // D-0089: a `fn` value in it held its closure; that ends with it.
        self.end_dropped_closures()?;
        // `[Object-End]`: an arena-backed identity (a reclaimed object, or
        // a local that `rawptr_of` gave an address) no longer claims its
        // cells; a later `reclaim` there establishes a fresh identity, and
        // the references it held in them are held no longer.
        if let Some((addr, rty)) = self.reclaimed_addr.get(&id).cloned() {
            let n = self.sizeof_ty(&rty);
            self.clear_side(addr, n);
        }
        if let Some((addr, _)) = self.reclaimed_addr.remove(&id) {
            if self.reclaimed_by_addr.get(&addr) == Some(&id) {
                self.reclaimed_by_addr.remove(&addr);
            }
        }
        Ok(())
    }

    // `[Object-End]` of an object whose contents were all moved out (a
    // destructured struct): no destructor, nothing inside destroyed.
    pub(super) fn end_moved_out(&mut self, id: u64) {
        self.remove_object(&id);
        if let Some((addr, rty)) = self.reclaimed_addr.remove(&id) {
            let n = self.sizeof_ty(&rty);
            self.clear_side(addr, n);
            if self.reclaimed_by_addr.get(&addr) == Some(&id) {
                self.reclaimed_by_addr.remove(&addr);
            }
        }
    }

    // The value an object holds: a reclaimed object's (a `Vec` element,
    // a Box's referent) lives in its raw cells, its record holding a
    // placeholder.
    pub(super) fn object_value(&self, id: u64) -> Option<Value> {
        match self.reclaimed_addr.get(&id).cloned() {
            Some((addr, rty)) => Some(self.arena_read(addr, &rty)),
            None => self.objects.get(&id).map(|o| o.value.clone()),
        }
    }

    pub(super) fn run_destructor(&mut self, id: u64, ty: &Type) -> Result<(), Flow> {
        match ty {
            // The built-in destructors read the object's value where it
            // lives: a handle or guard that is a `Vec`'s element is a
            // reclaimed object (CHG-0293: its record's placeholder made
            // the handle's destructor wait for nothing, and the thread
            // outlived its handle).
            Type::Handle(_) | Type::Guard(_) => match self.object_value(id) {
                Some(v) => self.builtin_destructor(&v, ty),
                None => Ok(()),
            },
            Type::Mutex(_) => Ok(()), // its only cleanup is `inner`, handled by destroy_composite
            Type::Named(name, args) => {
                if self.items.structs.contains_key(name) {
                    let dname = format!("{}::drop", name);
                    if let Some(f) = self.items.fns.get(&dname).cloned() {
                        if crate::interp::trace_events_on() && !name.starts_with("std::") {
                            eprintln!("EV drop {}", name);
                        }
                        self.call_fn_on_object(&f, args, id)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // `[Handle-Destructor]` and `[Guard-Drop]` of a value, wherever it is
    // held: a binding, an element, a field or a payload.
    pub(super) fn builtin_destructor(&mut self, v: &Value, ty: &Type) -> Result<(), Flow> {
        match ty {
            Type::Handle(_) => {
                // [Handle-Destructor] (spec/19 §1): destroying a handle
                // any other way than `join` -- `drop(h)`, or the
                // automatic sweep at block exit -- waits for the thread
                // exactly as `join` would, then discards its result
                // (destroying it as a temporary if it is a resource).
                let ell = match v {
                    Value::Handle(k, _) => *k,
                    _ => return Ok(()),
                };
                // During an unwind that ends the program, no wait: the
                // thread takes no further steps (spec/18 §1), so the wait
                // could never end; its result is abandoned with the
                // process. D-0204: during a contained fault's unwind, the
                // thread is cancelled and waited for -- no thread outlives
                // its handle -- and its own failure, if any, is discarded.
                let unwinding = self.faulting || self.fault_pending;
                if self.terminated || (unwinding && !self.contained_here()) {
                    self.shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Taken);
                    return Ok(());
                }
                if unwinding {
                    request_cancel(&self.shared, ell);
                }
                self.wait_for(Wait::Join(ell), |s| matches!(s.thread_status.get(&ell), Some(ThreadStatus::Done(..)) | Some(ThreadStatus::Failed(..)) | Some(ThreadStatus::Taken)))?;
                let taken = self.shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Taken);
                match taken {
                    Some(ThreadStatus::Done(rv, rty)) => {
                        if self.is_resource(&rty) {
                            let id = self.new_object(rty, rv);
                            self.destroy_object(id)?;
                        }
                    }
                    // D-0204: a failure nobody took is the destroyer's: it
                    // is raised again here, unless this thread is already
                    // unwinding, or asked for the stop (`cancel`).
                    Some(ThreadStatus::Failed(fid, _)) if !unwinding => {
                        let s = self.shared.lock().unwrap();
                        let (d, _) = s.failures[fid as usize].clone();
                        let asked = s.cancel.contains(&ell) && d.starts_with("diag.thread-cancelled");
                        drop(s);
                        if !asked {
                            return Err(Flow::Fault(d));
                        }
                    }
                    _ => {}
                }
                Ok(())
            }
            Type::Guard(_) => {
                // [Guard-Drop]: unlock the mutex this guard's path targets.
                if let Value::Guard { obj, path, .. } = v {
                    // The guard's path is the mutex's, then `inner`.
                    let key = (*obj, path[..path.len().saturating_sub(1)].to_vec());
                    // D-0204: released by a contained fault's unwind, the
                    // mutex's interior may be half-changed: poisoned.
                    let poison = (self.faulting || self.fault_pending) && self.contained_here();
                    let mut s = self.shared.lock().unwrap();
                    s.cobalt_locks.remove(&key);
                    if poison {
                        s.poisoned.insert(key);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // Calls fn Type::drop(ref<Type,exclusive> self) with `self` bound to a
    // fresh exclusive root path into `id` (spec/07 [Run-Destructor]).
    // The static pass checks `std`'s bodies as the program reaches them;
    // this evaluator calls a few by name (a destructor, `printf`'s
    // helpers), so one it has not checked is checked here, before it
    // runs (`typecheck::check_more`). A checked body costs one lookup.
    pub(super) fn ensure_checked(&self, f: &FnDecl, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        let ptr = f as *const FnDecl as usize;
        if self.items.checked_bodies.lock().unwrap().contains(&ptr) {
            return Ok(());
        }
        let Some(key) = self.items.fn_keys.get(&ptr) else { return Ok(()) };
        let targs: Vec<Type> = match f.type_params.iter().map(|tp| subst.get(tp).cloned()).collect::<Option<Vec<Type>>>() {
            Some(t) => t,
            None => crate::typecheck::opaque_args(f),
        };
        crate::typecheck::check_more(&self.items, key, &targs).map_err(Flow::Fault)?;
        self.items.checked_bodies.lock().unwrap().insert(ptr);
        Ok(())
    }

    pub(super) fn call_fn_on_object(&mut self, f: &Arc<FnDecl>, targs: &[Type], id: u64) -> Result<(), Flow> {
        // D-0077, as at every other call entry: the destructor recursion
        // (drop intrinsic -> destroy_object -> here -> the drop body ->
        // drop intrinsic ...) was the one call cycle without this guard,
        // and a 50,000-deep Box chain overflowed the host stack.
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        let subst = self.subst_map(&f.type_params, targs);
        self.ensure_checked(f, &subst)?;
        self.push_call_frame();
        let pointee = self.objects.get(&id).unwrap().ty.clone();
        let selfv = Value::Ref { obj: id, path: vec![], mode: Mode::Exclusive, pointee, token: self.mint_token(&Mode::Exclusive), range: None, anc: None };
        let selfobj = self.new_object(Type::Ref(Box::new(Type::Void), Mode::Exclusive), selfv);
        self.bind(&f.params[0].name, selfobj);
        let native = if self.is_std_vec_drop(f) { self.native_vec_drop(f, &subst) } else { None };
        let r = match native {
            Some(r) => r,
            None => self.exec_block_body(&f.body, &subst),
        };
        self.pop_frame(None)?;
        match r {
            Ok(_) => Ok(()),
            Err(Flow::Return(_)) => Ok(()),
            Err(other) => Err(other),
        }
    }

    pub(super) fn destroy_composite(&mut self, id: u64, tail: bool) -> Result<(), Flow> {
        // Recursively destroy every resource-bearing field/element/payload,
        // innermost first (spec/07 [Destroy-Composite]). A reclaimed
        // object's value lives in its raw cells, not in the object record
        // (which holds a placeholder): read it from there, or a struct
        // with no destructor of its own -- `Rc`'s box, a `Vec` element --
        // would never destroy the resources in its fields.
        let value = match self.reclaimed_addr.get(&id).cloned() {
            Some((addr, rty)) => Some(self.arena_read(addr, &rty)),
            None => self.objects.get(&id).map(|o| o.value.clone()),
        };
        let ty = self.objects.get(&id).map(|o| o.ty.clone());
        // A closure's captures are its fields; their types are recorded
        // per object (the `Type::Closure` tag carries none). Last first,
        // as for struct fields.
        if let (Some(Value::Struct(fields)), Some(Type::Closure(_))) = (&value, &ty) {
            let ftys = self.closure_ftys(id).unwrap_or_default();
            for (f, fty) in fields.iter().zip(ftys.iter()).rev() {
                if self.is_resource(fty) {
                    self.destroy_field_value(f, fty, false)?;
                }
            }
            return Ok(());
        }
        if let (Some(value), Some(ty)) = (value, ty) {
            self.destroy_composite_value(&value, &ty, tail)?;
        }
        Ok(())
    }

    // `tail` (D-0077 friction 22): what this destroys last is the last
    // observable effect of the Box teardown that asked for it.
    pub(super) fn destroy_composite_value(&mut self, v: &Value, ty: &Type, tail: bool) -> Result<(), Flow> {
        match (v, ty) {
            (Value::Struct(fields), Type::Named(name, args)) => {
                if let Some(s) = self.items.structs.get(name).cloned() {
                    let subst = self.subst_map(&s.type_params, args);
                    // [Destroy-Composite] (spec/07): same-length paths
                    // (sibling fields) tie-break by *reverse*
                    // declaration/index order -- last-declared field
                    // destroyed first, mirroring D-0008's block-scope
                    // reverse order. Was forward (declaration order),
                    // confirmed backwards by a real program whose
                    // fields' own destructors each write a distinguishing
                    // byte and observing the emitted order.
                    // The resource field destroyed last (the first declared).
                    let last = (0..fields.len()).find(|&i| self.is_resource(&self.apply_subst(&s.fields[i].ty, &subst)));
                    for (i, f) in fields.iter().enumerate().rev() {
                        let fty = self.apply_subst(&s.fields[i].ty, &subst);
                        if self.is_resource(&fty) {
                            self.destroy_field_value(f, &fty, tail && Some(i) == last)?;
                        }
                    }
                }
                Ok(())
            }
            (Value::Struct(fields), Type::Mutex(inner)) => {
                // layout: field 0 = inner
                if self.is_resource(inner) {
                    self.destroy_field_value(&fields[0], inner, tail)?;
                }
                Ok(())
            }
            (Value::Array(items), Type::Array(inner, _)) => {
                // Same reverse-index tie-break as the struct-field case
                // above -- last element destroyed first.
                if self.is_resource(inner) {
                    let n = items.len();
                    for (k, it) in items.iter().enumerate().rev() {
                        self.destroy_field_value(it, inner, tail && k == 0 && n > 0)?;
                    }
                }
                Ok(())
            }
            (Value::Enum(_, payload), Type::Named(name, args)) => {
                if let Some(e) = self.items.enums.get(name).cloned() {
                    let subst = self.subst_map(&e.type_params, args);
                    // find active variant's payload type by trying to match; we
                    // don't track discriminant here, so destroy whatever
                    // payload type resource-ness matches (best effort: try all
                    // variants' declared payload types that this value could
                    // be, using is_resource union already computed at
                    // is_resource(enum) time). We instead re-derive using the
                    // Enum's variant index stored alongside; but v is just the
                    // payload here per match arm above (Value::Enum(_, payload)),
                    // so `_` is the discriminant we do have:
                    if let Value::Enum(vi, _) = v {
                        if let Some(vd) = e.variants.get(*vi) {
                            if let Some(pty) = &vd.payload {
                                let pty = self.apply_subst(pty, &subst);
                                // `Value::Unit` in a non-unit resource
                                // payload slot marks one already moved out
                                // by a `match` binder (see eval_match) —
                                // nothing left here to destroy.
                                if self.is_resource(&pty) && !matches!(payload.as_ref(), Value::Unit) {
                                    self.destroy_field_value(payload, &pty, tail)?;
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // Destroys a resource value that is a field/element/payload (not a
    // top-level object): runs its destructor (if a named struct) and
    // recurses, without going through end-object (the storage belongs to
    // the container).
    pub(super) fn destroy_field_value(&mut self, v: &Value, ty: &Type, tail: bool) -> Result<(), Flow> {
        // D-0077 for the pure-value recursion (nested payloads with no
        // interpreted destructor anywhere): fault, never overflow.
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        // A handle or guard held in a field, element or payload has its
        // built-in destructor run as a binding's would (CHG-0293: it was
        // skipped, so `Job { .h = spawn(…) }` ended without waiting for
        // the thread and `Held { .g = lock(&m) }` without unlocking).
        if matches!(ty, Type::Handle(_) | Type::Guard(_)) {
            return self.builtin_destructor(v, ty);
        }
        if let Type::Named(name, args) = ty {
            if name == "std::Box" && args.len() == 1 {
                return self.destroy_box_value(v, &args[0], tail);
            }
        }
        if let Type::Named(name, args) = ty {
            if self.items.structs.contains_key(name) {
                let dname = format!("{}::drop", name);
                if let Some(f) = self.items.fns.get(&dname).cloned() {
                    if crate::interp::trace_events_on() && !name.starts_with("std::") {
                        eprintln!("EV drop {}", name);
                    }
                    // give the field its own temporary object so `self` can
                    // borrow it, then copy any mutation back isn't needed
                    // since the container is being torn down anyway.
                    let tmp = self.new_object(ty.clone(), v.clone());
                    self.call_fn_on_object(&f, args, tmp)?;
                    self.remove_object(&tmp);
                }
            }
        }
        self.destroy_composite_value(v, ty, tail)
    }

    // A `Box<T>` field, payload or element destroyed as `Box::drop` does
    // it (`drop(reclaim<T>(ptr))`, then `deallocate`), natively. Its
    // referent's destruction is its last observable effect, so a Box in
    // tail position hands it to the nearest enclosing teardown's loop
    // (unless a closure ended here is still to be destroyed after it):
    // a Box-linked chain of any length is destroyed in constant stack,
    // in the same order.
    pub(super) fn destroy_box_value(&mut self, v: &Value, t: &Type, tail: bool) -> Result<(), Flow> {
        let Value::Struct(fs) = v else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let Some(Value::Rawptr(addr, _)) = fs.first() else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let addr = *addr;
        // D-0298: a Box is one pointer; null marks one whose value
        // `into_inner` took and whose memory it freed: nothing to do.
        if addr == 0 {
            return Ok(());
        }
        let cells = self.sizeof_ty(t).max(1);
        let id = self.intrinsic_reclaim(addr, t.clone());
        // `drop` of the reclaimed place: [Destroy-Not-Solitary], as the
        // body's `drop(reclaim<T>(ptr))` would check it (a reference
        // `Box::get` gave, still live).
        if self.is_resource(t) && !self.solitary(id, &[]) {
            return Err(Flow::Fault("diag.destroy-while-aliased".into()));
        }
        if tail && self.box_draining && self.box_next.is_none() && self.ended_closures.lock().unwrap().is_empty() {
            self.box_next = Some((id, addr, cells));
            return Ok(());
        }
        let was = std::mem::replace(&mut self.box_draining, true);
        let saved = self.box_next.take();
        let mut next = Some((id, addr, cells));
        let mut r = Ok(());
        while let Some((id, addr, cells)) = next.take() {
            r = self.destroy_object_tail(id, true);
            self.box_deallocate(addr, cells);
            if r.is_err() {
                break;
            }
            next = self.box_next.take();
        }
        self.box_next = saved;
        self.box_draining = was;
        r
    }

    // `deallocate(p, n, _)` of a Box's cells.
    pub(super) fn box_deallocate(&mut self, addr: u64, n: u64) {
        self.clear_side(addr, n);
        let stale: Vec<u64> = self.reclaimed_addr.ids_in(addr, addr + n);
        for id in stale {
            if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                self.reclaimed_by_addr.remove(&a);
            }
            self.remove_object(&id);
        }
        self.allocations.remove(&addr);
    }
}

impl Interp {
    // D-0264: the first raise of the program's fault (`fault_pending`
    // false until now): the trace event, once, before the unwind runs the
    // destructors, as `cbrt`'s `fault` writes it.
    pub(super) fn first_fault(&mut self, d: &str) {
        if !self.fault_pending {
            self.fault_pending = true;
            // A contained fault (`[Fault-Contain]`, D-0204) is the thread's,
            // not the program's: no event, as `cbrt`'s `fault` writes none.
            if !self.contained_here() {
                crate::interp::fault_event(d);
            }
        }
    }
}

