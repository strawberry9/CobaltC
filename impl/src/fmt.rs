// `printf` and `String::appendf`'s format strings (`spec/21` §2f
// `rule.stdlib.format`, D-0038): parsing, the static check of each
// specifier against its argument's kind, and the text each produces. One
// source file for both implementations: the interpreter uses it as a
// module, the compiler's runtime (`cbrt`) includes this same file, and
// the front end checks formats with it, so all three agree exactly.

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Lit(Vec<u8>),
    Spec(Spec),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spec {
    pub left: bool,
    pub zero: bool,
    pub plus: bool,
    pub space: bool,
    pub alt: bool,
    pub width: usize,
    pub prec: Option<usize>,
    pub conv: u8,
}

// What a specifier accepts: an integer, a float, or text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Int,
    Float,
    Text,
    // `%v`: any value, as it is.
    Value,
}

impl Spec {
    pub fn kind(&self) -> Kind {
        match self.conv {
            b'd' | b'i' | b'u' | b'x' | b'X' | b'o' | b'b' => Kind::Int,
            b'f' | b'F' | b'e' | b'E' | b'g' | b'G' => Kind::Float,
            b'v' => Kind::Value,
            _ => Kind::Text,
        }
    }
}

// An argument's value, as the formatter sees it.
#[derive(Debug, Clone)]
pub enum Arg {
    // A value's bit pattern in its own width, and whether its type is signed.
    Int { bits: u128, signed: bool, width: u32 },
    // A float's value, and whether its type is `f32` (`%v` chooses its
    // digits in that type).
    Float(f64, bool),
    Text(Vec<u8>),
}

const LIMIT: usize = 4096;

// The pieces of `fmt`, or why it is not a format: an unknown conversion,
// a `%` at the end, a flag the conversion does not take.
pub fn parse(fmt: &[u8]) -> Result<Vec<Piece>, String> {
    let mut out = Vec::new();
    let mut lit = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] != b'%' {
            lit.push(fmt[i]);
            i += 1;
            continue;
        }
        i += 1;
        if fmt.get(i) == Some(&b'%') {
            lit.push(b'%');
            i += 1;
            continue;
        }
        let mut s = Spec::default();
        while let Some(&c) = fmt.get(i) {
            match c {
                b'-' => s.left = true,
                b'0' => s.zero = true,
                b'+' => s.plus = true,
                b' ' => s.space = true,
                b'#' => s.alt = true,
                _ => break,
            }
            i += 1;
        }
        let digits = |i: &mut usize| -> Result<usize, String> {
            let start = *i;
            while fmt.get(*i).map_or(false, |c| c.is_ascii_digit()) {
                *i += 1;
            }
            if start == *i {
                return Ok(0);
            }
            let n: usize = std::str::from_utf8(&fmt[start..*i]).unwrap().parse().unwrap_or(usize::MAX);
            if n > LIMIT {
                return Err(format!("a width or precision above {}", LIMIT));
            }
            Ok(n)
        };
        s.width = digits(&mut i)?;
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            s.prec = Some(digits(&mut i)?);
        }
        let Some(&conv) = fmt.get(i) else { return Err("a `%` with no conversion at the end".into()) };
        i += 1;
        if !b"diuxXobfFeEgGsv".contains(&conv) {
            return Err(format!("`%{}` is not a conversion", conv as char));
        }
        s.conv = conv;
        let ok = match s.kind() {
            Kind::Int if b"diu".contains(&conv) => !s.alt,
            Kind::Int => !s.plus && !s.space,
            Kind::Float => true,
            Kind::Text => !s.zero && !s.plus && !s.space && !s.alt,
            Kind::Value => !s.zero && !s.plus && !s.space && !s.alt && s.prec.is_none(),
        };
        if !ok {
            return Err(format!("a flag `%{}` does not take", conv as char));
        }
        if !lit.is_empty() {
            out.push(Piece::Lit(std::mem::take(&mut lit)));
        }
        out.push(Piece::Spec(s));
    }
    if !lit.is_empty() {
        out.push(Piece::Lit(lit));
    }
    Ok(out)
}

