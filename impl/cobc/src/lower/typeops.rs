// Type substitution and integer-literal helpers.

use super::*;

// ---- types and names ----

pub(super) fn subst_of(params: &[String], args: &[Type]) -> HashMap<String, Type> {
    params.iter().cloned().zip(args.iter().cloned()).collect()
}

pub(super) fn apply(t: &Type, s: &HashMap<String, Type>) -> Type {
    match t {
        Type::Named(n, args) if args.is_empty() && s.contains_key(n) => s[n].clone(),
        Type::Named(n, args) => Type::Named(n.clone(), args.iter().map(|a| apply(a, s)).collect()),
        Type::Ref(i, m) => Type::Ref(Box::new(apply(i, s)), m.clone()),
        Type::Slice(i, m) => Type::Slice(Box::new(apply(i, s)), m.clone()),
        Type::Rawptr(i) => Type::Rawptr(Box::new(apply(i, s))),
        Type::Array(i, n) => Type::Array(Box::new(apply(i, s)), *n),
        Type::Fn(ps, r) => Type::Fn(ps.iter().map(|p| apply(p, s)).collect(), Box::new(apply(r, s))),
        Type::Handle(i) => Type::Handle(Box::new(apply(i, s))),
        Type::Mutex(i) => Type::Mutex(Box::new(apply(i, s))),
        Type::Guard(i) => Type::Guard(Box::new(apply(i, s))),
        other => other.clone(),
    }
}

pub(super) fn resolved(t: &Type, params: &[String]) -> bool {
    match t {
        Type::Named(n, args) => !(args.is_empty() && params.contains(n)) && args.iter().all(|a| resolved(a, params)),
        Type::Ref(i, _) | Type::Slice(i, _) | Type::Rawptr(i) | Type::Array(i, _) | Type::Handle(i) | Type::Mutex(i) | Type::Guard(i) => resolved(i, params),
        Type::Fn(ps, r) => ps.iter().all(|p| resolved(p, params)) && resolved(r, params),
        _ => true,
    }
}

pub(super) fn unify(pattern: &Type, actual: &Type, params: &[String], m: &mut HashMap<String, Type>) {
    match (pattern, actual) {
        (Type::Named(n, args), _) if args.is_empty() && params.contains(n) => {
            m.entry(n.clone()).or_insert_with(|| actual.clone());
        }
        (Type::Named(n1, a1), Type::Named(n2, a2)) if n1 == n2 && a1.len() == a2.len() => {
            for (p, a) in a1.iter().zip(a2.iter()) {
                unify(p, a, params, m);
            }
        }
        (Type::Ref(p, _), Type::Ref(a, _)) | (Type::Slice(p, _), Type::Slice(a, _)) | (Type::Rawptr(p), Type::Rawptr(a)) | (Type::Array(p, _), Type::Array(a, _)) | (Type::Handle(p), Type::Handle(a)) | (Type::Mutex(p), Type::Mutex(a)) | (Type::Guard(p), Type::Guard(a)) => unify(p, a, params, m),
        (Type::Fn(ps, r), Type::Fn(as_, ar)) if ps.len() == as_.len() => {
            for (p, a) in ps.iter().zip(as_.iter()) {
                unify(p, a, params, m);
            }
            unify(r, ar, params, m);
        }
        _ => {}
    }
}

pub(super) fn is_bare_literal(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None)),
        ExprKind::Paren(inner) => is_bare_literal(inner),
        _ => false,
    }
}

pub(super) fn int_lit_type(suf: Option<&str>, expected: Option<&Type>) -> IntTy {
    IntTy::of_literal(suf, expected).0
}

// A type's `min`/`max` as the `i128` payload `c_int` expects: for the
// 128-bit types that is the bit pattern (`u128::MAX` is all ones).
pub(super) fn bounds(t: IntTy) -> (i128, i128) {
    match t {
        IntTy::U128 => (0, -1),
        IntTy::I128 => (i128::MIN, i128::MAX),
        _ => int_min_max(t),
    }
}

// Every value of `s` is a value of `d`: a `narrow` from `s` to `d`
// cannot fault.
pub(super) fn int_range_within(s: IntTy, d: IntTy) -> bool {
    let (sb, db) = (s.bitwidth(64), d.bitwidth(64));
    match (s.signed(), d.signed()) {
        (false, false) | (true, true) => sb <= db,
        (false, true) => sb < db,
        (true, false) => false,
    }
}

pub(super) fn round_up(x: u64, a: u64) -> u64 {
    if a <= 1 {
        x
    } else {
        (x + a - 1) / a * a
    }
}
