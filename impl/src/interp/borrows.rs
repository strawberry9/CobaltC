// Borrows, view operations and static assertions.

use super::*;

impl Interp {
    // D-0258: `K()` for a constant whose address the static pass admitted
    // (`[Ref-Form-Const]`): `&K` is one object, not a temporary.
    pub(super) fn addressed_const(&self, inner: &Expr) -> bool {
        if let ExprKind::Call(callee, args) = &inner.kind {
            if let (ExprKind::Path(segs, _), true) = (&callee.kind, args.is_empty()) {
                return self.items.const_addrs.lock().unwrap().contains(&segs.join("::"));
            }
        }
        false
    }

    pub(super) fn eval_borrow(&mut self, mode: Mode, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // mode-monotonicity (spec/08 [Borrow-Exceeds-Source]): an
        // exclusive borrow's source must be exclusive. Determined fresh
        // from `inner`'s own structure and the *current* value of
        // whatever it dereferences (`crosses_shared_ref`) — a plain
        // binding is never capped (implicit exclusive capability, per
        // D-0018); only actually crossing a `ref<_, shared>` right now,
        // in this expression, is. Checked before evaluating `inner` for
        // real so a rejected exclusive reborrow never partially commits
        // any of `inner`'s own side effects.
        if mode == Mode::Exclusive && self.crosses_shared_ref(inner, subst)? {
            return Err(Flow::Fault("diag.borrow-exceeds-source".into()));
        }
        // D-0258 `[Ref-Form-Const]`: `&K` of a constant the static pass
        // admitted (plain data, a literal): its value as one object with
        // program extent, made on the first borrow and never ended.
        if mode == Mode::Shared {
            if let ExprKind::Call(callee, args) = &inner.kind {
                if let (ExprKind::Path(segs, _), true) = (&callee.kind, args.is_empty()) {
                    let key = segs.join("::");
                    if self.items.const_addrs.lock().unwrap().contains(&key) {
                        let obj = match self.static_objs.get(&key) {
                            Some(&o) => o,
                            None => {
                                let v = self.eval_to_value(inner, subst)?;
                                let ty = self.items.fns.get(&key).map(|f| f.ret.clone()).unwrap_or(Type::Void);
                                let o = self.new_object(ty, v);
                                self.static_objs.insert(key, o);
                                o
                            }
                        };
                        let pointee = self.type_at(obj, &[]);
                        let token = self.mint_token(&mode);
                        let anc = self.record_token_ancestors(token, &[]);
                        return Ok(EvalResult::Val(Value::Ref { obj, path: vec![], mode, pointee, token, range: None, anc }));
                    }
                }
            }
        }
        self.write_ctx = mode == Mode::Exclusive;
        let r = self.eval(inner, subst);
        self.write_ctx = false;
        let r = r?;
        match r {
            EvalResult::Place(obj, path, excl) => {
                let src_ty = self.type_at(obj, &[]);
                let pointee = self.type_at(obj, &path);
                let ancestors = excl;
                if !self.clash(obj, &path, mode.clone(), &ancestors) {
                    let token = self.mint_token(&mode);
                    let anc = self.record_token_ancestors(token, &ancestors);
                    let v = Value::Ref { obj, path, mode, pointee, token, range: None, anc };
                    return Ok(EvalResult::Val(v));
                }
                let _ = src_ty;
                Err(Flow::Fault("diag.aliasing-conflict".into()))
            }
            EvalResult::Temp(_) => Err(Flow::Fault("diag.borrow-of-temporary".into())),
            EvalResult::Val(_) => Err(Flow::Fault("diag.borrow-of-non-place".into())),
        }
    }

    // D-0053: the `StringView` operation the static pass found at `e`.
    pub(super) fn view_op(&self, e: &Expr) -> Option<ViewOp> {
        self.items.view_ops.lock().unwrap().get(&(e as *const Expr as usize)).copied()
    }

