// Variant construction, expected types and literals.

use super::*;

impl Interp {
    pub(super) fn try_variant_construct(
        &mut self,
        segs: &[String],
        targs: &[Type],
        arg: &Expr,
        subst: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let name = segs.last().unwrap().as_str();
        let enum_name = match self.variant_enum(segs) {
            Some(e) => e,
            None => return Ok(None),
        };
        let e = self.items.enums.get(&enum_name).cloned().unwrap();
        let vi = e.variants.iter().position(|v| v.name == name).unwrap();
        if e.variants[vi].payload.is_none() {
            return Ok(None); // handled by eval_path as a bare value
        }
        let raw_payload = e.variants[vi].payload.clone().unwrap();
        let mut m: HashMap<String, Type> = HashMap::default();
        for (p, t) in e.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.apply_subst(t, subst));
        }
        // `rule.type.expected`: an expected type of this enum supplies the
        // type arguments not written (`Some(5000000000)` as an `Option<u64>`).
        if let Some(Type::Named(n, eargs)) = expected {
            if *n == enum_name && eargs.len() == e.type_params.len() {
                for (p, t) in e.type_params.iter().zip(eargs.iter()) {
                    if !m.contains_key(p) {
                        m.insert(p.clone(), t.clone());
                    }
                }
            }
        }
        // Evaluate with whatever's already known threaded down (so e.g.
        // `Ok(Vec::new())` can still resolve `Vec`'s own `T` from a wider
        // expected type, if one was threaded into this call), then
        // structurally infer any of *this* enum's still-unbound type
        // parameters from the argument's own runtime shape (`Some(5)`
        // fixes `Option<T>`'s `T`) against the RAW (unsubstituted)
        // payload type — unifying against an already-substituted
        // placeholder would just match itself and bind nothing.
        let hint_ty = self.apply_subst(&raw_payload, &m);
        let r = self.eval_expected(arg, subst, &hint_ty)?;
        if let Some(hint) = self.result_type_hint(&r) {
            self.unify_type_shape(&raw_payload, &hint, &mut m, &e.type_params);
        }
        let args: Vec<Type> = e.type_params.iter().map(|p| m.get(p).cloned().unwrap_or(Type::Void)).collect();
        let fsubst = self.subst_map(&e.type_params, &args);
        let pty = self.apply_subst(&raw_payload, &fsubst);
        let v = self.store_into_field(r, &pty)?;
        let ty = Type::Named(enum_name, args);
        let id = self.new_object(ty, Value::Enum(vi, Box::new(v)));
        Ok(Some(EvalResult::Temp(id)))
    }

    // Threads an expected type (`rule.type.expected`) into the small set
    // of positions where this interpreter can actually use one to resolve
    // an otherwise-unresolvable generic type parameter: a call with too
    // few arguments to infer from (`Vec::new<T>()`) and a nested struct
    // literal field (`Box { .value = Vec::new() }`). Everything else
    // falls back to ordinary (argument-inferred-only) evaluation.
    // Tags an untagged fault with `e`'s line, as `eval` does: some forms
    // (a variant constructor with its payload's expected type) are
    // evaluated here without passing through `eval` itself.
    pub(super) fn eval_expected(&mut self, e: &Expr, subst: &HashMap<String, Type>, expected: &Type) -> EvalOutcome {
        self.current_line = e.line;
        let r = self.eval_expected_inner(e, subst, expected);
        tag_at(r, e.line)
    }

    // A borrowed payload-less variant the checker let through as an
    // argument's temporary (`&None`).
    pub(super) fn variant_temp_borrow(&self, e: &Expr, inner: &Expr) -> bool {
        matches!(inner.kind, ExprKind::Path(..) | ExprKind::Paren(_)) && self.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize))
    }

    pub(super) fn eval_temp_borrow(&mut self, mode: &Mode, inner: &Expr, subst: &HashMap<String, Type>, pointee: Option<&Type>) -> EvalOutcome {
        // D-0073: the value becomes a temporary of this statement,
        // borrowed from its root; it ends when the statement does.
        let r = match pointee {
            Some(t) => self.eval_expected(inner, subst, t)?,
            None => self.eval(inner, subst)?,
        };
        let o = match r {
            EvalResult::Temp(o) => o,
            EvalResult::Val(v) => {
                let ty = self.type_of_value(&v);
                self.new_object(ty, v)
            }
            EvalResult::Place(..) => return Err(Flow::Fault("diag.borrow-of-non-place".into())),
        };
        // Adopted by this thread, as a binding adopts a temporary
        // (`join`'s result was established in another).
        if let Some(ob) = self.objects.get_mut(&o) {
            ob.owner_thread = self.current_thread;
        }
        self.stmt_temp(o);
        let pointee = self.type_at(o, &[]);
        let token = self.mint_token(&mode);
        Ok(EvalResult::Val(Value::Ref { obj: o, path: Vec::new(), mode: mode.clone(), pointee, token, range: None, anc: None }))
    }

    pub(super) fn eval_expected_inner(&mut self, e: &Expr, subst: &HashMap<String, Type>, expected: &Type) -> EvalOutcome {
        match &e.kind {
            // D-0084: a borrowed literal takes the referent type expected.
            ExprKind::Borrow(mode, inner) if (crate::typecheck::is_temp_value(inner) || self.variant_temp_borrow(e, inner)) && !self.addressed_const(inner) => match expected {
                Type::Ref(t, _) => {
                    let t = (**t).clone();
                    self.eval_temp_borrow(mode, inner, subst, Some(&t))
                }
                _ => self.eval_temp_borrow(mode, inner, subst, None),
            },
            ExprKind::ArrayLit(items) => match expected {
                Type::Array(inner, _) | Type::Simd(inner, _) => {
                    let inner = (**inner).clone();
                    self.eval_array_lit(items, subst, Some(&inner))
                }
                _ => self.eval_array_lit(items, subst, None),
            },
            ExprKind::Closure { .. } if matches!(expected, Type::Fn(..)) => {
                let r = self.eval(e, subst)?;
                if let (Type::Fn(_, ret), EvalResult::Temp(o)) = (expected, &r) {
                    if let Some(Type::Closure(cid)) = self.objects.get(o).map(|ob| ob.ty.clone()) {
                        self.closure_rets.borrow_mut().insert(cid, (**ret).clone());
                    }
                }
                Ok(r)
            }
            // D-0254: an operator where a `simd` is expected: both operands
            // are of that type.
            ExprKind::Binary(op, l, r)
                if matches!(expected, Type::Simd(..))
                    && matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr) =>
            {
                let lr = self.eval_expected(l, subst, expected)?;
                let lv = self.result_to_value(lr)?;
                let rr = self.eval_expected(r, subst, expected)?;
                let rv = self.result_to_value(rr)?;
                self.apply_binop(*op, lv, rv)
            }
            ExprKind::ArrayRepeat(item, n) => match expected {
                Type::Array(inner, _) | Type::Simd(inner, _) => {
                    let inner = (**inner).clone();
                    self.eval_array_repeat(item, *n, subst, Some(&inner))
                }
                _ => self.eval_array_repeat(item, *n, subst, None),
            },
            ExprKind::IntLit(v, None) if matches!(expected, Type::F32 | Type::F64) && self.items.number_literals.lock().unwrap().contains(&(e as *const Expr as usize)) => {
                // D-0090: `T x = 0` with `T` a floating-point type.
                Ok(EvalResult::Val(if *expected == Type::F32 { Value::F32(*v as f32) } else { Value::F64(*v as f64) }))
            }
            ExprKind::IntLit(v, suf) => self.eval_int_lit_expected(*v, suf.as_deref(), Some(expected)),
            // D-0037: an operator on literal expressions only is computed
            // in the number type its context expects.
            ExprKind::Binary(op, l, r) if is_literal_expr(e) && matches!(expected, Type::Int(_) | Type::F32 | Type::F64) => {
                let lv = self.eval_expected(l, subst, expected)?;
                let lv = self.result_to_value(lv)?;
                let rv = if matches!(op, BinOp::Shl | BinOp::Shr) { self.eval_to_value(r, subst)? } else {
                    let rv = self.eval_expected(r, subst, expected)?;
                    self.result_to_value(rv)?
                };
                let r = self.apply_binop(*op, lv, rv);
                tag_at(r, e.line)
            }
            ExprKind::Paren(inner) if is_literal_expr(inner) => self.eval_expected(inner, subst, expected),
            ExprKind::Unary(UnOp::Neg, inner) if is_literal_expr(inner) && !matches!(inner.kind, ExprKind::IntLit(..)) && matches!(expected, Type::Int(_) | Type::F32 | Type::F64) => {
                let v = self.eval_expected(inner, subst, expected)?;
                match self.result_to_value(v)? {
                    Value::Int(t, x) => {
                        let r = checked_neg(t, x).map_err(|_| Flow::Fault("diag.arith-overflow".into()));
                        tag_at(r.map(|r| EvalResult::Val(Value::Int(t, r))), e.line)
                    }
                    Value::F32(x) => Ok(EvalResult::Val(Value::F32(-x))),
                    Value::F64(x) => Ok(EvalResult::Val(Value::F64(-x))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            ExprKind::Unary(UnOp::BitNot, inner) if is_literal_expr(inner) && matches!(expected, Type::Int(_)) => {
                let v = self.eval_expected(inner, subst, expected)?;
                match self.result_to_value(v)? {
                    Value::Int(t, x) => {
                        let bw = t.bitwidth(ADDR_WIDTH);
                        let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                        Ok(EvalResult::Val(Value::Int(t, reinterpret_sign_pub(t, (!(x as u128)) & mask, bw))))
                    }
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            // An unsuffixed float literal takes `f32` from its context
            // (`[Literal-Type-From-Context]`), rounded to it, as `-L` does.
            ExprKind::FloatLit(v, None) if *expected == Type::F32 => Ok(EvalResult::Val(Value::F32(*v as f32))),
            ExprKind::Unary(UnOp::Neg, inner) if *expected == Type::F32 && matches!(inner.kind, ExprKind::FloatLit(_, None)) => {
                let ExprKind::FloatLit(v, _) = &inner.kind else { unreachable!() };
                Ok(EvalResult::Val(Value::F32(-(*v as f32))))
            }
            ExprKind::Unary(UnOp::Neg, inner)
                if matches!(inner.kind, ExprKind::IntLit(_, None)) && matches!(expected, Type::F32 | Type::F64) && self.items.number_literals.lock().unwrap().contains(&(&**inner as *const Expr as usize)) =>
            {
                let ExprKind::IntLit(v, _) = &inner.kind else { unreachable!() };
                Ok(EvalResult::Val(if *expected == Type::F32 { Value::F32(-(*v as f32)) } else { Value::F64(-(*v as f64)) }))
            }
            ExprKind::Unary(UnOp::Neg, inner) if matches!(inner.kind, ExprKind::IntLit(..)) => {
                if let ExprKind::IntLit(v, suf) = &inner.kind {
                    self.eval_neg_int_lit_expected(*v, suf.as_deref(), Some(expected))
                } else {
                    unreachable!()
                }
            }
            ExprKind::Call(callee, args) => {
                if let ExprKind::Path(segs, targs) = &callee.kind {
                    if args.len() == 1 && self.lookup(&segs[segs.len() - 1]).is_none() {
                        if let Some(res) = self.try_variant_construct(segs, targs, &args[0], subst, Some(expected))? {
                            return Ok(res);
                        }
                    }
                    let name = segs.join("::");
                    if name == "Mutex::new" && args.len() == 1 {
                        let hint = match (targs.first(), expected) {
                            (Some(t), _) => Some(self.apply_subst(t, subst)),
                            (None, Type::Mutex(i)) => Some((**i).clone()),
                            _ => None,
                        };
                        return self.mutex_new(&args[0], subst, hint.as_ref());
                    }
                    if let Some(f) = self.items.fns.get(&name).cloned() {
                        if !f.type_params.is_empty() {
                            return self.call_user_fn_expected(&f, targs, args, subst, Some(expected.clone()));
                        }
                    }
                }
                self.eval(e, subst)
            }
            ExprKind::StructLit(segs, targs, fields) => {
                self.eval_struct_lit(segs, targs, fields, subst, Some(expected.clone()))
            }
            // The expected type reaches the trailing expression of a
            // block, of each `if` branch and of each `match` arm, and
            // through parentheses (`rule.type.expected`).
            ExprKind::Paren(inner) => self.eval_expected(inner, subst, expected),
            ExprKind::Block(b) => self.exec_block_body_expected(b, subst, Some(expected)),
            ExprKind::If(c, t, f) => self.eval_if(c, t, f, subst, Some(expected)),
            ExprKind::Match(scrut, arms) => self.eval_match(scrut, arms, subst, Some(expected)),
            _ => self.eval(e, subst),
        }
    }

    // `rule.arith.literal`'s actual priority: an explicit suffix always
    // wins; otherwise the expected type from context *if it is a
    // concrete, fully-resolved integer type* (a bare unbound generic
    // type parameter reaching here -- e.g. `expected` is still
    // `Type::Named(param_name, [])` for some unresolved `T` -- gives no
    // real information, so falls through to the same default as no
    // expected type at all); otherwise `i32`. This was previously only
    // implemented at all for the un-suffixed-default case (`eval_inner`'s
    // own `IntLit` arm, unconditionally `i32`) -- every expected-type
    // context calls through here instead now.
    pub(super) fn eval_int_lit_expected(&self, v: u128, suf: Option<&str>, expected: Option<&Type>) -> EvalOutcome {
        let ty = IntTy::of_literal(suf, expected).0;
        if !in_range(ty, v as i128) {
            return Err(Flow::Fault("diag.literal-out-of-range".into()));
        }
        Ok(EvalResult::Val(Value::Int(ty, v as i128)))
    }

    // The "most negative literal" case (`-2147483648`, `-9223372036854
    // 775808`, ...): the *positive* magnitude (2^(bw-1)) does not fit
    // the type's positive range, but its negation is exactly `min(ty)`,
    // a perfectly legal value. Checks the range of the negated value,
    // not the positive magnitude, so this is accepted where a naive
    // "evaluate the literal, then negate" (the general `eval_unary`
    // path) would reject it. `bw==128` (`i128`'s own MIN) is special-
    // cased separately: `v as i128` for `v == 2^127` already
    // bit-reinterprets to `i128::MIN` (not a genuine positive value
    // i128 could hold), so a plain `-(v as i128)` would itself be a
    // second instance of the exact host-overflow bug `int_min_max` had
    // -- computed via bit-pattern identity instead, never negated.
    pub(super) fn eval_neg_int_lit_expected(&self, v: u128, suf: Option<&str>, expected: Option<&Type>) -> EvalOutcome {
        let ty = IntTy::of_literal(suf, expected).0;
        let bw = ty.bitwidth(ADDR_WIDTH);
        let neg: i128 = if bw >= 128 {
            if v == (1u128 << 127) {
                i128::MIN
            } else if v < (1u128 << 127) {
                -(v as i128)
            } else {
                return Err(Flow::Fault("diag.literal-out-of-range".into()));
            }
        } else {
            -(v as i128)
        };
        if !in_range(ty, neg) {
            return Err(Flow::Fault("diag.literal-out-of-range".into()));
        }
        Ok(EvalResult::Val(Value::Int(ty, neg)))
    }

    pub(super) fn eval_struct_lit(
        &mut self,
        segs: &[String],
        targs: &[Type],
        fields: &[(String, Expr)],
        subst: &HashMap<String, Type>,
        expected: Option<Type>,
    ) -> EvalOutcome {
        // The resolver has rewritten `segs` to the struct's item key
        // (bare at the root, `m::Name` inside a module).
        let name = segs.join("::");
        if let Some(sdecl) = self.items.structs.get(&name).cloned() {
            let mut m: HashMap<String, Type> = HashMap::default();
            for (i, t) in sdecl.type_params.iter().zip(targs.iter()) {
                m.insert(i.clone(), self.apply_subst(t, subst));
            }
            if m.len() < sdecl.type_params.len() {
                if let Some(exp) = &expected {
                    self.unify_type_shape(&Type::Named(name.clone(), sdecl.type_params.iter().map(|p| Type::Named(p.clone(), vec![])).collect()), exp, &mut m, &sdecl.type_params);
                }
            }
            // Evaluate every field with whatever type-parameter bindings
            // are already known (from explicit args or the expected type)
            // threaded down as ITS expected type, so a nested generic call
            // or struct literal can resolve too; then structurally infer
            // any parameter still unbound from the field values' own
            // runtime shape (D-0013, done from values here).
            let mut idxs = Vec::with_capacity(fields.len());
            let mut results = Vec::with_capacity(fields.len());
            for (fname, fexpr) in fields {
                let idx = sdecl
                    .fields
                    .iter()
                    .position(|f| &f.name == fname)
                    .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                let fty_hint = self.apply_subst(&sdecl.fields[idx].ty, &m);
                let r = { let r0 = self.eval_expected(fexpr, subst, &fty_hint)?; self.arg_now(r0)? };
                idxs.push(idx);
                results.push(r);
            }
            for (idx, r) in idxs.iter().zip(results.iter()) {
                if let Some(hint) = self.result_type_hint(r) {
                    self.unify_type_shape(&sdecl.fields[*idx].ty, &hint, &mut m, &sdecl.type_params);
                }
            }
            let args: Vec<Type> = sdecl.type_params.iter().map(|p| m.get(p).cloned().unwrap_or(Type::Void)).collect();
            let field_subst = self.subst_map(&sdecl.type_params, &args);
            let mut vals = vec![Value::Unit; sdecl.fields.len()];
            for (idx, r) in idxs.into_iter().zip(results.into_iter()) {
                let fty = self.apply_subst(&sdecl.fields[idx].ty, &field_subst);
                let v = self.store_into_field(r, &fty)?;
                vals[idx] = v;
            }
            // D-0118 `[Bitfield-Overflow]` at construction.
            if let Some((_, widths)) = self.bitstruct_decl(&name) {
                for (i, v) in vals.iter().enumerate() {
                    if let Value::Int(_, x) = v {
                        if *x < 0 || (*x as u128) > Self::bits_mask(widths[i]) {
                            return Err(Flow::Fault("diag.narrowing-overflow".into()));
                        }
                    }
                }
            }
            let ty = Type::Named(name.clone(), args);
            let id = self.new_object(ty, Value::Struct(vals));
            return Ok(EvalResult::Temp(id));
        }
        // enum variant with payload: `Vi(e)` parses as Call in this
        // grammar's postfix form, not StructLit; unreachable in practice.
        Err(Flow::Fault("diag.unbound-name".into()))
    }

    // Implements the plain-copy / resource-move logic of `store` for a
    // sub-range write (rule.value-object.store [Store-Sub-*]).
    pub(super) fn store_into_field(&mut self, r: EvalResult, fty: &Type) -> Result<Value, Flow> {
        if let EvalResult::Temp(o) = &r {
            if self.is_closure_obj(*o) {
                return Ok(self.closure_box(*o));
            }
        }
        if let EvalResult::Place(o, path, excl) = &r {
            if path.is_empty() && self.is_closure_obj(*o) && self.is_resource(&self.type_at(*o, &[])) {
                if !excl.is_empty() || !self.solitary(*o, excl) {
                    return Err(Flow::Fault("diag.move-while-aliased".into()));
                }
                self.invalidate_binding_pointing_at(*o);
                return Ok(self.closure_box(*o));
            }
        }
        if self.is_resource(fty) {
            match r {
                EvalResult::Temp(o) => {
                    let v = self.remove_object(&o).unwrap().value;
                    Ok(v)
                }
                EvalResult::Place(obj, path, excl) => {
                    if !path.is_empty() {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    if !excl.is_empty() || !self.solitary(obj, &excl) {
                        return Err(Flow::Fault("diag.move-while-aliased".into()));
                    }
                    let v = self.remove_object(&obj).unwrap().value;
                    self.invalidate_binding_pointing_at(obj);
                    Ok(v)
                }
                EvalResult::Val(v) => Ok(v),
            }
        } else {
            self.result_to_value(r)
        }
    }

    // `elem`: the element type an expected `array<τ, N>` gives each item
    // (`rule.type.expected`), so `[0, -1]` declared `array<i64, 2>` holds
    // i64s, and a nested literal passes its own element type down.
    pub(super) fn eval_array_lit(&mut self, items: &[Expr], subst: &HashMap<String, Type>, elem: Option<&Type>) -> EvalOutcome {
        let mut vals = Vec::new();
        let mut elem_ty = elem.cloned();
        for it in items {
            // With no element type from outside, the first element's type
            // is the expected type of the rest (`[0: u64, 5000000000]`).
            let r = match (elem, elem_ty.clone()) {
                (Some(t), _) => self.eval_expected(it, subst, t)?,
                (None, Some(t @ (Type::Int(_) | Type::F32 | Type::F64))) => self.eval_expected(it, subst, &t)?,
                _ => self.eval(it, subst)?,
            };
            let ty = self.result_type_hint(&r);
            if elem_ty.is_none() {
                elem_ty = ty;
            }
            let v = if self.is_resource(elem_ty.as_ref().unwrap_or(&Type::Void)) {
                self.store_into_field(r, elem_ty.as_ref().unwrap())?
            } else {
                self.result_to_value(r)?
            };
            vals.push(v);
        }
        let n = vals.len() as u128;
        let ety = elem_ty.unwrap_or(Type::Int(IntTy::I32));
        let ty = Type::Array(Box::new(ety), n);
        let id = self.new_object(ty, Value::Array(vals));
        Ok(EvalResult::Temp(id))
    }

    // `[v; N]` (D-0072): v read once (`[Read]`, a plain value), its value
    // in every element.
    pub(super) fn eval_array_repeat(&mut self, item: &Expr, n: u128, subst: &HashMap<String, Type>, elem: Option<&Type>) -> EvalOutcome {
        let r = match elem {
            Some(t) => self.eval_expected(item, subst, t)?,
            None => self.eval(item, subst)?,
        };
        let ety = elem.cloned().or_else(|| self.result_type_hint(&r)).unwrap_or(Type::Int(IntTy::I32));
        let v = self.result_to_value(r)?;
        let vals = vec![v; n as usize];
        let id = self.new_object(Type::Array(Box::new(ety), n), Value::Array(vals));
        Ok(EvalResult::Temp(id))
    }

    // `V(e)` for a variant V with a payload, not shadowed by a local.
    pub(super) fn is_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, _) => {
                    self.lookup(&segs[segs.len() - 1]).is_none() && !self.items.fns.contains_key(&segs.join("::")) && self.variant_enum(segs).is_some()
                }
                _ => false,
            },
            ExprKind::Paren(inner) => self.is_variant_literal(inner),
            _ => false,
        }
    }

    // A variant literal whose payload is a literal or another such: it
    // has no effects, so when it is evaluated cannot be observed.
    pub(super) fn is_pure_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(_, args) if self.is_variant_literal(e) => is_bare_literal_expr(&args[0]) || self.is_pure_variant_literal(&args[0]),
            ExprKind::Paren(inner) => self.is_pure_variant_literal(inner),
            _ => false,
        }
    }

    pub(super) fn result_type_hint(&self, r: &EvalResult) -> Option<Type> {
        match r {
            EvalResult::Val(v) => Some(self.type_of_value(v)),
            EvalResult::Place(obj, path, _) => Some(self.type_at(*obj, path)),
            EvalResult::Temp(o) => self.objects.get(o).map(|o| o.ty.clone()),
        }
    }
}
