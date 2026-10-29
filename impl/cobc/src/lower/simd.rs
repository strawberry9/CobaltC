// Fx: the lane-wise intrinsics of `simd<τ, N>` (D-0254, `spec/21` §0),
// on GCC/Clang vector types. A mask is the signed integer vector of the
// lanes' width, each lane -1 or 0; a vector comparison's result is cast to
// it, and a choice by mask is `(m & a) | (~m & b)` on the bits.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    pub(super) fn lower_simd_intrinsic(&mut self, name: &str, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize) -> R<V> {
        let loc = self.loc(line);
        match name {
            "simd_eq" | "simd_ne" | "simd_lt" | "simd_le" | "simd_gt" | "simd_ge" | "simd_min" | "simd_max" => {
                let first = if matches!(name, "simd_min" | "simd_max") { expected.filter(|t| matches!(t, Type::Simd(..))).cloned() } else { None };
                let Some((a, b, ty)) = self.simd_pair(&args[0], &args[1], first.as_ref())? else { return Ok(never()) };
                let ct = self.g.ctype(&ty)?;
                let mt = simd_mask(&ty);
                let mc = self.g.ctype(&mt)?;
                let sym = match name {
                    "simd_eq" => "==",
                    "simd_ne" => "!=",
                    "simd_lt" | "simd_min" => "<",
                    "simd_le" => "<=",
                    "simd_gt" | "simd_max" => ">",
                    _ => ">=",
                };
                if name == "simd_min" || name == "simd_max" {
                    // `a[i] < b[i] ? a[i] : b[i]` (`>` for max).
                    let c = format!("({{ {ct} _a = {a}, _b = {b}; {mc} _m = ({mc})(_a {sym} _b); ({ct})((_m & ({mc})_a) | (~_m & ({mc})_b)); }})", ct = ct, mc = mc, a = a, b = b, sym = sym);
                    return Ok(V { c, ty, obj: None });
                }
                Ok(V { c: format!("(({})({} {} {}))", mc, a, sym, b), ty: mt, obj: None })
            }
            "simd_select" => {
                let m = self.lower_expr(&args[0], None)?;
                if matches!(m.ty, Type::Never) {
                    return Ok(never());
                }
                let m = self.into_temp(m, line)?;
                let first = expected.filter(|t| matches!(t, Type::Simd(..))).cloned();
                let Some((a, b, ty)) = self.simd_pair(&args[1], &args[2], first.as_ref())? else { return Ok(never()) };
                let ct = self.g.ctype(&ty)?;
                let mc = self.g.ctype(&simd_mask(&ty))?;
                let c = format!("({{ {mc} _m = ({mc})({m} != 0); ({ct})((_m & ({mc}){a}) | (~_m & ({mc}){b})); }})", mc = mc, ct = ct, m = m.c, a = a, b = b);
                Ok(V { c, ty, obj: None })
            }
            "simd_any" | "simd_all" => {
                let m = self.lower_expr(&args[0], None)?;
                if matches!(m.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Simd(lane, n) = m.ty.clone() else { return internal("simd_any of a non-simd") };
                let Type::Int(it) = *lane else { return internal("simd_any of a float simd") };
                let m = self.into_temp(m, line)?;
                // Any lane nonzero: their bits or-ed; every lane nonzero:
                // each lane's `!= 0` mask (all ones or none) and-ed.
                let uc = c_uint_type(it);
                let size = (it.bitwidth(64) / 8) as u128;
                let (src, fold) = if name == "simd_any" { (m.c.clone(), Fold::Or) } else { (format!("({} != 0)", m.c), Fold::And) };
                let c = format!("((uint8_t)({{ {uc} _f; {red} _f != 0; }}))", uc = uc, red = halve_reduce(&src, uc, n, size, fold, "_f"));
                Ok(V { c, ty: Type::Bool, obj: None })
            }
            "simd_sum" | "simd_reduce_min" | "simd_reduce_max" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Simd(lane, n) = v.ty.clone() else { return internal("a reduction of a non-simd") };
                let v = self.into_temp(v, line)?;
                let lc = self.g.ctype(&lane)?;
                // An integer sum with every lane in [MIN / N, MAX / N]: no
                // prefix of at most N lanes leaves the type, so the left-to-
                // right checked sum cannot fault and equals the wrapped one,
                // which the C compiler sums as a vector (a mask's count, -1
                // or 0 in each lane, is always so). Any other: lane by lane.
                if let (true, Type::Int(it)) = (name == "simd_sum", &*lane) {
                    let (min, max) = bounds(*it);
                    let n_i = n as i128;
                    let out = if it.signed() {
                        format!("(_v < ({lc}){lo}) | (_v > ({lc}){hi})", lc = lc, lo = min / n_i, hi = max / n_i)
                    } else {
                        format!("(_v > ({lc}){hi}u)", lc = lc, hi = max / n_i)
                    };
                    let uc = c_uint_type(*it);
                    let size = (it.bitwidth(64) / 8) as u128;
                    let c = format!(
                        "({{ __typeof__({v}) _v = {v}; {uc} _far; {far} {lc} _r; if (!_far) {{ {uc} _u; {sum} _r = ({lc})_u; }} else {{ _r = _v[0]; for (int _i = 1; _i < {n}; _i++) {{ if (__builtin_add_overflow(_r, _v[_i], &_r)) cb_fault(\"diag.arith-overflow\", {loc}); }} }} _r; }})",
                        v = v.c, n = n, lc = lc, uc = uc, loc = loc,
                        far = halve_reduce(&out, uc, n, size, Fold::Or, "_far"),
                        sum = halve_reduce("_v", uc, n, size, Fold::Add, "_u")
                    );
                    return Ok(V { c, ty: (*lane).clone(), obj: None });
                }
                // An integer minimum or maximum is the same in any order.
                if let (true, Type::Int(it)) = (name != "simd_sum", &*lane) {
                    let size = (it.bitwidth(64) / 8) as u128;
                    let fold = if name == "simd_reduce_min" { Fold::Min } else { Fold::Max };
                    let c = format!("({{ {lc} _r; {red} _r; }})", lc = lc, red = halve_reduce(&v.c, &lc, n, size, fold, "_r"));
                    return Ok(V { c, ty: (*lane).clone(), obj: None });
                }
                // Left to right, from lane 0: the order `spec/21` fixes, so a
                // float sum is the same in every implementation.
                let step = match (name, &*lane) {
                    ("simd_sum", Type::Int(_)) => format!("if (__builtin_add_overflow(_r, _v[_i], &_r)) cb_fault(\"diag.arith-overflow\", {});", loc),
                    ("simd_sum", _) => "_r = _r + _v[_i];".to_string(),
                    ("simd_reduce_min", _) => "_r = _r < _v[_i] ? _r : _v[_i];".to_string(),
                    _ => "_r = _r > _v[_i] ? _r : _v[_i];".to_string(),
                };
                let c = format!("({{ __typeof__({v}) _v = {v}; {lc} _r = _v[0]; for (int _i = 1; _i < {n}; _i++) {{ {step} }} _r; }})", v = v.c, lc = lc, n = n, step = step);
                Ok(V { c, ty: (*lane).clone(), obj: None })
            }
            "simd_load" | "simd_store" => {
                let store = name == "simd_store";
                // A slice named by a place is reached in place, as `s[i]`'s
                // base is (a direct parameter's checks are its own then);
                // any other is a temporary.
                let is_place = matches!(&strip_parens(&args[0]).kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Deref(_));
                let (s, sty) = if is_place {
                    let p = self.lower_place(&args[0])?;
                    (V { c: p.c, ty: p.ty.clone(), obj: None }, p.ty)
                } else {
                    let v = self.lower_expr(&args[0], None)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let t = v.ty.clone();
                    (self.into_temp(v, line)?, t)
                };
                let Type::Slice(elem, _) = sty else { return internal("a simd load from a non-slice") };
                let i = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                if matches!(i.ty, Type::Never) {
                    return Ok(never());
                }
                let i = self.into_temp(i, line)?;
                let (vty, val) = if store {
                    // A literal is a `simd<τ, N>` of the slice's τ and its length.
                    let lit = match &strip_parens(&args[2]).kind {
                        ExprKind::ArrayLit(items) => Some(Type::Simd(elem.clone(), items.len() as u128)),
                        ExprKind::ArrayRepeat(_, n) => Some(Type::Simd(elem.clone(), *n)),
                        _ => None,
                    };
                    let v = self.lower_expr(&args[2], lit.as_ref())?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let ty = v.ty.clone();
                    (ty, Some(self.into_temp(v, line)?.c))
                } else {
                    (targs.first().map(|t| self.sub(t)).unwrap_or(Type::Void), None)
                };
                let Type::Simd(_, n) = vty.clone() else { return internal("a simd load of a non-simd") };
                let ct = self.g.ctype(&vty)?;
                // `[Index-Checked]` for every lane, then each lane's access is
                // checked as `s[i + k]`'s is.
                self.emit(&format!("if ((uint64_t){i} > ({s}).len || ({s}).len - (uint64_t){i} < {n}u) cb_fault(\"diag.index-out-of-bounds\", {loc});", i = i.c, s = s.c, n = n, loc = loc));
                let k = self.g.fresh("k");
                // A direct slice's checks are taken only while its flag says
                // they are needed: tested once for all the lanes.
                let flag = self.direct_live.get(&s.c).cloned();
                if let Some(f) = &flag {
                    self.emit(&format!("if ({}) {{", f));
                }
                self.emit(&format!("for (uint64_t {k} = 0; {k} < {n}u; {k}++) {{", k = k, n = n));
                let saved = std::mem::replace(&mut self.place_write, store);
                let r = self.slice_elem_access(&s.c, &format!("{} + {}", i.c, k), &elem, loc.clone());
                self.place_write = saved;
                r?;
                self.emit("}");
                if flag.is_some() {
                    self.emit("}");
                }
                if let Some(v) = val {
                    self.emit(&format!("{{ {ct} _v = {v}; memcpy(&({s}).data[{i}], &_v, sizeof _v); }}", ct = ct, v = v, s = s.c, i = i.c));
                    return Ok(unit());
                }
                let r = self.g.fresh("t");
                self.emit(&format!("{ct} {r}; memcpy(&{r}, &({s}).data[{i}], sizeof {r});", ct = ct, r = r, s = s.c, i = i.c));
                Ok(V { c: r, ty: vty, obj: None })
            }
            _ => internal("an unknown simd intrinsic"),
        }
    }

    // Two operands of one `simd` type: the first with `first` as its
    // expected type, when given; the second with the first's.
    fn simd_pair(&mut self, a: &Expr, b: &Expr, first: Option<&Type>) -> R<Option<(String, String, Type)>> {
        let av = self.lower_expr(a, first)?;
        if matches!(av.ty, Type::Never) {
            return Ok(None);
        }
        let av = self.into_temp(av, a.line)?;
        let bv = self.lower_expr(b, Some(&av.ty))?;
        if matches!(bv.ty, Type::Never) {
            return Ok(None);
        }
        let bv = self.into_temp(bv, b.line)?;
        Ok(Some((av.c, bv.c, av.ty)))
    }
}

