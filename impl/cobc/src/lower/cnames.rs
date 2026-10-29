// C spellings: mangled names, integer types and checks.

use super::*;

pub(super) fn mangle(t: &Type) -> String {
    match t {
        Type::Int(i) => i.name().to_string(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "bool".into(),
        Type::Str => "str".into(),
        Type::Void => "void".into(),
        Type::Never => "never".into(),
        Type::Ref(i, m) => format!("ref{}_{}", if *m == Mode::Shared { "s" } else { "x" }, mangle(i)),
        Type::Slice(i, m) => format!("slice{}_{}", if *m == Mode::Shared { "s" } else { "x" }, mangle(i)),
        Type::Rawptr(i) => format!("ptr_{}", mangle(i)),
        Type::Array(i, n) => format!("arr{}_{}", n, mangle(i)),
        Type::Simd(i, n) => format!("simd{}_{}", n, mangle(i)),
        Type::Fn(ps, r) => format!("fn{}_{}", ps.iter().map(mangle).collect::<Vec<_>>().join("_"), mangle(r)),
        Type::Handle(i) => format!("handle_{}", mangle(i)),
        Type::Mutex(i) => format!("mutex_{}", mangle(i)),
        Type::Guard(i) => format!("guard_{}", mangle(i)),
        Type::Named(n, args) => {
            if args.is_empty() {
                san(n)
            } else {
                format!("{}_L{}_R", san(n), args.iter().map(mangle).collect::<Vec<_>>().join("_"))
            }
        }
        Type::Closure(id) => format!("closure{}", id),
    }
}

// D-0118: a bit field's C mask and the C expression of a `u128` constant.
pub(super) fn c_bits_mask(backing: IntTy, w: u32) -> String {
    let m: u128 = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    if backing == IntTy::U128 {
        format!("(((cb_u128){}ull << 64) | (cb_u128){}ull)", (m >> 64) as u64, m as u64)
    } else {
        format!("(({}){}ull)", c_int_type(backing), m as u64)
    }
}

// D-0118: `(width, bit offset, backing type, field type)` of field `f` of
// a bitstruct declaration, or `None` when `sd` is an ordinary struct.
pub(super) fn bitfield_layout(sd: &StructDecl, f: &str) -> Option<(u32, u32, IntTy, Type)> {
    let backing = sd.bits?;
    let mut off = 0u32;
    for fd in &sd.fields {
        let w = fd.width?;
        if fd.name == f {
            return Some((w, off, backing, fd.ty.clone()));
        }
        off += w;
    }
    None
}

pub(super) fn c_int_type(t: IntTy) -> &'static str {
    match t {
        IntTy::I8 => "int8_t",
        IntTy::I16 => "int16_t",
        IntTy::I32 => "int32_t",
        IntTy::I64 | IntTy::Isize => "int64_t",
        IntTy::I128 => "cb_i128",
        IntTy::U8 => "uint8_t",
        IntTy::U16 => "uint16_t",
        IntTy::U32 => "uint32_t",
        IntTy::U64 | IntTy::Usize => "uint64_t",
        IntTy::U128 => "cb_u128",
    }
}

pub(super) fn c_uint_type(t: IntTy) -> &'static str {
    c_uint_of_width(t.bitwidth(64))
}

pub(super) fn c_uint_of_width(bw: u32) -> &'static str {
    match bw {
        8 => "uint8_t",
        16 => "uint16_t",
        32 => "uint32_t",
        64 => "uint64_t",
        _ => "cb_u128",
    }
}

pub(super) fn c_sint_of_width(bw: u32) -> &'static str {
    match bw {
        8 => "int8_t",
        16 => "int16_t",
        32 => "int32_t",
        64 => "int64_t",
        _ => "cb_i128",
    }
}

pub(super) fn c_int(t: IntTy, v: i128) -> String {
    let bits = v as u128;
    let hi = (bits >> 64) as u64;
    let lo = bits as u64;
    if hi == 0 && !matches!(t, IntTy::I128 | IntTy::U128) {
        format!("(({}){}ULL)", c_int_type(t), lo)
    } else {
        format!("(({})(((cb_u128){}ULL << 64) | (cb_u128){}ULL))", c_int_type(t), hi, lo)
    }
}

pub(super) fn c_float(f: f64, ty: &Type) -> String {
    let mut s = format!("{:?}", f);
    if !s.contains('.') && !s.contains('e') && !s.contains("inf") && !s.contains("NaN") {
        s.push_str(".0");
    }
    if matches!(ty, Type::F32) {
        format!("(float){}", s)
    } else {
        s
    }
}

// Whether `x : s` fits `d` (`[Narrow-Overflow]`'s premise), as a C condition.
pub(super) fn narrow_cond(s: IntTy, d: IntTy, x: &str) -> String {
    if matches!(d, IntTy::U128) {
        if s.signed() { format!("{} >= 0", x) } else { "1".to_string() }
    } else if matches!(s, IntTy::U128) {
        let (_, max) = bounds(d);
        format!("(cb_u128){} <= (cb_u128){}", x, c_int(IntTy::U128, max))
    } else {
        let (min, max) = bounds(d);
        format!("(cb_i128){x} >= {} && (cb_i128){x} <= {}", c_int(IntTy::I128, min), c_int(IntTy::I128, max), x = x)
    }
}

pub(super) fn narrow_check(s: IntTy, d: IntTy, x: &str, loc: &str) -> String {
    let dt = c_int_type(d);
    let cond = narrow_cond(s, d, x);
    format!("({{ if (!({})) cb_fault(\"diag.narrowing-overflow\", {}); ({}){}; }})", cond, loc, dt, x)
}

pub(super) fn to_int_check(d: IntTy, x: &str, loc: &str) -> String {
    let dt = c_int_type(d);
    let range = match d {
        IntTy::U128 => "_t >= 0.0 && _t < 340282366920938463463374607431768211456.0".to_string(),
        IntTy::I128 => "_t >= -170141183460469231731687303715884105728.0 && _t < 170141183460469231731687303715884105728.0".to_string(),
        _ => {
            let (min, max) = bounds(d);
            // The bounds as typed constants: a bare `-9223372036854775808`
            // in C is a minus applied to a literal that fits no signed
            // type, which GCC makes 128-bit and Clang makes unsigned
            // (so +2^63), and the lower-bound test then failed under Clang.
            format!(
                "_t >= (double){} && _t <= (double){} && (cb_i128)_t >= {} && (cb_i128)_t <= {}",
                c_int(d, min),
                c_int(d, max),
                c_int(IntTy::I128, min),
                c_int(IntTy::I128, max)
            )
        }
    };
    format!(
        "({{ double _f = (double){}; if (!isfinite(_f)) cb_fault(\"diag.narrowing-overflow\", {loc}); double _t = trunc(_f); if (!({})) cb_fault(\"diag.narrowing-overflow\", {loc}); ({})_t; }})",
        x, range, dt, loc = loc
    )
}
