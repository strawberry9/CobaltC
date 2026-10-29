// Expression evaluation: the dispatcher, results and paths.

use super::*;

impl Interp {
    // ================= Expressions =================

    pub fn eval(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        self.current_line = e.line;
        if user_line(e.line) {
            self.last_user_line = e.line;
        }
        let r = self.eval_inner(e, subst);
        if trace_on() {
            if let Err(Flow::Fault(d)) = &r {
                eprintln!("TRACE fault {} at line {} ({:?})", d, e.line, std::mem::discriminant(&e.kind));
            }
        }
        // Attach *this* expression's own line the first time an
        // untagged fault passes through `eval()` -- the innermost
        // expression whose evaluation actually produced it, since a
        // fault from a sub-expression's own `eval()` call already
        // tagged itself before propagating up through this one. Its own
        // line, not `current_line`: by the time an operator faults, its
        // operands' evaluation may have moved `current_line` elsewhere
        // (into a prelude body, whose lines have no location).
        // A line inside `std` is not one: the fault goes on untagged to
        // the program's own expression that called into `std`, whose line
        // it is reported at.
        if let Err(Flow::Fault(d)) = &r {
            if d.starts_with(EXIT_MARK) {
                self.exit_pending = true;
            } else {
                let d = d.clone();
                self.first_fault(&d);
            }
        }
        match r {
            Err(Flow::Fault(d)) if !d.contains('@') && user_line(e.line) => Err(Flow::Fault(format!("{d}@{}", e.line))),
            other => other,
        }
    }
    pub(super) fn eval_inner(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        match &e.kind {
            ExprKind::IntLit(v, suf) => self.eval_int_lit_expected(*v, suf.as_deref(), None),
            ExprKind::FloatLit(v, suf) => {
                if suf.as_deref() == Some("f32") {
                    Ok(EvalResult::Val(Value::F32(*v as f32)))
                } else {
                    Ok(EvalResult::Val(Value::F64(*v)))
                }
            }
            ExprKind::StrLit(s) => Ok(EvalResult::Val(Value::Str(Arc::from(s.as_bytes())))),
            ExprKind::BoolLit(b) => Ok(EvalResult::Val(Value::Bool(*b))),
            ExprKind::Unit => Ok(EvalResult::Val(Value::Unit)),
            ExprKind::Paren(inner) => self.eval(inner, subst),
            ExprKind::Path(segs, targs) => self.eval_path(segs, targs, subst),
            ExprKind::StructLit(segs, targs, fields) => self.eval_struct_lit(segs, targs, fields, subst, None),
            ExprKind::ArrayLit(items) => self.eval_array_lit(items, subst, None),
            ExprKind::ArrayRepeat(item, n) => self.eval_array_repeat(item, *n, subst, None),
            ExprKind::Unary(op, inner) => self.eval_unary(*op, inner, subst),
            ExprKind::Binary(op, l, r) => match self.view_op(e) {
                Some(vo) => self.eval_view_op(e, vo, subst),
                None => self.eval_binary(*op, l, r, subst),
            },
            ExprKind::Borrow(mode, inner) if (crate::typecheck::is_temp_value(inner) || self.variant_temp_borrow(e, inner)) && !self.addressed_const(inner) => self.eval_temp_borrow(mode, inner, subst, None),
            ExprKind::Borrow(mode, inner) => self.eval_borrow(mode.clone(), inner, subst),
            ExprKind::Deref(inner) => {
                let r = self.eval_deref(inner, subst)?;
                self.fn_through_shared(e, r)
            }
            ExprKind::Field(base, name) => {
                // D-0103: a field moved out of a temporary.
                if self.items.any_temp_moves.load(std::sync::atomic::Ordering::Relaxed) {
                    let r = self.items.temp_moves.lock().unwrap().get(&(e as *const Expr as usize)).cloned();
                    if let Some(r) = r {
                        return self.eval(&r, subst);
                    }
                }
                let r = self.eval_field(base, name, subst)?;
                self.fn_through_shared(e, r)
            }
            ExprKind::Index(base, idx) => {
                let r = self.eval_index(base, idx, subst)?;
                self.fn_through_shared(e, r)
            }
            ExprKind::SliceOf(mode, base, lo, hi) => match self.view_op(e) {
                Some(op) => self.eval_view_op(e, op, subst),
                None => {
                    // D-0103: a temporary sliced as a call's argument.
                    let temp_ok = self.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                    self.eval_slice_of(mode.clone(), base, lo, hi, subst, temp_ok)
                }
            },
            ExprKind::Dollar => match self.dollar.get(&self.current_thread).and_then(|d| d.last()) {
                Some(n) => Ok(EvalResult::Val(Value::Int(IntTy::Usize, *n as i128))),
                None => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            ExprKind::Call(callee, args) => self.eval_call(callee, args, subst),
            ExprKind::Assign(lhs, rhs) => self.eval_assign(lhs, rhs, subst),
            ExprKind::Propagate(inner) => self.eval_propagate(inner, subst),
            ExprKind::Block(b) => self.exec_block_body(b, subst),
            ExprKind::Unsafe(b) => {
                *self.unsafe_depth_by_thread.entry(self.current_thread).or_insert(0) += 1;
                let r = self.exec_block_body(b, subst);
                *self.unsafe_depth_by_thread.entry(self.current_thread).or_insert(0) -= 1;
                r
            }
            ExprKind::If(c, t, f) => self.eval_if(c, t, f, subst, None),
            ExprKind::While(c, b, step) => self.eval_while(c, b, step.as_deref(), subst),
            ExprKind::Match(scrut, arms) => {
                let hint = self.items.match_hints.lock().unwrap().get(&(e as *const Expr as usize)).cloned();
                self.eval_match(scrut, arms, subst, hint.as_ref())
            }
            ExprKind::Return(v) => {
                let r = match v {
                    Some(e) => match self.ret_stack().last().cloned().flatten() {
                        Some(rty) => self.eval_expected(e, subst, &rty)?,
                        None => self.eval(e, subst)?,
                    },
                    None => EvalResult::Val(Value::Unit),
                };
                let r = self.into_result_form(r)?;
                Err(Flow::Return(r))
            }
            ExprKind::Break => Err(Flow::Break),
            ExprKind::Continue => Err(Flow::Continue),
            ExprKind::Closure { is_move, captures, params, body, .. } => {
                self.eval_closure(*is_move, captures, params, body, e, subst)
            }
        }
    }

    pub(super) fn into_result_form(&mut self, r: EvalResult) -> EvalOutcome {
        match r {
            EvalResult::Place(obj, path, excl) => {
                let ty = self.type_at(obj, &path);
                if self.is_resource(&ty) && !path.is_empty() {
                    // `[Store-Binding-Place-Transfer-Sub]`: a part of an
                    // object (a temporary's field included) is not moved out.
                    return Err(Flow::Fault("diag.move-out-of-field".into()));
                }
                if self.is_resource(&ty) && path.is_empty() {
                    // [Authority-Transfer-Aliased]: a resource reached
                    // through a reference is still its owner's.
                    if !excl.is_empty() {
                        return Err(Flow::Fault("diag.move-while-aliased".into()));
                    }
                    self.invalidate_binding_pointing_at(obj);
                    Ok(EvalResult::Temp(obj))
                } else {
                    // `[Store-Result-Place-Copy]` reads through `[Read]`,
                    // not a raw peek — a plain place flowing out of a
                    // frame as its "kept" result is still subject to
                    // `¬clash(a, shared)`.
                    if !self.clash_check_read(obj, &path, &excl) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    let v = self.read_place(obj, &path);
                    Ok(EvalResult::Val(v))
                }
            }
            other => Ok(other),
        }
    }

    pub(super) fn eval_to_value(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<Value, Flow> {
        let r = self.eval(e, subst)?;
        self.result_to_value(r)
    }

    // An argument (or struct literal field) just evaluated (`rule.fn.call`
    // [Call]): a place that is not a resource is in value position and is
    // read now (`[LValue-To-RValue]`), before the next argument runs. A
    // resource place stays a place, for `store` to move.
    pub(super) fn arg_now(&mut self, r: EvalResult) -> Result<EvalResult, Flow> {
        if let EvalResult::Place(obj, path, _) = &r {
            let ty = self.type_at(*obj, path);
            // A reference or slice itself is handed on as it is (its own
            // path rules, D-0049's reborrow, apply at `store`).
            if !self.is_resource(&ty) && !matches!(ty, Type::Closure(_) | Type::Fn(..) | Type::Ref(..) | Type::Slice(..)) {
                // Copied (with any paths it holds) into a temporary of this
                // statement, which keeps the place's type for inference and
                // `match`, and which `store` adopts as it would a call's
                // result.
                let v = self.result_to_value(r)?;
                let id = self.new_object(ty, v);
                self.stmt_temp(id);
                return Ok(EvalResult::Temp(id));
            }
        }
        // CHG-0339: a temporary built for the argument (a struct literal, a
        // call's result) belongs to the statement until the call binds it
        // (`bind` adopts it): a fault in a later argument then ends it with
        // the statement's other temporaries, as `cobc` does.
        if let EvalResult::Temp(id) = &r {
            self.stmt_temp(*id);
        }
        Ok(r)
    }

    pub(super) fn result_to_value(&mut self, r: EvalResult) -> Result<Value, Flow> {
        match r {
            EvalResult::Val(v) => Ok(v),
            EvalResult::Place(obj, path, excl) => {
                // rule.init.definite-assignment (spec/11): a whole-object
                // read of a binding established by `[Let-Uninit]` and
                // never since written. Checked on the *root* object id
                // only (`path` may be empty, reading the whole binding,
                // or non-empty projecting into it -- either way the
                // underlying object is still the one `[Let-Uninit]`
                // established with nothing written).
                if self.uninit_bindings.contains(&obj) {
                    return Err(Flow::Fault("diag.use-of-uninitialized".into()));
                }
                let (resource, closure) = self.with_type_at(obj, &path, |t| (self.is_resource(t), matches!(t, Type::Closure(_))));
                if resource {
                    if trace_on() {
                        eprintln!("TRACE read-of-resource Place obj={} path={:?} ty={:?}", obj, path, self.type_at(obj, &path));
                    }
                    return Err(Flow::Fault("diag.read-of-resource".into()));
                }
                if !self.clash_check_read(obj, &path, &excl) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                // A closure read as a value is copied into a box of its own.
                if path.is_empty() && closure {
                    let ty = self.type_at(obj, &path);
                    let v = self.read_place(obj, &path);
                    if let Value::Closure(_) = v {
                        let (out, left) = self.split_closures(v)?;
                        self.write_place(obj, &path, left);
                        return Ok(out);
                    }
                    // D-0089: a closure that owns a resource moves: the
                    // binding holds no value afterwards.
                    if self.objects.get(&obj).map_or(false, |o| o.is_resource) {
                        self.invalidate_binding_pointing_at(obj);
                        return Ok(self.closure_box(obj));
                    }
                    let v = self.copy_closures(v)?;
                    let copy = self.new_object(ty, v);
                    if let Some(tys) = self.closure_field_types.get(&obj).cloned() {
                        self.closure_field_types.insert(copy, tys);
                    }
                    return Ok(self.closure_box(copy));
                }
                // D-0089: a copy of a `fn` value copies its closure; a value
                // holding a closure that owns a resource moves instead,
                // whole (a part cannot be moved out of its place).
                let v = self.read_place(obj, &path);
                if Self::holds_closure(&v) {
                    if self.holds_resource_closure(&v) {
                        // The `fn` slots whose closures own a resource are
                        // left empty (`moved_fn`); the rest are copied.
                        let (out, left) = self.split_closures(v)?;
                        self.write_place(obj, &path, left);
                        return Ok(out);
                    }
                    return self.copy_closures(v);
                }
                Ok(v)
            }
            EvalResult::Temp(o) => {
                let ty = self.objects.get(&o).unwrap().ty.clone();
                if self.is_resource(&ty) {
                    if trace_on() {
                        eprintln!("TRACE read-of-resource Temp obj={} ty={:?}", o, ty);
                    }
                    return Err(Flow::Fault("diag.read-of-resource".into()));
                }
                if matches!(ty, Type::Closure(_)) {
                    return Ok(self.closure_box(o));
                }
                let v = self.objects.get(&o).unwrap().value.clone();
                self.destroy_object(o)?;
                Ok(v)
            }
        }
    }

    // `excl`: the access-path tokens this very access was reached through
    // (an `EvalResult::Place`'s own chain) -- its ancestors, and nothing
    // else's. See `EvalResult::Place`.
    pub(super) fn clash_check_read(&self, obj: u64, path: &[Proj], excl: &[u64]) -> bool {
        !self.clash(obj, path, Mode::Shared, excl)
    }
    pub(super) fn clash_check_write(&self, obj: u64, path: &[Proj], excl: &[u64]) -> bool {
        !self.clash(obj, path, Mode::Exclusive, excl)
    }

    pub(super) fn eval_path(&mut self, segs: &[String], targs: &[Type], subst: &HashMap<String, Type>) -> EvalOutcome {
        // A one-segment path (every local variable read) borrows its name
        // rather than joining a new string.
        let name: std::borrow::Cow<str> = if segs.len() == 1 { std::borrow::Cow::Borrowed(segs[0].as_str()) } else { std::borrow::Cow::Owned(segs.join("::")) };
        let last = segs.last().unwrap().clone();
        // local binding first
        if segs.len() == 1 {
            if let Some((obj, moved)) = self.lookup(&last) {
                if moved {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if self.capture_cells.contains(&obj) {
                    // A borrow-captured name denotes `*self.f_i` (spec/15
                    // [Closure-Call]): resolve through the capture
                    // reference, whose token is this access's ancestor.
                    if let Value::Ref { obj: t, path, token, mode, .. } = self.read_place(obj, &[]) {
                        if !self.object_live(t) {
                            return Err(Flow::Fault("diag.stale-binding".into()));
                        }
                        self.affine(token, mode == Mode::Exclusive)?;
                        return Ok(EvalResult::Place(t, path, vec![token]));
                    }
                }
                return Ok(EvalResult::Place(obj, vec![], vec![]));
            }
        }
        // D-0258: a static is one object for the program's whole run, made
        // on its first use, owned by no frame and never ended.
        if let Some(obj) = self.static_object(&name)? {
            return Ok(EvalResult::Place(obj, vec![], vec![]));
        }
        // enum variant constructor (payload-less), e.g. `None`, `Sign::Pos`
        if let Some(enum_name) = self.variant_enum(segs) {
            if let Some(e) = self.items.enums.get(&enum_name).cloned() {
                let vi = e.variants.iter().position(|v| v.name == last).unwrap();
                if e.variants[vi].payload.is_none() {
                    let ty = self.enum_named_type(&enum_name, targs, subst, &e);
                    let v = Value::Enum(vi, Box::new(Value::Unit));
                    let id = self.new_object(ty, v);
                    return Ok(EvalResult::Temp(id));
                }
            }
        }
        // function value
        if self.items.fns.contains_key(name.as_ref()) || self.items.externs.contains_key(name.as_ref()) {
            return Ok(EvalResult::Val(Value::FnVal(name.into_owned())));
        }
        Err(Flow::Fault("diag.unbound-name".into()))
    }

    pub(super) fn enum_named_type(&self, name: &str, targs: &[Type], subst: &HashMap<String, Type>, _e: &Arc<EnumDecl>) -> Type {
        let args: Vec<Type> = targs.iter().map(|t| self.apply_subst(t, subst)).collect();
        Type::Named(name.to_string(), args)
    }

    // `Vi(e)` where `Vi` names a payload-carrying variant (spec/16
    // [Enum-Construct]). Returns `None` if `name` is not such a variant, so
    // the caller can fall through to an ordinary function call.
    // The enum a variant path names: `E::V` (or `m::E::V`) by its prefix,
    // a bare `V` through the flat table (an ambiguous bare variant has
    // already been rejected by the resolver).
    // A variant of an enum with type parameters, named with no type
    // arguments -- `None`, `Ok(5)`, `Some(None)`, `Err(x)` -- whose full
    // type only its context gives (`Ok(5)` fixes neither the `u64` nor the
    // error type of a `Result<u64, u8>`). Only one without effects (its
    // payload literals, names and such variants), so evaluating it later
    // than its place in the argument list is unobservable.
    pub(super) fn is_context_typed_variant(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Paren(x) => self.is_context_typed_variant(x),
            ExprKind::Path(segs, targs) => self.generic_variant(segs, targs, false),
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, targs) => self.generic_variant(segs, targs, true) && self.effect_free(&args[0]),
                _ => false,
            },
            _ => false,
        }
    }

