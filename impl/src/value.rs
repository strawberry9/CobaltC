// Runtime values, platform parameters (spec/06, spec/IMPLEMENTATION-NOTES.md),
// and arithmetic (spec/06 §3-4).
use crate::ast::{IntTy, Mode, Type};
use std::sync::Arc;

// Platform bindings (impl-defined per spec/06 §1, spec/IMPLEMENTATION-NOTES.md
// §1), documented here rather than derived: a 64-bit little-endian
// host (x86_64 or AArch64, Linux or Windows).
pub const ADDR_WIDTH: u32 = 64; // AddrWidth
pub const LITTLE_ENDIAN: bool = true; // BO
pub const ENUM_DISCRIMINANT_WIDTH: u32 = 4; // DW: smallest common choice, u32

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Proj {
    Field(usize),
    Index(usize),
    Payload, // enum payload
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(IntTy, i128),
    F32(f32),
    F64(f64),
    Bool(bool),
    // A `str` value: its UTF-8 bytes. A value, not an object -- copied
    // by cloning the handle (spec/21 type.str, non-resource).
    Str(Arc<[u8]>),
    Unit,
    Ref {
        obj: u64,
        path: Vec<Proj>,
        mode: Mode,
        pointee: Type,
        // Identifies this *access path* (spec/04 `AccessPathToken`), not
        // this occurrence of the value: copying a `ref` by value (argument
        // passing, a struct field read, ...) clones the token unchanged,
        // since it is the same path merely held by a new object; only
        // forming a genuinely new borrow (`&`/`&mut`, spec/08 `[Borrow]`)
        // mints a fresh one. Aliasing checks exclude by this id, not by
        // which object currently holds a copy of the value.
        token: u64,
    },
    Rawptr(u64, Type),
    FnVal(String),
    Closure(u64), // object id of the capture struct
    Struct(Vec<Value>),
    Array(Vec<Value>),
    Enum(usize, Box<Value>),
    Handle(u64, Type), // thread key, result type
    Guard {
        obj: u64,
        path: Vec<Proj>,
        inner: Type,
        token: u64,
    },
}

pub fn overlap(a: &[Proj], b: &[Proj]) -> bool {
    let n = a.len().min(b.len());
    a[..n] == b[..n]
}

pub fn permitted(m1: Mode, m2: Mode) -> bool {
    m1 == Mode::Shared && m2 == Mode::Shared
}

pub fn int_min_max(t: IntTy) -> (i128, i128) {
    let bw = t.bitwidth(ADDR_WIDTH);
    if t.signed() {
        // bw==128 (i128 itself): `(1i128 << 127) - 1`/`-(1i128 << 127)`
        // are host-level i128 overflow (bit-pattern 1<<127 IS i128::MIN,
        // so `-that` and `that - 1` both overflow i128's own range) --
        // silently "correct by wraparound coincidence" in a --release
        // build (overflow-checks off by default) but a real panic in
        // any debug build (confirmed: `i128 x = 100; i128 y = x + 1;`
        // crashed `target/debug/coby` with "attempt to subtract with
        // overflow" at this exact line). Use the real constants instead
        // of computing them.
        if bw >= 128 {
            (i128::MIN, i128::MAX)
        } else {
            let max = (1i128 << (bw - 1)) - 1;
            let min = -(1i128 << (bw - 1));
            (min, max)
        }
    } else if bw >= 128 {
        // u128's true range is [0, 2^128-1], which does not fit in
        // Value::Int's i128 payload above i128::MAX (2^127-1) -- a
        // real, confirmed, unfixed representation limit (see
        // impl/STATUS.md): a legal u128 literal like 2^127 is
        // currently rejected as `diag.literal-out-of-range`. This
        // returns the widest range the i128 payload can actually hold,
        // rather than a value that would itself overflow i128 to
        // compute (the previous `bw >= 127 => i128::MAX` branch had
        // the same effective bound already, just derived from an
        // imprecise threshold rather than stated as this limitation).
        (0, i128::MAX)
    } else {
        let max = (1i128 << bw) - 1;
        (0, max)
    }
}

// `u128` is the one type whose values do not fit the `i128` payload:
// its payload is the value's *bit pattern* (`v as u128 as i128`), so
// every payload is in range and every operation on it is done in
// `u128`. Every other type's payload is the value itself.
pub fn is_u128(t: IntTy) -> bool {
    matches!(t, IntTy::U128)
}

pub fn in_range(t: IntTy, v: i128) -> bool {
    if is_u128(t) {
        return true;
    }
    let (min, max) = int_min_max(t);
    v >= min && v <= max
}

// The mathematical value of a payload, for range checks across types.
fn as_u128_value(t: IntTy, v: i128) -> Option<u128> {
    if is_u128(t) {
        Some(v as u128)
    } else if v >= 0 {
        Some(v as u128)
    } else {
        None
    }
}

