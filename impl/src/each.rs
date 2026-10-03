// `foreach` (D-0042, spec/14 `rule.control.foreach`). The parser writes
// a `foreach` as a counted loop over hidden bindings, calling the
// `$each_*` intrinsics for everything that depends on the collection's
// type. Each tool expands a call here once it knows the type of its
// first argument (always a hidden binding): the expansion is ordinary
// code over `std`'s own functions and fields, which no program could
// write itself (`typecheck` checks it with `std`'s field visibility).
//
//   $each_drain(e, n)       e : C (moved)          the consuming form's holder
//   $each_len(h)            h : &C, &mut C, holder  the number of elements
//   $each_at(c, i, n)       c : &C                  element i, shared (n: the number of names)
//   $each_at_mut(c, i, n)   c : &mut C              element i, exclusive
//   $each_key(c, i)         c : &C or &mut C        HashMap: key i; otherwise i
//   $each_take(d, i, n)     d : holder              element i, moved out
//   $each_take_key(d, i)    d : holder              HashMap: key i, moved out; otherwise i
//   $each_range(b, e, n)    b : a range's bound     `e`, a literal expression, as b's integer type; () for none (D-0097)

use crate::ast::*;

pub const NAMES: &[&str] = &["$each_drain", "$each_len", "$each_at", "$each_at_mut", "$each_key", "$each_take", "$each_take_key", "$each_range"];

enum Coll {
    Vec,
    Array(u128),
    Set,
    Map,
    Drain,
    SetDrain,
    MapDrain,
    Slice(Mode),
}

fn coll(t: &Type) -> Option<Coll> {
    match t {
        Type::Array(_, n) => Some(Coll::Array(*n)),
        Type::Slice(_, m) => Some(Coll::Slice(m.clone())),
        Type::Named(n, _) => match n.as_str() {
            "std::Vec" => Some(Coll::Vec),
            "std::HashSet" => Some(Coll::Set),
            "std::HashMap" => Some(Coll::Map),
            "std::Drain" => Some(Coll::Drain),
            "std::SetDrain" => Some(Coll::SetDrain),
            "std::HashDrain" => Some(Coll::MapDrain),
            _ => None,
        },
        _ => None,
    }
}

// Whether a literal expression has a floating-point literal in it.
pub fn has_float(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::FloatLit(..) => true,
        ExprKind::Paren(x) | ExprKind::Unary(_, x) => has_float(x),
        ExprKind::Binary(_, a, b) => has_float(a) || has_float(b),
        _ => false,
    }
}

// Whether a literal expression is a negation at its top (`-3`, `(-3)`).
fn negated(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Unary(UnOp::Neg, _) => true,
        ExprKind::Paren(x) => negated(x),
        _ => false,
    }
}

