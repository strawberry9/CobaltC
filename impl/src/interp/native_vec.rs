// `std`'s `Vec::push` carried out natively when its buffer has room, and
// `Vec::filled` and `Vec::drop` for an element type of numbers or `bool`
// (CHG-0292): the steps their bodies take, through the same reads, writes
// and allocations the bodies' expressions reach, without evaluating them
// one node at a time. A body of another shape, or a case this file does
// not take, runs as written.

use super::*;

impl Interp {
    // Whether `f` is `std`'s own `Vec::push` (not a program's function of
    // the same name).
    pub(super) fn is_std_vec_push(&self, f: &Arc<FnDecl>) -> bool {
        f.name == "push"
            && f.assoc_type.as_deref() == Some("Vec")
            && !native_off()
            && self.items.fns.get("std::Vec::push").map_or(false, |g| Arc::ptr_eq(g, f))
    }

    // `Vec::push(v, x)`'s body, its parameters already bound in the call's
    // frame, when the buffer has room: `None` when the body is to run as
    // written. The body is
    //
    //     if (v.len == v.cap) { Vec::grow(v); }
    //     unsafe { *(v.ptr + reinterpret<isize>(v.len)) = x; }
    //     v.len = v.len + 1;
    //
    // and each step below is the one its expressions take: each field read
    // through `v` resolved as `eval_field` resolves it and checked as
    // `result_to_value` checks it, the element written as `eval_assign`
    // writes through a raw pointer, the length written by the third
    // statement itself; frames, statement ends and yields as the body's
    // blocks and statements make them.
    pub(super) fn native_vec_push(&mut self, f: &Arc<FnDecl>, subst: &HashMap<String, Type>) -> Option<EvalOutcome> {
        let body = &f.body;
        if body.stmts.len() != 3 || body.tail.is_some() {
            return None;
        }
        let len_lhs = match &body.stmts[2] {
            Stmt::Expr(e) => match &e.kind {
                ExprKind::Assign(lhs, _) => lhs,
                _ => return None,
            },
            _ => return None,
        };
        let inner = match &body.stmts[1] {
            Stmt::Expr(e) | Stmt::BlockLike(e) => match &e.kind {
                ExprKind::Unsafe(b) if b.stmts.len() == 1 && b.tail.is_none() => b,
                _ => return None,
            },
            _ => return None,
        };
        let (vb, false) = self.lookup("v")? else { return None };
        let (xb, false) = self.lookup("x")? else { return None };
        // What the first statement will find, looked at without an access:
        // a full buffer (or anything unexpected) runs the body as written.
        let Value::Ref { obj: vo, path: vp, mode: Mode::Exclusive, .. } = self.read_place(vb, &[]) else { return None };
        if !self.object_live(vo) || self.uninit_bindings.contains(&vo) {
            return None;
        }
        match self.type_at(vo, &vp) {
            Type::Named(n, args) if n == "std::Vec" && args.len() == 1 && !self.is_resource(&args[0]) => {}
            _ => return None,
        }
        let field = |i: usize| {
            let mut p = vp.clone();
            p.push(Proj::Field(i));
            p
        };
        let (len_p, cap_p) = (field(1), field(2));
        match (self.read_place(vo, &len_p), self.read_place(vo, &cap_p)) {
            (Value::Int(_, l), Value::Int(_, c)) if l < c => {}
            _ => return None,
        }
        // The body's block.
        self.push_frame();
        // `if (v.len == v.cap)`: both reads; a fault in them (none is
        // expected, the binding of `v` having just checked the path) is
        // left to the body as written, whose reads repeat these.
        self.current_line = stmt_line(&body.stmts[0]);
        let first = self.native_field_read(vb, 1).and_then(|_| self.native_field_read(vb, 2));
        if first.is_err() {
            let _ = self.pop_frame(None);
            return None;
        }
        let r = self.native_vec_push_rest(body, inner, len_lhs, vb, xb, subst);
        let r = match r {
            Ok(()) => self.pop_frame(None).map(|_| EvalResult::Val(Value::Unit)),
            Err(flow) => {
                if let Flow::Fault(d) = &flow {
                    let d = d.clone();
                    self.first_fault(&d);
                }
                let _ = self.pop_frame(None);
                Err(flow)
            }
        };
        Some(match r {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        })
    }