pub fn sizeof(ty: &Type) -> u64 {
    match ty {
        Type::Int(t) => (t.bitwidth(ADDR_WIDTH) / 8) as u64,
        Type::F32 => 4,
        Type::F64 => 8,
        Type::Bool => 1,
        // impl-defined (spec/06 [Sizeof-Str]): a (pointer, length) pair.
        Type::Str => 2 * (ADDR_WIDTH / 8) as u64,
        // impl-defined (D-0047): the borrow of the source, where the run
        // starts, and its length.
        Type::Slice(..) => 3 * (ADDR_WIDTH / 8) as u64,
        Type::Void | Type::Never => 0,
        Type::Ref(..) | Type::Rawptr(..) | Type::Fn(..) => (ADDR_WIDTH / 8) as u64,
        Type::Handle(..) | Type::Guard(..) => (ADDR_WIDTH / 8) as u64,
        // [Sizeof-Mutex]: the layout of `struct { τ inner; usize state; }`.
        Type::Mutex(inner) => mutex_size(sizeof(inner), alignof(inner)),
        Type::Array(inner, n) => sizeof(inner) * (*n as u64),
        Type::Named(..) => 0, // resolved by the interpreter's layout cache
        Type::Closure(_) => 0,
    }
}

// `[Sizeof-Mutex]`: `inner` at offset 0, the `usize` state after it at
// the next multiple of its own alignment, the whole rounded to the
// struct's alignment.
pub fn mutex_size(inner_size: u64, inner_align: u64) -> u64 {
    let w = (ADDR_WIDTH / 8) as u64;
    let align = inner_align.max(w);
    let state = inner_size.div_ceil(w) * w;
    (state + w).div_ceil(align) * align
}

