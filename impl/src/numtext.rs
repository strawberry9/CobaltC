// `parse<T>`'s scanner and conversion (`spec/21` §2d `rule.stdlib.text`
// `[Parse]`). One source file for both implementations: the interpreter
// uses it as a module, and the compiler's runtime (`cbrt`) includes this
// same file, so `coby` and `cobc` accept and reject exactly the same text.
//
// A result is `Ok(value)` or `Err(code)`: `EMPTY`, `OUT_OF_RANGE`, or an
// offset k >= 0 (`Invalid(k)`: the length of the longest prefix that can
// still begin a number).

pub const EMPTY: i64 = -2;
pub const OUT_OF_RANGE: i64 = -3;

fn digits(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i
}

// An integer of `bits` bits, as its two's-complement pattern in a u128.
// `number(T) := sign? digit+` (no `-` when unsigned).
pub fn parse_int(b: &[u8], signed: bool, bits: u32) -> Result<u128, i64> {
    if b.is_empty() {
        return Err(EMPTY);
    }
    let mut i = 0;
    let neg = b[0] == b'-';
    if b[0] == b'+' || (neg && signed) {
        i = 1;
    }
    let end = digits(b, i);
    if end == i || end < b.len() {
        return Err(end as i64);
    }
    let mut mag: u128 = 0;
    for &c in &b[i..end] {
        mag = match mag.checked_mul(10).and_then(|m| m.checked_add((c - b'0') as u128)) {
            Some(m) => m,
            None => return Err(OUT_OF_RANGE),
        };
    }
    let limit: u128 = match (signed, neg) {
        (false, _) => if bits == 128 { u128::MAX } else { (1u128 << bits) - 1 },
        (true, false) => (1u128 << (bits - 1)) - 1,
        (true, true) => 1u128 << (bits - 1),
    };
    if mag > limit {
        return Err(OUT_OF_RANGE);
    }
    Ok(if neg { (mag as i128).wrapping_neg() as u128 } else { mag })
}

// A float, correctly rounded in `f32` when `is_f32` (then widened, which
// is exact). `number(T) := sign? (digit+ ('.' digit+)? (('e'|'E') sign?
// digit+)? | 'inf') | 'NaN'`.
pub fn parse_float(b: &[u8], is_f32: bool) -> Result<f64, i64> {
    if b.is_empty() {
        return Err(EMPTY);
    }
    if b == b"NaN" {
        return Ok(f64::NAN);
    }
    let mut i = 0;
    if b[0] == b'+' || b[0] == b'-' {
        i = 1;
    }
    let rest = &b[i..];
    if rest.first() == Some(&b'i') {
        // `inf`: as far as it matches.
        let k = rest.iter().zip(b"inf").take_while(|(a, c)| a == c).count();
        if k == 3 && rest.len() == 3 {
            return Ok(if b[0] == b'-' { f64::NEG_INFINITY } else { f64::INFINITY });
        }
        return Err((i + k) as i64);
    }
    if i == 0 && rest.first() == Some(&b'N') {
        // `NaN`, but not all of it (the whole text was checked above).
        let k = rest.iter().zip(b"NaN").take_while(|(a, c)| a == c).count();
        return Err(k as i64);
    }
    let mut j = digits(b, i);
    if j == i {
        return Err(j as i64);
    }
    if j < b.len() && b[j] == b'.' {
        let k = digits(b, j + 1);
        if k == j + 1 {
            return Err(k as i64);
        }
        j = k;
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let e = digits(b, k);
        if e == k {
            return Err(e as i64);
        }
        j = e;
    }
    if j < b.len() {
        return Err(j as i64);
    }
    // The text is ASCII and in the grammar, which Rust's conversion
    // accepts and rounds correctly.
    let s = std::str::from_utf8(b).expect("ASCII");
    let v = if is_f32 { s.parse::<f32>().expect("in the grammar") as f64 } else { s.parse::<f64>().expect("in the grammar") };
    if v.is_infinite() {
        return Err(OUT_OF_RANGE);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers() {
        assert_eq!(parse_int(b"42", true, 32), Ok(42));
        assert_eq!(parse_int(b"-42", true, 32), Ok((-42i128) as u128));
        assert_eq!(parse_int(b"+7", false, 8), Ok(7));
        assert_eq!(parse_int(b"007", false, 8), Ok(7));
        assert_eq!(parse_int(b"", true, 32), Err(EMPTY));
        assert_eq!(parse_int(b"-", true, 32), Err(1));
        assert_eq!(parse_int(b"-1", false, 32), Err(0));
        assert_eq!(parse_int(b"12x4", true, 32), Err(2));
        assert_eq!(parse_int(b" 1", true, 32), Err(0));
        assert_eq!(parse_int(b"42\r", true, 32), Err(2));
        assert_eq!(parse_int(b"128", true, 8), Err(OUT_OF_RANGE));
        assert_eq!(parse_int(b"-128", true, 8), Ok((-128i128) as u128));
        assert_eq!(parse_int(b"255", false, 8), Ok(255));
        assert_eq!(parse_int(b"256", false, 8), Err(OUT_OF_RANGE));
        assert_eq!(parse_int(b"340282366920938463463374607431768211455", false, 128), Ok(u128::MAX));
        assert_eq!(parse_int(b"340282366920938463463374607431768211456", false, 128), Err(OUT_OF_RANGE));
        assert_eq!(parse_int(b"-170141183460469231731687303715884105728", true, 128), Ok(i128::MIN as u128));
        assert_eq!(parse_int(b"99999999999999999999999999999999999999999x", true, 32), Err(41));
    }

    #[test]
    fn floats() {
        assert_eq!(parse_float(b"0.1", false), Ok(0.1));
        assert_eq!(parse_float(b"1.0e-7", false), Ok(1.0e-7));
        assert_eq!(parse_float(b"-2.5E+3", false), Ok(-2500.0));
        assert_eq!(parse_float(b"0.1", true), Ok(0.1f32 as f64));
        assert_eq!(parse_float(b"inf", false), Ok(f64::INFINITY));
        assert_eq!(parse_float(b"-inf", false), Ok(f64::NEG_INFINITY));
        assert!(parse_float(b"NaN", false).unwrap().is_nan());
        assert_eq!(parse_float(b"-NaN", false), Err(1));
        assert_eq!(parse_float(b"Na", false), Err(2));
        assert_eq!(parse_float(b"in", false), Err(2));
        assert_eq!(parse_float(b"info", false), Err(3));
        assert_eq!(parse_float(b".5", false), Err(0));
        assert_eq!(parse_float(b"5.", false), Err(2));
        assert_eq!(parse_float(b"1e", false), Err(2));
        assert_eq!(parse_float(b"1e+", false), Err(3));
        assert_eq!(parse_float(b"1x", false), Err(1));
        assert_eq!(parse_float(b"1e400", false), Err(OUT_OF_RANGE));
        assert_eq!(parse_float(b"1e39", true), Err(OUT_OF_RANGE));
        assert_eq!(parse_float(b"1e-400", false), Ok(0.0));
        assert_eq!(parse_float(b"", false), Err(EMPTY));
    }
}