// `name`'s expansion, given the type of `args[0]`; an `Err` is the
// diagnostic (`diag.type-mismatch`: not a collection, or not one this
// form of `foreach` takes).
pub fn expand(name: &str, t0: &Type, args: &[Expr], line: usize) -> Result<Expr, String> {
    let mk = |kind: ExprKind| Expr { kind, line };
    let path = |key: &str| mk(ExprKind::Path(key.split("::").map(|s| s.to_string()).collect(), vec![]));
    let call = |key: &str, a: Vec<Expr>| mk(ExprKind::Call(Box::new(path(key)), a));
    let shared = |e: Expr| mk(ExprKind::Borrow(Mode::Shared, Box::new(e)));
    let excl = |e: Expr| mk(ExprKind::Borrow(Mode::Exclusive, Box::new(e)));
    let deref = |e: Expr| mk(ExprKind::Deref(Box::new(e)));
    let field = |e: Expr, f: &str| mk(ExprKind::Field(Box::new(e), f.to_string()));
    let bad = || Err("diag.type-mismatch".to_string());
    let a0 = args.first().cloned().ok_or("diag.type-mismatch")?;
    // `foreach (x in lo .. hi)` (D-0097): `a0` is a bound, `args[1]` the
    // other when it is a literal expression (given `a0`'s type), and the
    // number of names last.
    if name == "$each_range" {
        let detail = |m: String| Err(format!("diag.type-mismatch{}{}", crate::diagnostics::DETAIL_SEP, m));
        if !matches!(args.get(2).map(|e| &e.kind), Some(ExprKind::IntLit(1 | 2, _))) {
            return detail("a range is looped over with one name, the value, or two: its position and the value".to_string());
        }
        if !matches!(t0, Type::Int(_)) {
            return detail(format!("a range's bounds are integers, but this one is `{}`", t0));
        }
        return match args.get(1) {
            Some(e) if matches!(e.kind, ExprKind::Unit) => Ok(mk(ExprKind::Unit)),
            // Two literal bounds are typed through a hidden `$z` (a `usize`).
            Some(e) if matches!(t0, Type::Int(k) if !(*k).signed()) && negated(e) => {
                let plain = matches!(&a0.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0].starts_with("$z"));
                detail(if plain {
                    "a range of two plain-number bounds counts in `usize`, which cannot be negative; give one bound a signed type: `-3..0: i32`".to_string()
                } else {
                    format!("a range of `{}` cannot have a negative bound", t0)
                })
            }
            Some(e) if has_float(e) => detail(format!("a range's bounds are integers of one type: the other bound is `{}`, this one a floating-point literal", t0)),
            Some(e) => {
                let r = "$r".to_string();
                let bind = Stmt::Let { ty: Some(t0.clone()), name: r.clone(), init: Some(Box::new(e.clone())), line };
                Ok(mk(ExprKind::Block(Block { stmts: vec![bind], tail: Some(Box::new(path(&r))) })))
            }
            None => bad(),
        };
    }
    let a1 = || args.get(1).cloned().unwrap_or_else(|| mk(ExprKind::Unit));
    // The borrowed forms hold a reference; the consuming form a holder.
    let (inner, mode) = match t0 {
        Type::Ref(inner, m) => (&**inner, Some(m.clone())),
        other => (other, None),
    };
    let Some(c) = coll(inner) else {
        return bad();
    };
    // A reference written without `&` is looped over as the borrow it is:
    // its holder is the reference itself.
    let name = match (name, &mode) {
        ("$each_drain", Some(_)) => return Ok(a0),
        ("$each_take", Some(Mode::Shared)) => "$each_at",
        ("$each_take", Some(Mode::Exclusive)) => "$each_at_mut",
        ("$each_take_key", Some(_)) => "$each_key",
        _ => name,
    };
    // A HashMap has a key and a value: `foreach (k, v in m)` or, with its
    // position, `foreach (i, k, v in m)`; never one name. Three names are
    // for a map only.
    let names = match args.get(2).map(|e| &e.kind) {
        Some(ExprKind::IntLit(n, _)) => *n,
        _ => 0,
    };
    if name == "$each_at" || name == "$each_at_mut" {
        let map = matches!(c, Coll::Map);
        if (map && names == 1) || (!map && names == 3) {
            return bad();
        }
    }
    match (name, mode, c) {
        ("$each_drain", None, Coll::Vec | Coll::Array(_) | Coll::Set | Coll::Slice(_)) if matches!(args.get(1).map(|e| &e.kind), Some(ExprKind::IntLit(3, _))) => bad(),
        // A slice (D-0047) is a borrow already: looped over in its own mode.
        ("$each_drain", None, Coll::Slice(_)) => Ok(a0),
        ("$each_len", _, Coll::Slice(_)) => Ok(call("slice_len", vec![a0])),
        ("$each_take", None, Coll::Slice(Mode::Shared)) => Ok(shared(mk(ExprKind::Index(Box::new(a0), Box::new(a1()))))),
        ("$each_take", None, Coll::Slice(Mode::Exclusive)) => Ok(excl(mk(ExprKind::Index(Box::new(a0), Box::new(a1()))))),
        ("$each_take_key", None, Coll::Slice(_)) => Ok(a1()),
        ("$each_at", Some(Mode::Shared), Coll::Slice(_)) => Ok(shared(mk(ExprKind::Index(Box::new(deref(a0)), Box::new(a1()))))),
        ("$each_at_mut", Some(Mode::Exclusive), Coll::Slice(_)) => Ok(excl(mk(ExprKind::Index(Box::new(deref(a0)), Box::new(a1()))))),
        ("$each_key", Some(_), Coll::Slice(_)) => Ok(a1()),
        ("$each_drain", None, Coll::Vec) => Ok(call("std::Vec::drain", vec![a0])),
        ("$each_drain", None, Coll::Array(_)) => Ok(a0),
        ("$each_drain", None, Coll::Set) => Ok(call("std::HashSet::drain", vec![a0])),
        ("$each_drain", None, Coll::Map) => match args.get(1).map(|e| &e.kind) {
            Some(ExprKind::IntLit(2 | 3, _)) => Ok(call("std::HashMap::drain", vec![a0])),
            _ => bad(), // one name: a HashMap has a key and a value
        },

        ("$each_len", Some(_), Coll::Vec) => Ok(call("std::Vec::len", vec![shared(deref(a0))])),
        ("$each_len", Some(_), Coll::Set) => Ok(call("std::HashSet::len", vec![shared(deref(a0))])),
        ("$each_len", Some(_), Coll::Map) => Ok(call("std::HashMap::len", vec![shared(deref(a0))])),
        ("$each_len", _, Coll::Array(n)) => Ok(mk(ExprKind::IntLit(n, None))),
        ("$each_len", None, Coll::Drain) => Ok(call("std::Drain::total", vec![shared(a0)])),
        ("$each_len", None, Coll::SetDrain) => Ok(call("std::SetDrain::total", vec![shared(a0)])),
        ("$each_len", None, Coll::MapDrain) => Ok(call("std::HashDrain::total", vec![shared(a0)])),

        ("$each_at", Some(Mode::Shared), Coll::Vec) => Ok(call("std::Vec::index_shared", vec![a0, a1()])),
        ("$each_at", Some(Mode::Shared), Coll::Array(_)) => Ok(shared(mk(ExprKind::Index(Box::new(deref(a0)), Box::new(a1()))))),
        ("$each_at", Some(Mode::Shared), Coll::Set) => Ok(call("std::HashSet::key_at", vec![a0, a1()])),
        ("$each_at", Some(Mode::Shared), Coll::Map) => Ok(call("std::Vec::index_shared", vec![shared(field(deref(a0), "values")), a1()])),

        ("$each_at_mut", Some(Mode::Exclusive), Coll::Vec) => Ok(call("std::Vec::index_exclusive", vec![excl(deref(a0)), a1()])),
        ("$each_at_mut", Some(Mode::Exclusive), Coll::Array(_)) => Ok(excl(mk(ExprKind::Index(Box::new(deref(a0)), Box::new(a1()))))),
        ("$each_at_mut", Some(Mode::Exclusive), Coll::Map) => Ok(call("std::Vec::index_exclusive", vec![excl(field(deref(a0), "values")), a1()])),
        // A set's keys are not changed in place: `&mut` of a HashSet is refused.

        ("$each_key", Some(_), Coll::Map) => Ok(call("std::Vec::index_shared", vec![shared(field(deref(a0), "keys")), a1()])),
        ("$each_key", Some(_), Coll::Vec | Coll::Array(_) | Coll::Set) => Ok(a1()),

        ("$each_take", None, Coll::Drain) => Ok(call("std::Drain::take", vec![excl(a0)])),
        ("$each_take", None, Coll::Array(_)) => Ok(mk(ExprKind::Index(Box::new(a0), Box::new(a1())))),
        ("$each_take", None, Coll::SetDrain) => Ok(call("std::SetDrain::take", vec![excl(a0)])),
        ("$each_take", None, Coll::MapDrain) => Ok(call("std::HashDrain::take_value", vec![excl(a0)])),

        ("$each_take_key", None, Coll::MapDrain) => Ok(call("std::HashDrain::take_key", vec![excl(a0)])),
        ("$each_take_key", None, Coll::Drain | Coll::Array(_) | Coll::SetDrain) => Ok(a1()),

        _ => bad(),
    }
}