// The text of `pieces` with `args`, one per specifier, each of its kind.
pub fn render(pieces: &[Piece], args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut k = 0;
    for p in pieces {
        match p {
            Piece::Lit(b) => out.extend_from_slice(b),
            Piece::Spec(s) => {
                if let Some(a) = args.get(k) {
                    one(s, a, &mut out);
                }
                k += 1;
            }
        }
    }
    out
}

fn one(s: &Spec, a: &Arg, out: &mut Vec<u8>) {
    let (sign, prefix, body): (String, String, String) = match a {
        // `%v`: the value as it is, a float in its shortest exact form.
        Arg::Int { bits, signed, width } if s.conv == b'v' => {
            let d = Spec { conv: b'd', ..Spec::default() };
            int_text(&d, *bits, *signed, *width)
        }
        Arg::Float(v, is32) if s.conv == b'v' => (String::new(), String::new(), format_float(*v, *is32)),
        Arg::Int { bits, signed, width } => int_text(s, *bits, *signed, *width),
        Arg::Float(v, _) => float_text(s, *v),
        Arg::Text(t) => {
            let text = String::from_utf8_lossy(t).into_owned();
            let text: String = match s.prec {
                Some(p) => text.chars().take(p).collect(),
                None => text,
            };
            (String::new(), String::new(), text)
        }
    };
    // Width counts characters. `0` pads after the sign and prefix, for a
    // finite number with no precision on an integer; otherwise spaces.
    let len = sign.chars().count() + prefix.chars().count() + body.chars().count();
    let pad = s.width.saturating_sub(len);
    let finite = !matches!(a, Arg::Float(v, _) if !v.is_finite());
    let zero_ok = s.zero && !s.left && finite && !(s.kind() == Kind::Int && s.prec.is_some());
    let text = if s.left {
        format!("{}{}{}{}", sign, prefix, body, " ".repeat(pad))
    } else if zero_ok {
        format!("{}{}{}{}", sign, prefix, "0".repeat(pad), body)
    } else {
        format!("{}{}{}{}", " ".repeat(pad), sign, prefix, body)
    };
    out.extend_from_slice(text.as_bytes());
}

fn int_text(s: &Spec, bits: u128, signed: bool, width: u32) -> (String, String, String) {
    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
    let bits = bits & mask;
    let neg = signed && width > 0 && (bits >> (width - 1)) & 1 == 1;
    let (sign, digits, prefix) = match s.conv {
        b'd' | b'i' | b'u' => {
            let mag = if neg { (!bits).wrapping_add(1) & mask } else { bits };
            let sign = if neg {
                "-"
            } else if s.plus {
                "+"
            } else if s.space {
                " "
            } else {
                ""
            };
            (sign.to_string(), mag.to_string(), String::new())
        }
        c => {
            // The bits of the value in its type's width.
            let (d, p) = match c {
                b'x' => (format!("{:x}", bits), "0x"),
                b'X' => (format!("{:X}", bits), "0X"),
                b'o' => (format!("{:o}", bits), "0o"),
                _ => (format!("{:b}", bits), "0b"),
            };
            (String::new(), d, if s.alt { p.to_string() } else { String::new() })
        }
    };
    // A precision is a minimum number of digits; `.0` with 0 prints none.
    let digits = match s.prec {
        Some(0) if digits == "0" => String::new(),
        Some(p) if digits.len() < p => format!("{}{}", "0".repeat(p - digits.len()), digits),
        _ => digits,
    };
    (sign, prefix, digits)
}