    fn native_vec_push_rest(&mut self, body: &Block, inner: &Block, len_lhs: &Expr, vb: u64, xb: u64, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        self.native_stmt_end(&body.stmts[0])?;
        // `unsafe { *(v.ptr + reinterpret<isize>(v.len)) = x; }`
        self.push_frame();
        self.current_line = stmt_line(&inner.stmts[0]);
        let r = (|| {
            let Value::Rawptr(addr, pty) = self.native_field_read(vb, 0)? else {
                return Err(Flow::Fault("diag.type-mismatch".into()));
            };
            let Value::Int(_, n) = self.native_field_read(vb, 1)? else {
                return Err(Flow::Fault("diag.type-mismatch".into()));
            };
            let at = (addr as i128 + n * self.sizeof_ty(&pty) as i128) as u64;
            let x = self.result_to_value(EvalResult::Place(xb, Vec::new(), Vec::new()))?;
            self.arena_write(at, &pty, x);
            self.native_stmt_end(&inner.stmts[0])
        })();
        let popped = self.pop_frame(None);
        r?;
        popped?;
        self.native_stmt_end(&body.stmts[1])?;
        // `v.len = v.len + 1;`: the place, then the value, then `[Write]`
        // (`eval_assign`, `eval_assign_place`, `write_evaluated`).
        self.current_line = stmt_line(&body.stmts[2]);
        if self.crosses_shared_ref(len_lhs, subst)? {
            return Err(Flow::Fault("diag.write-through-shared".into()));
        }
        let (o, mut p, chain) = self.resolve_through_refs(vb, Vec::new(), Vec::new())?;
        p.push(Proj::Field(1));
        let Value::Int(t, n) = self.native_field_read(vb, 1)? else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let n = self.apply_binop(BinOp::Add, Value::Int(t, n), Value::Int(t, 1))?;
        let n = self.result_to_value(n)?;
        self.write_evaluated(o, p, chain, EvalResult::Val(n), &Type::Int(IntTy::Usize))?;
        self.native_stmt_end(&body.stmts[2])
    }

    // A field of the struct the reference bound to `name`'s object `b`
    // points at, read as `b.f` reads it.
    fn native_field_read(&mut self, b: u64, i: usize) -> Result<Value, Flow> {
        let (o, mut p, chain) = self.resolve_through_refs(b, Vec::new(), Vec::new())?;
        p.push(Proj::Field(i));
        self.result_to_value(EvalResult::Place(o, p, chain))
    }

    // The end of a statement run natively: its temporaries (it made none)
    // and the closures dropped meanwhile, as `exec_stmt` ends them; then
    // what `exec_block_stmts` does after each statement.
    fn native_stmt_end(&mut self, s: &Stmt) -> Result<(), Flow> {
        self.end_dropped_closures()?;
        self.native_after_stmt(s)
    }

    fn native_after_stmt(&mut self, s: &Stmt) -> Result<(), Flow> {
        if self.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed) {
            let names = self.items.early_ends.lock().unwrap().get(&(s as *const Stmt as usize)).cloned();
            if let Some(names) = names {
                for n in &names {
                    self.end_ref_binding(n)?;
                }
            }
        }
        self.yield_step();
        Ok(())
    }
}

impl Interp {
    // Whether `f` is `std`'s own `Vec::filled`.
    pub(super) fn is_std_vec_filled(&self, f: &Arc<FnDecl>) -> bool {
        f.name == "filled"
            && f.assoc_type.as_deref() == Some("Vec")
            && !native_off()
            && self.items.fns.get("std::Vec::filled").map_or(false, |g| Arc::ptr_eq(g, f))
    }