    // D-0053: a `StringView` operation, carried out by `std`: the
    // expression's own operands are evaluated in order, then the call.
    pub(super) fn eval_view_op(&mut self, e: &Expr, op: ViewOp, subst: &HashMap<String, Type>) -> EvalOutcome {
        let call = |me: &mut Self, name: &str, args: Vec<EvalResult>| me.invoke_callable(EvalResult::Val(Value::FnVal(name.to_string())), args, subst);
        match (op, &e.kind) {
            (ViewOp::SliceString, ExprKind::SliceOf(_, base, lo, hi)) => {
                // `&s` as the borrow's own rule forms it; through a
                // reference, the reference itself (auto-deref). D-0135: a
                // temporary `String`, as an argument, is an object of the
                // statement, borrowed from its root.
                let temp = self.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                let mut r = if temp && crate::typecheck::is_temp_value(base) {
                    self.eval_temp_borrow(&Mode::Shared, base, subst, None)?
                } else {
                    self.eval_borrow(Mode::Shared, base, subst)?
                };
                if let EvalResult::Val(Value::Ref { obj, path, pointee: Type::Ref(..), .. }) = &r {
                    r = EvalResult::Val(self.read_place(*obj, path));
                }
                let len = match &r {
                    EvalResult::Val(Value::Ref { obj, path, .. }) => match self.read_place(*obj, path) {
                        Value::Struct(fs) => match fs.first() {
                            Some(Value::Struct(v)) => match v.get(1) {
                                Some(Value::Int(_, n)) => *n as usize,
                                _ => 0,
                            },
                            _ => 0,
                        },
                        _ => 0,
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (l, h) = self.eval_bounds(len, lo, hi, subst)?;
                call(self, "std::String::view", vec![r, l, h])
            }
            (ViewOp::SliceView, ExprKind::SliceOf(_, base, lo, hi)) => {
                let v = self.eval_to_value(base, subst)?;
                let len = match &v {
                    Value::Struct(fs) => match fs.first() {
                        Some(Value::Struct(sl)) => match sl.get(2) {
                            Some(Value::Int(_, n)) => *n as usize,
                            _ => 0,
                        },
                        _ => 0,
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (l, h) = self.eval_bounds(len, lo, hi, subst)?;
                call(self, "std::StringView::sub", vec![EvalResult::Val(v), l, h])
            }
            (ViewOp::Eq { view_left, other_is_str, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.eval_to_value(a, subst)?;
                let vb = self.eval_to_value(b, subst)?;
                let (v, o) = if view_left { (va, vb) } else { (vb, va) };
                let name = if other_is_str { "std::StringView::eq_str" } else { "std::StringView::eq" };
                let r = call(self, name, vec![EvalResult::Val(v), EvalResult::Val(o)])?;
                match self.result_to_value(r)? {
                    Value::Bool(x) => Ok(EvalResult::Val(Value::Bool(x != negate))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            (ViewOp::Less { swap, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.eval_to_value(a, subst)?;
                let vb = self.eval_to_value(b, subst)?;
                let (x, y) = if swap { (vb, va) } else { (va, vb) };
                let r = call(self, "std::StringView::less", vec![EvalResult::Val(x), EvalResult::Val(y)])?;
                match self.result_to_value(r)? {
                    Value::Bool(v) => Ok(EvalResult::Val(Value::Bool(v != negate))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            (ViewOp::TextCmp, ExprKind::Binary(op, a, b)) => {
                let ra = self.eval(a, subst)?;
                let ta = self.text_operand(ra)?;
                let rb = self.eval(b, subst)?;
                let tb = self.text_operand(rb)?;
                match (ta, tb) {
                    (Ok(x), Ok(y)) => Ok(EvalResult::Val(Value::Bool(bytes_cmp(*op, &x, &y)))),
                    // Another instantiation of a generic body: not text.
                    (Err(x), Err(y)) => self.apply_binop(*op, x, y),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            (ViewOp::KeyCmp, ExprKind::Binary(op, a, b)) => {
                // `rule.type.expected`: an enum literal takes the other
                // operand's type (`x < Some(5000000000)`), as `==` does.
                let (lvar, rvar) = (self.is_variant_literal(a), self.is_variant_literal(b));
                let (ra, rb) = if rvar && !lvar {
                    let ra = self.eval(a, subst)?;
                    let rb = match self.result_type_hint(&ra) {
                        Some(t @ Type::Named(..)) => self.eval_expected(b, subst, &t)?,
                        _ => self.eval(b, subst)?,
                    };
                    (ra, rb)
                } else if lvar && !rvar && self.is_pure_variant_literal(a) {
                    let rb = self.eval(b, subst)?;
                    let ra = match self.result_type_hint(&rb) {
                        Some(t @ Type::Named(..)) => self.eval_expected(a, subst, &t)?,
                        _ => self.eval(a, subst)?,
                    };
                    (ra, rb)
                } else {
                    let ra = self.eval(a, subst)?;
                    let rb = self.eval(b, subst)?;
                    (ra, rb)
                };
                let (ta, x) = self.key_operand(ra)?;
                let (tb, y) = self.key_operand(rb)?;
                if trace_on() {
                    eprintln!("TRACE keycmp op={:?} ta={:?} x={:?} tb={:?} y={:?}", op, ta, x, tb, y);
                }
                let named_key = |t: &Type| matches!(t, Type::Named(n, _) if n != "std::String" && n != "std::StringView");
                if named_key(&ta) && (ta == tb || !named_key(&tb)) {
                    let o = self.key_cmp(&x, &y, &ta)?;
                    return Ok(EvalResult::Val(Value::Bool(ordering_holds(*op, o))));
                }
                // Another instantiation of a generic body: text (`[Cmp-Text]`)
                // or a scalar.
                match (self.text_bytes_of(&x, &ta), self.text_bytes_of(&y, &tb)) {
                    (Some(p), Some(q)) => Ok(EvalResult::Val(Value::Bool(bytes_cmp(*op, &p, &q)))),
                    _ => self.apply_binop(*op, x, y),
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // D-0271: a `KeyCmp` operand's type and value, read where it stands:
    // a place is not moved (a `String` field is compared, not taken); a
    // temporary is kept to the statement's end.
    pub(super) fn key_operand(&mut self, r: EvalResult) -> Result<(Type, Value), Flow> {
        match r {
            EvalResult::Place(obj, path, _) => {
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                Ok((self.type_at(obj, &path), self.read_place(obj, &path)))
            }
            EvalResult::Temp(o) => {
                self.stmt_temp(o);
                Ok((self.type_at(o, &[]), self.read_place(o, &[])))
            }
            r => {
                let v = self.result_to_value(r)?;
                Ok((self.type_of_value(&v), v))
            }
        }
    }

    // D-0108: a comparison operand's bytes when it is a `String` (read in
    // place; a temporary is kept to the statement's end), else its value.
    pub(super) fn text_operand(&mut self, r: EvalResult) -> Result<Result<Vec<u8>, Value>, Flow> {
        let is_string = |t: &Type| matches!(t, Type::Named(n, _) if n == "std::String");
        let (obj, path) = match r {
            EvalResult::Place(obj, path, _) if is_string(&self.type_at(obj, &path)) => (obj, path),
            EvalResult::Temp(o) if is_string(&self.type_at(o, &[])) => {
                self.stmt_temp(o);
                (o, Vec::new())
            }
            // (Under `TextCmp` a struct value is a `String`: `eq` and
            // `ordered` admit no other struct.)
            EvalResult::Val(v @ Value::Struct(_)) => {
                let o = self.new_object(Type::Named("std::String".into(), Vec::new()), v);
                self.stmt_temp(o);
                (o, Vec::new())
            }
            r => return Ok(Err(self.result_to_value(r)?)),
        };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        Ok(Ok(self.string_bytes(obj, &path)))
    }

    // A view's bounds, with `$` its length.
    pub(super) fn eval_bounds(&mut self, len: usize, lo: &Expr, hi: &Expr, subst: &HashMap<String, Type>) -> Result<(EvalResult, EvalResult), Flow> {
        self.dl().push(len);
        let l = self.eval_expected(lo, subst, &Type::Int(IntTy::Usize)).and_then(|r| self.result_to_value(r));
        let h = match &l {
            Ok(_) => Some(self.eval_expected(hi, subst, &Type::Int(IntTy::Usize)).and_then(|r| self.result_to_value(r))),
            Err(_) => None,
        };
        self.dl().pop();
        let l = l?;
        let h = h.expect("evaluated")?;
        Ok((EvalResult::Val(l), EvalResult::Val(h)))
    }

    // D-0053: a `StringView` value's bytes, read through its slice's
    // reference to the `String`'s `Vec<u8>`.
    pub(super) fn view_bytes(&self, v: &Value) -> Result<Vec<u8>, Flow> {
        let (at, n) = self.view_span(v)?;
        Ok((0..n as u64).map(|j| self.arena_byte(at + j)).collect())
    }

    // A `StringView` value's first byte's address in the arena, and its
    // length.
    pub(super) fn view_span(&self, v: &Value) -> Result<(u64, usize), Flow> {
        let Value::Struct(fs) = v else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let Some(Value::Struct(sl)) = fs.first() else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let (Some(Value::Ref { obj, path, .. }), Some(Value::Int(_, start)), Some(Value::Int(_, n))) = (sl.first(), sl.get(1), sl.get(2)) else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        if !self.object_live(*obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        let Value::Struct(vec) = self.read_place(*obj, path) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let Some(Value::Rawptr(addr, _)) = vec.first() else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        Ok((addr + *start as u64, *n as usize))
    }

    pub(super) fn arena_byte(&self, addr: u64) -> u8 {
        *self.arena.get(&addr).unwrap_or(&0)
    }

    // Whether `f` is `std`'s own `StringView::find` (not a program's
    // function of the same name).
    pub(super) fn is_std_view_find(&self, f: &Arc<FnDecl>) -> bool {
        f.name == "find"
            && f.assoc_type.as_deref() == Some("StringView")
            && self.items.fns.get("std::StringView::find").map_or(false, |g| Arc::ptr_eq(g, f))
    }

    // `StringView::find(v, t)` carried out natively, its parameters
    // already bound in the call's frame: its body's result, the first
    // position at which `t`'s bytes occur in `v`'s, without its call of
    // `StringView::at` and two slice reads per byte compared (scanning a
    // 121-root PEM bundle took 15 s). Both views are checked as a read of
    // their bytes is.
    pub(super) fn native_view_find(&mut self, f: &Arc<FnDecl>) -> Result<EvalResult, Flow> {
        let mut spans = Vec::with_capacity(2);
        for p in &f.params {
            let Some((obj, _)) = self.lookup(&p.name) else { return Err(Flow::Fault("diag.unbound-name".into())) };
            let v = self.read_place(obj, &[]);
            spans.push(self.view_span(&v)?);
        }
        let ((v, n), (t, m)) = (spans[0], spans[1]);
        let needle: Vec<u8> = (0..m as u64).map(|j| self.arena_byte(t + j)).collect();
        let mut found = None;
        if m <= n {
            for i in 0..=(n - m) as u64 {
                if needle.iter().enumerate().all(|(j, &b)| self.arena_byte(v + i + j as u64) == b) {
                    found = Some(i);
                    break;
                }
            }
        }
        let value = match found {
            Some(i) => Value::Enum(0, Box::new(Value::Int(IntTy::Usize, i as i128))),
            None => Value::Enum(1, Box::new(Value::Unit)),
        };
        let id = self.new_object(Type::Named("std::Option".into(), vec![Type::Int(IntTy::Usize)]), value);
        Ok(EvalResult::Temp(id))
    }

    /// `static_assert`'s condition (D-0052), computed before the program
    /// runs: a constant expression, evaluated as it would be at run time,
    /// with the type arguments `subst` of the instantiation it is in.
    // D-0068: a failed `static_assert`'s formatted message (`$fmt`'s text).
    pub fn eval_static_message(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<String, String> {
        self.push_frame();
        self.st().push(Vec::new());
        let r = self.eval(e, subst).and_then(|r| self.result_to_value(r));
        let temps = self.st().pop().unwrap_or_default();
        let _ = self.end_stmt_temps(temps);
        let _ = self.pop_frame(None);
        match r {
            Ok(Value::Str(b)) => Ok(String::from_utf8_lossy(&b).into_owned()),
            Ok(_) => Err("diag.type-mismatch".to_string()),
            Err(Flow::Fault(d)) => Err(d),
            Err(_) => Err("diag.const-not-constant".to_string()),
        }
    }

    pub fn eval_static_assert(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<bool, String> {
        self.push_frame();
        self.st().push(Vec::new());
        let r = self.eval_expected(e, subst, &Type::Bool).and_then(|r| self.result_to_value(r));
        let temps = self.st().pop().unwrap_or_default();
        let _ = self.end_stmt_temps(temps);
        let _ = self.pop_frame(None);
        match r {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(_) => Err("diag.type-mismatch".to_string()),
            Err(Flow::Fault(d)) => Err(d),
            Err(_) => Err("diag.const-not-constant".to_string()),
        }
    }
}

// D-0271: whether a comparison operator holds of an ordering.
pub(super) fn ordering_holds(op: BinOp, o: std::cmp::Ordering) -> bool {
    match op {
        BinOp::Eq => o.is_eq(),
        BinOp::Ne => o.is_ne(),
        BinOp::Lt => o.is_lt(),
        BinOp::Le => o.is_le(),
        BinOp::Gt => o.is_gt(),
        _ => o.is_ge(),
    }
}