// How `halve_reduce` combines two lanes.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Fold {
    Or,
    And,
    Add,
    Min,
    Max,
}

// C statements that fold the integer vector expression `v` (`n` lanes of
// `size` bytes, read as the C lane type `lc`) into the scalar `out` (of
// type `lc`, declared by the caller), by halving: each step adds (ors, …)
// the upper half of the vector to its lower half, so the C compiler keeps
// the work in vector registers, where a loop over the lanes went through
// memory one lane at a time (`simd_any` and the overflow tests were slower
// than scalar code). Each fold is associative and commutative on integers,
// so the order changes nothing; `Add` wants an unsigned `lc` (it wraps),
// `Min` and `Max` the lanes' own type.
pub(super) fn halve_reduce(v: &str, lc: &str, n: u128, size: u128, fold: Fold, out: &str) -> String {
    let comb = |a: &str, b: &str, t: &str| -> String {
        match fold {
            Fold::Or => format!("{} | {}", a, b),
            Fold::And => format!("{} & {}", a, b),
            Fold::Add => format!("{} + {}", a, b),
            Fold::Min | Fold::Max => {
                let rel = if fold == Fold::Min { "<" } else { ">" };
                if t.is_empty() {
                    format!("({a} {rel} {b} ? {a} : {b})", a = a, b = b, rel = rel)
                } else {
                    format!("((({a}) & ({t})({a} {rel} {b})) | (({b}) & ~({t})({a} {rel} {b})))", a = a, b = b, rel = rel, t = t)
                }
            }
        }
    };
    let mut s = format!("{{ __typeof__({v}) _x = {v}; typedef {lc} _h0t __attribute__((vector_size({b}))); _h0t _h0; memcpy(&_h0, &_x, {b}); ", v = v, lc = lc, b = n * size);
    let mut cur = "_h0".to_string();
    let mut lanes = n;
    let mut k = 0;
    while lanes > 2 {
        let half = lanes / 2;
        let hb = half * size;
        k += 1;
        let t = format!("_h{}t", k);
        s.push_str(&format!(
            "typedef {lc} {t} __attribute__((vector_size({hb}))); {t} _h{k}a, _h{k}b; memcpy(&_h{k}a, &{cur}, {hb}); memcpy(&_h{k}b, (char *)&{cur} + {hb}, {hb}); {t} _h{k} = {c}; ",
            lc = lc, t = t, hb = hb, k = k, cur = cur, c = comb(&format!("_h{}a", k), &format!("_h{}b", k), &t)
        ));
        cur = format!("_h{}", k);
        lanes = half;
    }
    s.push_str(&format!("{out} = ({lc})({c}); }}", out = out, lc = lc, c = comb(&format!("{}[0]", cur), &format!("{}[1]", cur), "")));
    s
}

// The mask type of a `simd`: signed integer lanes of the lanes' width.
pub(super) fn simd_mask(t: &Type) -> Type {
    let Type::Simd(lane, n) = t else { return Type::Void };
    let m = match &**lane {
        Type::Int(IntTy::I8 | IntTy::U8) => IntTy::I8,
        Type::Int(IntTy::I16 | IntTy::U16) => IntTy::I16,
        Type::Int(IntTy::I32 | IntTy::U32) | Type::F32 => IntTy::I32,
        _ => IntTy::I64,
    };
    Type::Simd(Box::new(Type::Int(m)), *n)
}
