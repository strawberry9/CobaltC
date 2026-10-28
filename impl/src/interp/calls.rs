// Calls: callables, user functions and type-parameter inference.

use super::*;

impl Interp {
    pub(super) fn invoke_callable(&mut self, f: EvalResult, args: Vec<EvalResult>, subst: &HashMap<String, Type>) -> EvalOutcome {
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        match f {
            EvalResult::Val(Value::FnVal(name)) => {
                if name == "$moved" {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if let Some(fdecl) = self.items.fns.get(&name).cloned() {
                    let inferred = self.infer_type_params(&fdecl, &[], &args);
                    self.ensure_checked(&fdecl, &inferred)?;
                    self.push_call_frame();
                    for (i, p) in fdecl.params.iter().enumerate() {
                        let pty = self.apply_subst(&p.ty, &inferred);
                        self.store_binding(&p.name, args[i].clone(), Some(pty))?;
                    }
                    let rty = self.apply_subst(&fdecl.ret, &inferred);
                    self.ret_stack().push(Some(rty.clone()));
                    let r = self.exec_block_body_expected(&fdecl.body, &inferred, Some(&rty));
                    self.ret_stack().pop();
                    self.pop_frame(result_keep(&r))?;
                    return match r {
                        Ok(v) => Ok(v),
                        Err(Flow::Return(v)) => Ok(v),
                        Err(other) => Err(other),
                    };
                }
                Err(Flow::Fault("diag.unbound-name".into()))
            }
            EvalResult::Val(Value::Closure(h)) => self.call_closure_vals(h.id(), args, subst),
            EvalResult::Place(o, path, excl) if !path.is_empty() => {
                // A `fn` value in a field or element.
                match self.read_place(o, &path) {
                    Value::FnVal(name) => self.invoke_callable(EvalResult::Val(Value::FnVal(name)), args, subst),
                    Value::Closure(c) => {
                        self.self_write_call(c.id(), o, &path, &excl)?;
                        self.call_closure_vals(c.id(), args, subst)
                    }
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                // A binding or field holding a *fn value* (`[T-Item]`: a
                // named function stored in a `fn(...)`-typed place) is
                // called by name; anything else here is a closure object.
                if let Some(Value::FnVal(name)) = self.objects.get(&o).map(|ob| ob.value.clone()) {
                    return self.invoke_callable(EvalResult::Val(Value::FnVal(name)), args, subst);
                }
                if let EvalResult::Place(_, _, excl) = &f {
                    self.self_write_call(o, o, &[], excl)?;
                }
                self.call_closure_vals(o, args, subst)
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    pub(super) fn call_closure_vals(&mut self, obj: u64, args: Vec<EvalResult>, subst: &HashMap<String, Type>) -> EvalOutcome {
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        let obj = self.closure_target(obj);
        let cid = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => *id,
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let info = self.closures.borrow().get(&cid).cloned().ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
        let params = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
        if args.len() != params.len() {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        self.push_call_frame();
        for (i, p) in params.iter().enumerate() {
            self.store_binding(&p.name, args[i].clone(), Some(self.apply_subst(&p.ty, subst)))?;
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

    pub(super) fn eval_call(&mut self, callee: &Expr, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        // enum variant constructor with a payload: `Vi(e)` (rule.agg.enum-construct).
        if let ExprKind::Path(segs, targs) = &callee.kind {
            if args.len() == 1 {
                // A variant may be named bare (`Has(x)`) or qualified by
                // its enum (`E::Has(x)`, `spec/17` §1's `M::E::Vi` form);
                // either way the *last* segment is what names it.
                if let Some(res) = self.try_variant_construct(segs, targs, &args[0], subst, None)? {
                    return Ok(res);
                }
            }
        }
        // `[Resolve-Unqualified]`: a local binding comes before an item,
        // and an item before an intrinsic of the same name (spec/21 §0).
        // A local is called below, as the value it holds.
        if let ExprKind::Path(segs, targs) = &callee.kind {
            let local = segs.len() == 1 && self.lookup(&segs[0]).is_some();
            if !local {
                let name = segs.join("::");
                if let Some(f) = self.items.fns.get(&name).cloned() {
                    self.pc_site = Some((callee.line, callee.col));
                    return self.call_user_fn(&f, targs, args, subst);
                }
                if let Some(ext) = self.items.externs.get(&name).cloned() {
                    return self.call_extern(&ext, args, subst);
                }
                if let Some(res) = self.try_intrinsic(&name, targs, args, subst)? {
                    return Ok(res);
                }
            }
        }
        // callee is an expression evaluating to a FnVal or a closure object
        let cv = self.eval(callee, subst)?;
        match cv {
            EvalResult::Val(Value::FnVal(name)) => {
                if name == "$moved" {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if let Some(f) = self.items.fns.get(&name).cloned() {
                    return self.call_user_fn(&f, &[], args, subst);
                }
                if let Some(ext) = self.items.externs.get(&name).cloned() {
                    return self.call_extern(&ext, args, subst);
                }
                Err(Flow::Fault("diag.unbound-name".into()))
            }
            EvalResult::Val(Value::Closure(h)) => self.call_closure(h.id(), args, subst),
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                // The value at the place: a field or element holding a
                // `fn` value (`(t.less)(a, b)`) is a part of its object.
                let held = match &cv {
                    // (through `read_place`: a place may be raw cells, as a
                    // `HashMap`'s value `*f` is)
                    EvalResult::Place(_, path, _) => Some(self.read_place(o, path)),
                    _ => self.objects.get(&o).map(|ob| ob.value.clone()),
                };
                if let Some(Value::Closure(c)) = held {
                    if let EvalResult::Place(_, path, excl) = &cv {
                        self.self_write_call(c.id(), o, path, excl)?;
                    }
                    return self.call_closure(c.id(), args, subst);
                }
                if let Some(Value::FnVal(name)) = held {
                    // As a direct call would: each argument at its
                    // parameter's type when the function is not generic.
                    let decl = self.items.fns.get(&name).cloned().filter(|f| f.type_params.is_empty());
                    let mut argvals = Vec::new();
                    for (i, a) in args.iter().enumerate() {
                        match decl.as_ref().and_then(|f| f.params.get(i)).filter(|p| is_bare_num_literal(a) && matches!(p.ty, Type::Int(_) | Type::F32 | Type::F64)) {
                            Some(p) => argvals.push({ let r0 = self.eval_expected(a, subst, &p.ty)?; self.arg_now(r0)? }),
                            None => argvals.push({ let r0 = self.eval(a, subst)?; self.arg_now(r0)? }),
                        }
                    }
                    return self.invoke_callable(EvalResult::Val(Value::FnVal(name)), argvals, subst);
                }
                // A closure called straight from an expression is a
                // temporary: it ends with the statement (with its
                // captures, if it owns any).
                if matches!(cv, EvalResult::Temp(_)) {
                    self.stmt_temp(o);
                }
                if let EvalResult::Place(_, path, excl) = &cv {
                    self.self_write_call(o, o, path, excl)?;
                }
                self.call_closure(o, args, subst)
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    pub(super) fn call_user_fn(&mut self, f: &Arc<FnDecl>, targs: &[Type], args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        if stack_exhausted() {
            return Err(Flow::Fault("diag.stack-exhausted".into()));
        }
        self.call_user_fn_expected(f, targs, args, subst, None)
    }

    pub(super) fn call_user_fn_expected(
        &mut self,
        f: &Arc<FnDecl>,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
        expected: Option<Type>,
    ) -> EvalOutcome {
        // The call's line: a fault binding a parameter (`[Call]`'s `store`,
        // D-0049's reborrow) is the call's, not a line of the callee's body
        // its lazy check may have left current (D-0219).
        let call_line = self.current_line;
        let site = self.pc_site.take();
        let targs: Vec<Type> = targs.iter().map(|t| self.apply_subst(t, subst)).collect();
        let mut argvals = Vec::with_capacity(args.len());
        let mut inferred_skip: Vec<usize> = Vec::new();
        if f.type_params.is_empty() {
            // Every parameter's type is already fully concrete (no
            // generic inference needed), so it's known *before*
            // evaluating any argument -- thread it through
            // (`rule.type.expected`) so an un-suffixed literal argument
            // outside i32's range (`f(5000000000)` where `f(i64 x)`)
            // gets its real type instead of always defaulting to i32.
            for (a, p) in args.iter().zip(f.params.iter()) {
                argvals.push({ let r0 = self.eval_expected(a, subst, &p.ty)?; self.arg_now(r0)? });
            }
            for a in args.iter().skip(f.params.len()) {
                argvals.push({ let r0 = self.eval(a, subst)?; self.arg_now(r0)? });
            }
        } else {
            // A parameter whose type the explicit type arguments and the
            // arguments to its left have made concrete is an expected
            // type for its argument (`Vec::push(&mut v, Some(5000000000))`).
            // D-0079: an unsuffixed literal whose parameter is not yet
            // known waits until the other arguments and the expected type
            // have fixed what they can, and is then evaluated at its
            // parameter's type (it has no effects, so this is unobservable).
            let mut deferred: Vec<usize> = Vec::new();
            // The deferred arguments evaluated at a type their context
            // fixed: that type, not their value's own (a `None` has no
            // payload to show one), is what they tell the inference.
            let mut fixed: Vec<usize> = Vec::new();
            for (i, a) in args.iter().enumerate() {
                let known = self.infer_skipping(f, &targs, &argvals, &deferred, None);
                let mut hint = f
                    .params
                    .get(i)
                    .map(|p| self.apply_subst(&p.ty, &known))
                    .filter(|t| !mentions_type_param(t, &f.type_params));
                // A generic call as the argument (`Arc::new(Channel::new(4))`
                // for an `Arc<Channel<u32>>`): what the call's expected type
                // fixes is the argument's, or the inner call's own `T` is
                // left to a default and its storage sized for that.
                if hint.is_none() && expected.is_some() && matches!(a.kind, ExprKind::Call(..)) {
                    let known = self.infer_skipping(f, &targs, &argvals, &deferred, expected.as_ref());
                    hint = f.params.get(i).map(|p| self.apply_subst(&p.ty, &known)).filter(|t| !mentions_type_param(t, &f.type_params));
                }
                argvals.push(match hint {
                    Some(t) => { let r0 = self.eval_expected(a, subst, &t)?; self.arg_now(r0)? },
                    // A payload-less variant of a generic enum (`None`)
                    // carries no type argument of its own: it waits too,
                    // so the expected type fixes it (`Vec::filled(n, None)`
                    // for a `Vec<Option<usize>>`), not a default.
                    None if is_bare_num_literal(a) || self.is_context_typed_variant(a) => {
                        deferred.push(i);
                        EvalResult::Val(Value::Unit)
                    }
                    None => { let r0 = self.eval(a, subst)?; self.arg_now(r0)? },
                });
            }
            if !deferred.is_empty() {
                let known = self.infer_skipping(f, &targs, &argvals, &deferred, expected.as_ref());
                for &i in &deferred {
                    let hint = f.params.get(i).map(|p| self.apply_subst(&p.ty, &known)).filter(|t| !mentions_type_param(t, &f.type_params));
                    argvals[i] = match hint {
                        Some(t) => {
                            fixed.push(i);
                            self.eval_expected(&args[i], subst, &t)?
                        }
                        None => self.eval(&args[i], subst)?,
                    };
                }
            }
            inferred_skip = fixed;
        }
        let inferred = self.infer_skipping(f, &targs, &argvals, &inferred_skip, expected.as_ref());
        self.ensure_checked(f, &inferred)?;
        self.push_call_frame();
        for (i, p) in f.params.iter().enumerate() {
            let pty = self.apply_subst(&p.ty, &inferred);
            // D-0049 (`rule.fn.bind-param`): an exclusive reference (or slice)
            // for a shared parameter is first reborrowed shared through it,
            // with `[Borrow]`'s check, at the call (D-0219: ordinary calls
            // bound it as it was, and a conflict surfaced only at a later
            // access in the callee).
            let r = self.weaken_arg(&argvals[i], Some(&pty)).and_then(|r| self.store_binding(&p.name, r, Some(pty.clone())));
            if let Err(e) = r {
                return Err(match e {
                    Flow::Fault(d) if !crate::diagnostics::has_location(&d) && user_line(call_line) => Flow::Fault(format!("{d}@{call_line}")),
                    other => other,
                });
            }
        }
        // D-0283: the call's point checks, with its parameters bound and
        // before the body runs (the fault is the call's).
        if let Some(pcs) = site.and_then(|s| crate::tier::point_checks_at(&self.items, s)) {
            if let Err(e) = self.run_point_checks(f, &pcs) {
                // As a fault from the body: the callee's frame ends first.
                self.pop_frame(None)?;
                return Err(match e {
                    Flow::Fault(d) if !crate::diagnostics::has_location(&d) && user_line(call_line) => Flow::Fault(format!("{d}@{call_line}")),
                    other => other,
                });
            }
        }
        let rty = self.apply_subst(&f.ret, &inferred);
        self.ret_stack().push(Some(rty.clone()));
        let r = if self.is_std_view_find(f) {
            self.native_view_find(f)
        } else if let Some(r) = if self.is_std_vec_push(f) { self.native_vec_push(f, &inferred) } else { None } {
            r
        } else if let Some(r) = if self.is_std_vec_filled(f) { self.native_vec_filled(f, &inferred, Some(&rty)) } else { None } {
            r
        } else {
            self.exec_block_body_expected(&f.body, &inferred, Some(&rty))
        };
        self.ret_stack().pop();
        self.pop_frame(result_keep(&r))?;
        let res = match r {
            Ok(v) => v,
            Err(Flow::Return(v)) => v,
            Err(other) => return Err(other),
        };
        // A struct, enum or array returned as a bare value (a local read
        // out of the callee's frame) carries no type of its own; give it
        // back as a temporary of the declared return type, which `match`
        // and field access need, ending with the caller's statement.
        match res {
            EvalResult::Val(v @ (Value::Struct(_) | Value::Enum(..) | Value::Array(_))) => {
                let id = self.new_object(rty, v);
                self.stmt_temp(id);
                Ok(EvalResult::Temp(id))
            }
            // A temporary built inside the body from a value's own type
            // (`Some(x)` with `x` a struct: `Option<void>`) takes the
            // declared return type, when that is concrete and names the
            // same type: `match` reads its payload's type from it.
            EvalResult::Temp(id) => {
                let is_res = self.is_resource(&rty);
                if let (Some(o), Type::Named(rn, _)) = (self.objects.get_mut(&id), &rty) {
                    if matches!(&o.ty, Type::Named(on, _) if on == rn) && o.ty != rty && !mentions_type_param(&rty, &f.type_params) {
                        o.ty = rty.clone();
                        o.is_resource = is_res;
                    }
                }
                Ok(EvalResult::Temp(id))
            }
            other => Ok(other),
        }
    }

    pub(super) fn infer_type_params(&self, f: &Arc<FnDecl>, explicit: &[Type], args: &[EvalResult]) -> HashMap<String, Type> {
        self.infer_type_params_expected(f, explicit, args, None)
    }

    // D-0012 §3c (argument-inferred) then D-0013 (expected-type-inferred):
    // a type parameter not fixed by an explicit `<...>` or by any
    // argument's own shape is solved by matching the function's declared
    // return type against the expected type at the call — the only route
    // by which a zero/insufficient-argument generic call like
    // `Vec::new<T>()` can ever learn `T` at all.
    pub(super) fn infer_type_params_expected(
        &self,
        f: &Arc<FnDecl>,
        explicit: &[Type],
        args: &[EvalResult],
        expected: Option<&Type>,
    ) -> HashMap<String, Type> {
        let mut m = HashMap::default();
        for (i, tp) in f.type_params.iter().enumerate() {
            if let Some(t) = explicit.get(i) {
                m.insert(tp.clone(), t.clone());
            }
        }
        if m.len() < f.type_params.len() {
            for (i, p) in f.params.iter().enumerate() {
                self.unify_type_param(&p.ty, args.get(i), &mut m, &f.type_params);
            }
        }
        if m.len() < f.type_params.len() {
            if let Some(exp) = expected {
                self.unify_type_shape(&f.ret, exp, &mut m, &f.type_params);
            }
        }
        m
    }

    // `infer_type_params_expected` with the arguments at `skip` (literals
    // not yet evaluated, D-0079) left out.
    pub(super) fn infer_skipping(&self, f: &Arc<FnDecl>, explicit: &[Type], args: &[EvalResult], skip: &[usize], expected: Option<&Type>) -> HashMap<String, Type> {
        let mut m = HashMap::default();
        for (i, tp) in f.type_params.iter().enumerate() {
            if let Some(t) = explicit.get(i) {
                m.insert(tp.clone(), t.clone());
            }
        }
        for (i, p) in f.params.iter().enumerate() {
            if !skip.contains(&i) {
                self.unify_type_param(&p.ty, args.get(i), &mut m, &f.type_params);
            }
        }
        if m.len() < f.type_params.len() {
            if let Some(exp) = expected {
                self.unify_type_shape(&f.ret, exp, &mut m, &f.type_params);
            }
        }
        m
    }

    pub(super) fn unify_type_param(&self, decl_ty: &Type, arg: Option<&EvalResult>, m: &mut HashMap<String, Type>, params: &[String]) {
        let arg = match arg {
            Some(a) => a,
            None => return,
        };
        // A closure is a value of its `fn(…) : R` type (D-0073), which a
        // type parameter it fixes takes -- not its capture struct's.
        let hint = self.result_type_hint(arg);
        let actual = if matches!(decl_ty, Type::Fn(..)) || matches!(hint, Some(Type::Closure(_))) { self.callable_type(arg).or(hint) } else { hint };
        let actual = match actual {
            Some(t) => t,
            None => return,
        };
        self.unify_type_shape(decl_ty, &actual, m, params);
    }

    // Structural unification of a (possibly type-parameter-containing)
    // declared type against a concrete runtime type, recording every bare
    // type-parameter name it finds bound (D-0012 §3c's argument-inferred
    // case, done structurally rather than via a separate type checker).
    pub(super) fn unify_type_shape(&self, decl_ty: &Type, actual: &Type, m: &mut HashMap<String, Type>, params: &[String]) {
        match (decl_ty, actual) {
            // Only one of the callee's own type parameters is bound: a
            // declared type named without arguments (`handle<Noisy>`) is a
            // type, not a name to infer -- and a program's struct `T` must
            // not stop `std`'s generics from inferring theirs. Before D-0208
            // `get(handle<Noisy> h)` bound `Noisy := Void` from the handle's
            // shape, and the body's `Ok(join(h))` was then read as a value
            // (`try_join` of a resource result faulted `diag.read-of-resource`).
            (Type::Named(n, args), _) if args.is_empty() && params.contains(n) => {
                m.entry(n.clone()).or_insert_with(|| actual.clone());
            }
            (Type::Ref(dinner, _), Type::Ref(ainner, _)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Rawptr(dinner), Type::Rawptr(ainner)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Array(dinner, _), Type::Array(ainner, _)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Slice(dinner, _), Type::Slice(ainner, _)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Handle(dinner), Type::Handle(ainner)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Mutex(dinner), Type::Mutex(ainner)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Guard(dinner), Type::Guard(ainner)) => self.unify_type_shape(dinner, ainner, m, params),
            (Type::Fn(dps, dr), Type::Fn(aps, ar)) if dps.len() == aps.len() => {
                for (dp, ap) in dps.iter().zip(aps.iter()) {
                    self.unify_type_shape(dp, ap, m, params);
                }
                self.unify_type_shape(dr, ar, m, params);
            }
            (Type::Named(_, dargs), Type::Named(_, aargs)) => {
                for (dp, ap) in dargs.iter().zip(aargs.iter()) {
                    self.unify_type_shape(dp, ap, m, params);
                }
            }
            _ => {}
        }
    }

    // The `fn(…) : R` type of a function value or closure, for inference.
    pub(super) fn callable_type(&self, r: &EvalResult) -> Option<Type> {
        let v = match r {
            EvalResult::Val(v) => v.clone(),
            EvalResult::Place(o, p, _) => self.read_place(*o, p),
            EvalResult::Temp(o) => match self.objects.get(o) {
                Some(ob) if matches!(ob.ty, Type::Closure(_)) => Value::Closure(crate::value::ClosureRef::view(*o)),
                Some(ob) => ob.value.clone(),
                None => return None,
            },
        };
        let v = match (&v, r) {
            (Value::Struct(_), EvalResult::Place(o, p, _)) if p.is_empty() && self.is_closure_obj(*o) => Value::Closure(crate::value::ClosureRef::view(*o)),
            _ => v,
        };
        match v {
            Value::FnVal(name) => {
                let f = self.items.fns.get(&name)?;
                if !f.type_params.is_empty() {
                    return None;
                }
                Some(Type::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone())))
            }
            Value::Closure(o) => {
                let o = self.closure_target(o.id());
                let Some(Type::Closure(cid)) = self.objects.get(&o).map(|ob| ob.ty.clone()) else { return None };
                let ps: Vec<Type> = self.closure_params.borrow().get(&cid)?.iter().map(|p| p.ty.clone()).collect();
                let r = self.closure_rets.borrow().get(&cid).cloned()?;
                Some(Type::Fn(ps, Box::new(r)))
            }
            _ => None,
        }
    }
}