    // `segs` names a variant (with a payload, or without) of an enum with
    // type parameters, and no type arguments are written.
    pub(super) fn generic_variant(&self, segs: &[String], targs: &[Type], with_payload: bool) -> bool {
        if !targs.is_empty() || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) {
            return false;
        }
        let Some(name) = self.variant_enum(segs) else { return false };
        let Some(en) = self.items.enums.get(&name) else { return false };
        !en.type_params.is_empty() && en.variants.iter().any(|v| Some(&v.name) == segs.last() && v.payload.is_some() == with_payload)
    }

    pub(super) fn effect_free(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Path(..) => true,
            ExprKind::Paren(x) | ExprKind::Unary(crate::ast::UnOp::Neg, x) => self.effect_free(x),
            _ => self.is_context_typed_variant(e),
        }
    }

    pub(super) fn variant_enum(&self, segs: &[String]) -> Option<String> {
        let last = segs.last()?;
        if segs.len() >= 2 {
            let key = segs[..segs.len() - 1].join("::");
            if let Some(e) = self.items.enums.get(&key) {
                return if e.variants.iter().any(|v| &v.name == last) { Some(key) } else { None };
            }
            // `m::V`: the enum of module `m` with that variant.
            if let Some(e) = self.items.module_variant_enum(&key, last) {
                return Some(e);
            }
        }
        self.items.enum_of_variant.get(last).cloned()
    }
}