    // `Vec::filled(n, x)`'s body, its parameters bound, for `T` a number
    // type or `bool` and one thread running: `None` when the body is to
    // run as written. The body is
    //
    //     Vec<T> out = Vec::new();
    //     for (usize i = 0; i < n; i += 1) { Vec::push(&mut out, clone(&x)); }
    //     out
    //
    // Its first statement and its result run as written. The loop's
    // effects are its pushes': each element's bytes written where `push`
    // writes them, the buffer grown when full as `Vec::grow` grows it (the
    // same allocations, copies and deallocations, in the same order), and
    // `out`'s fields left as the last push leaves them. Of `T`'s values a
    // `clone` is the value; reading `n` and borrowing `x`, parameters no
    // other path reaches, cannot fault; with one thread `yield_step` does
    // nothing.
    pub(super) fn native_vec_filled(&mut self, f: &Arc<FnDecl>, subst: &HashMap<String, Type>, expected: Option<&Type>) -> Option<EvalOutcome> {
        if self.active_threads > 1 {
            return None;
        }
        let body = &f.body;
        let tail = body.tail.as_deref()?;
        if body.stmts.len() != 2 || !matches!(&body.stmts[0], Stmt::Let { name, .. } if name == "out") {
            return None;
        }
        // The loop's statements: none may end a binding early.
        let lp = match &body.stmts[1] {
            Stmt::Expr(e) | Stmt::BlockLike(e) => match &e.kind {
                ExprKind::Block(b) => b,
                _ => return None,
            },
            _ => return None,
        };
        let inner = match lp.tail.as_deref().map(|e| &e.kind) {
            Some(ExprKind::While(_, b, Some(_))) => b,
            _ => return None,
        };
        if self.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed) {
            let ends = self.items.early_ends.lock().unwrap();
            let all = body.stmts.iter().chain(lp.stmts.iter()).chain(inner.stmts.iter());
            if all.into_iter().any(|s| ends.contains_key(&(s as *const Stmt as usize))) {
                return None;
            }
        }
        let t = subst.get("T")?.clone();
        if !matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool) {
            return None;
        }
        let (nb, false) = self.lookup("n")? else { return None };
        let (xb, false) = self.lookup("x")? else { return None };
        let Value::Int(_, n) = self.read_place(nb, &[]) else { return None };
        let x = self.read_place(xb, &[]);
        if n <= 0 || self.uninit_bindings.contains(&xb) {
            return None;
        }
        self.push_frame();
        let r = (|| {
            self.exec_stmt(&body.stmts[0], subst)?;
            self.native_fill(n as u64, &x, &t)?;
            self.eval_stmt_scoped(tail, subst, expected)
        })();
        Some(match self.native_body_end(r) {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        })
    }

    // A body's block ended, its result `r`, as `exec_block_body_inner`
    // ends one.
    fn native_body_end(&mut self, r: EvalOutcome) -> EvalOutcome {
        match r {
            Ok(res) => {
                let res = self.result_form_or_unwind(res)?;
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(flow) => {
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                Err(flow)
            }
        }
    }

    // The pushes of `n` copies of `x : t` onto the empty `out`.
    fn native_fill(&mut self, n: u64, x: &Value, t: &Type) -> Result<(), Flow> {
        let Some((ob, false)) = self.lookup("out") else { return Err(Flow::Fault("diag.unbound-name".into())) };
        let Value::Struct(fs) = self.read_place(ob, &[]) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let (Some(Value::Rawptr(mut ptr, _)), Some(Value::Int(_, 0)), Some(Value::Int(_, 0))) = (fs.first().cloned(), fs.get(1).cloned(), fs.get(2).cloned()) else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let size = self.sizeof_ty(t);
        let align = self.alignof_ty(t);
        let mut cap = 0u64;
        for len in 0..n {
            if len == cap {
                // `Vec::grow`: allocate, copy what is there, free the old.
                let new_cap = if cap == 0 { 4 } else { cap * 2 };
                let bytes = new_cap * size;
                let p = self.bump_alloc(bytes, align.max(1));
                self.allocations.insert(p, (bytes, align));
                let moved: Vec<u8> = (0..len * size).map(|i| *self.arena.get(&(ptr + i)).unwrap_or(&0)).collect();
                for (i, b) in moved.into_iter().enumerate() {
                    self.arena.insert(p + i as u64, b);
                }
                self.clear_side(p, len * size);
                if cap > 0 {
                    let stale: Vec<u64> = self.reclaimed_addr.ids_in(ptr, ptr + cap * size);
                    self.check_not_foreign(&stale)?;
                    self.clear_side(ptr, cap * size);
                    for id in stale {
                        if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                            self.reclaimed_by_addr.remove(&a);
                        }
                        self.remove_object(&id);
                    }
                    self.allocations.remove(&ptr);
                }
                ptr = p;
                cap = new_cap;
            }
            self.arena_write(ptr + len * size, t, x.clone());
        }
        let fields = Value::Struct(vec![Value::Rawptr(ptr, t.clone()), Value::Int(IntTy::Usize, n as i128), Value::Int(IntTy::Usize, cap as i128)]);
        self.write_place(ob, &[], fields);
        Ok(())
    }
}