fn float_text(s: &Spec, v: f64) -> (String, String, String) {
    let upper = s.conv.is_ascii_uppercase();
    let sign = if v.is_sign_negative() && !v.is_nan() {
        "-"
    } else if s.plus {
        "+"
    } else if s.space {
        " "
    } else {
        ""
    };
    let a = v.abs();
    let body = if v.is_nan() {
        "nan".to_string()
    } else if v.is_infinite() {
        "inf".to_string()
    } else {
        let p = s.prec.unwrap_or(6);
        match s.conv.to_ascii_lowercase() {
            b'f' => fixed(a, p, s.alt),
            b'e' => exp(a, p, s.alt),
            _ => general(a, p, s.alt),
        }
    };
    let body = if upper { body.to_ascii_uppercase() } else { body };
    (sign.to_string(), String::new(), body)
}

// `a` with `p` digits after the point, correctly rounded; `#` keeps the
// point when `p` is 0.
fn fixed(a: f64, p: usize, alt: bool) -> String {
    let t = format!("{:.*}", p, a);
    if p == 0 && alt {
        format!("{}.", t)
    } else {
        t
    }
}

// `d.ddde±XX`: `p` digits after the point, an exponent of at least two
// digits.
fn exp(a: f64, p: usize, alt: bool) -> String {
    let t = format!("{:.*e}", p, a);
    let (m, e) = t.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let m = if p == 0 && alt { format!("{}.", m) } else { m.to_string() };
    format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
}

// C's `%g`: `p` significant digits (0 means 1), in `%e` form when the
// exponent is below -4 or at least `p`, else `%f` form; trailing zeros
// (and a trailing point) removed unless `#`.
fn general(a: f64, p: usize, alt: bool) -> String {
    let p = p.max(1);
    let x: i32 = if a == 0.0 {
        0
    } else {
        let t = format!("{:.*e}", p - 1, a);
        t.split_once('e').unwrap().1.parse().unwrap()
    };
    let t = if x < -4 || x >= p as i32 { exp(a, p - 1, alt) } else { fixed(a, (p as i32 - 1 - x) as usize, alt) };
    if alt {
        return t;
    }
    let (m, e) = match t.find('e') {
        Some(k) => (t[..k].to_string(), t[k..].to_string()),
        None => (t.clone(), String::new()),
    };
    let m = if m.contains('.') { m.trim_end_matches('0').trim_end_matches('.').to_string() } else { m };
    format!("{}{}", m, e)
}