pub fn alignof(ty: &Type) -> u64 {
    match ty {
        Type::Mutex(inner) => alignof(inner).max(8),
        Type::Str | Type::Slice(..) => (ADDR_WIDTH / 8) as u64,
        _ => sizeof(ty).max(1),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithError {
    Overflow,
    DivByZero,
    DivOverflow,
    ShiftOutOfRange,
    NarrowOverflow,
}

pub fn checked_add(t: IntTy, a: i128, b: i128) -> Result<i128, ArithError> {
    if is_u128(t) {
        return (a as u128).checked_add(b as u128).map(|r| r as i128).ok_or(ArithError::Overflow);
    }
    // `i128` itself can overflow the host type; never a wraparound.
    let r = a.checked_add(b).ok_or(ArithError::Overflow)?;
    if in_range(t, r) {
        Ok(r)
    } else {
        Err(ArithError::Overflow)
    }
}
pub fn checked_sub(t: IntTy, a: i128, b: i128) -> Result<i128, ArithError> {
    if is_u128(t) {
        return (a as u128).checked_sub(b as u128).map(|r| r as i128).ok_or(ArithError::Overflow);
    }
    let r = a.checked_sub(b).ok_or(ArithError::Overflow)?;
    if in_range(t, r) {
        Ok(r)
    } else {
        Err(ArithError::Overflow)
    }
}
pub fn checked_mul(t: IntTy, a: i128, b: i128) -> Result<i128, ArithError> {
    if is_u128(t) {
        return (a as u128).checked_mul(b as u128).map(|r| r as i128).ok_or(ArithError::Overflow);
    }
    let r = a.checked_mul(b).ok_or(ArithError::Overflow)?;
    if in_range(t, r) {
        Ok(r)
    } else {
        Err(ArithError::Overflow)
    }
}
pub fn checked_div(t: IntTy, a: i128, b: i128) -> Result<i128, ArithError> {
    if b == 0 {
        return Err(ArithError::DivByZero);
    }
    if is_u128(t) {
        return Ok(((a as u128) / (b as u128)) as i128);
    }
    let (min, _) = int_min_max(t);
    if t.signed() && a == min && b == -1 {
        return Err(ArithError::DivOverflow);
    }
    Ok(a / b) // Rust's integer division truncates toward zero, matching spec
}
pub fn checked_rem(t: IntTy, a: i128, b: i128) -> Result<i128, ArithError> {
    if b == 0 {
        return Err(ArithError::DivByZero);
    }
    if is_u128(t) {
        return Ok(((a as u128) % (b as u128)) as i128);
    }
    let (min, _) = int_min_max(t);
    if t.signed() && a == min && b == -1 {
        return Err(ArithError::DivOverflow);
    }
    Ok(a % b)
}
// `<` on payloads, respecting `u128`'s bit-pattern payload.
pub fn int_lt(t: IntTy, a: i128, b: i128) -> bool {
    if is_u128(t) {
        (a as u128) < (b as u128)
    } else {
        a < b
    }
}
pub fn checked_neg(t: IntTy, a: i128) -> Result<i128, ArithError> {
    let (min, _) = int_min_max(t);
    if a == min {
        return Err(ArithError::Overflow);
    }
    Ok(-a)
}
pub fn checked_shl(t: IntTy, a: i128, n: u32) -> Result<i128, ArithError> {
    let bw = t.bitwidth(ADDR_WIDTH);
    if n >= bw {
        return Err(ArithError::ShiftOutOfRange);
    }
    let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
    let r = ((a as u128 & mask) << n) & mask;
    Ok(reinterpret_bits(t, r, bw))
}
pub fn checked_shr(t: IntTy, a: i128, n: u32) -> Result<i128, ArithError> {
    let bw = t.bitwidth(ADDR_WIDTH);
    if n >= bw {
        return Err(ArithError::ShiftOutOfRange);
    }
    if t.signed() {
        Ok(a >> n) // arithmetic shift (sign-preserving) on i128
    } else {
        let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
        let r = (a as u128 & mask) >> n;
        Ok(r as i128)
    }
}
fn reinterpret_bits(t: IntTy, bits: u128, bw: u32) -> i128 {
    if t.signed() {
        let sign_bit = 1u128 << (bw - 1);
        if bits & sign_bit != 0 {
            let full: u128 = if bw >= 128 { 0 } else { !0u128 << bw };
            (bits | full) as i128
        } else {
            bits as i128
        }
    } else {
        bits as i128
    }
}
pub fn narrow(from: IntTy, to: IntTy, v: i128) -> Result<i128, ArithError> {
    // Either side `u128`: compare mathematical values.
    if is_u128(to) {
        return match as_u128_value(from, v) {
            Some(u) => Ok(u as i128),
            None => Err(ArithError::NarrowOverflow),
        };
    }
    if is_u128(from) {
        let u = v as u128;
        let (_, max) = int_min_max(to);
        return if u <= max as u128 { Ok(u as i128) } else { Err(ArithError::NarrowOverflow) };
    }
    // widen is always safe; narrow checks range in the target type.
    let src_dom_subset_of_dst = {
        let (smin, smax) = int_min_max(from);
        let (dmin, dmax) = int_min_max(to);
        smin >= dmin && smax <= dmax
    };
    if src_dom_subset_of_dst {
        return Ok(v);
    }
    if in_range(to, v) {
        Ok(v)
    } else {
        Err(ArithError::NarrowOverflow)
    }
}
pub fn narrow_wrapping(to: IntTy, v: i128) -> i128 {
    let bw = to.bitwidth(ADDR_WIDTH);
    let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
    reinterpret_bits(to, (v as u128) & mask, bw)
}
pub fn reinterpret_sign(to: IntTy, v: i128, from_bw: u32) -> i128 {
    let mask: u128 = if from_bw >= 128 { u128::MAX } else { (1u128 << from_bw) - 1 };
    reinterpret_bits(to, (v as u128) & mask, from_bw)
}
pub fn wrapping_op(t: IntTy, a: i128, b: i128, op: char) -> i128 {
    // Modulo 2^128 on the host type is modulo 2^bitwidth after masking,
    // for signed and unsigned payloads alike.
    let r = match op {
        '+' => a.wrapping_add(b),
        '-' => a.wrapping_sub(b),
        '*' => a.wrapping_mul(b),
        _ => unreachable!(),
    };
    narrow_wrapping(t, r)
}
pub fn saturating_op(t: IntTy, a: i128, b: i128, op: char) -> i128 {
    if is_u128(t) {
        let (x, y) = (a as u128, b as u128);
        return match op {
            '+' => x.saturating_add(y),
            '-' => x.saturating_sub(y),
            '*' => x.saturating_mul(y),
            _ => unreachable!(),
        } as i128;
    }
    let (min, max) = int_min_max(t);
    let r = match op {
        '+' => a.checked_add(b),
        '-' => a.checked_sub(b),
        '*' => a.checked_mul(b),
        _ => unreachable!(),
    };
    match r {
        Some(r) => r.clamp(min, max),
        // Host overflow only happens for i128 itself, in the direction the
        // operands' signs indicate.
        None => {
            let positive = match op {
                '+' => b > 0,
                '-' => b < 0,
                '*' => (a < 0) == (b < 0),
                _ => unreachable!(),
            };
            if positive { max } else { min }
        }
    }
}

pub type Arced<T> = Arc<T>;

// `print`'s text for a float (`rule.stdlib.print` [Print-Float], spec/21
// §2a): the shortest decimal digits that read back as `v` in its own
// type, laid out positionally for a decimal exponent k with -7 < k < 21
// (always with a fraction, `1.0`), and as `d.ddde±k` otherwise (`1.0e21`,
// `1.5e-7`). `cbrt::format_float` is the same function, so `coby` and
// `cobc` print the same bytes.
pub fn format_float(v: f64, is_f32: bool) -> String {
    crate::fmt::format_float(v, is_f32)
}
