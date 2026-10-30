// Unary and binary operators, sizes, and literal operands.

use super::*;

impl Interp {
    pub(super) fn eval_unary(&mut self, op: UnOp, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                return self.eval_neg_int_lit_expected(*v, suf.as_deref(), None);
            }
        }
        let v = self.eval_to_value(inner, subst)?;
        self.unary_value(op, v)
    }

    // An operator on one value; on a `simd` (an array value, D-0254) lane
    // by lane, each lane as the scalar operator.
    pub(super) fn unary_value(&mut self, op: UnOp, v: Value) -> EvalOutcome {
        if let Value::Array(lanes) = v {
            let mut out = Vec::with_capacity(lanes.len());
            for l in lanes {
                out.push(self.unary_value(op, l).and_then(|r| self.result_to_value(r))?);
            }
            return Ok(EvalResult::Val(Value::Array(out)));
        }
        match (op, &v) {
            (UnOp::Neg, Value::Int(t, x)) => {
                let r = checked_neg(*t, *x).map_err(|_| Flow::Fault("diag.arith-overflow".into()))?;
                Ok(EvalResult::Val(Value::Int(*t, r)))
            }
            (UnOp::Neg, Value::F32(x)) => Ok(EvalResult::Val(Value::F32(-x))),
            (UnOp::Neg, Value::F64(x)) => Ok(EvalResult::Val(Value::F64(-x))),
            (UnOp::Not, Value::Bool(b)) => Ok(EvalResult::Val(Value::Bool(!b))),
            (UnOp::BitNot, Value::Int(t, x)) => {
                let bw = t.bitwidth(ADDR_WIDTH);
                let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                let r = (!(*x as u128)) & mask;
                Ok(EvalResult::Val(Value::Int(*t, reinterpret_sign_pub(*t, r, bw))))
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    pub(super) fn eval_binary(&mut self, op: BinOp, l: &Expr, r: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        if op == BinOp::And {
            let lv = self.eval_to_value(l, subst)?;
            return match lv {
                Value::Bool(false) => Ok(EvalResult::Val(Value::Bool(false))),
                Value::Bool(true) => Ok(EvalResult::Val(self.eval_to_value(r, subst)?)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            };
        }
        if op == BinOp::Or {
            let lv = self.eval_to_value(l, subst)?;
            return match lv {
                Value::Bool(true) => Ok(EvalResult::Val(Value::Bool(true))),
                Value::Bool(false) => Ok(EvalResult::Val(self.eval_to_value(r, subst)?)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            };
        }
        // D-0254: an array literal beside a `simd` is one of its type (its
        // lanes' type read from the other operand's value). On the left it
        // is evaluated second only when every element is a literal, so no
        // effect changes its order.
        fn unparen(e: &Expr) -> &Expr {
            match &e.kind {
                ExprKind::Paren(x) => unparen(x),
                _ => e,
            }
        }
        let arr_lit = |e: &Expr| matches!(&unparen(e).kind, ExprKind::ArrayLit(_) | ExprKind::ArrayRepeat(..));
        let pure_lit = |e: &Expr| match &unparen(e).kind {
            ExprKind::ArrayLit(items) => items.iter().all(is_bare_literal_expr),
            ExprKind::ArrayRepeat(it, _) => is_bare_literal_expr(&**it),
            _ => false,
        };
        if arr_lit(l) && pure_lit(l) && !arr_lit(r) {
            let rv = self.eval_to_value(r, subst)?;
            if let Some(t) = Self::simd_type_of(&rv) {
                let res = self.eval_expected(l, subst, &t)?;
                let lv = self.result_to_value(res)?;
                return self.apply_binop(op, lv, rv);
            }
            let lv = self.eval_to_value(l, subst)?;
            return self.apply_binop(op, lv, rv);
        }
        if arr_lit(r) && !arr_lit(l) {
            let lv = self.eval_to_value(l, subst)?;
            let rv = match Self::simd_type_of(&lv) {
                Some(t) => {
                    let res = self.eval_expected(r, subst, &t)?;
                    self.result_to_value(res)?
                }
                None => self.eval_to_value(r, subst)?,
            };
            return self.apply_binop(op, lv, rv);
        }
        // `rule.type.expected`: a bare unsuffixed literal on the *left*
        // takes its type from the right operand's determined type. A
        // literal has no effects, so evaluating the right side first is
        // unobservable (D-0007 is not violated).
        // The same for a bare unsuffixed float literal: it is `f32` when
        // the other operand is (`rule.type.expected`), else `f64`.
        if let (Some(f), None) = (bare_float_literal(l), bare_float_literal(r)) {
            let rv = self.eval_to_value(r, subst)?;
            let lv = if matches!(rv, Value::F32(_)) { Value::F32(f as f32) } else { Value::F64(f) };
            return self.apply_binop(op, lv, rv);
        }
        // A literal *expression* on the left (`((1 << 40) - 1) & k`, D-0037)
        // likewise, when it is not a bare literal (handled below) and the
        // operator is not a shift (whose right side is its amount).
        if crate::ast::is_literal_expr(l) && !is_bare_literal_expr(l) && !crate::ast::is_literal_expr(r) && !matches!(op, BinOp::Shl | BinOp::Shr) {
            let rv = self.eval_to_value(r, subst)?;
            let lv = match Self::number_type_of(&rv) {
                Some(t) => {
                    let res = self.eval_expected(l, subst, &t)?;
                    self.result_to_value(res)?
                }
                None => self.eval_to_value(l, subst)?,
            };
            return self.apply_binop(op, lv, rv);
        }
        if is_bare_literal_expr(l) && !is_bare_literal_expr(r) {
            let rv = self.eval_to_value(r, subst)?;
            let lv = if let Value::Int(t, _) = &rv {
                match &l.kind {
                    ExprKind::IntLit(v, suf) => self
                        .eval_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                        .and_then(|res| self.result_to_value(res))?,
                    ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
                        ExprKind::IntLit(v, suf) => self
                            .eval_neg_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                            .and_then(|res| self.result_to_value(res))?,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                }
            } else if let Value::F32(_) | Value::F64(_) = &rv {
                // D-0090: a literal of a `number` type parameter, at a
                // floating-point instantiation (`0 - x`).
                let ft = if matches!(rv, Value::F32(_)) { Type::F32 } else { Type::F64 };
                let res = self.eval_expected(l, subst, &ft)?;
                self.result_to_value(res)?
            } else {
                self.eval_to_value(l, subst)?
            };
            return self.apply_binop(op, lv, rv);
        }
        // `rule.type.expected`: an enum literal compared with `==`/`!=`
        // takes its type from the other operand (`x == Some(5000000000)`
        // for `x : Option<u64>`). On the left, the other side goes first
        // only when the literal's payload is itself a literal, so the
        // order cannot be observed (D-0007).
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            let (lvar, rvar) = (self.is_variant_literal(l), self.is_variant_literal(r));
            if rvar && !lvar {
                let lr = self.eval(l, subst)?;
                let lt = self.result_type_hint(&lr);
                let lv = self.result_to_value(lr)?;
                let rr = match lt {
                    Some(t @ Type::Named(..)) => self.eval_expected(r, subst, &t)?,
                    _ => self.eval(r, subst)?,
                };
                let rv = self.result_to_value(rr)?;
                return self.apply_binop(op, lv, rv);
            }
            if lvar && !rvar && self.is_pure_variant_literal(l) {
                let rr = self.eval(r, subst)?;
                let rt = self.result_type_hint(&rr);
                let rv = self.result_to_value(rr)?;
                let lr = match rt {
                    Some(t @ Type::Named(..)) => self.eval_expected(l, subst, &t)?,
                    _ => self.eval(l, subst)?,
                };
                let lv = self.result_to_value(lr)?;
                return self.apply_binop(op, lv, rv);
            }
        }
        let lv = self.eval_to_value(l, subst)?;
        // raw pointer offset: p + n
        if let (Value::Rawptr(addr, pty), BinOp::Add | BinOp::Sub) = (&lv, op) {
            let rv = self.eval_to_value(r, subst)?;
            if let Value::Int(_, n) = rv {
                self.require_unsafe()?;
                let sz = self.sizeof_ty(pty);
                let delta = (n as i128) * sz as i128;
                let new_addr = if op == BinOp::Add { (*addr as i128) + delta } else { (*addr as i128) - delta };
                return Ok(EvalResult::Val(Value::Rawptr(new_addr as u64, pty.clone())));
            }
        }
        // `rule.type.expected`: once the left operand's concrete type is
        // known (evaluation is left-to-right, D-0007), thread it as the
        // expected type for a bare-literal right operand -- otherwise
        // `i64 y = ...; y == 5000000000` defaulted the literal to i32
        // and rejected it, even though the comparison's real type is
        // unambiguous from the left side. A literal on the *left* with
        // nothing yet evaluated to inform it correctly still defaults
        // to i32 (nothing else could know better that early).
        let lit_r = crate::ast::is_literal_expr(r) && !is_bare_literal_expr(r) && bare_float_literal(r).is_none() && !crate::ast::is_literal_expr(l);
        let rv = if lit_r && matches!(op, BinOp::Shl | BinOp::Shr) {
            // a shift's amount is a `u32` (`[T-Shift]`)
            let res = self.eval_expected(r, subst, &Type::Int(IntTy::U32))?;
            self.result_to_value(res)?
        } else if let (true, Some(t)) = (lit_r, Self::number_type_of(&lv)) {
            // D-0037: `k & ((1 << 40) - 1)` computes the mask in `k`'s type
            let res = self.eval_expected(r, subst, &t)?;
            self.result_to_value(res)?
        } else if let (Value::F32(_), Some(f)) = (&lv, bare_float_literal(r)) {
            Value::F32(f as f32)
        } else if let (Value::Int(t, _), ExprKind::IntLit(v, suf)) = (&lv, &r.kind) {
            self.eval_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                .and_then(|res| self.result_to_value(res))?
        } else if let Value::Int(t, _) = &lv {
            if let ExprKind::Unary(UnOp::Neg, inner) = &r.kind {
                if let ExprKind::IntLit(v, suf) = &inner.kind {
                    self.eval_neg_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                        .and_then(|res| self.result_to_value(res))?
                } else {
                    self.eval_to_value(r, subst)?
                }
            } else {
                self.eval_to_value(r, subst)?
            }
        } else if let (Value::F32(_) | Value::F64(_), true) = (&lv, is_bare_literal_expr(r)) {
            // D-0090: a literal of a `number` type parameter, at a
            // floating-point instantiation (`x < 0`).
            let ft = if matches!(lv, Value::F32(_)) { Type::F32 } else { Type::F64 };
            let res = self.eval_expected(r, subst, &ft)?;
            self.result_to_value(res)?
        } else {
            self.eval_to_value(r, subst)?
        };
        self.apply_binop(op, lv, rv)
    }

    pub(super) fn apply_binop(&mut self, op: BinOp, lv: Value, rv: Value) -> EvalOutcome {
        use BinOp::*;
        if op == Ne && trace_on() {
            eprintln!("TRACE ne compare lv={:?} rv={:?}", lv, rv);
        }
        match (op, lv, rv) {
            (Eq, a, b) => Ok(EvalResult::Val(Value::Bool(self.values_eq(&a, &b)))),
            (Ne, a, b) => Ok(EvalResult::Val(Value::Bool(!self.values_eq(&a, &b)))),
            (op, Value::Int(t, a), Value::Int(_, b)) => self.int_binop(op, t, a, b),
            // D-0254: a `simd`'s operators, lane by lane, each lane the
            // scalar operator with its checks; the first lane that faults
            // (from lane 0) is the fault.
            (op, Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                let mut out = Vec::with_capacity(a.len());
                for (x, y) in a.into_iter().zip(b) {
                    let r = self.apply_binop(op, x, y)?;
                    out.push(self.result_to_value(r)?);
                }
                Ok(EvalResult::Val(Value::Array(out)))
            }
            // D-0108: `false < true`; `str` in byte order.
            (op @ (Lt | Le | Gt | Ge), Value::Bool(a), Value::Bool(b)) => Ok(EvalResult::Val(Value::Bool(bytes_cmp(op, &[a as u8], &[b as u8])))),
            (op @ (Lt | Le | Gt | Ge), Value::Str(a), Value::Str(b)) => Ok(EvalResult::Val(Value::Bool(bytes_cmp(op, &a, &b)))),
            (op, Value::F32(a), Value::F32(b)) => Ok(EvalResult::Val(self.float_binop(op, a as f64, b as f64, true))),
            (op, Value::F64(a), Value::F64(b)) => Ok(EvalResult::Val(self.float_binop(op, a, b, false))),
            (op, a, b) => {
                if trace_on() {
                    eprintln!("TRACE apply_binop mismatch op={:?} a={:?} b={:?}", op, a, b);
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
        }
    }

    // D-0254: the `simd` type of an array value, from its lanes' values.
    pub(super) fn simd_type_of(v: &Value) -> Option<Type> {
        let Value::Array(lanes) = v else { return None };
        let lane = match lanes.first()? {
            Value::Int(t, _) => Type::Int(*t),
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            _ => return None,
        };
        Some(Type::Simd(Box::new(lane), lanes.len() as u128))
    }

    pub(super) fn values_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Ref { obj: o1, path: p1, .. }, Value::Ref { obj: o2, path: p2, .. }) => o1 == o2 && p1 == p2,
            (Value::Rawptr(x, _), Value::Rawptr(y, _)) => x == y,
            // Compare payload only, not the `IntTy` tag: a well-typed
            // program's `==`/`!=` never compares across integer types
            // (`[T-Cmp]` requires one `τ`), but this interpreter has no
            // separate static-typing pass to guarantee every value's tag
            // was assigned consistently with its context (e.g. an
            // unsuffixed literal always defaults to `i32`, spec/06 §6,
            // even where the surrounding expected type is `u8`) — see
            // impl/STATUS.md. Comparing the numeric value is what a
            // correctly-typed program's same-type comparison reduces to
            // anyway.
            (Value::Int(_, x), Value::Int(_, y)) => x == y,
            _ => a == b,
        }
    }

    pub(super) fn int_binop(&mut self, op: BinOp, t: IntTy, a: i128, b: i128) -> EvalOutcome {
        use BinOp::*;
        let mk = |r: i128| EvalResult::Val(Value::Int(t, r));
        match op {
            Add => checked_add(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Sub => checked_sub(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Mul => checked_mul(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Div => checked_div(t, a, b).map(mk).map_err(|e| Flow::Fault(arith_err_diag(e))),
            Rem => checked_rem(t, a, b).map(mk).map_err(|e| Flow::Fault(arith_err_diag(e))),
            Shl => checked_shl(t, a, b as u32).map(mk).map_err(|_| Flow::Fault("diag.shift-amount-out-of-range".into())),
            Shr => checked_shr(t, a, b as u32).map(mk).map_err(|_| Flow::Fault("diag.shift-amount-out-of-range".into())),
            BitAnd => Ok(mk(a & b)),
            BitOr => Ok(mk(a | b)),
            BitXor => Ok(mk(a ^ b)),
            Lt => Ok(EvalResult::Val(Value::Bool(int_lt(t, a, b)))),
            Le => Ok(EvalResult::Val(Value::Bool(int_lt(t, a, b) || a == b))),
            Gt => Ok(EvalResult::Val(Value::Bool(int_lt(t, b, a)))),
            Ge => Ok(EvalResult::Val(Value::Bool(int_lt(t, b, a) || a == b))),
            Eq => Ok(EvalResult::Val(Value::Bool(a == b))),
            Ne => Ok(EvalResult::Val(Value::Bool(a != b))),
            And | Or => unreachable!(),
        }
    }

    pub(super) fn float_binop(&self, op: BinOp, a: f64, b: f64, is32: bool) -> Value {
        use BinOp::*;
        let wrap = |x: f64| if is32 { Value::F32(x as f32) } else { Value::F64(x) };
        match op {
            Add => wrap(a + b),
            Sub => wrap(a - b),
            Mul => wrap(a * b),
            Div => wrap(a / b),
            Lt => Value::Bool(a < b),
            Le => Value::Bool(a < b || a == b),
            Gt => Value::Bool(b < a),
            Ge => Value::Bool(b < a || a == b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            _ => unreachable!(),
        }
    }

    // NOTE: must recurse through `self.sizeof_ty`/`self.alignof_ty` for
    // every *compound* type (Array, Mutex), not delegate to the free
    // `sizeof`/`alignof` functions in value.rs -- those know nothing
    // about user-declared struct/enum layouts, so e.g. a bare
    // `sizeof(Type::Array(inner,n))` silently computes 0 for any
    // struct/enum-typed `inner` (found via `sizeof<array<Foo,2>>()`
    // returning 0 during this pass's sanity audit, despite `encode`/
    // `decode`'s own array arm -- which does call `self.sizeof_ty` --
    // being correct all along).
    pub(super) fn sizeof_ty(&self, ty: &Type) -> u64 {
        match ty {
            Type::Named(name, args) => {
                if let Some(layout) = self.struct_layout(name, args) {
                    return layout.size;
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    return layout.size;
                }
                sizeof(ty)
            }
            Type::Array(inner, n) => self.sizeof_ty(inner) * (*n as u64),
            // [Sizeof-Mutex] (spec/06 sec 4): the layout of
            // `struct { τ inner; usize state; }`.
            Type::Mutex(inner) => crate::value::mutex_size(self.sizeof_ty(inner), self.alignof_ty(inner)),
            _ => sizeof(ty),
        }
    }
    pub(super) fn alignof_ty(&self, ty: &Type) -> u64 {
        match ty {
            Type::Named(name, args) => {
                if let Some(layout) = self.struct_layout(name, args) {
                    return layout.align;
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    return layout.align;
                }
                alignof(ty)
            }
            Type::Array(inner, _) => self.alignof_ty(inner),
            Type::Mutex(inner) => self.alignof_ty(inner).max(8),
            _ => alignof(ty),
        }
    }

    // `[T-Alt]`: an unsuffixed literal operand takes the other operand's
    // type. A literal has no effects, so evaluating the other operand
    // first when the literal is on the left is unobservable (D-0007).
    pub(super) fn eval_alt_operands(&mut self, l: &Expr, r: &Expr, subst: &HashMap<String, Type>) -> Result<(Value, Value), Flow> {
        // `rule.type.expected`: the other operand's type is the expected
        // type of a literal operand, and of a literal *expression* (D-0037:
        // `k & ((1 << 40) - 1)` computes the mask in `k`'s type).
        let lit_l = crate::ast::is_literal_expr(l);
        let lit_r = crate::ast::is_literal_expr(r);
        if lit_l && !lit_r {
            let b = self.eval_to_value(r, subst)?;
            let a = match Self::number_type_of(&b) {
                Some(t) => {
                    let res = self.eval_expected(l, subst, &t)?;
                    self.result_to_value(res)?
                }
                None => self.eval_to_value(l, subst)?,
            };
            return Ok((a, b));
        }
        let a = self.eval_to_value(l, subst)?;
        let b = match (Self::number_type_of(&a), lit_r && !lit_l) {
            (Some(t), true) => {
                let res = self.eval_expected(r, subst, &t)?;
                self.result_to_value(res)?
            }
            _ => self.eval_to_value(r, subst)?,
        };
        Ok((a, b))
    }

    pub(super) fn number_type_of(v: &Value) -> Option<Type> {
        match v {
            Value::Int(t, _) => Some(Type::Int(*t)),
            Value::F32(_) => Some(Type::F32),
            Value::F64(_) => Some(Type::F64),
            _ => None,
        }
    }
}