impl Interp {
    // Whether `f` is `std`'s own `Vec::drop`, the destructor.
    pub(super) fn is_std_vec_drop(&self, f: &Arc<FnDecl>) -> bool {
        f.name == "drop"
            && f.assoc_type.as_deref() == Some("Vec")
            && !native_off()
            && self.items.fns.get("std::Vec::drop").map_or(false, |g| Arc::ptr_eq(g, f))
    }

    // `Vec::drop(self)`'s body, `self` bound, for `T` a number type or
    // `bool` and one thread running: `None` when the body is to run as
    // written. The body is
    //
    //     usize mut_i = 0;
    //     while (mut_i < self.len) { unsafe { drop(reclaim<T>(self.ptr + reinterpret<isize>(mut_i))); } mut_i = mut_i + 1; }
    //     if (self.cap > 0) { unsafe { deallocate(…); } }
    //
    // Its loop re-attaches each element's cells as an object and destroys
    // it at once. Of a `T` with no resource and no reference, and with no
    // element object there already (one a reference into the vector made),
    // that leaves nothing behind, so it is skipped; any element object
    // there runs the body as written. The deallocation runs as written.
    pub(super) fn native_vec_drop(&mut self, f: &Arc<FnDecl>, subst: &HashMap<String, Type>) -> Option<EvalOutcome> {
        if self.active_threads > 1 {
            return None;
        }
        let body = &f.body;
        let tail = body.tail.as_deref()?;
        if body.stmts.len() != 2 || !matches!(tail.kind, ExprKind::If(..)) || !matches!(&body.stmts[0], Stmt::Let { name, .. } if name == "mut_i") {
            return None;
        }
        let inner = match &body.stmts[1] {
            Stmt::Expr(e) | Stmt::BlockLike(e) => match &e.kind {
                ExprKind::While(_, b, None) => b,
                _ => return None,
            },
            _ => return None,
        };
        if self.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed) {
            let ends = self.items.early_ends.lock().unwrap();
            if body.stmts.iter().chain(inner.stmts.iter()).any(|s| ends.contains_key(&(s as *const Stmt as usize))) {
                return None;
            }
        }
        let t = subst.get("T")?.clone();
        if !matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool) {
            return None;
        }
        let (sb, false) = self.lookup(&f.params.first()?.name)? else { return None };
        let Value::Ref { obj: vo, path: vp, .. } = self.read_place(sb, &[]) else { return None };
        let Value::Struct(fs) = self.read_place(vo, &vp) else { return None };
        let (Some(Value::Rawptr(ptr, _)), Some(Value::Int(_, len))) = (fs.first(), fs.get(1)) else { return None };
        let end = ptr + (*len as u64) * self.sizeof_ty(&t);
        if !self.reclaimed_addr.ids_in(*ptr, end).is_empty() {
            return None;
        }
        self.push_frame();
        let r = self.eval_stmt_scoped(tail, subst, None);
        Some(match self.native_body_end(r) {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        })
    }
}

fn stmt_line(s: &Stmt) -> usize {
    match s {
        Stmt::Let { line, .. } => *line,
        Stmt::Expr(e) | Stmt::BlockLike(e) => e.line,
        Stmt::Destructure { init, .. } => init.line,
    }
}

// `COBALTC_COBY_NO_NATIVE=1` runs `std`'s bodies as written everywhere this
// file would shortcut one, so the two can be compared.
fn native_off() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| std::env::var("COBALTC_COBY_NO_NATIVE").is_ok())
}
