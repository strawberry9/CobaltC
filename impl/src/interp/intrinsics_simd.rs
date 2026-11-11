// D-0254: the lane-wise intrinsics of `simd<τ, N>` (`spec/21` §0,
// `rule.stdlib.simd`). A `simd` is an array value here; every lane is
// computed as the scalar operation computes it, from lane 0 up, so a fault
// is the first lane's that faults.

use super::*;

impl Interp {
    pub(super) fn try_simd_intrinsic(&mut self, name: &str, targs: &[Type], args: &[Expr], subst: &HashMap<String, Type>) -> Result<Option<EvalResult>, Flow> {
        macro_rules! ok {
            ($v:expr) => {
                return Ok(Some(EvalResult::Val($v)))
            };
        }
        let lanes = |v: Value| -> Result<Vec<Value>, Flow> {
            match v {
                Value::Array(l) => Ok(l),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            }
        };
        match name {
            "simd_eq" | "simd_ne" | "simd_lt" | "simd_le" | "simd_gt" | "simd_ge" => {
                let (a, b) = self.eval_simd_pair(&args[0], &args[1], subst)?;
                let mt = Self::mask_lane(&a);
                let op = match name {
                    "simd_eq" => BinOp::Eq,
                    "simd_ne" => BinOp::Ne,
                    "simd_lt" => BinOp::Lt,
                    "simd_le" => BinOp::Le,
                    "simd_gt" => BinOp::Gt,
                    _ => BinOp::Ge,
                };
                let mut out = Vec::new();
                for (x, y) in lanes(a)?.into_iter().zip(lanes(b)?) {
                    let r = self.apply_binop(op, x, y)?;
                    let t = matches!(self.result_to_value(r)?, Value::Bool(true));
                    out.push(Value::Int(mt, if t { -1 } else { 0 }));
                }
                ok!(Value::Array(out))
            }
            "simd_min" | "simd_max" => {
                let (a, b) = self.eval_simd_pair(&args[0], &args[1], subst)?;
                let mut out = Vec::new();
                for (x, y) in lanes(a)?.into_iter().zip(lanes(b)?) {
                    out.push(self.lane_pick(name == "simd_min", x, y)?);
                }
                ok!(Value::Array(out))
            }
            "simd_select" => {
                let m = self.eval_to_value(&args[0], subst)?;
                let (a, b) = self.eval_simd_pair(&args[1], &args[2], subst)?;
                let mut out = Vec::new();
                for ((k, x), y) in lanes(m)?.into_iter().zip(lanes(a)?).zip(lanes(b)?) {
                    let on = !matches!(k, Value::Int(_, 0));
                    out.push(if on { x } else { y });
                }
                ok!(Value::Array(out))
            }
            "simd_any" | "simd_all" => {
                let m = lanes(self.eval_to_value(&args[0], subst)?)?;
                let on = |k: &Value| !matches!(k, Value::Int(_, 0));
                ok!(Value::Bool(if name == "simd_any" { m.iter().any(on) } else { m.iter().all(on) }))
            }
            "simd_sum" | "simd_reduce_min" | "simd_reduce_max" => {
                let mut it = lanes(self.eval_to_value(&args[0], subst)?)?.into_iter();
                let mut acc = it.next().ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                for l in it {
                    acc = match name {
                        "simd_sum" => {
                            let r = self.apply_binop(BinOp::Add, acc, l)?;
                            self.result_to_value(r)?
                        }
                        _ => self.lane_pick(name == "simd_reduce_min", acc, l)?,
                    };
                }
                ok!(acc)
            }
            "simd_load" | "simd_store" => {
                let store = name == "simd_store";
                let n = match targs.first().map(|t| self.apply_subst(t, subst)) {
                    Some(Type::Simd(_, n)) => n as usize,
                    _ => 0,
                };
                let (obj, path, chain) = self.collection_operand(&args[0], subst, store)?;
                let aty = self.type_at(obj, &path);
                let len = self.indexed_len(obj, &path, &aty)?;
                let i = match self.eval_to_value(&args[1], subst)? {
                    Value::Int(_, k) if k >= 0 => k as usize,
                    Value::Int(..) => return Err(Flow::Fault("diag.index-out-of-bounds".into())),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if !store {
                    if i.checked_add(n).map_or(true, |e| e > len) {
                        return Err(Flow::Fault("diag.index-out-of-bounds".into()));
                    }
                    let mut out = Vec::with_capacity(n);
                    for k in 0..n {
                        let r = self.index_at(obj, path.clone(), chain.clone(), &aty, i + k, false, subst)?;
                        out.push(self.result_to_value(r)?);
                    }
                    ok!(Value::Array(out));
                }
                // A literal is a `simd<τ, N>` of the slice's τ and its length.
                let lit_n = match &args[2].kind {
                    ExprKind::ArrayLit(items) => Some(items.len() as u128),
                    ExprKind::ArrayRepeat(_, n) => Some(*n),
                    _ => None,
                };
                let v = match (lit_n, &aty) {
                    (Some(n), Type::Slice(e, _)) => {
                        let t = Type::Simd(Box::new(self.apply_subst(e, subst)), n);
                        let r = self.eval_expected(&args[2], subst, &t)?;
                        self.result_to_value(r)?
                    }
                    _ => self.eval_to_value(&args[2], subst)?,
                };
                let v = lanes(v)?;
                if i.checked_add(v.len()).map_or(true, |e| e > len) {
                    return Err(Flow::Fault("diag.index-out-of-bounds".into()));
                }
                for (k, l) in v.into_iter().enumerate() {
                    match self.index_at(obj, path.clone(), chain.clone(), &aty, i + k, true, subst)? {
                        EvalResult::Place(o, p, _) => self.write_place(o, &p, l),
                        _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                    }
                }
                ok!(Value::Unit)
            }
            _ => Ok(None),
        }
    }

    // Two operands of one `simd` type: an array literal beside a `simd`
    // takes its type (`[0; 4]`), as for the operators.
    fn eval_simd_pair(&mut self, l: &Expr, r: &Expr, subst: &HashMap<String, Type>) -> Result<(Value, Value), Flow> {
        let lit = |e: &Expr| matches!(&e.kind, ExprKind::ArrayLit(_) | ExprKind::ArrayRepeat(..));
        let a = self.eval_to_value(l, subst)?;
        let b = match (lit(r), Self::simd_type_of(&a)) {
            (true, Some(t)) => {
                let res = self.eval_expected(r, subst, &t)?;
                self.result_to_value(res)?
            }
            _ => self.eval_to_value(r, subst)?,
        };
        Ok((a, b))
    }

    // `simd_min`'s lane: `a < b ? a : b` (`simd_max`: `a > b ? a : b`), so a
    // NaN in `a` gives `b`.
    fn lane_pick(&mut self, min: bool, a: Value, b: Value) -> Result<Value, Flow> {
        let r = self.apply_binop(if min { BinOp::Lt } else { BinOp::Gt }, a.clone(), b.clone())?;
        Ok(if matches!(self.result_to_value(r)?, Value::Bool(true)) { a } else { b })
    }

    // The mask lane type of a `simd` value: signed, of its lanes' width.
    fn mask_lane(v: &Value) -> IntTy {
        let w = match v {
            Value::Array(l) => match l.first() {
                Some(Value::Int(t, _)) => t.bitwidth(64),
                Some(Value::F32(_)) => 32,
                _ => 64,
            },
            _ => 64,
        };
        match w {
            8 => IntTy::I8,
            16 => IntTy::I16,
            32 => IntTy::I32,
            _ => IntTy::I64,
        }
    }

    // A slice operand, reached as `s[i]`'s base is (`eval_index`).
    fn collection_operand(&mut self, e: &Expr, subst: &HashMap<String, Type>, write: bool) -> Result<(u64, Vec<Proj>, Vec<u64>), Flow> {
        let r = self.eval(e, subst)?;
        if let EvalResult::Temp(o) = r {
            self.stmt_temp(o);
        }
        let (obj, path, chain) = match r {
            EvalResult::Place(o, p, chain) => (o, p, chain),
            EvalResult::Temp(o) => (o, vec![], vec![]),
            EvalResult::Val(Value::Ref { obj, path, token, mode, .. }) => {
                self.affine(token, mode == Mode::Exclusive || write)?;
                (obj, path, vec![token])
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        self.resolve_through_refs(obj, path, chain)
    }
}