// The shortest decimal text that reads back as `v` (in `f32` when
// `is_f32`): `print`'s and `%v`'s form of a float (`rule.stdlib.print`
// `[Print-Float]`). `coby` and `cbrt` both call this one.
pub fn format_float(v: f64, is_f32: bool) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    // `{:e}` is Rust's shortest round-trip form (`-1.2345e-7`, `1e21`);
    // it settles the digit count n. The n-digit decimal nearest `v`, ties
    // to even (`{:.*e}`), is the one printed whenever it reads back as
    // `v`: among equally short candidates, the nearest, as JavaScript's
    // and Python's number-to-text rules choose.
    let sci = if is_f32 { format!("{:e}", v as f32) } else { format!("{:e}", v) };
    let n = sci.split('e').next().unwrap_or("").chars().filter(|c| c.is_ascii_digit()).count();
    let nearest = if is_f32 { format!("{:.*e}", n - 1, v as f32) } else { format!("{:.*e}", n - 1, v) };
    let reads_back = if is_f32 { nearest.parse::<f32>().map_or(false, |r| r == v as f32) } else { nearest.parse::<f64>().map_or(false, |r| r == v) };
    let sci = if reads_back { nearest } else { sci };
    let (neg, sci) = match sci.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, sci.as_str()),
    };
    let (mant, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let k: i32 = exp.parse().expect("{:e}'s exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if -7 < k && k < 21 {
        if k >= 0 {
            let k = k as usize;
            let int_len = k + 1;
            if digits.len() <= int_len {
                out.push_str(&digits);
                out.push_str(&"0".repeat(int_len - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        } else {
            out.push_str("0.");
            out.push_str(&"0".repeat((-k - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        out.push_str(if digits.len() > 1 { &digits[1..] } else { "0" });
        out.push('e');
        out.push_str(&k.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(fmt: &str, args: &[Arg]) -> String {
        String::from_utf8(render(&parse(fmt.as_bytes()).unwrap(), args)).unwrap()
    }
    fn i(v: i128, width: u32) -> Arg {
        Arg::Int { bits: v as u128, signed: true, width }
    }
    fn u(v: u128, width: u32) -> Arg {
        Arg::Int { bits: v, signed: false, width }
    }

    #[test]
    fn matches_c() {
        assert_eq!(f("%d|%5d|%-5d|%05d|%+d|% d", &[i(42, 32), i(42, 32), i(42, 32), i(-42, 32), i(42, 32), i(42, 32)]), "42|   42|42   |-0042|+42| 42");
        assert_eq!(f("%x|%X|%#x|%#06x|%o|%b|%x", &[u(255, 8), u(255, 8), u(255, 8), u(255, 8), u(8, 8), u(5, 8), i(-1, 8)]), "ff|FF|0xff|0x00ff|10|101|ff");
        assert_eq!(f("%.3d|%8.3d|%.0d", &[i(7, 32), i(-7, 32), i(0, 32)]), "007|    -007|");
        assert_eq!(f("%u", &[u(u64::MAX as u128, 64)]), "18446744073709551615");
        assert_eq!(f("%d", &[i(i128::MIN, 128)]), "-170141183460469231731687303715884105728");
        assert_eq!(f("%f|%.2f|%8.3f|%-8.1f|%08.2f|%.0f|%#.0f", &[
            Arg::Float(3.14159, false), Arg::Float(3.14159, false), Arg::Float(-3.14159, false), Arg::Float(2.5, false), Arg::Float(-2.5, false), Arg::Float(2.5, false), Arg::Float(3.0, false)
        ]), "3.141590|3.14|  -3.142|2.5     |-0002.50|2|3.");
        assert_eq!(f("%e|%.2E|%e", &[Arg::Float(1234.5, false), Arg::Float(0.000123, false), Arg::Float(0.0, false)]), "1.234500e+03|1.23E-04|0.000000e+00");
        assert_eq!(f("%g|%g|%g|%g|%.3g|%#g|%G", &[
            Arg::Float(100000.0, false), Arg::Float(1000000.0, false), Arg::Float(0.0001, false), Arg::Float(0.00001, false), Arg::Float(3.14159, false), Arg::Float(1.5, false), Arg::Float(1e-10, false)
        ]), "100000|1e+06|0.0001|1e-05|3.14|1.50000|1E-10");
        assert_eq!(f("%f|%F|%5.1f|%+f", &[Arg::Float(f64::NAN, false), Arg::Float(f64::INFINITY, false), Arg::Float(f64::NEG_INFINITY, false), Arg::Float(f64::INFINITY, false)]), "nan|INF| -inf|+inf");
        assert_eq!(f("[%s|%6s|%-6s|%.2s|%3s]", &[
            Arg::Text(b"hi".to_vec()), Arg::Text(b"hi".to_vec()), Arg::Text(b"hi".to_vec()), Arg::Text("h\u{e9}llo".as_bytes().to_vec()), Arg::Text("\u{e9}".as_bytes().to_vec())
        ]), "[hi|    hi|hi    |h\u{e9}|  \u{e9}]");
        assert_eq!(f("100%% %s", &[Arg::Text(b"done".to_vec())]), "100% done");
        assert_eq!(f("%v|%v|%v|%v|%5v|%-5v|", &[i(-42, 32), Arg::Float(0.1, false), Arg::Float(0.1f32 as f64, true), Arg::Text(b"t".to_vec()), i(7, 8), i(7, 8)]), "-42|0.1|0.1|t|    7|7    |");
    }

    #[test]
    fn rejects() {
        for bad in ["%", "%q", "%5", "%#d", "%0s", "%+x", "% s", "%#s", "%5000d", "%.2v", "%05v", "%+v"] {
            assert!(parse(bad.as_bytes()).is_err(), "{}", bad);
        }
    }
}
