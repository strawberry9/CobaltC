// The intrinsics.

use super::*;

impl Interp {
    // ---- prelude intrinsics (spec/21 §0) ----

    pub(super) fn try_intrinsic(
        &mut self,
        name: &str,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let t0 = || -> Type { Type::Void };
        let targ = |i: usize, s: &Interp| -> Type {
            targs.get(i).map(|t| s.apply_subst(t, subst)).unwrap_or_else(t0)
        };
        macro_rules! ok { ($v:expr) => { return Ok(Some(EvalResult::Val($v))) } }
        // D-0254: the lane-wise intrinsics, and the wrapping ones on a `simd`.
        if name.starts_with("simd_") {
            if let Some(r) = self.try_simd_intrinsic(name, targs, args, subst)? {
                return Ok(Some(r));
            }
        }
        match name {
            // D-0116: `clone(&x)`: a plain `T` is read (as `*r` reads it);
            // a resource `T` calls its `T::clone`, declared or derived.
            "clone" => {
                let rv = self.eval_to_value(&args[0], subst)?;
                let Value::Ref { obj, path, token, mode, .. } = rv.clone() else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                self.affine(token, mode == Mode::Exclusive)?;
                let ty = self.type_at(obj, &path);
                if !self.is_resource(&ty) {
                    let v = self.result_to_value(EvalResult::Place(obj, path, vec![token]))?;
                    ok!(v);
                }
                let Type::Named(n, _) = &ty else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                let key = format!("{}::clone", n);
                if !self.items.fns.contains_key(&key) {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                let r = self.invoke_callable(EvalResult::Val(Value::FnVal(key)), vec![EvalResult::Val(rv)], subst)?;
                return Ok(Some(r));
            }
            "drop" => {
                let r = self.eval(&args[0], subst)?;
                match r {
                    EvalResult::Place(obj, path, excl) => {
                        if !path.is_empty() {
                            return Err(Flow::Fault("diag.move-out-of-field".into()));
                        }
                        // [Destroy-Not-Solitary]: no other live path may
                        // reach the object being destroyed. A place
                        // reached *through a reference* (`drop(*r)`) is
                        // never solitary while the object's own owner
                        // binding exists: the owner's root path is another
                        // valid path on it.
                        if self.is_resource(&self.type_at(obj, &path)) && (!self.solitary(obj, &excl) || (!excl.is_empty() && self.lookup_owns(obj))) {
                            return Err(Flow::Fault("diag.destroy-while-aliased".into()));
                        }
                        if self.lookup_owns(obj) {
                            self.invalidate_binding_pointing_at(obj);
                        }
                        self.destroy_object(obj)?;
                    }
                    EvalResult::Temp(o) => self.destroy_object(o)?,
                    EvalResult::Val(_) => {}
                }
                ok!(Value::Unit)
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" => {
                let (a, b) = self.eval_alt_operands(&args[0], &args[1], subst)?;
                let op = if name.ends_with("add") { '+' } else if name.ends_with("sub") { '-' } else { '*' };
                // D-0254: on a `simd`, lane by lane.
                if let (Value::Array(xs), Value::Array(ys)) = (&a, &b) {
                    let mut out = Vec::with_capacity(xs.len());
                    for (x, y) in xs.iter().zip(ys) {
                        let (Value::Int(t, x), Value::Int(_, y)) = (x, y) else {
                            return Err(Flow::Fault("diag.type-mismatch".into()));
                        };
                        out.push(Value::Int(*t, if name.starts_with("wrapping") { wrapping_op(*t, *x, *y, op) } else { saturating_op(*t, *x, *y, op) }));
                    }
                    ok!(Value::Array(out));
                }
                let (Value::Int(t, x), Value::Int(_, y)) = (a, b) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let r = if name.starts_with("wrapping") { wrapping_op(t, x, y, op) } else { saturating_op(t, x, y, op) };
                ok!(Value::Int(t, r))
            }
            "checked_add" | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem" => {
                let (a, b) = self.eval_alt_operands(&args[0], &args[1], subst)?;
                let (Value::Int(t, x), Value::Int(_, y)) = (a, b) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let res = match name {
                    "checked_add" => checked_add(t, x, y),
                    "checked_sub" => checked_sub(t, x, y),
                    "checked_mul" => checked_mul(t, x, y),
                    "checked_div" => checked_div(t, x, y),
                    _ => checked_rem(t, x, y),
                };
                let opt_ty = Type::Named("std::Option".into(), vec![Type::Int(t)]);
                let v = match res {
                    Ok(r) => Value::Enum(0, Box::new(Value::Int(t, r))),
                    Err(_) => Value::Enum(1, Box::new(Value::Unit)),
                };
                let id = self.new_object(opt_ty, v);
                return Ok(Some(EvalResult::Temp(id)));
            }
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let dst = targ(0, self);
                return Ok(Some(EvalResult::Val(self.convert(name, v, &dst)?)));
            }
            // D-0122: `narrow` that answers `None` instead of faulting.
            "checked_narrow" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let dst = targ(0, self);
                let opt_ty = Type::Named("std::Option".into(), vec![dst.clone()]);
                let r = match (&v, &dst) {
                    (Value::Int(st, x), Type::Int(dt)) => match narrow(*st, *dt, *x) {
                        Ok(r) => Value::Enum(0, Box::new(Value::Int(*dt, r))),
                        Err(_) => Value::Enum(1, Box::new(Value::Unit)),
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let id = self.new_object(opt_ty, r);
                return Ok(Some(EvalResult::Temp(id)));
            }
            // D-0123: bits of the operand's unsigned image of its width.
            "count_ones" | "leading_zeros" | "trailing_zeros" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let Value::Int(t, x) = v else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                let bw = t.bitwidth(ADDR_WIDTH);
                let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                let u = (x as u128) & mask;
                let n: u32 = match name {
                    "count_ones" => u.count_ones(),
                    "leading_zeros" => if u == 0 { bw } else { u.leading_zeros() - (128 - bw) },
                    _ => if u == 0 { bw } else { u.trailing_zeros() },
                };
                ok!(Value::Int(IntTy::U32, n as i128));
            }
            "rotate_left" | "rotate_right" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let k = self.eval_to_value(&args[1], subst)?;
                let (Value::Int(t, x), Value::Int(_, k)) = (v, k) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                let bw = t.bitwidth(ADDR_WIDTH);
                let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                let u = (x as u128) & mask;
                let k = (k as u128 % bw as u128) as u32;
                let r = if k == 0 { u } else if name == "rotate_left" { ((u << k) | (u >> (bw - k))) & mask } else { ((u >> k) | (u << (bw - k))) & mask };
                ok!(Value::Int(t, reinterpret_sign_pub(t, r, bw)));
            }
            // D-0121: an access to raw storage that is an observable event.
            "read_volatile" | "write_volatile" => {
                let p = self.eval_to_value(&args[0], subst)?;
                let Value::Rawptr(addr, pty) = p else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                self.require_unsafe()?;
                if name == "read_volatile" {
                    let v = self.arena_read(addr, &pty);
                    ok!(v);
                }
                let r = self.eval_expected(&args[1], subst, &pty)?;
                let v = self.result_to_value(r)?;
                self.arena_write(addr, &pty, v);
                ok!(Value::Unit);
            }
            // D-0091: IEEE-754's correctly rounded square root and its
            // exact roundings to an integral value (`round`: half away from
            // zero), each at the operand's type.
            // D-0101: the platform's `log`, `exp`, … (the C library's, as
            // cobc's are, so both give the same bits on one platform).
            // D-0136: `std::math`'s, realized natively.
            "std::sqrt" | "std::floor" | "std::ceil" | "std::round" | "std::trunc" | "std::ln" | "std::exp" | "std::log2" | "std::log10" | "std::sin"
            | "std::cos" | "std::tan" | "std::asin" | "std::acos" | "std::atan" => {
                let name = &name["std::".len()..];
                let v = self.eval_to_value(&args[0], subst)?;
                let f64op = |x: f64| match name {
                    "sqrt" => x.sqrt(), "floor" => x.floor(), "ceil" => x.ceil(), "round" => round_f64(x), "trunc" => x.trunc(),
                    "ln" => x.ln(), "exp" => x.exp(), "log2" => x.log2(), "log10" => x.log10(), "sin" => x.sin(), "cos" => x.cos(),
                    "asin" => x.asin(), "acos" => x.acos(), "atan" => x.atan(), _ => x.tan(),
                };
                let f32op = |x: f32| match name {
                    "sqrt" => x.sqrt(), "floor" => x.floor(), "ceil" => x.ceil(), "round" => round_f32(x), "trunc" => x.trunc(),
                    "ln" => x.ln(), "exp" => x.exp(), "log2" => x.log2(), "log10" => x.log10(), "sin" => x.sin(), "cos" => x.cos(),
                    "asin" => x.asin(), "acos" => x.acos(), "atan" => x.atan(), _ => x.tan(),
                };
                return match v {
                    Value::F64(x) => ok!(Value::F64(f64op(x))),
                    Value::F32(x) => ok!(Value::F32(f32op(x))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                };
            }
            "std::atan2" | "std::powf" => {
                let name = &name["std::".len()..];
                let a = self.eval_to_value(&args[0], subst)?;
                let b = self.eval_to_value(&args[1], subst)?;
                return match (a, b) {
                    (Value::F64(x), Value::F64(y)) => ok!(Value::F64(if name == "atan2" { x.atan2(y) } else { x.powf(y) })),
                    (Value::F32(x), Value::F32(y)) => ok!(Value::F32(if name == "atan2" { x.atan2(y) } else { x.powf(y) })),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                };
            }
            // D-0052: computed before the program ran (`check_static_asserts`).
            "static_assert" => ok!(Value::Unit),
            "min_value" | "max_value" => {
                // `rule.arith.limits`: `min(τ)`/`max(τ)` of spec/06 §1.
                // `u128`'s maximum is held as its bit pattern, as the
                // literal is.
                // D-0051: a float's are its largest finite value, negated
                // for `min_value`.
                match targ(0, self) {
                    Type::F32 => ok!(Value::F32(if name == "max_value" { f32::MAX } else { f32::MIN })),
                    Type::F64 => ok!(Value::F64(if name == "max_value" { f64::MAX } else { f64::MIN })),
                    _ => {}
                }
                let Type::Int(t) = targ(0, self) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let v = match (name, t) {
                    ("max_value", IntTy::U128) => -1,
                    ("max_value", _) => int_min_max(t).1,
                    _ => int_min_max(t).0,
                };
                ok!(Value::Int(t, v))
            }
            "sizeof" => ok!(Value::Int(IntTy::Usize, self.sizeof_ty(&targ(0, self)) as i128)),
            "alignof" => ok!(Value::Int(IntTy::Usize, self.alignof_ty(&targ(0, self)) as i128)),
            // `dangling<T>()`: impl-defined, T-aligned and not null; `alignof<T>()`
            // itself, below every arena address (spec/IMPLEMENTATION-NOTES.md §5).
            "dangling" => {
                let t = targ(0, self);
                ok!(Value::Rawptr(self.alignof_ty(&t), t))
            }
            // D-0252: C's `NULL`, address 0, which no arena object has.
            "null_rawptr" => ok!(Value::Rawptr(0, targ(0, self))),
            _ => self.intrinsic_raw(name, targs, args, subst),
        }
    }
}

// `round`: half away from zero, computed from `trunc` (exact: the dropped
// fraction `x - t` is representable) rather than taken from the C library,
// whose `round` on Windows (MinGW over msvcrt) gives 1 for
// 0.49999999999999994 (D-0255). cbrt.h's `cb_round` is the same.
fn round_f64(x: f64) -> f64 {
    let t = x.trunc();
    if (x - t).abs() >= 0.5 {
        t + 1f64.copysign(x)
    } else {
        t
    }
}

fn round_f32(x: f32) -> f32 {
    let t = x.trunc();
    if (x - t).abs() >= 0.5 {
        t + 1f32.copysign(x)
    } else {
        t
    }
}
