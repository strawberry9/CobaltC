// A static, ahead-of-execution pass over the whole reachable program,
// implementing (a narrowed, honestly-scoped subset of) spec/12's typing
// rules, spec/06 `rule.arith.literal`'s literal defaulting/range check,
// and `rule.control.flow-analysis` (spec/14 §6) in full: the forward
// dataflow over each function body's syntactic CFG with the facts
// `valid(x)`, `init(x)` and `deriv(r, x.π, m)` in the three-valued
// lattice {T, F, ?}, joined at `if`/`match` merges and iterated to a
// fixed point around `while`, and its discharge table applied at every
// reliance point -- so the set of programs this pass *refutes* is
// exactly the spec's static rejection set for those rules (stale
// bindings after a visible move or drop, definite assignment, aliasing
// conflicts against visibly-formed references including the D-0011
// elided call, `solitary` for moves and destroys, `¬live-resource-at`
// for a resource overwrite) plus the literal-operand value-range
// refutations (arithmetic, shift, literal array index). The `escaped`
// fact is not tracked: it only ever distinguishes "proven" from
// "unknown", and this interpreter performs every dynamic check anyway
// (nothing is elided on a proof), so the distinction is unobservable
// here. See `Flow` below.
//
// Design: this is independent from `interp.rs`'s dynamic evaluator, not
// built on top of it — two separately-derived implementations of "what
// type does this expression have" is a deliberate cross-check property
// (if they ever disagree, that is a real bug in one of them), not
// duplication for its own sake.
//
// Generics are checked per *concrete instantiation*, monomorphization-
// style (a worklist of `(name, type-args)` pairs, starting from `main`,
// growing as calls to generic functions with statically-resolvable type
// arguments are discovered) — matching how this interpreter's own
// generics are resolved (structural unification, not a parametric
// calculus), and meaning an instantiation with a statically-unresolvable
// type argument (e.g. `Vec::new::<T>()` with no `let`-annotation or other
// expected-type context to fix `T`) is simply not statically checked —
// the dynamic evaluator remains the backstop for it, as for everything
// else this pass cannot confidently resolve. Being conservative (return
// `Ok(None)` = "unknown, don't check") rather than guessing is the rule
// throughout: this pass must never reject a program the spec accepts.

use crate::ast::*;
use crate::interp::Items;
use crate::value::permitted;
use crate::fxhash::{HashMap, HashSet};

type TResult = Result<Option<Type>, String>;

// [Extern-Non-Ffi-Type] (spec/20 rule.trust.extern-call): `FfiType ::=
// integer types | f32 | f64 | bool | void | rawptr<tau>` -- every
// extern fn's parameter and return types must be one of these.
fn is_ffi_type(t: &Type) -> bool {
    matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Void | Type::Rawptr(_))
}

// D-0254: the intrinsics whose two operands are one `simd` type.
const SIMD_PAIR_INTRINSICS: &[&str] = &["simd_eq", "simd_ne", "simd_lt", "simd_le", "simd_gt", "simd_ge", "simd_min", "simd_max"];

// D-0254: a `simd`'s mask type: signed integer lanes of the lanes' width.
fn simd_mask_of(t: &Type) -> Option<Type> {
    let Type::Simd(lane, n) = t else { return None };
    let m = match &**lane {
        Type::Int(IntTy::I8 | IntTy::U8) => IntTy::I8,
        Type::Int(IntTy::I16 | IntTy::U16) => IntTy::I16,
        Type::Int(IntTy::I32 | IntTy::U32) | Type::F32 => IntTy::I32,
        _ => IntTy::I64,
    };
    Some(Type::Simd(Box::new(Type::Int(m)), *n))
}

// D-0254 `rule.stdlib.simd` (spec/21 §0): the type of a lane-wise intrinsic,
// or `None` for an intrinsic that is not one (nor a wrapping or saturating
// one on a `simd`). Operands of unknown type are left to the run.
fn simd_intrinsic_type(name: &str, targs: &[Type], arg_tys: &[Option<Type>]) -> Result<Option<Type>, String> {
    let arg = |k: usize| arg_tys.get(k).cloned().flatten().filter(|t| !has_unresolved_marker(t) && !matches!(t, Type::Never));
    let simd = |k: usize| arg(k).filter(|t| matches!(t, Type::Simd(..)));
    let lane_of = |t: &Type| match t {
        Type::Simd(l, _) => (**l).clone(),
        _ => Type::Void,
    };
    let need_simd = |k: usize| -> Result<Option<Type>, String> {
        match arg(k) {
            None => Ok(None),
            Some(t @ Type::Simd(..)) => Ok(Some(t)),
            Some(t) => Err(mismatch(format!("`{}` takes a `simd`, and this is `{}`", name, t))),
        }
    };
    let same = |a: &Option<Type>, b: &Option<Type>| -> Result<(), String> {
        if let (Some(x), Some(y)) = (a, b) {
            if x != y {
                return Err(mismatch(format!("`{}` takes two of one `simd` type, and these are `{}` and `{}`", name, x, y)));
            }
        }
        Ok(())
    };
    match name {
        "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" if simd(0).is_some() || simd(1).is_some() => {
            let (a, b) = (need_simd(0)?, need_simd(1)?);
            same(&a, &b)?;
            let t = a.or(b).unwrap();
            if !matches!(lane_of(&t), Type::Int(_)) {
                return Err(mismatch(format!("`{}` on a `simd` takes integer lanes", name)));
            }
            Ok(Some(t))
        }
        "simd_eq" | "simd_ne" | "simd_lt" | "simd_le" | "simd_gt" | "simd_ge" | "simd_min" | "simd_max" => {
            let (a, b) = (need_simd(0)?, need_simd(1)?);
            same(&a, &b)?;
            match a.or(b) {
                Some(t) if name == "simd_min" || name == "simd_max" => Ok(Some(t)),
                Some(t) => Ok(simd_mask_of(&t)),
                None => Ok(None),
            }
        }
        "simd_select" => {
            let (m, a, b) = (need_simd(0)?, need_simd(1)?, need_simd(2)?);
            same(&a, &b)?;
            if let (Some(m), Some(t)) = (&m, a.as_ref().or(b.as_ref())) {
                if simd_mask_of(t).as_ref() != Some(m) {
                    return Err(mismatch(format!("`simd_select` takes a mask of `{}`'s shape, `{}`, and this is `{}`", t, simd_mask_of(t).unwrap(), m)));
                }
            }
            Ok(a.or(b))
        }
        "simd_any" | "simd_all" => {
            if let Some(m) = need_simd(0)? {
                if !matches!(lane_of(&m), Type::Int(t) if t.signed()) {
                    return Err(mismatch(format!("`{}` tests a mask (signed integer lanes, -1 or 0), and this is `{}`", name, m)));
                }
            }
            Ok(Some(Type::Bool))
        }
        "simd_sum" | "simd_reduce_min" | "simd_reduce_max" => Ok(need_simd(0)?.map(|t| lane_of(&t))),
        "simd_load" => {
            let Some(v @ Type::Simd(lane, _)) = targs.first() else {
                return Err(mismatch("`simd_load` names the vector type it makes: `simd_load<simd<f32, 4>>(s, i)`".to_string()));
            };
            match arg(0) {
                Some(Type::Slice(e, _)) if *e == **lane => {}
                Some(t) => return Err(mismatch(format!("`simd_load<{}>` reads a `slice<{}, _>`, and this is `{}`", v, lane, t))),
                None => {}
            }
            Ok(Some(v.clone()))
        }
        "simd_store" => {
            if let (Some(Type::Slice(e, m)), Some(v)) = (arg(0), need_simd(2)?) {
                if m != Mode::Exclusive || *e != lane_of(&v) {
                    return Err(mismatch(format!("`simd_store` writes a `{}` into a `slice<{}, exclusive>`, and this is `slice<{}, {}>`", v, lane_of(&v), e, if m == Mode::Exclusive { "exclusive" } else { "shared" })));
                }
            } else if let Some(t) = arg(0).filter(|t| !matches!(t, Type::Slice(..))) {
                return Err(mismatch(format!("`simd_store` writes into a `slice<τ, exclusive>`, and this is `{}`", t)));
            }
            Ok(Some(Type::Void))
        }
        _ => Ok(None),
    }
}

// D-0254 `[T-Simd-Op]`: an operator with a `simd` operand. Both operands
// are the same `simd` type; `+ - *` on any lanes, `/` on float lanes,
// `& | ^` and the shifts (by a `simd` of counts) on integer lanes; `==` and
// `!=` compare the whole value; the orderings are `simd_lt` and its kin.
fn simd_binary_type(op: BinOp, at: Option<&Type>, bt: Option<&Type>) -> TResult {
    use BinOp::*;
    let (Some(ta), Some(tb)) = (at, bt) else { return Ok(at.or(bt).cloned()) };
    if has_unresolved_marker(ta) || has_unresolved_marker(tb) || matches!(ta, Type::Never) || matches!(tb, Type::Never) {
        return Ok(Some(if matches!(ta, Type::Simd(..)) { ta.clone() } else { tb.clone() }));
    }
    let sym = match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%", Shl => "<<", Shr => ">>", BitAnd => "&", BitOr => "|", BitXor => "^",
        Eq => "==", Ne => "!=", Lt => "<", Le => "<=", Gt => ">", Ge => ">=", And => "&&", Or => "||",
    };
    if ta != tb {
        return Err(mismatch(format!("`{}` on a `simd` takes two of one type, lane by lane, and these are `{}` and `{}`; a scalar is made a vector with `[x; N]`", sym, ta, tb)));
    }
    let Type::Simd(lane, _) = ta else { unreachable!() };
    let int = matches!(&**lane, Type::Int(_));
    match op {
        Add | Sub | Mul => Ok(Some(ta.clone())),
        Div if !int => Ok(Some(ta.clone())),
        Div | Rem => Err(mismatch(format!("`{}` on a `simd` is defined for float lanes only: no processor divides integer lanes at once; divide lane by lane", sym))),
        BitAnd | BitOr | BitXor | Shl | Shr if int => Ok(Some(ta.clone())),
        BitAnd | BitOr | BitXor | Shl | Shr => Err(mismatch(format!("`{}` on a `simd` is defined for integer lanes only", sym))),
        Eq | Ne => Ok(Some(Type::Bool)),
        Lt | Le | Gt | Ge => Err(mismatch(format!("`{}` does not order two vectors; compare them lane by lane with `simd_lt`, `simd_le`, `simd_gt` or `simd_ge`, which give a mask", sym))),
        And | Or => Err(mismatch(format!("`{}` takes `bool`s; masks combine with `&` and `|`", sym))),
    }
}

// D-0249, D-0253: the type of an atomic's one cell, from the type of a
// reference to it: `T` for `Atomic<T>`, `rawptr<T>` for `AtomicPtr<T>`.
fn atomic_cell_ty(t: &Type) -> Option<Type> {
    let Type::Ref(a, _) = t else { return None };
    match &**a {
        Type::Named(n, ta) if n == "std::Atomic" => ta.first().cloned(),
        Type::Named(n, ta) if n == "std::AtomicPtr" => ta.first().map(|t| Type::Rawptr(Box::new(t.clone()))),
        _ => None,
    }
}

// D-0252: `FfiType` also has every `ffi-struct`: a struct, not a
// resource or a `bitstruct`, with at least one field, each an FfiType
// other than `void`, an `ffi-struct`, or `array<τ, N>` (N > 0) of one of
// those. Its layout is C's (D-0250), so it crosses by value. `None` when
// `t` crosses; otherwise the field path to the part C could not receive
// (empty for `t` itself) and what that part is, for the message.
fn ffi_type_problem(items: &Items, t: &Type, top: bool) -> Option<(Vec<String>, String)> {
    let here = |what: String| Some((Vec::new(), what));
    match t {
        Type::Void if top => None,
        _ if is_ffi_type(t) && !matches!(t, Type::Void) => None,
        Type::Array(inner, n) if !top && *n > 0 => ffi_type_problem(items, inner, false),
        Type::Array(..) if top => here(format!("`{}`, an array, which C passes only as a pointer (`rawptr`)", t)),
        Type::Named(name, _) if items.enums.contains_key(name) => here(format!("`{}`, an enum (a C enum is an integer type here)", t)),
        Type::Named(name, targs) if items.structs.contains_key(name) => {
            let s = &items.structs[name];
            // A struct that holds a resource is one; the message names the
            // field that does, stopping at `std`'s types.
            if s.resource || (name.starts_with("std::") && is_resource_ty(items, t)) {
                return here(format!("`{}`, a resource", t));
            }
            if s.bits.is_some() {
                return here(format!("`{}`, a bitstruct (it crosses as its backing integer)", t));
            }
            if s.fields.is_empty() {
                return here(format!("`{}`, a struct with no fields", t));
            }
            let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(targs.iter().cloned()).collect();
            for f in &s.fields {
                if let Some((mut path, what)) = ffi_type_problem(items, &apply_subst(&f.ty, &subst), false) {
                    path.insert(0, f.name.clone());
                    return Some((path, what));
                }
            }
            None
        }
        _ => here(format!("`{}`", t)),
    }
}

// The module part of an item's qualified key (`modres::qualify`): one
// trailing segment for `m::f`, two for an associated `m::T::name`. For an
// item of one of `std`'s submodules that is `std`, whose submodules share
// one privacy (D-0136).
fn declaring_module(key: &str, assoc: bool) -> String {
    let mut segs: Vec<&str> = key.split("::").collect();
    segs.pop();
    if assoc {
        segs.pop();
    }
    segs.join("::")
}

// `rule.module.visibility` `[Field-Not-Visible]`: a field is visible where
// its struct's module is, or anywhere if `export`.
fn field_visible(struct_key: &str, export: bool, from_module: &str) -> bool {
    if export {
        return true;
    }
    let decl_module = struct_key.rsplit_once("::").map_or("", |(m, _)| m);
    decl_module.is_empty() || from_module == decl_module || from_module.starts_with(&format!("{}::", decl_module))
}

// `[Type-Recursive]` (D-0045): a struct or enum that contains itself by
// value, through its fields, payloads, array elements or a `mutex`, has
// no finite size. A `ref`, `rawptr`, `fn`, `handle` or `guard` holds no
// value inline, so a type reached through one (`Vec`, `Rc`, `Box` are
// built on `rawptr`) ends the search. A generic chain that only grows
// (`struct S<T> { S<array<T, 2>> s; }`) is as infinite; 64 levels deep
// counts as recursive.
fn contains_by_value(items: &Items, t: &Type, stack: &mut Vec<Type>) -> bool {
    match t {
        Type::Array(inner, _) | Type::Mutex(inner) => contains_by_value(items, inner, stack),
        Type::Named(n, args) => {
            if stack.contains(t) || stack.len() > 64 {
                return true;
            }
            let (params, parts): (Vec<String>, Vec<Type>) = if let Some(s) = items.structs.get(n) {
                (s.type_params.clone(), s.fields.iter().map(|f| f.ty.clone()).collect())
            } else if let Some(e) = items.enums.get(n) {
                (e.type_params.clone(), e.variants.iter().filter_map(|v| v.payload.clone()).collect())
            } else {
                return false;
            };
            let sub: HashMap<String, Type> = params.into_iter().zip(args.iter().cloned()).collect();
            stack.push(t.clone());
            let found = parts.iter().any(|p| contains_by_value(items, &apply_subst(p, &sub), stack));
            stack.pop();
            found
        }
        _ => false,
    }
}

// The first (by name) struct or enum that contains itself by value.
fn recursive_type(items: &Items) -> Option<(String, usize)> {
    let mut decls: Vec<(&String, Vec<String>, usize)> = items.structs.iter().map(|(k, s)| (k, s.type_params.clone(), s.line)).collect();
    decls.extend(items.enums.iter().map(|(k, e)| (k, e.type_params.clone(), e.line)));
    decls.sort_by(|a, b| a.0.cmp(b.0));
    for (k, params, line) in decls {
        let t = Type::Named(k.clone(), params.iter().map(|p| Type::Named(p.clone(), Vec::new())).collect());
        if contains_by_value(items, &t, &mut Vec::new()) {
            return Some((k.clone(), line));
        }
    }
    None
}

pub fn check_program(items: &Items) -> Result<Vec<StaticAssert>, String> {
    if let Some((name, line)) = recursive_type(items) {
        let shown = name.rsplit("::").next().unwrap_or(&name).to_string();
        let msg = format!("`{0}` contains a `{0}` by value (directly or through its fields), so it would have no finite size; hold the inner one through `Box<{0}>` (`Option<Box<{0}>>` for \"none\"), `Rc<{0}>`, or an index into a `Vec`", shown);
        return Err(format!("{}@{}", crate::modres::named("diag.recursive-type", msg), line));
    }
    // `[Const-Not-Constant]` (D-0036): a `const` is not a resource, and
    // its initializer is a constant expression. (Cycles were refused
    // by `consts`.)
    let mut consts: Vec<(&String, &std::sync::Arc<FnDecl>)> = items.fns.iter().filter(|(_, f)| f.is_const).collect();
    consts.sort_by_key(|(k, _)| k.as_str());
    for (_, f) in consts {
        let init_ok = f.body.tail.as_ref().map_or(false, |e| const_expr(items, e));
        if is_resource_ty(items, &f.ret) || !init_ok {
            return Err(format!("diag.const-not-constant@{}", f.line));
        }
    }

    // D-0258 `[Static-Well-Formed]` (`spec/17` §1a′): a `static` holds an
    // atomic (`Atomic<T>`, `T` an integer type, or `AtomicBool`) and its
    // first value is a literal of the lanes' type; each is reported where
    // it is declared.
    let mut statics: Vec<&std::sync::Arc<StaticDecl>> = items.statics.values().collect();
    statics.sort_by_key(|s| s.line);
    for s in statics {
        let lane = static_lane(&s.ty);
        let ok_init = match (&lane, &s.init.kind) {
            (Some(Type::Int(_)), ExprKind::IntLit(..)) => true,
            (Some(Type::Bool), ExprKind::BoolLit(_)) => true,
            (Some(Type::Int(_)), ExprKind::Unary(UnOp::Neg, inner)) => matches!(inner.kind, ExprKind::IntLit(..)),
            _ => false,
        };
        if lane.is_none() {
            let msg = format!("a `static` holds an atomic, `Atomic<T>` (`T` an integer type) or `AtomicBool`, which every thread may change; `{}` is not one (a constant is `const`, and any other value lives in `main` and is passed on)", s.ty);
            return Err(format!("{}@{}", crate::modres::named("diag.type-mismatch", msg), s.line));
        }
        if !ok_init {
            let msg = format!("a `static`'s first value is a literal of `{}`: `static {} {} = 0;`", lane.as_ref().unwrap(), s.ty, s.name);
            return Err(format!("{}@{}", crate::modres::named("diag.type-mismatch", msg), s.line));
        }
        if let Some(Type::Int(it)) = &lane {
            if let ExprKind::IntLit(v, _) = &s.init.kind {
                if !crate::value::in_range(*it, *v as i128) {
                    return Err(format!("{}@{}", crate::modres::named("diag.literal-out-of-range", format!("`{}` does not fit `{}`, the static's lane type", v, Type::Int(*it))), s.line));
                }
            }
        }
    }

    // Whole-program static fact, checked unconditionally for every
    // declared extern fn regardless of whether it's ever called --
    // `[Extern-Non-Ffi-Type]` is `disposition: rejected` on the
    // declaration itself, the same shape as `rule.trust.unsafe`'s own
    // ahead-of-execution facts this pass already checks per call site,
    // but this one has no call site to hang the check on if the extern
    // is simply never used.
    let mut exts: Vec<&std::sync::Arc<ExternDecl>> = items.externs.values().collect();
    exts.sort_by_key(|e| e.line);
    for ext in exts {
        let tys = ext.params.iter().map(|p| &p.ty).chain(std::iter::once(&ext.ret));
        for t in tys {
            if let Some((path, what)) = ffi_type_problem(items, t, true) {
                let part = if path.is_empty() { what } else { format!("`{}`, whose field `{}` is {}", t, path.join("."), what) };
                let msg = format!(
                    "`{}` cannot cross to C: its signature has {}; an extern's types are integers, `f32`, `f64`, `bool`, `rawptr<T>` and plain structs of those (D-0252)",
                    ext.name, part
                );
                return Err(format!("{}@{}", crate::modres::named("diag.extern-non-ffi-type", msg), ext.line));
            }
        }
    }
    // D-0257 `[Extern-Entry]`: a function C calls has FfiType parameters
    // and result, a name that is not the runtime's (`cb_…`, `cobalt_…`),
    // and no namesake among the other entry points (C has one name
    // space); each is reported where it is declared.
    let mut entries: Vec<(&String, &std::sync::Arc<FnDecl>)> = items.fns.iter().filter(|(_, f)| f.c_entry).collect();
    entries.sort_by_key(|(_, f)| f.line);
    let mut entry_names: HashMap<&str, &String> = HashMap::default();
    for (key, f) in &entries {
        let tys = f.params.iter().map(|p| &p.ty).chain(std::iter::once(&f.ret));
        for t in tys {
            if let Some((path, what)) = ffi_type_problem(items, t, true) {
                let part = if path.is_empty() { what } else { format!("`{}`, whose field `{}` is {}", t, path.join("."), what) };
                let msg = format!(
                    "`{}` cannot be called from C: its signature has {}; a function C calls takes and gives integers, `f32`, `f64`, `bool`, `rawptr<T>` and plain structs of those (D-0257)",
                    f.name, part
                );
                return Err(format!("{}@{}", crate::modres::named("diag.extern-non-ffi-type", msg), f.line));
            }
        }
        if f.name.starts_with("cb_") || f.name.starts_with("cobalt_") {
            let msg = format!("`{}` is a C name the runtime reserves (`cb_…` and `cobalt_…`); choose another for a function C calls", f.name);
            return Err(format!("{}@{}", crate::modres::named("diag.duplicate-item", msg), f.line));
        }
        if let Some(other) = entry_names.insert(f.name.as_str(), key) {
            let msg = format!("`{}` and `{}` would both be C's `{}`: two functions C calls cannot share a name", other, key, f.name);
            return Err(format!("{}@{}", crate::modres::named("diag.duplicate-item", msg), f.line));
        }
    }
    let is_library = !entries.is_empty() && !items.fns.contains_key("main");
    // `rule.fn.program` `[Program-No-Main]`: exactly one non-generic
    // `fn main()` or `fn main() : u8` at the root, with no parameters
    // (an `extern` main lives in `externs`, so it is absent here too).
    // A root `main` of another shape is reported where it is declared.
    // D-0257: a library — functions C calls and no `main` — has none, and
    // its entry points are checked as `main` would be.
    let main = match items.fns.get("main") {
        Some(m) if m.type_params.is_empty() && m.params.is_empty() && matches!(m.ret, Type::Void | Type::Int(IntTy::U8)) => Some(m.clone()),
        Some(m) => return Err(format!("diag.no-main@{}", m.line)),
        None if is_library => None,
        None => return Err("diag.no-main".to_string()),
    };
    // `diag.bad-destructor-signature` (spec/07 §1): `fn T::drop` must
    // take exactly one `ref<T, exclusive>` and return nothing.
    for (key, f) in items.fns.iter() {
        if f.name == "drop" {
            if f.assoc_type.is_some() {
                // The type's qualified key: the function's own key without
                // `::drop` (`spec/17` §1: `M::T::drop` for `T` declared in `M`).
                let t = key.strip_suffix("::drop").unwrap_or(key);
                let ok = f.params.len() == 1
                    && matches!(&f.params[0].ty, Type::Ref(inner, Mode::Exclusive) if matches!(&**inner, Type::Named(n, _) if n == t))
                    && f.ret == Type::Void;
                if !ok {
                    return Err("diag.bad-destructor-signature".to_string());
                }
                // D-0075: a destructor runs when a resource is destroyed; a
                // plain value is copied and never destroyed, so its type
                // must be a resource in every instantiation.
                let params: Vec<String> = items.structs.get(t).map(|s| s.type_params.clone()).or_else(|| items.enums.get(t).map(|e| e.type_params.clone())).unwrap_or_default();
                let plain_args: Vec<Type> = params.iter().map(|_| Type::Int(IntTy::I32)).collect();
                if !is_resource_ty(items, &Type::Named(t.to_string(), plain_args)) {
                    let shown = t.strip_prefix("std::").unwrap_or(t);
                    return Err(format!(
                        "{}@{}",
                        crate::modres::named("diag.destructor-on-plain-type", format!("`{}` has a destructor but is not a resource, so it would never run; declare `resource struct {}` (or `resource enum`)", shown, shown)),
                        f.line
                    ));
                }
            }
        }
    }
    let mut ck = Ck {
        items,
        checked: HashSet::default(),
        worklist: if main.is_some() { vec![("main".to_string(), vec![])] } else { entries.iter().map(|(k, _)| ((*k).clone(), vec![])).collect() },
        instances: 0,
        max_instances: MAX_INSTANCES,
        static_asserts: Vec::new(),
        static_assert_keys: HashSet::default(),
    };
    // `rule.fn.program`: a program is well-formed iff *every* item is
    // well-typed and every static rule is satisfied -- called or not.
    // Every non-generic function is checked as itself; every generic
    // body once with its type parameters opaque (`rule.type.kind`), in
    // addition to each concrete instantiation discovered from `main`.
    // The prelude's items are the exception (`check_all` says when not):
    // `std` is verified whole by its own suite run, and a program can only
    // observe the `std` items it reaches, so those are checked as the
    // worklist reaches them, through calls and function values. The
    // evaluators and `cobc` call a few more by name (destructors,
    // `printf`'s helpers): they ask `check_more` for any body
    // `Items::checked_fns` does not list before using it.
    let all = check_all();
    let mut keys: Vec<&String> = items.fns.keys().collect();
    keys.sort();
    // The generic bodies (type parameters opaque) are pushed last, so
    // they are checked first: what they record (D-0090's literals) is in
    // place before any instantiation of them is checked (`run` keeps that
    // order for the generics reached later too).
    let mut generic = Vec::new();
    for key in keys {
        let f = &items.fns[key];
        if !all && crate::prelude::is_prelude_line(f.line) {
            continue;
        }
        if f.type_params.is_empty() {
            ck.worklist.push((key.clone(), vec![]));
        } else {
            generic.push((key.clone(), f.type_params.iter().map(|t| Type::Named(marker_name(t), vec![])).collect()));
        }
    }
    ck.worklist.extend(generic.into_iter().rev());
    let _ = main; // looked up fresh from the worklist below
    ck.run()?;
    // [Call-Multi-Ref-Return-Rejected] (spec/10 rule.temporal.elision):
    // a function returning a reference must have exactly one reference-
    // typed parameter. A declaration-level fact, checked for every
    // declared function whether or not it is called -- after the
    // bodies, so that a body's own `[Ref-Escape-Rejected]` (which
    // spec/conformance.md's `conf.reference-escape-rejected` names for
    // the zero-parameter shape) is what gets reported for it.
    // Reported for the first such function by line, so the diagnostic
    // does not depend on the table's order.
    let mut bad: Vec<(&String, &std::sync::Arc<FnDecl>)> = items
        .fns
        .iter()
        // D-0134: `StringView::of` views a `str`, a value that never ends:
        // its view borrows nothing that could.
        .filter(|(k, f)| k.as_str() != "std::StringView::of" && is_borrow_ty(&f.ret) && f.params.iter().filter(|p| is_borrow_ty(&p.ty)).count() != 1)
        .collect();
    bad.sort_by_key(|(k, f)| (f.line, (*k).clone()));
    if let Some((key, f)) = bad.first() {
        let refs: Vec<String> = f.params.iter().filter(|p| is_borrow_ty(&p.ty)).map(|p| format!("`{}`", p.name)).collect();
        let shown = key.strip_prefix("std::").unwrap_or(key);
        let msg = if refs.is_empty() {
            format!("`{}` returns a reference but takes none, so there is nothing it could refer to; return an owned value", shown)
        } else {
            format!(
                "`{}` returns a reference but takes {} reference parameters ({}), so which one it borrows from is not known; take the others by value, split the function, or return an owned value",
                shown,
                refs.len(),
                refs.join(", ")
            )
        };
        return Err(format!("{}@{}", crate::modres::named("diag.lifetime-elision-ambiguous", msg), f.line));
    }
    Ok(ck.static_asserts)
}

// `[Generic-Instantiation-Unbounded]` (CHG-0357): how deep a type argument
// may nest, and how many concrete instantiations a program may need.
pub const INSTANCE_DEPTH: usize = 64;
pub const MAX_INSTANCES: u32 = 100_000;

// How many type constructors a type nests (`i32` 1, `Box<Vec<i32>>` 3).
pub fn type_depth(t: &Type) -> usize {
    1 + match t {
        Type::Named(_, args) => args.iter().map(type_depth).max().unwrap_or(0),
        Type::Ref(x, _) | Type::Slice(x, _) | Type::Rawptr(x) | Type::Array(x, _) | Type::Simd(x, _) | Type::Handle(x) | Type::Mutex(x) | Type::Guard(x) => type_depth(x),
        Type::Fn(ps, r) => ps.iter().map(type_depth).max().unwrap_or(0).max(type_depth(r)),
        _ => 0,
    }
}

struct Ck<'a> {
    items: &'a Items,
    checked: HashSet<(String, Vec<Type>)>,
    worklist: Vec<(String, Vec<Type>)>,
    // `[Generic-Instantiation-Unbounded]`: concrete instantiations checked
    // so far, and how many a program may need.
    instances: u32,
    max_instances: u32,
    static_asserts: Vec<StaticAssert>,
    static_assert_keys: HashSet<(usize, String)>,
}

// Whether every item is checked, `std`'s included (`COBALTC_CHECK_ALL=1`,
// the way the suites verify `std` itself), or only the program's own
// items and the `std` items they reach -- the default, since a program
// can only observe the items it reaches, and `std` grows.
fn check_all() -> bool {
    static ALL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALL.get_or_init(|| std::env::var_os("COBALTC_CHECK_ALL").map_or(false, |v| !v.is_empty() && v != "0"))
}

/// Checks one more function instantiation after `check_program`: a `std`
/// item the pass did not reach but a tool needs. The evaluators call some
/// `std` functions by name (a destructor, `printf`'s helpers, `Vec`'s
/// growth), and `cobc` lowers whatever it is asked for; each asks here
/// before running or lowering a body `Items::checked_fns` does not list.
/// What the check records joins `items`' tables. A diagnostic cannot
/// arise for a well-typed `std` (its own suite run checks it whole) and
/// is reported as the program's if it does. The D-0162 literal records
/// are not kept: the program's text is final by now.
pub fn check_more(items: &Items, name: &str, targs: &[Type]) -> Result<(), String> {
    let checked = items.checked_fns.lock().unwrap().clone();
    let mut ck = Ck { items, checked, worklist: vec![(name.to_string(), targs.to_vec())], instances: 0, max_instances: MAX_INSTANCES, static_asserts: Vec::new(), static_assert_keys: HashSet::default() };
    SILENT.with(|s| s.set(s.get() + 1));
    let r = ck.run();
    SILENT.with(|s| s.set(s.get() - 1));
    r
}

/// A generic function's type parameters as the opaque markers its body
/// is checked with (`rule.type.kind`).
pub fn opaque_args(f: &FnDecl) -> Vec<Type> {
    f.type_params.iter().map(|t| Type::Named(marker_name(t), vec![])).collect()
}

thread_local! {
    // Above zero while `check_more` runs: its literal records are dropped.
    static SILENT: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

/// A `static_assert` (D-0052) to compute once the static pass accepts the
/// program: its condition, the type arguments of the instantiation it was
/// met in, its line and its message.
pub struct StaticAssert {
    key: (usize, String),
    pub cond: Expr,
    pub subst: HashMap<String, Type>,
    pub line: usize,
    pub message: Option<String>,
    // D-0068: a message with arguments, as `$fmt(format, a…)`, computed
    // only when the assertion fails.
    pub message_fmt: Option<Expr>,
}

// Local per-function-body checking state.
struct Body<'a> {
    items: &'a Items,
    // D-0073: the expression being checked is an argument of a call whose
    // result is not a reference, so `&` of a temporary may be taken
    // there (`arg_temp_ok` is set for the argument; `cur_temp_ok` holds
    // it for that one expression, reset by every nested check).
    arg_temp_ok: bool,
    cur_temp_ok: bool,
    // The module lexically containing the function being checked
    // (`rule.module.visibility`'s M for its field clause).
    module: String,
    gamma: HashMap<String, Type>,
    // `rule.control.flow-analysis` state at the current program point.
    flow: Flow,
    // One entry per open block: the names it declared, each with the
    // facts of the same-named binding it shadowed (restored at exit).
    scopes: Vec<Vec<ScopeEntry>>,
    // rule.temporal.ref-escape (spec/10 §2): lexical block identities.
    // `block_stack` is the chain of open blocks (innermost last);
    // `block_parent` records lexical enclosure for blocks already closed.
    next_block: usize,
    block_stack: Vec<usize>,
    block_parent: HashMap<usize, usize>,
    // `decl-block(x)` for every tracked binding, and, for a reference-
    // typed binding initialized by a syntactically visible borrow (or a
    // D-0011-elided call on one), the referent block and root binding
    // that borrow named (`referent-block`); `None` = unknown.
    decl_block: HashMap<String, usize>,
    ref_origin: HashMap<String, Option<(usize, String)>>,
    params: HashSet<String>,
    // `referent-block` of every borrow / elided call already walked,
    // recorded while its referent was still in scope (a block's result
    // is stored by the enclosing statement only after the block has
    // exited). Keyed by the expression's address, stable for the
    // lifetime of the item table.
    borrow_referents: HashMap<usize, Option<(usize, String)>>,
    // True while checking an argument whose declared parameter type
    // still mentions an unresolved type parameter of the callee (or an
    // intrinsic's untyped argument): a nested generic call there has no
    // expected type *yet* (it is inferred afterwards from the other
    // arguments' shapes), so `[Generic-Call-Uninferable]` must not fire
    // for it.
    infer_poisoned: bool,
    // Checking a `foreach` expansion (src/each.rs), which reaches `std`'s
    // fields as `std` itself does.
    privileged: bool,
    // Locals initialized directly from a closure literal, and whether
    // that literal was a `move` closure (`rule.conc.spawn`).
    closure_moves: HashMap<String, bool>,
    // Locals initialized directly from a `move` closure literal whose body
    // writes a capture: a call borrows it exclusively (D-0239), which the
    // flow facts can refute (D-0245).
    closure_self_writes: HashSet<String>,
    // Locals initialized directly from a closure literal that owns no
    // resource: passed or stored by value it is copied (`[Fn-Value-Read]`),
    // and keeps its borrows (CHG-0299).
    closure_copied: HashSet<String>,
    // D-0282: reference bindings that are copies of another (`auto r2 =
    // r1;`): a copy of a reference carries the same path, so the two never
    // conflict with each other.
    ref_copies: HashMap<String, String>,
    // D-0281 step 4c: the hidden reference of `[Compound-Assign]` (`{ auto
    // t = &mut p; *t = *t op e; }`) while `e` is checked: it is live
    // through `e`, though its uses have the position of `p`.
    compound_held: Vec<String>,
    // Locals initialized directly from a `move` closure literal that captured
    // a resource: resource-typed by derivation (`rule.type.is-resource`), so
    // passed or stored by value it is consumed (CHG-0302).
    closure_owning: HashSet<String>,
    // A binding initialized by a closure literal: its parameter count
    // (`[T-Call]`). Forgotten at the end of the binding's scope.
    closure_arity: HashMap<String, usize>,
    // While checking a `move` closure's body: its captured names and
    // their (outer) types where known. Inside the body a captured name
    // is `self.f_i`, a field of the closure object -- moving a resource
    // out of it, or `drop`ping it, is `diag.move-out-of-field`
    // (`[Store-Binding-Place-Transfer-Sub]`, `[Destroy-Projection]`).
    move_captures: HashMap<String, Option<Type>>,
    // While checking any closure body: every captured name (bound
    // outside, so not unbound inside).
    capture_names: HashSet<String>,
    // The `break`/`continue` states collected for each enclosing `while`.
    loops: Vec<LoopCtx>,
    // False while a `while` body is being iterated to its fixed point:
    // flow-analysis refutations are then suppressed, since a fact that
    // is `F` on the first pass may join to `?` once the back edge is
    // taken into account. Type errors are reported regardless (they do
    // not depend on the flow state).
    report: bool,
    // Set by a parent for the one sub-expression it evaluates in *place
    // position* (spec/13 §1: the operand of `&`/`&mut`, the target of
    // `=`, the base of `.f`/`[i]`, a `match` scrutinee, `drop`'s operand,
    // a resource in a store position); consumed by `check_expr_inner`
    // immediately, so nothing deeper sees it. A place expression not in
    // place position is read (`[LValue-To-RValue]`).
    place_pos: bool,
    // D-0218: set by `*e` (and passed through parentheses) for its operand,
    // whose reference is dereferenced, not read out as a value; consumed by
    // `check_expr` immediately, as `place_pos` is.
    read_exempt: bool,
    // The binding a whole-binding assignment is about to give a value
    // (D-0033 (1)): its lookup is not `[Binding-Lookup-Stale]`.
    reinit_target: Option<String>,
    ret: Type,
    // Inside a closure's body: the closure's result type, if known, which
    // a `return` there gives (spec/15 [Closure-Call] is a call of the body).
    closure_ret: Option<Option<Type>>,
    // D-0090: in a generic body checked with its type parameters opaque,
    // each parameter's marker and its bound.
    marker_bounds: HashMap<String, Bound>,
    subst: HashMap<String, Type>,
    new_instantiations: Vec<(String, Vec<Type>)>,
    // `static_assert`s met in this body (D-0052), computed after the pass.
    static_asserts: Vec<StaticAssert>,
    // Lexical `unsafe { }` nesting depth, tracked ahead of execution --
    // `rule.trust.unsafe` `[Unsafe-Rejected]` is `disposition: rejected`
    // (a true whole-program static fact, unlike the dynamic evaluator's
    // own `unsafe_depth` counter, which only ever fires for code that
    // actually executes).
    unsafe_depth: u32,
    // Lexical `while` nesting depth -- `rule.control.while`'s
    // `disposition: rejected` `diag.break-outside-loop` fact (a `break`/
    // `continue` lexically outside any enclosing `while`), same
    // ahead-of-execution treatment as `unsafe_depth` above. Not reset
    // across a closure body boundary (`check_closure` doesn't reset it,
    // matching `unsafe_depth`'s own existing precedent there) -- a
    // narrower approximation than true lexical scoping of `break`, but
    // conservative in the safe direction (never *rejects* a program that
    // should be accepted; at worst under-rejects a `break` inside a
    // closure nested inside a `while`, which the dynamic evaluator's own
    // existing "unexpected control flow" fallback still catches as a
    // hard failure either way).
    while_depth: u32,
    // Mirrors `interp.rs`'s `Interp::current_line` exactly, for exactly
    // the same reason: `check_expr`'s own wrapper tags the innermost
    // untagged fault with `e.line` as it passes through, and this field
    // lets the few checks that construct a fault *outside* `check_expr`
    // (`check_stmt`'s own direct rejections; `Ck::run`'s return-type
    // mismatch check, after `check_block` has already returned) fall
    // back to "the last expression line this check actually looked at"
    // rather than carry no location at all.
    current_line: usize,
    current_col: usize,
}

impl<'a> Ck<'a> {
    fn run(&mut self) -> Result<(), String> {
        // Every function is checked, and of the errors found the one
        // earliest in the source is reported (one without a location,
        // last): the order the worklist reaches functions in is not one a
        // reader can see.
        let mut errors: Vec<String> = Vec::new();
        while let Some((name, targs)) = self.worklist.pop() {
            let key = (name.clone(), targs.clone());
            if self.checked.contains(&key) {
                continue;
            }
            let f = match self.items.fns.get(&name) {
                Some(f) => f.clone(),
                None => continue, // extern or unresolved; nothing to type-check
            };
            if f.type_params.len() != targs.len() {
                continue; // can't build a concrete substitution; skip (conservative)
            }
            // A generic's body is checked with its type parameters opaque
            // before any instantiation of it (what that records must be
            // in place first); a `std` generic reached here has not been.
            if !targs.is_empty() {
                let opaque: Vec<Type> = f.type_params.iter().map(|t| Type::Named(marker_name(t), vec![])).collect();
                if opaque != targs && !self.checked.contains(&(name.clone(), opaque.clone())) {
                    self.worklist.push((name.clone(), targs));
                    self.worklist.push((name, opaque));
                    continue;
                }
            }
            // `[Generic-Instantiation-Unbounded]` (spec/15 §5, CHG-0357):
            // the instantiations a program needs are finite. A type
            // argument nested deeper than INSTANCE_DEPTH, or more than
            // max_instances concrete instantiations, is a family that does
            // not end (`f<T>` calling `f<Box<T>>`); it is rejected, never
            // left partly unchecked.
            if !targs.is_empty() && !targs.iter().any(has_unresolved_marker) {
                self.instances += 1;
                let deep = targs.iter().find(|t| type_depth(t) > INSTANCE_DEPTH);
                if deep.is_some() || self.instances > self.max_instances {
                    let shown = name.strip_prefix("std::").unwrap_or(&name);
                    let why = match deep {
                        Some(t) => format!("`{}` is instantiated at a type argument nested more than {} deep (`{}…`): it calls itself, or a function that calls it, at a larger type each time, so its instantiations never end; pass the larger value as a type the function already has (a `Vec`, a `Box` of one fixed type) or split the function", shown, INSTANCE_DEPTH, t.to_string().chars().take(60).collect::<String>()),
                        None => format!("the program needs more than {} instantiations of its generic functions (`{}` among them), so they do not end", self.max_instances, shown),
                    };
                    return Err(format!("{}@{}", crate::modres::named("diag.instantiation-unbounded", why), f.line));
                }
            }
            self.checked.insert(key.clone());
            self.items.checked_fns.lock().unwrap().insert(key);
            self.items.checked_bodies.lock().unwrap().insert(std::sync::Arc::as_ptr(&f) as usize);
            let subst: HashMap<String, Type> = f.type_params.iter().cloned().zip(targs.iter().cloned()).collect();
            let mut gamma = HashMap::default();
            for p in &f.params {
                gamma.insert(p.name.clone(), apply_subst(&p.ty, &subst));
            }
            let ret = apply_subst(&f.ret, &subst);
            let mut body = Body {
                items: self.items,
                module: declaring_module(&name, f.assoc_type.is_some()),
                gamma,
                flow: Flow::default(),
                scopes: vec![Vec::new()],
                next_block: 1,
                block_stack: vec![0],
                block_parent: HashMap::default(),
                decl_block: HashMap::default(),
                ref_origin: HashMap::default(),
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                borrow_referents: HashMap::default(),
                arg_temp_ok: false,
                cur_temp_ok: false,
                infer_poisoned: false,
                privileged: false,
                closure_moves: HashMap::default(),
                closure_self_writes: HashSet::default(),
                closure_copied: HashSet::default(),
                ref_copies: HashMap::default(),
                compound_held: Vec::new(),
                closure_owning: HashSet::default(),
                closure_arity: HashMap::default(),
                move_captures: HashMap::default(),
                capture_names: HashSet::default(),
                loops: Vec::new(),
                report: true,
                place_pos: false,
                read_exempt: false,
                reinit_target: None,
                ret: ret.clone(),
                closure_ret: None,
                marker_bounds: f
                    .type_params
                    .iter()
                    .zip(f.type_bounds.iter())
                    .zip(targs.iter())
                    .filter_map(|((tp, b), t)| match (b, t) {
                        (Some(b), Type::Named(n, a)) if a.is_empty() && *n == marker_name(tp) => Some((n.clone(), *b)),
                        _ => None,
                    })
                    .collect(),
                subst,
                new_instantiations: Vec::new(),
                static_asserts: Vec::new(),
                unsafe_depth: 0,
                while_depth: 0,
                current_line: 0,
                current_col: 0,
            };
            // D-0116: a derived `T::clone` whose `T` is not `clone` (a field
            // with no clone) is never reached -- every call is rejected
            // first -- so its body, which clones that field, is not checked.
            if f.derived && body.clone_problem(&ret).is_some() {
                continue;
            }
            // Entry state (spec/14 §6): parameters `valid = T`, `init = T`.
            for p in &f.params {
                body.declare(&p.name, true);
            }
            let key = crate::tier::body_key(&name, &targs);
            tier_report::begin(key.clone(), &f.body);
            let checked = body.check_fn_body(&f.body, &ret);
            if let Err(d) = &checked {
                tier_report::with(|r| r.error = Some(d.clone()));
            }
            if let Some(why) = tier_report::end() {
                self.items.tracking_needed.lock().unwrap().entry(key).or_insert(why);
            }
            let bt = match checked {
                Ok(bt) => bt,
                Err(d) => {
                    if crate::interp::trace_on_pub() {
                        eprintln!("TRACE static reject in `{}` <{:?}>: {}", name, targs, d);
                    }
                    errors.push(d);
                    continue;
                }
            };
            if let Some(bt) = &bt {
                if !ty_compat(bt, &ret) {
                    errors.push(format!("{}{}", mismatch(format!("`{}` returns `{}`, but its body gives `{}`", name, Type::shown_apart(&ret, &bt).0, Type::shown_apart(&ret, &bt).1)), crate::diagnostics::at(body.current_line, body.current_col)));
                    continue;
                }
            }
            self.worklist.extend(body.new_instantiations);
            for a in body.static_asserts {
                if self.static_assert_keys.insert(a.key.clone()) {
                    self.static_asserts.push(a);
                }
            }
        }
        match errors.into_iter().enumerate().min_by_key(|(i, d)| {
            let (_, l, c) = crate::diagnostics::split_location_col(d);
            (l.unwrap_or(usize::MAX), c, *i)
        }) {
            Some((_, d)) => Err(d),
            None => Ok(()),
        }
    }
}

fn apply_subst(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Named(name, args) => {
            if args.is_empty() {
                if let Some(t) = subst.get(name) {
                    return t.clone();
                }
            }
            Type::Named(name.clone(), args.iter().map(|a| apply_subst(a, subst)).collect())
        }
        Type::Ref(inner, m) => Type::Ref(Box::new(apply_subst(inner, subst)), m.clone()),
        Type::Slice(inner, m) => Type::Slice(Box::new(apply_subst(inner, subst)), m.clone()),
        Type::Rawptr(inner) => Type::Rawptr(Box::new(apply_subst(inner, subst))),
        Type::Array(inner, n) => Type::Array(Box::new(apply_subst(inner, subst)), *n),
        Type::Fn(ps, r) => Type::Fn(ps.iter().map(|p| apply_subst(p, subst)).collect(), Box::new(apply_subst(r, subst))),
        Type::Handle(inner) => Type::Handle(Box::new(apply_subst(inner, subst))),
        Type::Mutex(inner) => Type::Mutex(Box::new(apply_subst(inner, subst))),
        Type::Guard(inner) => Type::Guard(Box::new(apply_subst(inner, subst))),
        other => other.clone(),
    }
}

// Whether `actual` cannot be `decl` whatever the type parameters in
// `params` become: different constructors, or different named types.
// Numbers are lenient (a literal takes its type later), and anything
// not yet resolved matches.
fn shape_conflict(decl: &Type, actual: &Type, params: &HashSet<String>) -> bool {
    let numeric = |t: &Type| matches!(t, Type::Int(_) | Type::F32 | Type::F64);
    match (decl, actual) {
        (Type::Named(p, a), _) if a.is_empty() && params.contains(p) => false,
        (_, a) if has_unresolved_marker(a) => false,
        (d, a) if numeric(d) && numeric(a) => false,
        (Type::Named(n1, a1), Type::Named(n2, a2)) => {
            n1 != n2 || a1.len() != a2.len() || a1.iter().zip(a2.iter()).any(|(x, y)| shape_conflict(x, y, params))
        }
        (Type::Ref(d, _), Type::Ref(a, _)) | (Type::Slice(d, _), Type::Slice(a, _)) | (Type::Array(d, _), Type::Array(a, _)) => shape_conflict(d, a, params),
        (Type::Rawptr(d), Type::Rawptr(a)) | (Type::Handle(d), Type::Handle(a)) | (Type::Mutex(d), Type::Mutex(a)) | (Type::Guard(d), Type::Guard(a)) => shape_conflict(d, a, params),
        (Type::Fn(dps, dr), Type::Fn(aps, ar)) => {
            dps.len() != aps.len() || dps.iter().zip(aps.iter()).any(|(x, y)| shape_conflict(x, y, params)) || shape_conflict(dr, ar, params)
        }
        (d, a) => std::mem::discriminant(d) != std::mem::discriminant(a),
    }
}

// Binds the type parameters named in `params` that `decl_ty` mentions to
// what `actual` has in their places. A bare name that is not a type
// parameter (`String`, a user struct) is a concrete type and binds
// nothing: the caller's exactness check compares it as written. (Before
// 2026-09-30 every bare name was bound, so `HashMap<String, V>` took a
// `HashMap<u64, i32>`, `String` being rewritten to `u64` — a hole.)
fn unify_type_shape(decl_ty: &Type, actual: &Type, m: &mut HashMap<String, Type>, params: &[String]) {
    match (decl_ty, actual) {
        (Type::Named(n, args), _) if args.is_empty() && params.iter().any(|p| p == n) => {
            m.entry(n.clone()).or_insert_with(|| actual.clone());
        }
        (Type::Ref(d, _), Type::Ref(a, _)) | (Type::Slice(d, _), Type::Slice(a, _)) => unify_type_shape(d, a, m, params),
        (Type::Rawptr(d), Type::Rawptr(a)) => unify_type_shape(d, a, m, params),
        (Type::Array(d, _), Type::Array(a, _)) => unify_type_shape(d, a, m, params),
        (Type::Handle(d), Type::Handle(a)) => unify_type_shape(d, a, m, params),
        (Type::Mutex(d), Type::Mutex(a)) => unify_type_shape(d, a, m, params),
        (Type::Guard(d), Type::Guard(a)) => unify_type_shape(d, a, m, params),
        (Type::Fn(dps, dr), Type::Fn(aps, ar)) if dps.len() == aps.len() => {
            for (dp, ap) in dps.iter().zip(aps.iter()) {
                unify_type_shape(dp, ap, m, params);
            }
            unify_type_shape(dr, ar, m, params);
        }
        (Type::Named(_, dargs), Type::Named(_, aargs)) => {
            for (dp, ap) in dargs.iter().zip(aargs.iter()) {
                unify_type_shape(dp, ap, m, params);
            }
        }
        _ => {}
    }
}

// Structural type equality for the purpose of a mismatch diagnostic.
// Exact per D-0006 (nominal identity, no implicit conversion) -- e.g.
// `i32` and `i64` are never compatible, matching the gap
// impl/STATUS.md's dynamic-only evaluator documented (it compares raw
// integer payloads, ignoring the declared width/signedness tag).
// `[T-Never]` (spec/12 sec 5): "an expression of type never is accepted
// at any expected type" -- symmetric, since every call site here
// compares two things that could each be the never-typed side (two
// branches of an if/match, a function body's inferred type against its
// declared return type, ...). Confirmed missing by a real reproduction:
// `fn g() : i64 { return 5000000000; }` was rejected `diag.type-mismatch`
// (the block's own type, Never, compared unequal to i64) purely because
// this was a bare `a == b`, before this fix.
fn ty_compat(a: &Type, b: &Type) -> bool {
    a == b || matches!(a, Type::Never) || matches!(b, Type::Never)
}

// ---------------------------------------------------------------------
// rule.control.flow-analysis (spec/14 §6)
// ---------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tri {
    T,
    F,
    U,
}

// D-0282 (`rule.control.frame-references`): the state the rules for
// frame-held references keep per function body (its uses, for liveness by
// last use), and the report D-0281's measurements read: with
// COBALTC_TIER_REPORT=FILE the static pass appends to FILE, for each body,
// the conflict checks it made, those it could decide neither way, the
// point checks left to the run, and the reference bindings it gave no
// origin. The report changes nothing.
mod tier_report {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::io::Write;

    // A binding as liveness sees it: its name, where it is declared and
    // where its scope ends (D-0111's last use, by lexical scope).
    pub struct Decl {
        pub name: String,
        pub pos: (usize, usize),
        pub end: (usize, usize),
    }

    // A use of a name, by position, and the declaration it resolves to
    // (`PARAM` for a parameter or anything declared outside the body).
    pub const PARAM: usize = usize::MAX;

    #[derive(Default)]
    pub struct Rep {
        pub uses: Vec<(String, usize, usize, usize)>,
        pub decls: Vec<Decl>,
        // The extents of the body's loops (first and last line).
        pub loops: Vec<(usize, usize)>,
        pub func: String,
        pub checks: usize,
        pub undecided: usize,
        pub unknown_origin: usize,
        pub inherited: usize,
        pub tier_d: usize,
        pub point_checks: usize,
        // D-0281 step 4c: the first conflict left to the run's tracking.
        pub left: Option<String>,
        pub error: Option<String>,
        pub events: Vec<String>,
    }

    thread_local! {
        pub static CUR: RefCell<Option<Rep>> = RefCell::new(None);
    }

    pub fn kind_of(e: &crate::ast::Expr) -> String {
        use crate::ast::ExprKind as K;
        match &e.kind {
            K::Paren(i) => kind_of(i),
            K::Call(c, _) => match &c.kind {
                K::Path(segs, _) => format!("call {}", segs.join("::")),
                _ => "call (value)".to_string(),
            },
            K::Path(segs, _) => format!("name {}", segs.join("::")),
            K::Field(_, f) => format!("field .{}", f),
            K::Index(..) => "index".to_string(),
            K::Deref(_) => "deref".to_string(),
            K::Borrow(..) => "borrow".to_string(),
            K::SliceOf(..) => "slice-of".to_string(),
            K::If(..) => "if".to_string(),
            K::Match(..) => "match".to_string(),
            K::Block(_) => "block".to_string(),
            K::StrLit(_) => "literal".to_string(),
            _ => "other".to_string(),
        }
    }

    // The rules are always on (D-0282); the report is written only when
    // COBALTC_TIER_REPORT names a file.
    pub fn on() -> bool {
        true
    }

    pub fn begin(func: String, body: &crate::ast::Block) {
        let mut w = Walker { rep: Rep { func, ..Rep::default() }, visible: HashMap::new() };
        w.block(body);
        CUR.with(|c| *c.borrow_mut() = Some(w.rep));
    }

    // Whether the binding `name` visible at (line, col) is used after that
    // point, or anywhere in a loop that contains the point and that its
    // declaration precedes (a binding declared in a loop's body is a new
    // binding each time round).
    pub fn live_after(name: &str, line: usize, col: usize) -> bool {
        CUR.with(|c| {
            let c = c.borrow();
            let Some(r) = c.as_ref() else { return true };
            let at = (line, col);
            let decl = r
                .decls
                .iter()
                .enumerate()
                .filter(|(_, d)| d.name == name && d.pos <= at && at <= d.end)
                .max_by_key(|(_, d)| d.pos)
                .map(|(i, _)| i)
                .unwrap_or(PARAM);
            let decl_line = if decl == PARAM { 0 } else { r.decls[decl].pos.0 };
            let in_loops: Vec<&(usize, usize)> = r.loops.iter().filter(|(a, b)| *a <= line && line <= *b && decl_line < *a).collect();
            r.uses.iter().any(|(n, l, k, d)| *d == decl && n == name && ((*l, *k) > at || in_loops.iter().any(|(a, b)| a <= l && l <= b)))
        })
    }

    // The last position inside an expression or a block: a scope's end.
    fn end_e(e: &crate::ast::Expr, m: &mut (usize, usize)) {
        let mut w = Walker { rep: Rep::default(), visible: HashMap::new() };
        w.expr(e);
        *m = (*m).max((e.line, e.col));
        for (_, l, c, _) in &w.rep.uses {
            *m = (*m).max((*l, *c));
        }
        for (_, b) in &w.rep.loops {
            *m = (*m).max((*b, usize::MAX));
        }
        for d in &w.rep.decls {
            *m = (*m).max(d.pos);
        }
    }

    fn end_b(b: &crate::ast::Block) -> (usize, usize) {
        use crate::ast::Stmt;
        let mut m = (0, 0);
        for s in &b.stmts {
            match s {
                Stmt::Let { init, line, col, .. } => {
                    m = m.max((*line, *col));
                    if let Some(e) = init {
                        end_e(e, &mut m);
                    }
                }
                Stmt::Destructure { init, .. } => end_e(init, &mut m),
                Stmt::Expr(e) | Stmt::BlockLike(e) => end_e(e, &mut m),
            }
        }
        if let Some(t) = &b.tail {
            end_e(t, &mut m);
        }
        m
    }

    struct Walker {
        rep: Rep,
        // The declarations in scope, innermost last, by name.
        visible: HashMap<String, Vec<usize>>,
    }

    impl Walker {
        fn declare(&mut self, name: &str, pos: (usize, usize), end: (usize, usize), scope: &mut Vec<String>) {
            self.rep.decls.push(Decl { name: name.to_string(), pos, end });
            self.visible.entry(name.to_string()).or_default().push(self.rep.decls.len() - 1);
            scope.push(name.to_string());
        }

        fn close(&mut self, scope: Vec<String>) {
            for n in scope {
                if let Some(v) = self.visible.get_mut(&n) {
                    v.pop();
                }
            }
        }

        fn use_name(&mut self, name: &str, line: usize, col: usize) {
            let d = self.visible.get(name).and_then(|v| v.last().copied()).unwrap_or(PARAM);
            self.rep.uses.push((name.to_string(), line, col, d));
        }

        fn block(&mut self, b: &crate::ast::Block) {
            use crate::ast::Stmt;
            let end = end_b(b);
            let mut scope = Vec::new();
            for s in &b.stmts {
                match s {
                    Stmt::Let { init, name, line, col, .. } => {
                        if let Some(e) = init {
                            self.expr(e);
                        }
                        self.declare(name, (*line, *col), end, &mut scope);
                    }
                    Stmt::Destructure { init, names, .. } => {
                        self.expr(init);
                        for n in names {
                            self.declare(n, (init.line, init.col), end, &mut scope);
                        }
                    }
                    Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e),
                }
            }
            if let Some(t) = &b.tail {
                self.expr(t);
            }
            self.close(scope);
        }

        fn expr(&mut self, e: &crate::ast::Expr) {
            use crate::ast::ExprKind as K;
            match &e.kind {
                K::Path(segs, _) if segs.len() == 1 => self.use_name(&segs[0], e.line, e.col),
                K::Paren(x) | K::Unary(_, x) | K::Propagate(x) | K::Deref(x) | K::Borrow(_, x) | K::Field(x, _) | K::ArrayRepeat(x, _) => self.expr(x),
                K::Binary(_, a, b) | K::Index(a, b) | K::Assign(a, b) => {
                    self.expr(a);
                    self.expr(b);
                }
                K::SliceOf(_, a, b, c) => {
                    self.expr(a);
                    self.expr(b);
                    self.expr(c);
                }
                K::Call(c, xs) => {
                    self.expr(c);
                    for x in xs {
                        self.expr(x);
                    }
                }
                K::StructLit(_, _, fs) => {
                    for (_, x) in fs {
                        self.expr(x);
                    }
                }
                K::ArrayLit(xs) => {
                    for x in xs {
                        self.expr(x);
                    }
                }
                K::Block(b) | K::Unsafe(b) => self.block(b),
                K::If(c, t, x) => {
                    self.expr(c);
                    self.block(t);
                    if let Some(x) = x {
                        self.expr(x);
                    }
                }
                K::While(c, b, x) => {
                    let mut end = (e.line, e.col);
                    end_e(c, &mut end);
                    end = end.max(end_b(b));
                    if let Some(x) = x {
                        end_e(x, &mut end);
                    }
                    self.rep.loops.push((e.line, end.0));
                    self.expr(c);
                    self.block(b);
                    if let Some(x) = x {
                        self.expr(x);
                    }
                }
                K::Match(c, arms) => {
                    self.expr(c);
                    for a in arms {
                        // A binder is declared for its arm's body.
                        let mut scope = Vec::new();
                        if let Some(b) = &a.binder {
                            let mut end = (a.body.line, a.body.col);
                            end_e(&a.body, &mut end);
                            self.declare(b, (a.body.line, 0), end, &mut scope);
                        }
                        self.expr(&a.body);
                        self.close(scope);
                    }
                }
                K::Return(Some(x)) => self.expr(x),
                K::Closure { captures, body, .. } => {
                    for c in captures {
                        self.use_name(c, e.line, e.col);
                    }
                    self.block(body);
                }
                _ => {}
            }
        }
    }

    pub fn with(f: impl FnOnce(&mut Rep)) {
        CUR.with(|c| {
            if let Some(r) = c.borrow_mut().as_mut() {
                f(r)
            }
        });
    }

    // A conflict this body leaves to the run's tracking, `why` (the first
    // is kept).
    pub fn left(why: String) {
        with(|r| {
            if r.left.is_none() {
                r.left = Some(why);
            }
        });
    }

    // The body's end: the report written, and what it left to the run.
    pub fn end() -> Option<String> {
        let r = CUR.with(|c| c.borrow_mut().take())?;
        let left = r.left.clone();
        let Some(path) = std::env::var_os("COBALTC_TIER_REPORT").filter(|v| !v.is_empty()) else { return left };
        let mut out = format!("F\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.func, r.checks, r.undecided, r.unknown_origin, r.inherited, r.tier_d, r.point_checks);
        if let Some(e) = &r.error {
            out.push_str(&format!("E\t{}\n", e.replace('\n', " ")));
        }
        for e in &r.events {
            out.push_str(e);
            out.push('\n');
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(out.as_bytes());
        }
        left
    }
}

fn tri_join(a: Tri, b: Tri) -> Tri {
    if a == b {
        a
    } else {
        Tri::U
    }
}

// One element of a syntactic projection path `π`: a field name, or an
// index (its literal value, or `None` for a non-literal index, which
// overlaps every index).
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
enum PElem {
    Field(String),
    Index(Option<u128>),
    // D-0070: the elements a slice borrows, `lo .. hi` when both bounds
    // are literals.
    Range(Option<(u128, u128)>),
}

// A slice's range as a path step: its bounds when both are literals.
fn range_elem(lo: &Expr, hi: &Expr) -> PElem {
    match (literal_value(lo), literal_value(hi)) {
        (Some(a), Some(b)) if a >= 0 && b >= a => PElem::Range(Some((a as u128, b as u128))),
        _ => PElem::Range(None),
    }
}

// D-0282: overlap for the rules of frame-held references. `Yes` when the two paths certainly
// meet (equal fields and literal indices, one a prefix of the other, every
// element in the longer one's remainder present); `Maybe` when they meet
// only for some values (an index or a range bound that is not a literal, a
// range in the remainder that may be empty): a point check at run time;
// `No` when they cannot meet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ov {
    No,
    Maybe,
    Yes,
}

// Two place (or borrow) expressions that name the same place, written the
// same way: a name, fields, literal or same-name indices, dereferences.
fn same_place(a: &Expr, b: &Expr) -> bool {
    match (&strip_parens_tc(a).kind, &strip_parens_tc(b).kind) {
        (ExprKind::Path(x, _), ExprKind::Path(y, _)) => x == y,
        (ExprKind::Borrow(m, x), ExprKind::Borrow(n, y)) => m == n && same_place(x, y),
        (ExprKind::Field(x, f), ExprKind::Field(y, g)) => f == g && same_place(x, y),
        (ExprKind::Deref(x), ExprKind::Deref(y)) => same_place(x, y),
        (ExprKind::Index(x, i), ExprKind::Index(y, j)) => {
            same_place(x, y)
                && match (&strip_parens_tc(i).kind, &strip_parens_tc(j).kind) {
                    (ExprKind::IntLit(u, _), ExprKind::IntLit(v, _)) => u == v,
                    (ExprKind::Path(p, _), ExprKind::Path(q, _)) => p == q,
                    _ => false,
                }
        }
        _ => false,
    }
}

fn overlap3(a: &[PElem], b: &[PElem]) -> Ov {
    let mut ov = Ov::Yes;
    for (x, y) in a.iter().zip(b.iter()) {
        match (x, y) {
            (PElem::Field(f), PElem::Field(g)) => {
                if f != g {
                    return Ov::No;
                }
            }
            (PElem::Index(Some(i)), PElem::Index(Some(j))) => {
                if i != j {
                    return Ov::No;
                }
            }
            (PElem::Index(_), PElem::Index(_)) => ov = Ov::Maybe,
            (PElem::Range(Some((l, h))), PElem::Index(Some(i))) | (PElem::Index(Some(i)), PElem::Range(Some((l, h)))) => {
                if !(l <= i && i < h) {
                    return Ov::No;
                }
            }
            (PElem::Range(Some((l1, h1))), PElem::Range(Some((l2, h2)))) => {
                if l1.max(l2) >= h1.min(h2) {
                    return Ov::No;
                }
            }
            (PElem::Range(_), _) | (_, PElem::Range(_)) | (PElem::Index(_), _) | (_, PElem::Index(_)) => ov = Ov::Maybe,
        }
    }
    // The longer path's remainder: a literal empty range touches nothing;
    // any other range may be empty.
    let rest = if a.len() > b.len() { &a[b.len()..] } else { &b[a.len()..] };
    for e in rest {
        match e {
            // An empty range still meets its container's fields
            // (`conf.empty-slice-meets-vec-fields`) but not always its
            // elements: the run decides.
            PElem::Range(Some((l, h))) if l >= h => ov = Ov::Maybe,
            PElem::Range(Some(_)) => {}
            PElem::Range(None) => ov = Ov::Maybe,
            _ => {}
        }
    }
    ov
}

// "two paths π1, π2 overlap iff one is a prefix of the other, with
// literal indices compared by value and a non-literal index overlapping
// every index"
fn paths_overlap(a: &[PElem], b: &[PElem]) -> bool {
    for (x, y) in a.iter().zip(b.iter()) {
        match (x, y) {
            (PElem::Field(f), PElem::Field(g)) => {
                if f != g {
                    return false;
                }
            }
            (PElem::Index(Some(i)), PElem::Index(Some(j))) => {
                if i != j {
                    return false;
                }
            }
            (PElem::Index(_), PElem::Index(_)) => {}
            // D-0070: a range meets a literal index inside it or a literal
            // range it intersects; one with a bound that is not a literal
            // is left to the dynamic check (`spec/14`).
            (PElem::Range(Some((l, h))), PElem::Index(Some(i))) | (PElem::Index(Some(i)), PElem::Range(Some((l, h)))) => {
                if !(l <= i && i < h) {
                    return false;
                }
            }
            (PElem::Range(Some((l1, h1))), PElem::Range(Some((l2, h2)))) => {
                if l1.max(l2) >= h1.min(h2) {
                    return false;
                }
            }
            (PElem::Range(_), PElem::Index(_) | PElem::Range(_)) | (PElem::Index(_), PElem::Range(_)) => return false,
            _ => return false,
        }
    }
    true
}

// D-0071: the referent of a reference or guard binding `r` is a root
// of its own, `*r`, so a reborrow `&*r` / `&r.f` is a `deriv` fact
// against it. The name cannot collide with a binding's.
fn referent_root(r: &str) -> String {
    format!("*{r}")
}

// The binding a root belongs to: `r` for `*r`.
fn root_binding(root: &str) -> &str {
    root.strip_prefix('*').unwrap_or(root)
}

// `deriv(r, x.π, m)`: the reference binding `r` currently holds a
// reference derived from `x`'s object at projection path `π` with mode
// `m`, formed by a visible borrow of that place or a D-0011-elided call
// on such. Keyed per `r`; an absent key is `F`.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
struct DerivKey {
    root: String,
    path: Vec<PElem>,
    mode: Mode,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Flow {
    // The program point is not reachable (after `return`/`break`/
    // `continue`): no fact is asserted, so nothing is refuted, and the
    // state is the identity of `join`.
    unreachable: bool,
    // Only bindings declared in this body (parameters included) are
    // tracked; a name absent here (a closure capture, an item) has no
    // fact and is never checked.
    valid: HashMap<String, Tri>,
    init: HashMap<String, Tri>,
    deriv: HashMap<String, HashMap<DerivKey, Tri>>,
    // spec/14's value-range row: a binding "bound by a local declaration
    // to a literal with no intervening write" -- present only while that
    // holds (a write, any borrow, a capture, or the block's end removes
    // it; a merge keeps it only where every side agrees).
    lits: HashMap<String, i128>,
}

// A shadowed binding's facts, kept aside while the shadowing block is
// open and restored at its exit (facts belong to bindings, not names).
#[derive(Clone, Debug)]
struct SavedFacts {
    valid: Tri,
    init: Tri,
    own_deriv: Option<HashMap<DerivKey, Tri>>,
    rooted: Vec<(String, DerivKey, Tri)>,
    lit: Option<i128>,
}

#[derive(Default)]
struct LoopCtx {
    breaks: Vec<Flow>,
    continues: Vec<Flow>,
}

struct ScopeEntry {
    name: String,
    flow: Option<SavedFacts>,
    decl_block: Option<usize>,
    ref_origin: Option<Option<(usize, String)>>,
}

impl Flow {
    fn is_tracked(&self, name: &str) -> bool {
        self.valid.contains_key(name)
    }

    // `τ x = e;` / `auto x = e;` / `τ x;` / a parameter / a `match`
    // binder / a destructured field: `valid(x) := T`, `init(x) := T`
    // (with an initializer) or `F` (without); any `deriv` fact about a
    // previous binding of the same name is dropped.
    fn declare(&mut self, name: &str, initialized: bool) {
        self.lits.remove(name);
        self.valid.insert(name.to_string(), Tri::T);
        self.init.insert(name.to_string(), if initialized { Tri::T } else { Tri::F });
        self.deriv.remove(name);
        self.clear_rooted(name);
        self.clear_rooted(&referent_root(name));
    }

    // Every `deriv(·, x.·, ·) := F`.
    fn clear_rooted(&mut self, x: &str) {
        for facts in self.deriv.values_mut() {
            facts.retain(|k, _| k.root != x);
        }
        self.deriv.retain(|_, f| !f.is_empty());
    }

    // Block exit for a binding declared in the block: `valid(x) := F`,
    // `deriv(x, ·, ·) := F`; since the name can no longer denote this
    // binding, it is simply untracked from here on. Facts rooted at it
    // are dropped too: a reference to it that survives the block is
    // `rule.temporal.ref-escape`'s concern, and a same-named outer
    // binding must not inherit them.
    fn remove(&mut self, name: &str) {
        self.lits.remove(name);
        self.valid.remove(name);
        self.init.remove(name);
        self.deriv.remove(name);
        self.clear_rooted(name);
        self.clear_rooted(&referent_root(name));
    }

    fn save(&self, name: &str) -> SavedFacts {
        let mut rooted = Vec::new();
        for (r, facts) in &self.deriv {
            for (k, v) in facts {
                if k.root == name {
                    rooted.push((r.clone(), k.clone(), *v));
                }
            }
        }
        SavedFacts {
            valid: *self.valid.get(name).unwrap_or(&Tri::U),
            init: *self.init.get(name).unwrap_or(&Tri::U),
            own_deriv: self.deriv.get(name).cloned(),
            rooted,
            lit: self.lits.get(name).copied(),
        }
    }

    fn restore(&mut self, name: &str, s: SavedFacts) {
        self.valid.insert(name.to_string(), s.valid);
        self.init.insert(name.to_string(), s.init);
        if let Some(d) = s.own_deriv {
            self.deriv.insert(name.to_string(), d);
        }
        for (r, k, v) in s.rooted {
            self.deriv.entry(r).or_default().insert(k, v);
        }
        if let Some(v) = s.lit {
            self.lits.insert(name.to_string(), v);
        }
    }

    // Pointwise join at a merge point; an unreachable side is the
    // identity. A name tracked on only one side is dropped: it can only
    // be a binding whose block has already ended on the other side.
    fn join(&self, other: &Flow) -> Flow {
        if self.unreachable {
            return other.clone();
        }
        if other.unreachable {
            return self.clone();
        }
        let mut out = Flow::default();
        for (n, v) in &self.lits {
            if other.lits.get(n) == Some(v) {
                out.lits.insert(n.clone(), *v);
            }
        }
        for (n, a) in &self.valid {
            if let Some(b) = other.valid.get(n) {
                out.valid.insert(n.clone(), tri_join(*a, *b));
                let ia = self.init.get(n).copied().unwrap_or(Tri::U);
                let ib = other.init.get(n).copied().unwrap_or(Tri::U);
                out.init.insert(n.clone(), tri_join(ia, ib));
            }
        }
        let mut rs: HashSet<&String> = self.deriv.keys().collect();
        rs.extend(other.deriv.keys());
        for r in rs {
            if !out.valid.contains_key(r) {
                continue;
            }
            let empty = HashMap::default();
            let fa = self.deriv.get(r).unwrap_or(&empty);
            let fb = other.deriv.get(r).unwrap_or(&empty);
            let mut keys: HashSet<&DerivKey> = fa.keys().collect();
            keys.extend(fb.keys());
            let mut merged = HashMap::default();
            for k in keys {
                if !out.valid.contains_key(root_binding(&k.root)) {
                    continue;
                }
                let v = tri_join(fa.get(k).copied().unwrap_or(Tri::F), fb.get(k).copied().unwrap_or(Tri::F));
                if v != Tri::F {
                    merged.insert(k.clone(), v);
                }
            }
            if !merged.is_empty() {
                out.deriv.insert(r.clone(), merged);
            }
        }
        out
    }
}

// `is-resource` (spec/12 §3) from the item table alone; a bare type
// parameter or an unknown name is treated as plain (conservative: it
// yields no consumption fact, hence no refutation).
// Whether a value of `ty` is or holds a function value (`[Eq-Fn]`):
// through struct fields, enum payloads and array elements, not through a
// reference (`==` on a reference is its identity, `[Eq-Ref]`).
fn contains_fn(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Fn(..) | Type::Closure(_) => true,
        Type::Array(inner, _) => contains_fn(items, inner),
        Type::Named(name, args) => {
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| contains_fn(items, &apply_subst(&f.ty, &subst)))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| contains_fn(items, &apply_subst(p, &subst))))
            } else {
                false
            }
        }
        _ => false,
    }
}

// Whether `[Eq-Struct]`'s field-by-field comparison of a `ty` would
// reach a slice, which has no equality (D-0047): a `StringView` inside an
// aggregate is one (its text equality, D-0053, is the comparison of two
// views, not of what holds them).
fn contains_slice(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Slice(..) => true,
        Type::Array(inner, _) => contains_slice(items, inner),
        Type::Named(name, args) => {
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| contains_slice(items, &apply_subst(&f.ty, &subst)))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| contains_slice(items, &apply_subst(p, &subst))))
            } else {
                false
            }
        }
        _ => false,
    }
}

// A type-mismatch with what was expected and found, for the reader
// (D-0072).
fn fn_shown(f: &FnDecl) -> String {
    let n = match &f.assoc_type {
        Some(a) => format!("{}::{}", a, f.name),
        None => f.name.clone(),
    };
    n.strip_prefix("std::").unwrap_or(&n).to_string()
}

fn mismatch(msg: String) -> String {
    format!("diag.type-mismatch{}{}", crate::diagnostics::DETAIL_SEP, msg)
}

// Whether a value of `ty` holds a reference or slice, directly or in a
// field, element or payload.
fn holds_ref(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Ref(..) | Type::Slice(..) => true,
        Type::Array(inner, _) => holds_ref(items, inner),
        Type::Named(name, args) => {
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| holds_ref(items, &apply_subst(&f.ty, &subst)))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| holds_ref(items, &apply_subst(p, &subst))))
            } else {
                false
            }
        }
        _ => false,
    }
}

// D-0281 step 4c: whether a value of type `ty` keeps a reference in data
// -- a field, an element, a payload, a container's cells -- rather than
// being one itself as a frame holds it (a reference, a slice, a view, an
// `Option` or `Result` of one, a `fn` value): tier D, whose conflicts
// only the run's tracking finds. A `fn` value in data counts when
// `fn_refs`: some closure of the program keeps a reference in its
// environment.
pub fn keeps_refs_in_data(items: &Items, ty: &Type, fn_refs: bool) -> bool {
    fn keeps(items: &Items, t: &Type, fn_refs: bool, seen: &mut HashSet<Type>) -> bool {
        match t {
            Type::Ref(..) | Type::Slice(..) | Type::Guard(..) | Type::Handle(..) => true,
            Type::Closure(_) | Type::Fn(..) => fn_refs,
            Type::Array(inner, _) | Type::Rawptr(inner) | Type::Mutex(inner) => keeps(items, inner, fn_refs, seen),
            Type::Named(name, args) => {
                if is_borrow_ty(t) {
                    return true;
                }
                if !seen.insert(t.clone()) {
                    return false;
                }
                if args.iter().any(|a| keeps(items, a, fn_refs, seen)) {
                    return true;
                }
                if let Some(s) = items.structs.get(name) {
                    let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    s.fields.iter().any(|f| keeps(items, &apply_subst(&f.ty, &subst), fn_refs, seen))
                } else if let Some(e) = items.enums.get(name) {
                    let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| keeps(items, &apply_subst(p, &subst), fn_refs, seen)))
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    let frame_held = match ty {
        Type::Ref(..) | Type::Slice(..) | Type::Closure(_) | Type::Fn(..) => true,
        // D-0287: `foreach`'s holder of a view's parts, as a view.
        Type::Named(n, a) if n == "std::StringView" || n == "std::Parts" => a.is_empty(),
        Type::Named(n, a) if n == "std::Option" || n == "std::Result" => a.first().map_or(false, |x| matches!(x, Type::Ref(..) | Type::Slice(..)) || matches!(x, Type::Named(m, _) if m == "std::StringView")) && a.iter().skip(1).all(|x| !keeps(items, x, fn_refs, &mut HashSet::default())),
        _ => false,
    };
    if frame_held {
        return false;
    }
    keeps(items, ty, fn_refs, &mut HashSet::default())
}

// D-0282: whether storage of type `ty` keeps references: in itself, or
// as a container's elements (`Vec<ref<T>>`, whose cells sit behind a raw
// pointer `holds_ref` does not follow).
fn stores_refs(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Named(_, args) => holds_ref(items, ty) || args.iter().any(|a| stores_refs(items, a)),
        Type::Array(inner, _) => stores_refs(items, inner),
        t => is_borrow_ty(t) || holds_ref(items, t),
    }
}

// D-0218: whether a value of `ty` holds an exclusive reference or slice in
// itself (a field, an element of an array, a payload); what a reference or
// raw pointer reaches is not part of the value.
fn holds_excl_ref(items: &Items, ty: &Type, depth: u32) -> bool {
    if depth > 32 {
        return false;
    }
    match ty {
        Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive) => true,
        Type::Array(inner, _) => holds_excl_ref(items, inner, depth + 1),
        Type::Named(name, args) => {
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| holds_excl_ref(items, &apply_subst(&f.ty, &subst), depth + 1))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| holds_excl_ref(items, &apply_subst(p, &subst), depth + 1)))
            } else {
                false
            }
        }
        _ => false,
    }
}

// D-0111: whether a value of `t` may hold a reference anywhere: in a
// field, a payload, an element behind a raw pointer (`Vec<StringView>`),
// a type argument, a closure's captures.
// D-0111, D-0133 `[Ref-Binding-Last-Use]`: a local of this type may end
// after its last use -- a reference, slice or `StringView`, or any type
// that holds a reference and owns no resource and no `fn` value (so its
// early end destroys nothing; it only stops holding its paths).
pub fn ends_at_last_use(items: &Items, t: &Type) -> bool {
    matches!(t, Type::Ref(..) | Type::Slice(..))
        || matches!(t, Type::Named(n, _) if n == "std::StringView")
        || (!matches!(t, Type::Rawptr(_) | Type::Fn(..) | Type::Closure(_))
            && may_hold_ref(items, t, 0)
            && !is_resource_ty(items, t)
            && !contains_fn(items, t))
}

// D-0204: whether a value of `t` may hold an exclusive reference (or
// slice) through which its holder could change data that is not its own:
// in a field, a payload, an element, a type argument or a closure's
// captures (any closure counts). A thread that locks a mutex of such an
// interior -- a channel of exclusive references among them -- is lent
// another's data, as one given such a value at `spawn` is.
pub fn may_hold_excl_ref(items: &Items, t: &Type) -> bool {
    excl_ref_in(items, t, 0)
}

fn excl_ref_in(items: &Items, t: &Type, depth: usize) -> bool {
    if depth > 16 {
        return true;
    }
    match t {
        Type::Ref(_, m) | Type::Slice(_, m) => *m == Mode::Exclusive,
        Type::Closure(_) => true,
        Type::Array(inner, _) | Type::Rawptr(inner) | Type::Handle(inner) | Type::Mutex(inner) | Type::Guard(inner) => excl_ref_in(items, inner, depth + 1),
        Type::Named(name, args) => {
            if args.iter().any(|a| excl_ref_in(items, a, depth + 1)) {
                return true;
            }
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| excl_ref_in(items, &apply_subst(&f.ty, &subst), depth + 1))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| excl_ref_in(items, &apply_subst(p, &subst), depth + 1)))
            } else {
                false
            }
        }
        _ => false,
    }
}

fn may_hold_ref(items: &Items, t: &Type, depth: usize) -> bool {
    if depth > 16 {
        return true;
    }
    match t {
        Type::Ref(..) | Type::Slice(..) | Type::Fn(..) | Type::Closure(_) => true,
        Type::Array(inner, _) | Type::Rawptr(inner) | Type::Handle(inner) | Type::Mutex(inner) | Type::Guard(inner) => may_hold_ref(items, inner, depth + 1),
        Type::Named(..) if holds_ref(items, t) => true,
        Type::Named(_, args) => args.iter().any(|a| may_hold_ref(items, a, depth + 1)),
        _ => false,
    }
}

// D-0109: shared if either is.
fn weaker_mode(a: Option<&Mode>, b: &Mode) -> Mode {
    if a == Some(&Mode::Shared) || *b == Mode::Shared { Mode::Shared } else { Mode::Exclusive }
}

fn is_std_string(t: &Type) -> bool {
    matches!(t, Type::Named(n, a) if n == "std::String" && a.is_empty())
}

fn is_resource_ty(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Handle(_) | Type::Mutex(_) | Type::Guard(_) => true,
        Type::Array(inner, _) => is_resource_ty(items, inner),
        Type::Named(name, args) => {
            if let Some(s) = items.structs.get(name) {
                if s.resource {
                    return true;
                }
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| is_resource_ty(items, &apply_subst(&f.ty, &subst)))
            } else if let Some(e) = items.enums.get(name) {
                if e.resource {
                    return true;
                }
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                e.variants
                    .iter()
                    .any(|v| v.payload.as_ref().map(|p| is_resource_ty(items, &apply_subst(p, &subst))).unwrap_or(false))
            } else {
                false
            }
        }
        _ => false,
    }
}

// D-0194: whether destroying any value of `ty` only frees memory: no
// destructor of the program's, nothing released outside the program (a
// file, a socket, a process, a thread, a lock), no `fn` value (whose
// closure may own anything), no reference. Plain data, `std`'s memory
// types (`String`, `Vec`, `HashMap`, `HashSet`, `Queue`, `Box`, `Json`)
// over quiet types, and structs, enums and arrays made of quiet types,
// none of them declared `resource` or given a destructor of their own.
// A live value of a quiet type may be written over: `x = v` destroys it
// (`[Write-Quiet-Replace]`, spec/05), as `overwrite` would.
pub fn quiet_ty(items: &Items, ty: &Type) -> bool {
    fn go(items: &Items, ty: &Type, seen: &mut Vec<String>) -> bool {
        match ty {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Rawptr(_) => true,
            Type::Array(inner, _) => go(items, inner, seen),
            Type::Named(name, args) => {
                const MEMORY: &[&str] = &["std::String", "std::Vec", "std::HashMap", "std::HashSet", "std::Queue", "std::Box", "std::Json", "std::JsonMember", "std::Option", "std::Result"];
                if MEMORY.contains(&name.as_str()) {
                    if !args.iter().all(|a| go(items, a, seen)) {
                        return false;
                    }
                    if !matches!(name.as_str(), "std::Option" | "std::Result" | "std::Json" | "std::JsonMember") {
                        return true;
                    }
                } else if type_is_owner(items, name) {
                    return false;
                }
                // A recursive type (`Json`) is quiet if the rest is.
                let key = ty.to_string();
                if seen.contains(&key) {
                    return true;
                }
                seen.push(key);
                let ok = if let Some(s) = items.structs.get(name) {
                    let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    s.fields.iter().all(|f| go(items, &apply_subst(&f.ty, &subst), seen))
                } else if let Some(e) = items.enums.get(name) {
                    let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    e.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| go(items, &apply_subst(p, &subst), seen)))
                } else {
                    false
                };
                seen.pop();
                ok
            }
            _ => false,
        }
    }
    go(items, ty, &mut Vec::new())
}

// D-0049: a struct or enum that owns something by itself, whatever its
// fields hold: declared `resource`, or with a destructor.
pub fn type_is_owner(items: &Items, name: &str) -> bool {
    items.structs.get(name).map_or(false, |s| s.resource)
        || items.enums.get(name).map_or(false, |e| e.resource)
        || items.fns.contains_key(&format!("{}::drop", name))
}

// D-0049: whether every value of `ty` owns a resource (so writing over a
// valid one is `[Write-Resource-Overwrite-Rejected]` without looking at
// the value). A derived enum with a variant that owns nothing (`None` of
// an `Option<Box<T>>`) does not; neither does a struct made only of such.
pub fn always_owns(items: &Items, ty: &Type) -> bool {
    match ty {
        Type::Handle(_) | Type::Guard(_) | Type::Mutex(_) => true,
        Type::Array(inner, n) => *n > 0 && always_owns(items, inner),
        Type::Named(name, args) => {
            if !is_resource_ty(items, ty) {
                return false;
            }
            if type_is_owner(items, name) {
                return true;
            }
            if let Some(s) = items.structs.get(name) {
                let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                s.fields.iter().any(|f| always_owns(items, &apply_subst(&f.ty, &subst)))
            } else if let Some(e) = items.enums.get(name) {
                let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                !e.variants.is_empty()
                    && e.variants.iter().all(|v| v.payload.as_ref().map_or(false, |p| always_owns(items, &apply_subst(p, &subst))))
            } else {
                false
            }
        }
        _ => false,
    }
}

#[derive(PartialEq)]
enum OperandKind {
    Place,
    Temporary,
    NonPlace,
}

// spec/13 §1: place expressions are a name, `*e`, `e.f`, `e[i]` and
// `reclaim<τ>(p)`. A projection whose root is an aggregate literal or a
// call result is a place rooted at a temporary (`[Temp-Root]`), which
// `[Ref-Form-Temporary]` rejects; anything else is not a place at all.
// A projection of a reference (`get(b).f`, `[Field-Access-Auto-Deref]`)
// is `(*e).f`: a place through that reference, whatever `e` is.
fn borrow_operand_kind(e: &Expr, is_ref: &dyn Fn(&Expr) -> bool) -> OperandKind {
    fn root(e: &Expr, projected: bool, is_ref: &dyn Fn(&Expr) -> bool) -> OperandKind {
        match &e.kind {
            ExprKind::Paren(inner) => root(inner, projected, is_ref),
            ExprKind::Path(..) | ExprKind::Deref(_) => OperandKind::Place,
            ExprKind::Field(base, _) | ExprKind::Index(base, _) => {
                if is_ref(base) {
                    OperandKind::Place
                } else {
                    root(base, true, is_ref)
                }
            }
            ExprKind::Call(callee, _) => {
                if let ExprKind::Path(segs, _) = &callee.kind {
                    if segs.len() == 1 && segs[0] == "reclaim" {
                        return OperandKind::Place;
                    }
                }
                if projected {
                    OperandKind::Temporary
                } else {
                    OperandKind::NonPlace
                }
            }
            ExprKind::StructLit(..) | ExprKind::ArrayLit(..) | ExprKind::ArrayRepeat(..) => {
                if projected {
                    OperandKind::Temporary
                } else {
                    OperandKind::NonPlace
                }
            }
            _ => OperandKind::NonPlace,
        }
    }
    root(e, false, is_ref)
}

const TEXT_ORDER_HELP: &str = "`<`, `<=`, `>`, `>=` order two `StringView`s (or two `str`s, two `String`s), not a view and a `str`; make both views";

const MOVE_OUT_HELP: &str = "a resource cannot be moved out of part of a struct: move the whole struct, take it apart (`Name { f1, f2 } = x;`), or exchange the part with `replace(&mut x.f, v)`";

// A value that is a fresh temporary: a call's result or an aggregate
// literal (D-0073). The checker admits `&` of one only as an argument
// (`[Ref-Form-Temporary-Argument]`), so the evaluators recognise such a
// borrow by this shape alone -- a rewritten copy of an expression (a
// short-circuit condition in `cobc`) is recognised as well.
// A binding's name as the program wrote it (a local constant's or a
// shadowing binding's hidden name ends in `$…`).
fn shown_name(x: &str) -> &str {
    x.split('$').next().filter(|s| !s.is_empty()).unwrap_or(x)
}

// The holder of a borrow, as a message names it: `foreach`'s hidden
// bindings are the loop.
fn is_loop_holder(h: &str) -> bool {
    h.starts_with("$c") || h.starts_with("$d") || h.starts_with("$e")
}

// A place as a message names it: its root and fields, an element as `[…]`.
fn shown_place(root: &str, path: &[PElem]) -> String {
    let mut out = shown_name(root).to_string();
    // `(*r)[0 .. 1]`, not `*r[0 .. 1]`.
    if out.starts_with('*') && matches!(path.first(), Some(PElem::Index(_) | PElem::Range(_))) {
        out = format!("({})", out);
    }
    for e in path {
        match e {
            PElem::Field(f) => {
                out.push('.');
                out.push_str(f);
            }
            PElem::Index(Some(i)) => out.push_str(&format!("[{}]", i)),
            PElem::Index(None) => out.push_str("[…]"),
            PElem::Range(Some((l, h))) => out.push_str(&format!("[{} .. {}]", l, h)),
            PElem::Range(None) => out.push_str("[… .. …]"),
        }
    }
    out
}

// What a callee does through a reference, as a message says it.
// A callee as a message names it: without `std::`.
fn shown_callee(key: &str) -> &str {
    key.strip_prefix("std::").unwrap_or(key)
}

fn shown_access(m: &Mode) -> &'static str {
    match m {
        Mode::Exclusive => "writes",
        Mode::Shared => "reads",
    }
}

fn holder_text(h: &str) -> String {
    if is_loop_holder(h) {
        "the `foreach` loop".to_string()
    } else {
        format!("`{}`", shown_name(h))
    }
}

pub fn is_temp_value(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Paren(inner) => is_temp_value(inner),
        ExprKind::Call(callee, _) => !matches!(&callee.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "reclaim"),
        ExprKind::StructLit(..) | ExprKind::ArrayLit(..) | ExprKind::ArrayRepeat(..) => true,
        // D-0084: an operator's result, such as `&(k << 32 | c)` for a
        // map key, and a number or `bool` literal, such as `&3`, which
        // takes the referent type the parameter expects.
        ExprKind::Binary(..) | ExprKind::Unary(..) => true,
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(..) => true,
        // `f()?`: the payload `?` gives is a value, as a call's result is.
        ExprKind::Propagate(..) => true,
        _ => false,
    }
}

fn root_local_name(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Path(segs, _) if segs.len() == 1 => Some(segs[0].clone()),
        ExprKind::Paren(inner) => root_local_name(inner),
        _ => None,
    }
}

impl<'a> Body<'a> {
    // D-0090: an operator on a type parameter's value (`at`/`bt`, one of
    // them a marker): allowed when its bound provides it.
    fn check_bounded_binary(&mut self, op: BinOp, at: Option<&Type>, bt: Option<&Type>) -> TResult {
        use BinOp::*;
        let m = match (at, bt) {
            (Some(a), _) if is_marker(a) => a.clone(),
            (_, Some(b)) => b.clone(),
            _ => return Err(unbounded_msg("an operator")),
        };
        let Type::Named(mn, _) = &m else { return Err(unbounded_msg("an operator")) };
        let (need, sym) = match op {
            Eq => (Bound::Eq, "=="),
            Ne => (Bound::Eq, "!="),
            Lt => (Bound::Ordered, "<"),
            Le => (Bound::Ordered, "<="),
            Gt => (Bound::Ordered, ">"),
            Ge => (Bound::Ordered, ">="),
            Add => (Bound::Number, "+"),
            Sub => (Bound::Number, "-"),
            Mul => (Bound::Number, "*"),
            Div => (Bound::Number, "/"),
            Rem => (Bound::Number, "%"),
            BitAnd => (Bound::Integer, "&"),
            BitOr => (Bound::Integer, "|"),
            BitXor => (Bound::Integer, "^"),
            Shl => (Bound::Integer, "<<"),
            Shr => (Bound::Integer, ">>"),
            _ => return Err(unbounded_msg("a logical operator")),
        };
        if !self.marker_bounds.get(mn).map_or(false, |have| have.implies(need)) {
            return Err(self.bound_msg(mn, sym, need));
        }
        // Both operands of the parameter's type (a shift's amount is a
        // `u32`, as always).
        let same = |t: Option<&Type>| t.map_or(true, |t| *t == m);
        let ok = if matches!(op, Shl | Shr) {
            same(at) && bt.map_or(true, |t| *t == Type::Int(IntTy::U32))
        } else {
            same(at) && same(bt)
        };
        if !ok {
            let show = |t: Option<&Type>| t.map_or("?".to_string(), |t| t.to_string());
            return Err(mismatch(format!("the operands of `{}` are `{}` and `{}`; both must be the type parameter's type", sym, show(at), show(bt))));
        }
        Ok(Some(if matches!(op, Eq | Ne | Lt | Le | Gt | Ge) { Type::Bool } else { m }))
    }

    // D-0271: whether the concrete type `t` satisfies `b`: the bound's own
    // types (`Bound::holds_for`), and for `eq` and `ordered` every key type
    // (D-0110: a struct or enum whose fields or payloads are all key
    // types), ordered as `Vec::sort` orders it (`[Cmp-Key]`).
    pub(super) fn bound_holds(&self, b: Bound, t: &Type) -> bool {
        b.holds_for(t) || (matches!(b, Bound::Eq | Bound::Ordered) && self.named_key_type(t))
    }

    // D-0271: a place of a struct or enum key type that holds a `String`:
    // a comparison borrows it shared, as it borrows a `String` (D-0108).
    fn is_resource_key_place(&self, e: &Expr) -> bool {
        matches!(self.static_place_type(e), Some(t) if self.named_key_type(&t) && is_resource_ty(self.items, &t))
    }

    // A struct or enum key type (not `String`, whose comparisons are
    // `[Cmp-Text]`'s).
    pub(super) fn named_key_type(&self, t: &Type) -> bool {
        matches!(t, Type::Named(..)) && !is_std_string(t) && is_key_type(self.items, t)
    }

    fn bound_msg(&self, marker: &str, sym: &str, need: Bound) -> String {
        let n = &marker[1..];
        match self.marker_bounds.get(marker) {
            Some(have) => crate::modres::named(
                "diag.unbounded-type-parameter",
                format!("`{}` on a `{}` needs `{}: {}`, but `{}` is declared `{}`", sym, n, n, need.name(), n, have.name()),
            ),
            None => crate::modres::named(
                "diag.unbounded-type-parameter",
                format!("`{}` on a `{}` needs a bound: declare `<{}: {}>` (D-0090), or pass the operation in as a `fn` parameter", sym, n, n, need.name()),
            ),
        }
    }

    // The key `f` is filed under in `items.fns`: its module-qualified
    // path. `f_key` rebuilds only the simple name, which names a
    // different function whenever the program's own item hides a `std`
    // one of the same name (a program's `swap<T>` hides `std::swap`):
    // std's `swap<Vec<usize>>` was then checked against the program's
    // body, and std's own body at that type never at all.
    fn item_key(&self, f: &std::sync::Arc<FnDecl>) -> String {
        let simple = f_key(f);
        if self.items.fns.get(&simple).is_some_and(|g| std::sync::Arc::ptr_eq(g, f)) {
            return simple;
        }
        self.items.fns.iter().find(|(_, g)| std::sync::Arc::ptr_eq(g, f)).map(|(k, _)| k.clone()).unwrap_or(simple)
    }

    fn record_instantiation(&mut self, name: &str, targs: &[Type]) {
        if targs.iter().all(|t| !has_unresolved_marker(t)) {
            self.new_instantiations.push((name.to_string(), targs.to_vec()));
        }
    }

    fn resolve_fn_path(&self, callee: &Expr) -> Option<std::sync::Arc<FnDecl>> {
        if let ExprKind::Path(segs, _) = &callee.kind {
            let name = segs.join("::");
            return self.items.fns.get(&name).cloned();
        }
        None
    }

    // ---- rule.control.flow-analysis: scopes, places, discharge ----

    // A flow-analysis refutation: `disposition: rejected` for this
    // instance, with the rule's own diagnostic. Suppressed while a loop
    // body is being iterated to its fixed point, and at an unreachable
    // program point (no fact is asserted there).
    fn refute(&self, diag: &str) -> Result<(), String> {
        if self.report && !self.flow.unreachable {
            Err(diag.to_string())
        } else {
            Ok(())
        }
    }

    fn declare(&mut self, name: &str, initialized: bool) {
        let entry = ScopeEntry {
            name: name.to_string(),
            flow: if self.flow.is_tracked(name) { Some(self.flow.save(name)) } else { None },
            decl_block: self.decl_block.get(name).copied(),
            ref_origin: self.ref_origin.get(name).cloned(),
        };
        self.flow.declare(name, initialized);
        let here = *self.block_stack.last().unwrap_or(&0);
        self.decl_block.insert(name.to_string(), here);
        self.ref_origin.remove(name);
        if let Some(sc) = self.scopes.last_mut() {
            sc.push(entry);
        }
    }

    fn enter_scope(&mut self) {
        let id = self.next_block;
        self.next_block += 1;
        if let Some(parent) = self.block_stack.last() {
            self.block_parent.insert(id, *parent);
        }
        self.block_stack.push(id);
        self.scopes.push(Vec::new());
    }

    // `[Block-Exit]`'s transfer: every binding the block declared ends.
    fn exit_scope(&mut self) {
        let sc = self.scopes.pop().unwrap_or_default();
        self.block_stack.pop();
        for e in sc.into_iter().rev() {
            self.closure_arity.remove(&e.name);
            self.flow.remove(&e.name);
            self.decl_block.remove(&e.name);
            self.ref_origin.remove(&e.name);
            if let Some(f) = e.flow {
                self.flow.restore(&e.name, f);
            }
            if let Some(b) = e.decl_block {
                self.decl_block.insert(e.name.clone(), b);
            }
            if let Some(o) = e.ref_origin {
                self.ref_origin.insert(e.name.clone(), o);
            }
        }
    }

    // The function body block: its trailing expression is the
    // function's result, which escapes to the caller.
    fn check_fn_body(&mut self, b: &Block, ret: &Type) -> TResult {
        let saved_gamma = self.gamma.clone();
        self.enter_scope();
        let mut last_stmt_ty: Option<Type> = None;
        let mut r = Ok(None);
        match self.check_block_stmts(b) {
            Ok(t) => last_stmt_ty = t,
            Err(e) => r = Err(e),
        }
        if r.is_ok() {
            r = match &b.tail {
                Some(e) => match self.check_escape_to_caller(e) {
                    Err(d) => Err(format!("{d}{}", crate::diagnostics::at(e.line, e.col))),
                    Ok(()) => self.check_store_operand(e, Some(ret)),
                },
                None => Ok(Some(match last_stmt_ty {
                    Some(Type::Never) => Type::Never,
                    _ => Type::Void,
                })),
            };
        }
        self.exit_scope();
        self.gamma = saved_gamma;
        r
    }

    // ---- rule.temporal.ref-escape (spec/10 §2, §3) ----

    // Does block `outer` strictly lexically enclose block `inner`?
    fn strictly_encloses(&self, outer: usize, inner: usize) -> bool {
        let mut b = inner;
        while let Some(p) = self.block_parent.get(&b) {
            if *p == outer {
                return true;
            }
            b = *p;
        }
        false
    }

    // `referent-block(e)` for a borrow expression, together with the
    // referent's root binding: a place rooted at a non-reference binding
    // `x` names `decl-block(x)`; one rooted at `*r` or a projection
    // through a reference-typed binding `r` names whatever the visible
    // borrow that initialized `r` named, else `unknown` (`None`). An
    // elided call (`[Call-Elided-Lifetime]`) is treated as the borrow
    // its one reference argument is, or as `decl-block(x)` when that
    // argument is a reference-typed binding `x` initialized by a visible
    // borrow.
    fn referent_of(&self, e: &Expr) -> Option<(usize, String)> {
        if let Some(r) = self.borrow_referents.get(&(e as *const Expr as usize)) {
            return r.clone();
        }
        match &e.kind {
            ExprKind::Paren(inner) => self.referent_of(inner),
            ExprKind::Borrow(_, inner) | ExprKind::SliceOf(_, inner, _, _) => self.referent_of_place(inner),
            ExprKind::Call(callee, args) => {
                let f = self.resolve_fn_path(callee)?;
                let ref_positions: Vec<usize> = f
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| is_borrow_ty(&p.ty))
                    .map(|(i, _)| i)
                    .collect();
                if ref_positions.len() != 1 || !is_borrow_ty(&f.ret) {
                    return None;
                }
                let arg = args.get(ref_positions[0])?;
                match &arg.kind {
                    ExprKind::Borrow(..) | ExprKind::SliceOf(..) => self.referent_of(arg),
                    ExprKind::Path(segs, _) if segs.len() == 1 => {
                        let x = &segs[0];
                        if self.lookup_local(x).as_ref().map_or(false, is_borrow_ty) && matches!(self.ref_origin.get(x), Some(Some(_))) {
                            Some((*self.decl_block.get(x)?, x.clone()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn referent_of_place(&self, e: &Expr) -> Option<(usize, String)> {
        match &e.kind {
            ExprKind::Paren(inner) | ExprKind::Field(inner, _) | ExprKind::Index(inner, _) => self.referent_of_place(inner),
            ExprKind::Deref(inner) => match &inner.kind {
                ExprKind::Path(segs, _) if segs.len() == 1 => self.ref_origin.get(&segs[0]).cloned().flatten(),
                ExprKind::Paren(p) => self.referent_of_place(&Expr { kind: ExprKind::Deref(p.clone()), line: e.line, col: e.col }),
                _ => None,
            },
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                let x = &segs[0];
                match self.lookup_local(x) {
                    // A reference, slice or view binding holds a borrow: a
                    // place reached through it (`&s[i]`, `&s[lo .. hi]`, a
                    // slice of a slice, `v.bytes`) has that borrow's origin,
                    // not the binding's own block.
                    Some(t) if is_borrow_ty(&t) => self.ref_origin.get(x).cloned().flatten(),
                    Some(_) => Some((*self.decl_block.get(x)?, x.clone())),
                    None => None,
                }
            }
            _ => None,
        }
    }

    // The referents of the references `e` stores, each with whether a
    // closure holds it: `e` itself when it is a borrow or an elided call;
    // a closure without `move`, which is a struct of borrows of its
    // captures (`[Closure-Form-Borrow]`, spec/15; a capture that is itself
    // a reference is left out, as in `closure_facts_of`); the field,
    // element and payload initializers of an aggregate literal (a
    // reference stored into a field of an aggregate whose own value
    // escapes escapes with it); the results of a block, `if` or `match`.
    fn collect_stored_borrows(&self, e: &Expr, out: &mut Vec<(usize, String, bool)>) {
        match &e.kind {
            ExprKind::Borrow(..) | ExprKind::SliceOf(..) => {
                if let Some((b, root)) = self.referent_of(e) {
                    out.push((b, root, false));
                }
            }
            ExprKind::Closure { is_move: false, captures, .. } => {
                for c in captures {
                    if self.lookup_local(c).map_or(false, |t| !is_borrow_ty(&t)) {
                        if let Some(b) = self.decl_block.get(c) {
                            out.push((*b, c.clone(), true));
                        }
                    }
                }
            }
            ExprKind::Paren(inner) => self.collect_stored_borrows(inner, out),
            ExprKind::Call(callee, args) => {
                if let ExprKind::Path(segs, _) = &callee.kind {
                    if args.len() == 1 && self.items.enum_of_variant.contains_key(segs.last().unwrap()) && !self.items.fns.contains_key(&segs.join("::")) {
                        self.collect_stored_borrows(&args[0], out);
                        return;
                    }
                }
                if let Some((b, root)) = self.referent_of(e) {
                    out.push((b, root, false));
                }
            }
            ExprKind::StructLit(_, _, fields) => {
                for (_, fe) in fields {
                    self.collect_stored_borrows(fe, out);
                }
            }
            ExprKind::ArrayLit(items) => {
                for it in items {
                    self.collect_stored_borrows(it, out);
                }
            }
            ExprKind::ArrayRepeat(it, _) => self.collect_stored_borrows(it, out),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => {
                if let Some(t) = &b.tail {
                    self.collect_stored_borrows(t, out);
                }
            }
            ExprKind::If(_, t, f) => {
                if let Some(tt) = &t.tail {
                    self.collect_stored_borrows(tt, out);
                }
                if let Some(fe) = f {
                    self.collect_stored_borrows(fe, out);
                }
            }
            ExprKind::Match(_, arms) => {
                for a in arms {
                    self.collect_stored_borrows(&a.body, out);
                }
            }
            _ => {}
        }
    }

    // `[Ref-Escape-Rejected]`, store into a binding whose decl-block is
    // `dest`: rejected when the referent block is known and `dest`
    // strictly encloses it.
    fn check_escape_into(&self, dest: usize, e: &Expr) -> Result<(), String> {
        let mut bs = Vec::new();
        self.collect_stored_borrows(e, &mut bs);
        for (br, root, closure) in bs {
            if self.strictly_encloses(dest, br) {
                return Err(crate::modres::named(
                    "diag.reference-escapes-scope",
                    if closure {
                        format!("this stores a closure that borrows `{}` in a place that outlives it; capture by value with `move [...]`, or keep the closure inside that block", shown_name(&root))
                    } else {
                        "this stores a reference in a place that outlives what it refers to, which ends with its own block; store an owned value instead, or keep the reference inside that block".to_string()
                    },
                ));
            }
        }
        Ok(())
    }

    // `[Ref-Escape-Rejected]`, `B = caller` (the function's result):
    // rejected when the referent block is known and the referent's root
    // binding is not a reference-typed parameter.
    fn check_escape_to_caller(&self, e: &Expr) -> Result<(), String> {
        let mut bs = Vec::new();
        self.collect_stored_borrows(e, &mut bs);
        for (_, root, closure) in bs {
            let ref_param = self.params.contains(&root) && self.lookup_local(&root).as_ref().map_or(false, is_borrow_ty);
            if !ref_param {
                return Err(crate::modres::named(
                    "diag.reference-escapes-scope",
                    if closure {
                        format!("the returned closure borrows `{}`, which ends when this function returns; capture it by value with `move [...]`", shown_name(&root))
                    } else {
                        format!("the result refers to `{}`, which ends when this function returns; a returned reference must come from a reference parameter -- return an owned value instead", shown_name(&root))
                    },
                ));
            }
        }
        Ok(())
    }

    // The syntactic place `x.π` an expression denotes, when its root is
    // a tracked binding reached without crossing a reference: a bare
    // name, or field/index projections of one whose static type at each
    // step is the aggregate itself. Anything rooted at `*r`, at an
    // auto-dereferenced reference, or at a temporary yields `None`
    // ("never T, never F").
    fn place_of(&self, e: &Expr) -> Option<(String, Vec<PElem>)> {
        match &e.kind {
            ExprKind::Paren(inner) => self.place_of(inner),
            ExprKind::Path(segs, _) if segs.len() == 1 && self.flow.is_tracked(&segs[0]) => Some((segs[0].clone(), Vec::new())),
            ExprKind::Field(base, f) => {
                let (x, mut p) = self.place_of(base)?;
                let t = self.place_type(&x, &p)?;
                if !matches!(t, Type::Named(..)) {
                    return None;
                }
                p.push(PElem::Field(f.clone()));
                Some((x, p))
            }
            ExprKind::Index(base, idx) => {
                let (x, mut p) = self.place_of(base)?;
                let t = self.place_type(&x, &p)?;
                if !matches!(t, Type::Array(..) | Type::Simd(..)) {
                    return None;
                }
                let lit = literal_value(idx).and_then(|v| if v >= 0 { Some(v as u128) } else { None });
                p.push(PElem::Index(lit));
                Some((x, p))
            }
            _ => None,
        }
    }

    // D-0071: a place reached through a tracked reference or guard
    // binding `r` -- `*r`, `(*r).f`, `r.f` (auto-deref), `r[i]` -- as a
    // path from the root `*r`. Only the `¬clash` side-conditions use it:
    // what `r` refers to is not a binding of this body, so it has no
    // `valid`/`init` fact.
    fn deref_place_of(&self, e: &Expr) -> Option<(String, Vec<PElem>)> {
        let through = |base: &Expr| -> Option<(String, Vec<PElem>)> {
            if let Some(p) = self.deref_place_of(base) {
                return Some(p);
            }
            let ExprKind::Path(segs, _) = &strip_parens_tc(base).kind else { return None };
            if segs.len() != 1 || !self.flow.is_tracked(&segs[0]) {
                return None;
            }
            match self.lookup_local(&segs[0])? {
                Type::Ref(..) | Type::Guard(_) => Some((referent_root(&segs[0]), Vec::new())),
                _ => None,
            }
        };
        match &e.kind {
            ExprKind::Paren(inner) => self.deref_place_of(inner),
            ExprKind::Deref(inner) => {
                let ExprKind::Path(segs, _) = &strip_parens_tc(inner).kind else { return None };
                if segs.len() != 1 || !self.flow.is_tracked(&segs[0]) {
                    return None;
                }
                match self.lookup_local(&segs[0])? {
                    Type::Ref(..) | Type::Guard(_) => Some((referent_root(&segs[0]), Vec::new())),
                    _ => None,
                }
            }
            ExprKind::Field(base, f) => {
                let (x, mut p) = through(base)?;
                if !matches!(self.place_type(&x, &p)?, Type::Named(..)) {
                    return None;
                }
                p.push(PElem::Field(f.clone()));
                Some((x, p))
            }
            ExprKind::Index(base, idx) => {
                let (x, mut p) = through(base)?;
                if !matches!(self.place_type(&x, &p)?, Type::Array(..) | Type::Simd(..)) {
                    return None;
                }
                let lit = literal_value(idx).and_then(|v| if v >= 0 { Some(v as u128) } else { None });
                p.push(PElem::Index(lit));
                Some((x, p))
            }
            _ => None,
        }
    }

    // The static type at `x.π`, from `gamma` and the declarations.
    fn place_type(&self, x: &str, path: &[PElem]) -> Option<Type> {
        let mut t = match x.strip_prefix('*') {
            Some(r) => match self.lookup_local(r)? {
                Type::Ref(t, _) | Type::Guard(t) => *t,
                _ => return None,
            },
            None => self.lookup_local(x)?,
        };
        for el in path {
            t = match (el, &t) {
                (PElem::Field(f), Type::Named(sname, args)) => {
                    let sd = self.items.structs.get(sname)?;
                    let subst: HashMap<String, Type> = sd.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    let fd = sd.fields.iter().find(|fd| &fd.name == f)?;
                    apply_subst(&fd.ty, &subst)
                }
                (PElem::Index(_), Type::Array(inner, _) | Type::Simd(inner, _)) => (**inner).clone(),
                _ => return None,
            };
        }
        Some(t)
    }

    // The static type of a place-shaped expression, computed without
    // walking it (no side effects): names from `gamma`, projections
    // through struct fields and arrays (auto-dereferencing references),
    // `*e`, and reference-returning calls (whose *mode* is what
    // `crosses_shared` needs). `None` = not determinable here.
    fn static_place_type(&self, e: &Expr) -> Option<Type> {
        match &e.kind {
            ExprKind::Paren(inner) => self.static_place_type(inner),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup_local(&segs[0]),
            ExprKind::Deref(inner) => match self.static_place_type(inner)? {
                Type::Ref(t, _) | Type::Guard(t) | Type::Rawptr(t) => Some(*t),
                _ => None,
            },
            ExprKind::Field(base, f) => {
                let bt = match self.static_place_type(base)? {
                    Type::Ref(t, _) | Type::Guard(t) => *t,
                    t => t,
                };
                if let Type::Named(sname, args) = bt {
                    let sd = self.items.structs.get(&sname)?;
                    let subst: HashMap<String, Type> = sd.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    let fd = sd.fields.iter().find(|fd| &fd.name == f)?;
                    Some(apply_subst(&fd.ty, &subst))
                } else {
                    None
                }
            }
            ExprKind::Index(base, _) => {
                let bt = match self.static_place_type(base)? {
                    Type::Ref(t, _) | Type::Guard(t) => *t,
                    t => t,
                };
                if let Some(t) = indexed_elem(&bt).filter(|_| !matches!(bt, Type::Array(..))) {
                    return Some(t);
                }
                match bt {
                    Type::Array(t, _) | Type::Simd(t, _) => Some(*t),
                    _ => None,
                }
            }
            ExprKind::Call(callee, args) => {
                let f = self.resolve_fn_path(callee)?;
                if f.type_params.is_empty() {
                    Some(f.ret.clone())
                } else if matches!(f.ret, Type::Ref(..)) {
                    // A generic call's reference result, its type arguments
                    // read off its arguments' shapes, so a field or element
                    // through it is a place (`Box::get(p).segs[0]`); where
                    // they stay unknown only its mode is ever consulted.
                    let mut m = HashMap::default();
                    for (p, a) in f.params.iter().zip(args.iter()) {
                        let shape = match &strip_parens_tc(a).kind {
                            ExprKind::Borrow(md, inner) => self.static_place_type(inner).map(|t| Type::Ref(Box::new(t), md.clone())),
                            _ => self.static_place_type(a),
                        };
                        if let Some(t) = shape {
                            unify_type_shape(&p.ty, &t, &mut m, &f.type_params);
                        }
                    }
                    Some(apply_subst(&f.ret, &m))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // Does evaluating `e` as a place cross a `ref<_, shared>`: an
    // explicit `*r` with `r` shared, or a field/index auto-deref through
    // a shared reference, at any hop?
    // D-0053: whether `&base[..]` makes a `StringView`: `Some(true)` for a
    // `String` (through a reference too), `Some(false)` for a `StringView`.
    fn view_base(&self, base: &Expr) -> Option<bool> {
        let t = self.static_place_type(base)?;
        let t = match t {
            Type::Ref(inner, _) => *inner,
            t => t,
        };
        match &t {
            Type::Named(n, _) if n == "std::String" => Some(true),
            Type::Named(n, _) if n == "std::StringView" => Some(false),
            _ => None,
        }
    }

    fn crosses_shared(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Paren(inner) => self.crosses_shared(inner),
            // D-0218: `*r` with `r` itself read through a shared reference
            // (`*s.inner`, `*v[0]`, `**p`) is shared too.
            ExprKind::Deref(inner) => match self.static_place_type(inner) {
                Some(Type::Ref(_, Mode::Shared)) => true,
                Some(Type::Ref(_, Mode::Exclusive)) => self.crosses_shared(inner),
                _ => false,
            },
            ExprKind::Field(base, _) | ExprKind::Index(base, _) => {
                matches!(self.static_place_type(base), Some(Type::Ref(_, Mode::Shared)) | Some(Type::Slice(_, Mode::Shared))) || self.crosses_shared(base)
            }
            _ => false,
        }
    }

    fn place_is_resource(&self, x: &str, path: &[PElem]) -> bool {
        self.place_type(x, path).map(|t| is_resource_ty(self.items, &t)).unwrap_or(false)
    }

    // `¬clash(a, m)` for an access at place `x.π`: refuted when some
    // `deriv(r, x.π', m') = T` with `π, π'` overlapping and
    // `¬permitted(m, m')`.
    fn flow_clash(&self, x: &str, path: &[PElem], mode: Mode, diag: &str) -> Result<(), String> {
        self.flow_clash_saying(x, path, mode, diag, None)
    }

    // `flow_clash`, the message naming the access (`need`) when it is not
    // a plain read, write or borrow.
    fn flow_clash_saying(&self, x: &str, path: &[PElem], mode: Mode, diag: &str, need: Option<&str>) -> Result<(), String> {
        let mut holders: Vec<(&String, &DerivKey)> = Vec::new();
        for (h, facts) in self.flow.deriv.iter() {
            for (k, v) in facts {
                if *v == Tri::T && k.root == x && paths_overlap(&k.path, path) && !permitted(mode.clone(), k.mode.clone()) {
                    holders.push((h, k));
                }
            }
        }
        if self.report && !self.flow.unreachable && tier_report::on() {
            let mut undecided: Vec<String> = Vec::new();
            for (h, facts) in self.flow.deriv.iter() {
                for (k, v) in facts {
                    if *v == Tri::U && k.root == x && paths_overlap(&k.path, path) && !permitted(mode.clone(), k.mode.clone()) && tier_report::live_after(h, self.current_line, self.current_col) {
                        undecided.push(format!("U\t{}\t{}\t{:?}\t{}", self.current_line, x, k.mode, h));
                    }
                }
            }
            undecided.sort();
            let line = self.current_line;
            let mut maybe = false;
            for (h, facts) in self.flow.deriv.iter() {
                for (k, v) in facts {
                    if *v != Tri::F && k.root == x && !permitted(mode.clone(), k.mode.clone()) && overlap3(&k.path, path) == Ov::Maybe && tier_report::live_after(h, self.current_line, self.current_col) {
                        maybe = true;
                    }
                }
            }
            if maybe {
                tier_report::with(|r| {
                    r.point_checks += 1;
                    r.events.push(format!("P\t{}\t{}\taccess", line, x));
                });
                tier_report::left(format!("line {}: an access to `{}` that only values decide", line, shown_name(x)));
            }
            tier_report::with(|r| {
                r.checks += 1;
                if !undecided.is_empty() {
                    r.undecided += 1;
                    r.events.push(format!("{}\t{}", undecided[0], line));
                }
            });
            if !undecided.is_empty() {
                tier_report::left(format!("line {}: an access to `{}` that a reference may reach on some paths", line, shown_name(x)));
            }
        }
        // The first holder by name, so the message is the same every run.
        if let Some((h, k)) = holders.into_iter().min_by(|a, b| a.0.cmp(b.0)) {
            let need = match (need, diag, &mode) {
                (Some(n), _, _) => n.to_string(),
                (None, "diag.aliasing-conflict", Mode::Exclusive) => "so it cannot be written or borrowed `mut` here".to_string(),
                (None, "diag.aliasing-conflict", Mode::Shared) => "so it cannot be read here".to_string(),
                _ => "so it cannot be moved or destroyed here".to_string(),
            };
            let excl = k.mode == Mode::Exclusive;
            let who = match (is_loop_holder(h), excl) {
                (true, true) => format!("the `foreach` loop over `{}` borrows it exclusively", shown_name(x)),
                (true, false) => format!("the `foreach` loop over `{}` still borrows it", shown_name(x)),
                (false, true) => format!("{} borrows `{}` exclusively", holder_text(h), shown_name(x)),
                (false, false) => format!("{} still borrows `{}`", holder_text(h), shown_name(x)),
            };
            self.refute(&crate::modres::named(diag, format!("{}, {}; end that borrow first", who, need)))?;
        }
        Ok(())
    }

    // `solitary(a_x)`: refuted when some `deriv(·, x.·, ·) = T`.
    fn flow_solitary(&self, x: &str, diag: &str) -> Result<(), String> {
        let mut holders: Vec<&String> = Vec::new();
        for (h, facts) in self.flow.deriv.iter() {
            for (k, v) in facts {
                if *v == Tri::T && k.root == x {
                    holders.push(h);
                }
            }
        }
        if let Some(h) = holders.into_iter().min() {
            let who = if is_loop_holder(h) { format!("the `foreach` loop over `{}` still borrows it", shown_name(x)) } else { format!("{} still borrows `{}`", holder_text(h), shown_name(x)) };
            self.refute(&crate::modres::named(diag, format!("{}, so it cannot be moved or destroyed here; end that borrow first", who)))?;
        }
        Ok(())
    }

    // `[Read]` of a place in value position: definite assignment
    // (`init-state(o_x) = valid`, where `unknown` rejects --
    // `rule.init.definite-assignment`), `[Read-Resource-Rejected]`, and
    // `¬clash(a, shared)`.
    fn flow_read(&mut self, e: &Expr) -> Result<(), String> {
        let (x, path) = match self.place_of(e) {
            Some(p) => p,
            None => match self.deref_place_of(e) {
                Some((x, path)) => return self.flow_clash(&x, &path, Mode::Shared, "diag.aliasing-conflict"),
                None => return Ok(()),
            },
        };
        if self.flow.init.get(&x) != Some(&Tri::T) {
            self.refute(&uninit_msg(&x))?;
        }
        if self.place_is_resource(&x, &path) {
            // A name the desugaring made (`$e…`, a `foreach`'s value) is
            // not the program's to see.
            let shown = if x.starts_with('$') {
                "this value".to_string()
            } else if path.is_empty() {
                format!("`{}`", x)
            } else {
                "this part of `".to_string() + &x + "`"
            };
            self.refute(&format!(
                "diag.read-of-resource{}{} holds a resource, which is never copied: move it, lend it (`&`), or make a copy with its type's `clone`",
                crate::diagnostics::DETAIL_SEP,
                shown
            ))?;
        }
        self.flow_clash(&x, &path, Mode::Shared, "diag.aliasing-conflict")
    }

    // `[Write]` to a place: a whole-object write may initialize (and,
    // for a resource-typed binding, must not overwrite a live one --
    // `¬live-resource-at`, refuted when `valid(x) = T ∧ init(x) = T`);
    // a projection write requires the object initialized
    // (`[Write-Partial-Init]`, `unknown` rejects); both require
    // `¬clash(a, exclusive)`. Then `init(x) := T` for a whole write.
    fn flow_write(&mut self, lhs: &Expr) -> Result<(), String> {
        if let Some((x, _)) = self.place_of(lhs) {
            self.flow.lits.remove(&x);
        }
        let (x, path) = match self.place_of(lhs) {
            Some(p) => p,
            None => match self.deref_place_of(lhs) {
                Some((x, path)) => {
                    // D-0095: a write through a valid exclusive reference
                    // reaches a live object (whatever could end it would
                    // first clash with this reference's live borrow), so
                    // an always-owning place there holds a live resource.
                    if self.deref_ref_valid(lhs) {
                        self.refute_live_part_overwrite(&x, &path, lhs)?;
                    }
                    return self.flow_clash(&x, &path, Mode::Exclusive, "diag.aliasing-conflict");
                }
                None => return Ok(()),
            },
        };
        if path.is_empty() {
            // D-0049: statically only where every value of the type owns
            // something; otherwise the value decides, at run time.
            if self.place_is_resource(&x, &[])
                && self.place_type(&x, &[]).map_or(false, |t| always_owns(self.items, &t) && !quiet_ty(self.items, &t))
                && self.flow.valid.get(&x) == Some(&Tri::T)
                && self.flow.init.get(&x) == Some(&Tri::T)
            {
                let ty = self.place_type(&x, &[]).map(|t| t.to_string()).unwrap_or_default();
                let shown = x.rsplit("::").next().unwrap_or(&x);
                self.refute(&format!("diag.overwrite-of-live-resource{}`{}` still holds a `{}`, which this assignment would lose", crate::diagnostics::DETAIL_SEP, shown, ty))?;
            }
        } else if self.flow.init.get(&x) != Some(&Tri::T) {
            self.refute(&uninit_msg(&x))?;
        } else if self.flow.valid.get(&x) == Some(&Tri::T) {
            // D-0095: a field or element of a valid, initialized object
            // is never empty (moving one out is rejected, and a trusted
            // raw move-out may not take cells a live object claims:
            // spec/20 [Rawptr-Move-Out]'s side-condition).
            self.refute_live_part_overwrite(&x, &path, lhs)?;
        }
        self.flow_clash(&x, &path, Mode::Exclusive, "diag.aliasing-conflict")?;
        if path.is_empty() && !self.flow.unreachable {
            self.flow.init.insert(x, Tri::T);
        }
        Ok(())
    }

    // D-0095: `[Write-Resource-Overwrite-Rejected]` refuted statically for
    // a part of a live object (a field, an element, `*r`): such a place
    // always holds a value, so it holds a live resource exactly when
    // every value of its type owns one (D-0049's `always_owns`; an
    // `Option<String>` field may be `None`, and stays the run time's).
    fn refute_live_part_overwrite(&mut self, x: &str, path: &[PElem], lhs: &Expr) -> Result<(), String> {
        // An element is refuted only at a literal index inside a fixed
        // array: at any other index the place may not exist, and then
        // the write is `diag.index-out-of-bounds`, which only the run
        // decides (a `Vec` element is reached through a call, not here).
        for (i, step) in path.iter().enumerate() {
            match step {
                PElem::Field(_) => {}
                PElem::Index(Some(k)) => match self.place_type(x, &path[..i]) {
                    Some(Type::Array(_, n) | Type::Simd(_, n)) if (*k as u64) < n as u64 => {}
                    _ => return Ok(()),
                },
                PElem::Index(None) | PElem::Range(_) => return Ok(()),
            }
        }
        let Some(ty) = self.place_type(x, path) else { return Ok(()) };
        if !always_owns(self.items, &ty) || quiet_ty(self.items, &ty) {
            return Ok(());
        }
        let place = match &strip_parens_tc(lhs).kind {
            ExprKind::Deref(_) => "the place this reference reaches".to_string(),
            _ => "this field or element".to_string(),
        };
        self.refute(&format!(
            "diag.overwrite-of-live-resource{}{} still holds a `{}`, which this assignment would lose; write `overwrite(&mut place, v)` to destroy the old value, or `replace(&mut place, v)` to keep it",
            crate::diagnostics::DETAIL_SEP,
            place,
            ty
        ))
    }

    // D-0095: whether `lhs` writes through a reference binding (`*r`, a
    // field of `*r`, or `r.f`, which reaches through `r` implicitly) the
    // flow analysis knows is valid here.
    fn deref_ref_valid(&self, lhs: &Expr) -> bool {
        let valid_ref = |segs: &[String]| segs.len() == 1 && self.flow.valid.get(&segs[0]) == Some(&Tri::T);
        let mut e = strip_parens_tc(lhs);
        loop {
            match &e.kind {
                ExprKind::Field(base, _) | ExprKind::Index(base, _) => e = strip_parens_tc(base),
                ExprKind::Deref(inner) => {
                    return match &strip_parens_tc(inner).kind {
                        ExprKind::Path(segs, _) => valid_ref(segs),
                        _ => false,
                    };
                }
                ExprKind::Path(segs, _) => {
                    return segs.len() == 1 && matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..))) && valid_ref(segs);
                }
                _ => return false,
            }
        }
    }

    // `[Borrow]` of a place: `¬clash(a0, m)`.
    fn flow_borrow(&mut self, inner: &Expr, mode: &Mode) -> Result<(), String> {
        if let Some((x, _)) = self.place_of(inner) {
            self.flow.lits.remove(&x);
        }
        if let Some((x, path)) = self.place_of(inner).or_else(|| self.deref_place_of(inner)) {
            self.flow_clash(&x, &path, mode.clone(), "diag.aliasing-conflict")?;
        }
        Ok(())
    }

    // A consuming use of a whole resource binding (transfer/relocate):
    // `solitary(a_x)`, then `valid(x) := F` and every
    // `deriv(·, x, ·) := F`.
    fn flow_consume(&mut self, x: &str, diag: &str) -> Result<(), String> {
        self.flow_solitary(x, diag)?;
        if !self.flow.unreachable {
            self.flow.valid.insert(x.to_string(), Tri::F);
            self.flow.clear_rooted(x);
            // D-0071: what a moved guard or handle held goes with it,
            // out of this analysis' sight.
            self.flow.clear_rooted(&referent_root(x));
            self.flow.deriv.remove(x);
        }
        Ok(())
    }

    // An expression in a *store* position (a local-declaration
    // initializer, an argument, a field/element/payload initializer, a
    // `return`/result operand, the value of an assignment): a
    // resource-typed whole binding is moved rather than read; a
    // resource-typed field of a live object may not be moved out
    // (`[Store-Binding-Place-Transfer-Sub]`); anything else is an
    // ordinary value-position expression.
    fn check_store_operand(&mut self, e: &Expr, expected: Option<&Type>) -> TResult {
        // D-0282: a value moved out through a reference
        // (`drop(*r)`, `Atomic<u64> c = *r`): the reference still holds the
        // place, so the move conflicts with it, always.
        if tier_report::on() && self.report && !self.flow.unreachable {
            if let ExprKind::Deref(inner) = &strip_parens_tc(e).kind {
                if let ExprKind::Path(segs, _) = &strip_parens_tc(inner).kind {
                    let moves = self.static_place_type(e).map_or(false, |t| is_resource_ty(self.items, &t) || matches!(&t, Type::Named(n, _) if n == "std::Atomic" || n == "std::Vec" || n == "std::String"));
                    if segs.len() == 1 && moves && matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..))) {
                        let msg = format!("`*{}` cannot be moved out: `{}` still refers to it", segs[0], segs[0]);
                        let diag = format!("{}{}", crate::modres::named("diag.move-while-aliased", msg), crate::diagnostics::at(e.line, e.col));
                        self.refute(&diag)?;
                    }
                }
            }
        }
        // Tagged with the operand's line, as `check_expr` tags its own: a
        // refutation raised here (a move out of a field) is not inside one.
        let t = match self.check_store_operand_inner(e, expected) {
            Err(d) if !d.contains('@') => return Err(format!("{d}{}", crate::diagnostics::at(e.line, e.col))),
            other => other?,
        };
        // D-0071: a binding that holds borrows without being a reference
        // (a spawn's handle), stored elsewhere, takes them out of sight. A
        // closure that owns no resource is copied, not moved, and keeps
        // them (CHG-0299, `[Fn-Value-Read]`).
        if let ExprKind::Path(segs, _) = &strip_parens_tc(e).kind {
            // D-0282: a view (or an `Option`/`Result` of a
            // reference) is copied, not moved, and keeps its borrow.
            let copied_view = tier_report::on()
                && match self.lookup_local(&segs[0]) {
                    Some(Type::Named(n, a)) if n == "std::StringView" => a.is_empty(),
                    Some(Type::Named(n, a)) if n == "std::Option" || n == "std::Result" => a.first().map_or(false, |x| matches!(x, Type::Ref(..) | Type::Slice(..)) || matches!(x, Type::Named(m, _) if m == "std::StringView")),
                    _ => false,
                };
            if segs.len() == 1 && !copied_view && !self.flow.unreachable && self.flow.deriv.contains_key(&segs[0]) && !matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..) | Type::Slice(..))) && !self.closure_copied.contains(&segs[0]) {
                self.flow.deriv.remove(&segs[0]);
            }
        }
        Ok(t)
    }

    fn check_store_operand_inner(&mut self, e: &Expr, expected: Option<&Type>) -> TResult {
        if let Some(name) = root_local_name(e) {
            if let Some(Some(t)) = self.move_captures.get(&name) {
                if is_resource_ty(self.items, t) {
                    return Err("diag.move-out-of-field".to_string());
                }
            }
        }
        if let Some((x, path)) = self.place_of(e) {
            if self.place_is_resource(&x, &path) || (path.is_empty() && self.closure_owning.contains(&x)) {
                self.place_pos = true;
                let t = self.check_expr(e, expected)?;
                if path.is_empty() {
                    self.flow_consume(&x, "diag.move-while-aliased")?;
                } else {
                    self.refute(&crate::modres::named("diag.move-out-of-field", MOVE_OUT_HELP.to_string()))?;
                }
                return Ok(t);
            }
        }
        // `[Store-Binding-Place-Transfer-Sub]` on a temporary's field
        // (`f().x`, `[Temp-Root]`): a part of a whole object, statically.
        if matches!(strip_parens_tc(e).kind, ExprKind::Field(..) | ExprKind::Index(..))
            && borrow_operand_kind(e, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)) == OperandKind::Temporary
        {
            // D-0103: a resource field of a temporary struct (without a
            // destructor) is moved out, and the temporary's other fields
            // end at once: `f().x` is `{ S { x, y, … } = f(); x }`.
            if let ExprKind::Field(base, fname) = &strip_parens_tc(e).kind {
                let saved = self.flow.clone();
                let bt = self.check_expr(base, None);
                self.flow = saved;
                if let Ok(Some(Type::Named(sname, _))) = bt {
                    let sd = self.items.structs.get(&sname).cloned();
                    let has_drop = self.items.fns.contains_key(&format!("{}::drop", sname));
                    if let Some(sd) = sd {
                        let fty = sd.fields.iter().find(|f| &f.name == fname).map(|f| f.ty.clone());
                        if !has_drop && fty.map_or(false, |t| is_resource_ty(self.items, &t)) {
                            let line = e.line;
                            let col = e.col;
                            let fields: Vec<String> = sd.fields.iter().map(|f| f.name.clone()).collect();
                            let rep = std::sync::Arc::new(Expr {
                                kind: ExprKind::Block(Block {
                                    stmts: vec![Stmt::Destructure { struct_name: sname.clone(), names: fields.clone(), fields, rest: false, init: base.clone() }],
                                    tail: Some(Box::new(Expr { kind: ExprKind::Path(vec![fname.clone()], vec![]), line, col })),
                                }),
                                line,
                                col,
                            });
                            let t = self.check_expr(&rep, expected)?;
                            self.items.temp_moves.lock().unwrap().insert(e as *const Expr as usize, rep);
                            self.items.any_temp_moves.store(true, std::sync::atomic::Ordering::Relaxed);
                            return Ok(t);
                        }
                    }
                }
            }
            let t = self.check_expr(e, expected)?;
            if t.as_ref().map_or(false, |t| is_resource_ty(self.items, t)) {
                return Err(format!("{}{}", crate::modres::named("diag.move-out-of-field", MOVE_OUT_HELP.to_string()), crate::diagnostics::at(e.line, e.col)));
            }
            return Ok(t);
        }
        self.check_expr(e, expected)
    }

    // The `deriv`-forming initializers of a reference-typed binding: a
    // visible borrow `&_m x.π`, or a D-0011-elided call (exactly one
    // reference-typed parameter and a reference-typed result) whose
    // reference argument is such a borrow.
    fn deriv_facts_of(&self, e: &Expr) -> Option<DerivKey> {
        match &e.kind {
            ExprKind::Paren(inner) => self.deriv_facts_of(inner),
            ExprKind::Borrow(m, inner) => self.borrowed_place(inner).map(|(root, path)| DerivKey { root, path, mode: m.clone() }),
            // D-0070: a slice derives from its range of the source.
            ExprKind::SliceOf(m, inner, lo, hi) => self.slice_source(inner).map(|(root, mut path)| {
                path.push(range_elem(lo, hi));
                DerivKey { root, path, mode: m.clone() }
            }),
            ExprKind::Call(callee, args) => {
                let f = self.resolve_fn_path(callee)?;
                let ref_positions: Vec<usize> = f
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| is_borrow_ty(&p.ty))
                    .map(|(i, _)| i)
                    .collect();
                if ref_positions.len() != 1 || !is_borrow_ty(&f.ret) {
                    return None;
                }
                let arg = args.get(ref_positions[0])?;
                match &arg.kind {
                    ExprKind::Borrow(m, inner) => self.place_of(inner).or_else(|| self.deref_place_of(inner)).map(|(root, path)| DerivKey { root, path, mode: m.clone() }),
                    ExprKind::SliceOf(m, inner, lo, hi) => self.place_of(inner).or_else(|| self.deref_place_of(inner)).map(|(root, mut path)| {
                        path.push(range_elem(lo, hi));
                        DerivKey { root, path, mode: m.clone() }
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // A call whose result is a reference (`String::as_bytes(&s)`).
    fn returns_ref(&self, e: &Expr) -> bool {
        matches!(self.value_shape(e), Some(Type::Ref(..)))
    }

    // The type an expression's value has, as far as its declaration shows
    // it without checking it again: a local's type, a call's declared
    // result, and for `Option::unwrap`/`expect` and `Result::unwrap`/
    // `expect` (a generic `T`) the payload of their argument's shape, so
    // `Option::unwrap(HashMap::get_mut(&mut m, &k)).f` is a place through
    // the reference that call gives.
    fn value_shape(&self, e: &Expr) -> Option<Type> {
        match &strip_parens_tc(e).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup_local(&segs[0]),
            ExprKind::Call(callee, args) => {
                let f = self.resolve_fn_path(callee)?;
                let std_fn = |key: &str| self.items.fns.get(key).map_or(false, |g| std::sync::Arc::ptr_eq(g, &f));
                if (std_fn("std::Option::unwrap") || std_fn("std::Option::expect") || std_fn("std::Result::unwrap") || std_fn("std::Result::expect")) && !args.is_empty() {
                    return match self.value_shape(&args[0])? {
                        Type::Named(n, targs) if n == "std::Option" || n == "std::Result" => targs.first().cloned(),
                        _ => None,
                    };
                }
                Some(f.ret.clone())
            }
            _ => None,
        }
    }

    // The place a visible borrow `&_m e` targets: a place of this body,
    // an element of a `Vec` (below), or one through a reference (D-0071).
    // The source a slice `&m base[lo .. hi]` takes its range of: `base`'s
    // place, or for a reference binding `r` (auto-deref) its referent `*r`,
    // not the binding (D-0070 through a reference).
    fn slice_source(&self, base: &Expr) -> Option<(String, Vec<PElem>)> {
        if let ExprKind::Path(segs, _) = &strip_parens_tc(base).kind {
            if segs.len() == 1 && self.flow.is_tracked(&segs[0]) && matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..))) {
                return Some((referent_root(&segs[0]), Vec::new()));
            }
        }
        self.borrowed_place(base)
    }

    fn borrowed_place(&self, inner: &Expr) -> Option<(String, Vec<PElem>)> {
        self.place_of(inner).or_else(|| self.vec_element_root(inner)).or_else(|| self.deref_place_of(inner))
    }

    // D-0071: `spawn(f, a1, …)` -- the new thread's parameters hold every
    // visible borrow among its arguments until `join`, so a handle bound
    // to it carries their `deriv` facts.
    fn spawn_facts_of(&self, e: &Expr) -> Vec<DerivKey> {
        let ExprKind::Call(_, args) = &strip_parens_tc(e).kind else { return Vec::new() };
        let mut v: Vec<DerivKey> = args.iter().skip(1).filter_map(|a| self.deriv_facts_of(a)).collect();
        v.extend(self.spawn_lent_refs(args));
        v
    }

    // D-0213 `[Thread-Lend]`: a reference binding `r` passed to `spawn`
    // whole is lent to the thread, which holds a path borrowed from it
    // until `join`: a borrow of `*r` in the mode the parameter takes.
    fn spawn_lent_refs(&self, args: &[Expr]) -> Vec<DerivKey> {
        let params: Option<Vec<Type>> = args.first().and_then(|f| match &strip_parens_tc(f).kind {
            ExprKind::Path(..) => self.resolve_fn_path(f).map(|fd| fd.params.iter().map(|p| p.ty.clone()).collect()),
            _ => None,
        });
        args.iter()
            .skip(1)
            .enumerate()
            .filter_map(|(i, a)| match &strip_parens_tc(a).kind {
                ExprKind::Path(segs, _) if segs.len() == 1 && self.flow.is_tracked(&segs[0]) => match self.lookup_local(&segs[0]) {
                    Some(Type::Ref(_, m)) => {
                        let mode = match params.as_ref().and_then(|ps| ps.get(i)) {
                            Some(Type::Ref(_, Mode::Shared)) => Mode::Shared,
                            _ => m,
                        };
                        Some(DerivKey { root: referent_root(&segs[0]), path: Vec::new(), mode })
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    // D-0088: a call whose result holds a reference without being one
    // (`HashMap::get`'s `Option<ref<V>>`) says nothing of where the
    // reference came from; it is taken to borrow from the call's first
    // reference argument (the container, as a receiver would be), in
    // that argument's mode. `Option::unwrap(e)` and the like hold what
    // `e` holds.
    fn contained_ref_facts(&self, e: &Expr) -> Vec<DerivKey> {
        let ExprKind::Call(callee, args) = &strip_parens_tc(e).kind else { return Vec::new() };
        let Some(f) = self.resolve_fn_path(callee) else { return Vec::new() };
        let std_fn = |key: &str| self.items.fns.get(key).map_or(false, |g| std::sync::Arc::ptr_eq(g, &f));
        if (std_fn("std::Option::unwrap") || std_fn("std::Option::expect") || std_fn("std::Result::unwrap") || std_fn("std::Result::expect")) && !args.is_empty() {
            return self.contained_ref_facts(&args[0]);
        }
        if is_borrow_ty(&f.ret) || !holds_ref(self.items, &f.ret) {
            return Vec::new();
        }
        f.params
            .iter()
            .zip(args.iter())
            .filter(|(p, _)| is_borrow_ty(&p.ty))
            .take(1)
            .filter_map(|(_, a)| match &strip_parens_tc(a).kind {
                ExprKind::Borrow(m, inner) => self.borrowed_place(inner).map(|(root, path)| DerivKey { root, path, mode: m.clone() }),
                ExprKind::SliceOf(m, inner, lo, hi) => self.borrowed_place(inner).map(|(root, mut path)| {
                    path.push(range_elem(lo, hi));
                    DerivKey { root, path, mode: m.clone() }
                }),
                _ => None,
            })
            .collect()
    }

    // A closure without `move` borrows each capture for as long as it
    // exists (spec/15 §6), exclusively when its body writes it, as a
    // spawn handle holds its arguments' borrows: a binding holding one
    // carries a `deriv` fact per captured local. A capture that is itself
    // a reference is left out (the closure borrows the reference, and
    // what is reached through it is the reference's own fact).
    fn closure_facts_of(&self, e: &Expr) -> Vec<DerivKey> {
        let ExprKind::Closure { captures, body, .. } = &strip_parens_tc(e).kind else { return Vec::new() };
        captures
            .iter()
            .filter(|c| self.flow.is_tracked(c) && !self.lookup_local(c).as_ref().map_or(true, is_borrow_ty))
            .map(|c| DerivKey {
                root: c.clone(),
                path: Vec::new(),
                mode: if crate::interp::closure_body_writes(body, c) { Mode::Exclusive } else { Mode::Shared },
            })
            .collect()
    }

    // `&v[i]` on a `Vec` is `Vec::index_shared(&v, i)` (`[Index-Vec]`), an
    // elided call on a borrow of `v`: derived from `v` as that call is.
    fn vec_element_root(&self, e: &Expr) -> Option<(String, Vec<PElem>)> {
        let ExprKind::Index(base, idx) = &strip_parens_tc(e).kind else { return None };
        match self.static_place_type(base)? {
            Type::Named(n, _) if n == "std::Vec" => self.place_of(base).map(|(r, mut p)| {
                // D-0282: the element, so two of them are told apart.
                if tier_report::on() {
                    p.push(PElem::Index(literal_value(idx).filter(|v| *v >= 0).map(|v| v as u128)));
                }
                (r, p)
            }),
            _ => None,
        }
    }

    // D-0282: the facts a reference value inherits
    // from where it came from when it is not a visible borrow -- another
    // reference binding (its facts, or its referent `*r` when it has
    // none), a field or a dereference of one, a single-reference call on
    // one (D-0011), `foreach`'s hidden helpers, a `match`/`if`/block's
    // results. `None` when the value comes from nothing that can hold a
    // reference of this frame (a call with no reference argument: static
    // data), `Some(empty)` when its source is unknown (tier D: a
    // reference read out of a container or a struct).
    fn inherited_facts(&self, e: &Expr, depth: usize) -> Option<Vec<(DerivKey, Tri)>> {
        self.inherited_facts_in(e, depth, None)
    }

    // `inherited_facts` inside a `match` arm: `binder` stands for what the
    // scrutinee holds.
    fn inherited_facts_in(&self, e: &Expr, depth: usize, binder: Option<(&str, &Expr)>) -> Option<Vec<(DerivKey, Tri)>> {
        const TIER_D: &str = "<tier D>";
        let tier_d = || Some(vec![(DerivKey { root: TIER_D.to_string(), path: Vec::new(), mode: Mode::Shared }, Tri::T)]);
        if depth > 12 {
            return Some(Vec::new());
        }
        let e = strip_parens_tc(e);
        // A sub-view (`String::view(&s, lo, hi)`, `StringView::sub`): the
        // range of its source it covers.
        if let ExprKind::Call(callee, args) = &e.kind {
            if let ExprKind::Path(segs, _) = &callee.kind {
                let k = segs.join("::");
                if (k == "std::String::view" || k == "std::StringView::sub") && args.len() == 3 {
                    let r = range_elem(&args[1], &args[2]);
                    return self.inherited_facts_in(&args[0], depth + 1, binder).map(|v| {
                        v.into_iter()
                            .map(|(mut k, t)| {
                                k.path.push(r.clone());
                                (k, t)
                            })
                            .collect()
                    });
                }
            }
        }
        if let Some(k) = self.deriv_facts_of(e) {
            return Some(vec![(k, Tri::T)]);
        }
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 && binder.map_or(false, |(b, _)| b == segs[0]) => {
                let (_, scrut) = binder.unwrap();
                self.inherited_facts_in(scrut, depth + 1, None)
            }
            // A name that is no local of this frame (`None`, a constant):
            // static data.
            ExprKind::Path(segs, _) if segs.len() != 1 || self.lookup_local(&segs[0]).is_none() => None,
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                let r = &segs[0];
                // Through a reference binding: a reborrow of its referent
                // `*r` (D-0071), which keeps `r` itself alive; a copied
                // view or `Option` of a reference holds what it holds.
                if !matches!(self.lookup_local(r), Some(Type::Ref(..)) | Some(Type::Slice(..))) {
                    if let Some(f) = self.flow.deriv.get(r) {
                        return Some(f.iter().map(|(k, v)| (k.clone(), *v)).collect());
                    }
                }
                match self.lookup_local(r) {
                    Some(Type::Ref(_, m)) | Some(Type::Slice(_, m)) => Some(vec![(DerivKey { root: referent_root(r), path: Vec::new(), mode: m }, Tri::T)]),
                    Some(t) if holds_ref(self.items, &t) => Some(vec![(DerivKey { root: referent_root(r), path: Vec::new(), mode: Mode::Shared }, Tri::T)]),
                    // An owner of this frame (`&v[0].f` reaches `v`).
                    Some(_) if self.flow.is_tracked(r) => Some(vec![(DerivKey { root: r.clone(), path: Vec::new(), mode: Mode::Shared }, Tri::T)]),
                    _ => Some(Vec::new()),
                }
            }
            // A reference read out of a field of an aggregate that holds
            // references (`h.r` of a `struct Hold`): a stored reference.
            ExprKind::Field(base, _) if self.value_shape(base).or_else(|| self.static_place_type(base)).map_or(false, |t| self.tier_d_type(&t)) => tier_d(),
            ExprKind::Field(base, _) | ExprKind::Deref(base) => self.inherited_facts_in(base, depth + 1, binder),
            ExprKind::Borrow(m, inner) | ExprKind::SliceOf(m, inner, _, _) => {
                // The projection down to the reference it goes through:
                // its fields, cut at the first index (an element is
                // inside the container the path names so far).
                let mut b: &Expr = inner;
                let mut fields: Vec<String> = Vec::new();
                loop {
                    match &strip_parens_tc(b).kind {
                        ExprKind::Field(x, f) => {
                            fields.push(f.clone());
                            b = x;
                        }
                        ExprKind::Index(x, _) => {
                            fields.clear();
                            b = x;
                        }
                        ExprKind::Deref(x) => b = x,
                        _ => break,
                    }
                }
                fields.reverse();
                // A borrow of a temporary (`&padded()[0 .. $]`): it ends
                // with the statement (D-0135).
                if let ExprKind::Call(cal, _) = &strip_parens_tc(b).kind {
                    // A constant (`&HALF`) is static data, not a temporary.
                    let is_const = self.resolve_fn_path(cal).map_or(false, |f| f.is_const);
                    if !is_const && self.value_shape(b).map_or(false, |t| !is_borrow_ty(&t) && !holds_ref(self.items, &t)) {
                        return Some(vec![(DerivKey { root: "<temporary>".to_string(), path: Vec::new(), mode: m.clone() }, Tri::T)]);
                    }
                }
                self.inherited_facts_in(b, depth + 1, binder).map(|v| v.into_iter().map(|(mut k, t)| {
                    k.mode = m.clone();
                    k.path.extend(fields.iter().map(|f| PElem::Field(f.clone())));
                    (k, t)
                }).collect())
            }
            ExprKind::Call(callee, args) => {
                if let ExprKind::Path(segs, _) = &callee.kind {
                    if segs.len() == 1 && segs[0].starts_with("$each") {
                        // What the loop walks: its reference arguments (a
                        // hidden index is a number, not a source). Over a
                        // `HashMap`, a key binder reaches its `keys` and a
                        // value binder its `values`.
                        let map_field = match (segs[0].as_str(), args.first().and_then(|a| self.value_shape(a).or_else(|| self.static_place_type(a)))) {
                            ("$each_take_key" | "$each_key", Some(Type::Ref(t, _))) if matches!(&*t, Type::Named(n, _) if n == "std::HashMap") => Some("keys"),
                            ("$each_take" | "$each_at" | "$each_at_mut", Some(Type::Ref(t, _))) if matches!(&*t, Type::Named(n, _) if n == "std::HashMap") => Some("values"),
                            _ => None,
                        };
                        let mut out = Vec::new();
                        for a in args {
                            let is_value = match &strip_parens_tc(a).kind {
                                ExprKind::Path(ps, _) if ps.len() == 1 => self.lookup_local(&ps[0]).map_or(false, |t| !is_borrow_ty(&t) && !holds_ref(self.items, &t) && !matches!(t, Type::Named(..))),
                                ExprKind::IntLit(..) | ExprKind::Binary(..) => true,
                                _ => false,
                            };
                            if !is_value {
                                out.extend(self.inherited_facts_in(a, depth + 1, binder).unwrap_or_default());
                            }
                        }
                        if let Some(fl) = map_field {
                            for (k, _) in out.iter_mut() {
                                k.path.push(PElem::Field(fl.to_string()));
                            }
                        }
                        return Some(out);
                    }
                }
                let f = self.resolve_fn_path(callee)?;
                // A sub-view (`String::view(&s, lo, hi)`, `StringView::sub`):
                // the range of its source it covers.
                if let ExprKind::Path(segs, _) = &callee.kind {
                    let k = segs.join("::");
                    if (k == "std::String::view" || k == "std::StringView::sub") && args.len() == 3 {
                        let r = range_elem(&args[1], &args[2]);
                        return self.inherited_facts_in(&args[0], depth + 1, binder).map(|v| v.into_iter().map(|(mut k, t)| {
                            k.path.push(r.clone());
                            (k, t)
                        }).collect());
                    }
                }
                // A generic whose declared result holds a reference only
                // through a type argument (`Vec::pop` of a `Vec<ref<T>>`)
                // gives back a reference that was stored: tier D.
                if !is_borrow_ty(&f.ret) && !holds_ref(self.items, &f.ret) {
                    return tier_d();
                }
                let refs: Vec<&Expr> = f.params.iter().zip(args.iter()).filter(|(p, _)| is_borrow_ty(&p.ty) || holds_ref(self.items, &p.ty)).map(|(_, a)| a).collect();
                if refs.is_empty() {
                    return None;
                }
                // Through a reference to storage that itself holds
                // references (a `Vec<ref<T>>`, a struct with a reference
                // field), the result may be one of those stored references,
                // which reaches what it reaches, not that storage: tier D.
                let referent = match &strip_parens_tc(refs[0]).kind {
                    ExprKind::Borrow(_, inner) | ExprKind::SliceOf(_, inner, _, _) => self.static_place_type(inner),
                    ExprKind::Path(segs, _) if segs.len() == 1 => match self.lookup_local(&segs[0]) {
                        Some(Type::Ref(inner, _) | Type::Slice(inner, _)) => Some(*inner),
                        _ => None,
                    },
                    _ => match self.value_shape(refs[0]) {
                        Some(Type::Ref(inner, _) | Type::Slice(inner, _)) => Some(*inner),
                        _ => None,
                    },
                };
                let reaches_stored = referent.map_or(false, |t| stores_refs(self.items, &t));
                if reaches_stored {
                    return tier_d();
                }
                let mut out = Vec::new();
                // The result borrows from the first reference argument
                // (D-0011; D-0088 for a result that holds one), not from the
                // others (a key, a needle).
                for a in refs.iter().take(1) {
                    match self.inherited_facts_in(a, depth + 1, binder) {
                        Some(v) if v.is_empty() => return Some(Vec::new()),
                        Some(v) => out.extend(v),
                        // Static data (a literal where a view is expected):
                        // so is what the result borrows.
                        None => return None,
                    }
                }
                Some(out)
            }
            ExprKind::If(_, then_b, else_e) => {
                // A branch that does not complete (or holds nothing of this
                // frame) adds nothing; the facts are certain when one branch
                // gives them all, unknown when branches differ.
                let mut parts: Vec<Vec<(DerivKey, Tri)>> = Vec::new();
                for t in then_b.tail.iter().map(|b| &**b).chain(else_e.iter().map(|b| &**b)) {
                    match self.inherited_facts_in(t, depth + 1, binder) {
                        Some(v) if v.is_empty() => return Some(Vec::new()),
                        Some(v) => parts.push(v),
                        None => {}
                    }
                }
                let single = parts.len() == 1;
                Some(parts.into_iter().flatten().map(|(k, t)| (k, if single { t } else { Tri::U })).collect())
            }
            ExprKind::Block(b) => b.tail.as_ref().and_then(|t| self.inherited_facts_in(t, depth + 1, binder)),
            ExprKind::Match(scrut, arms) => {
                let mut out: Vec<Vec<(DerivKey, Tri)>> = Vec::new();
                for a in arms {
                    // An arm whose result is its binder gives what the
                    // scrutinee holds.
                    let binder_result = match (&a.binder, &strip_parens_tc(&a.body).kind) {
                        (Some(b), ExprKind::Path(segs, _)) => segs.len() == 1 && &segs[0] == b,
                        (Some(b), ExprKind::Block(blk)) => blk.stmts.is_empty() && blk.tail.as_ref().map_or(false, |t| matches!(&strip_parens_tc(t).kind, ExprKind::Path(segs, _) if segs.len() == 1 && &segs[0] == b)),
                        _ => false,
                    };
                    let got = if binder_result {
                        self.inherited_facts_in(scrut, depth + 1, binder)
                    } else {
                        self.inherited_facts_in(&a.body, depth + 1, a.binder.as_deref().map(|b| (b, &**scrut)).or(binder))
                    };
                    match got {
                        Some(v) if v.is_empty() => return Some(Vec::new()),
                        Some(v) => out.push(v),
                        None => {}
                    }
                }
                let single = out.len() == 1;
                Some(out.into_iter().flatten().map(|(k, t)| (k, if single { t } else { Tri::U })).collect())
            }
            // A branch that does not complete gives nothing.
            ExprKind::Return(_) | ExprKind::Break | ExprKind::Continue => None,
            ExprKind::StrLit(_) => None,
            // An element read out of a container of references, or a
            // reference out of a struct's field (a value, not a binding of
            // this frame): a stored reference, tier D.
            ExprKind::Index(..) => tier_d(),
            _ => Some(Vec::new()),
        }
    }

    // D-0281 §7: a type that holds a reference without being one (a view,
    // an `Option`/`Result` of a reference, a closure): an aggregate kept in
    // data, tier D, whose conflicts are checked when the program runs.
    fn tier_d_type(&self, t: &Type) -> bool {
        let frame_ref = match t {
            Type::Ref(..) | Type::Slice(..) | Type::Closure(_) | Type::Fn(..) => true,
            // D-0287: `foreach`'s holder of a view's parts is a view of
            // the same text.
            Type::Named(n, a) if n == "std::StringView" || n == "std::Parts" => a.is_empty(),
            Type::Named(n, a) if n == "std::Option" || n == "std::Result" => a.first().map_or(false, |x| matches!(x, Type::Ref(..) | Type::Slice(..)) || matches!(x, Type::Named(m, _) if m == "std::StringView")),
            _ => false,
        };
        !frame_ref && holds_ref(self.items, t)
    }

    // The binding whose path a reference binding carries: itself, or what
    // it was copied from.
    fn path_id(&self, r: &str) -> String {
        let mut cur = r.to_string();
        for _ in 0..64 {
            match self.ref_copies.get(&cur) {
                Some(n) if *n != cur => cur = n.clone(),
                _ => break,
            }
        }
        cur
    }

    // `set_deriv` with facts of any truth value (inherited facts).
    fn set_deriv_tri(&mut self, r: &str, facts: Vec<(DerivKey, Tri)>) {
        if self.flow.unreachable {
            return;
        }
        self.flow.deriv.remove(r);
        self.flow.clear_rooted(&referent_root(r));
        if !facts.is_empty() {
            let mut m: HashMap<DerivKey, Tri> = HashMap::default();
            for (k, v) in facts {
                let e = m.entry(k).or_insert(v);
                if *e != v {
                    *e = Tri::U;
                }
            }
            self.flow.deriv.insert(r.to_string(), m);
        }
    }

    // D-0282: at a binding `set_deriv` gave no fact: inherit,
    // or record the binding as of unknown (or static) origin.
    fn tier_inherit(&mut self, r: &str, e: Option<&Expr>) -> Result<(), String> {
        // The facts are made on every pass over a loop's body, the report
        // only on the last: a fact made in the body reaches the loop's
        // head and exit (`check_while`'s fixed point) only so.
        if !tier_report::on() || self.flow.unreachable || self.flow.deriv.contains_key(r) {
            return Ok(());
        }
        let report = self.report;
        if let Some(ExprKind::Path(segs, _)) = e.map(|e| &strip_parens_tc(e).kind) {
            let is_ref = |t: Option<Type>| matches!(t, Some(Type::Ref(..) | Type::Slice(..)));
            if segs.len() == 1 && segs[0] != r && is_ref(self.lookup_local(&segs[0])) && is_ref(self.lookup_local(r)) {
                let id = self.path_id(&segs[0]);
                self.ref_copies.insert(r.to_string(), id);
            }
        }
        let line = self.current_line;
        let ty = self.lookup_local(r).map(|t| t.to_string()).unwrap_or_default();
        let kind = e.map(tier_report::kind_of).unwrap_or_else(|| "binder".to_string());
        // A binding whose type is an aggregate holding a reference (a
        // struct, an array, an enum other than `Option`/`Result` of one)
        // is tier D by its type (D-0281 §7), whatever it was made from.
        if let Some(t) = self.lookup_local(r) {
            if self.tier_d_type(&t) {
                if report {
                    tier_report::with(|rep| {
                        rep.tier_d += 1;
                        rep.events.push(format!("D\t{}\t{}\t{}\t{}", line, r, ty, kind));
                    });
                    tier_report::left(format!("line {}: `{}` keeps references in data (tier D)", line, r));
                }
                return Ok(());
            }
        }
        // `r = pass(r)`: what the old `r` reached is not a fact rooted at
        // the new `r`'s own referent (the old facts were cleared when `r`
        // was assigned, so `r` here is known only as `*r`).
        let own = referent_root(r);
        let inherited = e.map(|e| self.inherited_facts(e, 0).map(|v| v.into_iter().filter(|(k, _)| k.root != own).collect::<Vec<_>>()));
        match inherited {
            Some(None) if !report => {}
            Some(None) => tier_report::with(|rep| rep.events.push(format!("S\t{}\t{}\t{}\t{}", line, r, ty, kind))),
            Some(Some(v)) if v.iter().any(|(k, _)| k.root == "<tier D>") => {
                if !report {
                    return Ok(());
                }
                tier_report::with(|rep| {
                    rep.tier_d += 1;
                    rep.events.push(format!("D\t{}\t{}\t{}\t{}", line, r, ty, kind));
                });
                tier_report::left(format!("line {}: `{}` was read out of data that keeps references (tier D)", line, r));
            }
            Some(Some(v)) if v.iter().any(|(k, _)| k.root == "<temporary>") => {
                if tier_report::live_after(r, self.current_line, self.current_col) {
                    let msg = format!("`{}` holds a view of a temporary that ends with this statement, and is used later (D-0135)", r);
                    let diag = format!("{}{}", crate::modres::named("diag.destroy-while-aliased", msg), crate::diagnostics::at(self.current_line, self.current_col));
                    self.refute(&diag)?;
                }
            }
            Some(Some(v)) if !v.is_empty() => {
                if report {
                    tier_report::with(|rep| {
                        rep.inherited += 1;
                    });
                }
                self.set_deriv_tri(r, v);
            }
            _ if !report => {}
            _ => {
                tier_report::with(|rep| {
                    rep.unknown_origin += 1;
                    rep.events.push(format!("O\t{}\t{}\t{}\t{}", line, r, ty, kind));
                });
                tier_report::left(format!("line {}: what `{}` borrows is not known before the run", line, r));
            }
        }
        Ok(())
    }

    // `auto r = e;` / `r = e;` for a reference-typed `r`: a visible
    // borrow (or elided call on one) sets that one `deriv` fact and
    // clears the others; anything else clears them all.
    fn set_deriv(&mut self, r: &str, facts: Vec<DerivKey>) {
        if self.flow.unreachable {
            return;
        }
        self.ref_copies.remove(r);

        self.flow.deriv.remove(r);
        // What was borrowed through `r`'s old referent is not borrowed
        // through its new one (D-0071).
        self.flow.clear_rooted(&referent_root(r));
        if !facts.is_empty() {
            self.flow.deriv.insert(r.to_string(), facts.into_iter().map(|k| (k, Tri::T)).collect());
        }
    }

    fn check_block(&mut self, b: &Block, expected: Option<&Type>) -> TResult {
        let last_stmt_ty = self.check_block_stmts(b)?;
        match &b.tail {
            Some(e) => self.check_store_operand(e, expected),
            // `[T-Block]` (spec/12 §5): no trailing expression -> `unit`,
            // *except* a block whose last statement is an expression
            // statement of type `never` is itself `never` (an `if`/`else
            // if` chain's final `else { return ...; }` arm, for example).
            None => Ok(Some(match last_stmt_ty {
                Some(Type::Never) => Type::Never,
                _ => Type::Void,
            })),
        }
    }

    // D-0111 `[Ref-Binding-Last-Use]`: after statement `s`, each of
    // `names` (a local reference, slice or `StringView` binding of this
    // block, mentioned by no later statement) stops holding its path: its
    // `deriv` facts end. Not while another binding's facts are rooted in
    // it (a reference borrowed through it, or one to it). The statement
    // is recorded for the runtimes, which end the binding there.
    // A block's statements, each checked in turn; after each, the local
    // reference bindings whose last use it was end (D-0111). The last
    // statement's type.
    fn check_block_stmts(&mut self, b: &Block) -> Result<Option<Type>, String> {
        let mut last_stmt_ty: Option<Type> = None;
        let (plan, mentions, tail) = early_end_plan(b);
        let mut waiting: Vec<(String, usize)> = Vec::new();
        for (i, s) in b.stmts.iter().enumerate() {
            last_stmt_ty = self.check_stmt(s)?;
            waiting.extend(plan[i].iter().cloned());
            if !waiting.is_empty() {
                let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut waiting).into_iter().partition(|(n, d)| !self.ref_end_blocked(b, n, *d, i, &mentions, &tail));
                waiting = later;
                let now: Vec<String> = now.into_iter().map(|(n, _)| n).collect();
                if !now.is_empty() {
                    self.end_ref_bindings(s, &now);
                }
            }
        }
        Ok(last_stmt_ty)
    }

    // Whether `n` (declared by statement `d`) must outlive statement `i`
    // although no later statement mentions it: a binding that holds
    // references, shares a statement with `n` since `n`'s declaration (so
    // may hold a reference derived from it: `parts = split(v, …)`,
    // `Vec::push(&mut rs, r)`), and is mentioned later.
    fn ref_end_blocked(&self, b: &Block, n: &str, d: usize, i: usize, mentions: &[HashSet<String>], tail: &HashSet<String>) -> bool {
        let later = |m: &str| tail.contains(m) || mentions[i + 1..].iter().any(|ms| ms.contains(m));
        (d..=i).filter(|&j| j == d || mentions[j].contains(n)).any(|j| {
            let declared = match &b.stmts[j] {
                Stmt::Let { name, .. } => Some(name.as_str()),
                _ => None,
            };
            mentions[j].iter().map(|m| m.as_str()).chain(declared).any(|m| {
                m != n
                    && self.lookup_local(m).map_or(false, |t| {
                        may_hold_ref(self.items, &t, 0) && (declared == Some(m) || !inert_ref(self.items, &t) || stmt_rebinds(&b.stmts[j], m))
                    })
                    && later(m)
            })
        })
    }

    fn end_ref_bindings(&mut self, s: &Stmt, names: &[String]) {
        if self.flow.unreachable {
            return;
        }
        let mut ended = Vec::new();
        for n in names {
            let is_ref = self.lookup_local(n).map_or(false, |t| ends_at_last_use(self.items, &t));
            if !is_ref {
                continue;
            }
            let rr = referent_root(n);
            let rooted_here = self.flow.deriv.iter().any(|(h, fs)| h != n && fs.keys().any(|k| k.root == *n || k.root == rr));
            if rooted_here {
                continue;
            }
            self.flow.deriv.remove(n);
            ended.push(n.clone());
        }
        if !ended.is_empty() {
            let mut t = self.items.early_ends.lock().unwrap();
            let e = t.entry(s as *const Stmt as usize).or_default();
            for n in ended {
                if !e.contains(&n) {
                    e.push(n);
                }
            }
            self.items.any_early_ends.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // A nested block opens a scope: the bindings it declares end at its
    // exit (`[Block-Exit]`), for both `gamma` and the flow facts.
    fn check_nested_block(&mut self, b: &Block, expected: Option<&Type>) -> TResult {
        let saved_gamma = self.gamma.clone();
        self.enter_scope();
        let r = self.check_block(b, expected);
        let r = match r {
            Ok(t) => self.check_block_exit().map(|_| t),
            e => e,
        };
        self.exit_scope();
        self.gamma = saved_gamma;
        r
    }

    // `[Block-Exit]` destroys what the block declared. A binding declared
    // outside it that holds a reference to one of those (a `deriv` fact
    // rooted there) conflicts with that destroy: when it surely does and
    // is used later, an error (D-0282); when it may, the run decides.
    fn check_block_exit(&mut self) -> Result<(), String> {
        if !tier_report::on() || !self.report || self.flow.unreachable {
            return Ok(());
        }
        let Some(sc) = self.scopes.last() else { return Ok(()) };
        let dying: HashSet<&str> = sc.iter().map(|e| e.name.as_str()).collect();
        let mut held: Vec<(String, String, Tri)> = Vec::new();
        for (r, facts) in &self.flow.deriv {
            if dying.contains(r.as_str()) {
                continue;
            }
            for (k, t) in facts {
                if dying.contains(k.root.as_str()) && *t != Tri::F {
                    held.push((r.clone(), k.root.clone(), *t));
                }
            }
        }
        held.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        let (line, col) = (self.current_line, self.current_col);
        for (r, x, t) in held {
            if t == Tri::T && tier_report::live_after(&r, line, col) {
                let msg = format!("`{}` is destroyed at the end of its block while `{}` still refers to it, and `{}` is used later", shown_name(&x), shown_name(&r), shown_name(&r));
                let diag = format!("{}{}", crate::modres::named("diag.destroy-while-aliased", msg), crate::diagnostics::at(line, col));
                return self.refute(&diag);
            }
            tier_report::left(format!("line {}: `{}` may refer to `{}` when its block ends", line, shown_name(&r), shown_name(&x)));
        }
        Ok(())
    }

    // Returns the statement's own expression type for `Stmt::Expr`/
    // `Stmt::BlockLike` (needed by `check_block` for spec/12 `[T-Block]`'s
    // "a block whose last statement is an expression statement of type
    // `never` is itself `never`" clause); `None` for a declaration.
    // Thin wrapper (see `Body::current_line`'s doc comment): tags an
    // untagged fault raised directly in `check_stmt_inner` (not via a
    // `check_expr` call, which already tags itself) with the last
    // expression line this checker actually looked at.
    fn check_stmt(&mut self, s: &Stmt) -> Result<Option<Type>, String> {
        match self.check_stmt_inner(s) {
            Err(d) if !d.contains('@') => Err(format!("{d}{}", crate::diagnostics::at(self.current_line, self.current_col))),
            other => other,
        }
    }

    fn check_stmt_inner(&mut self, s: &Stmt) -> Result<Option<Type>, String> {
        match s {
            Stmt::Let { ty, name, init, .. } => {
                // `foreach (x in &f())` (D-0042): the hidden `$c` binding
                // would borrow a temporary. Name the form that iterates one.
                if name.starts_with("$c") {
                    if let Some(ExprKind::Borrow(_, inner)) = init.as_ref().map(|e| &e.kind) {
                        if is_temp_value(inner) {
                            return Err(crate::modres::named(
                                "diag.borrow-of-non-place",
                                "`foreach (x in &e)` borrows only a place, and this is a value: a call's result, a literal, or a constant (which stands for its value); iterate it by value, `foreach (x in e)`, or bind it first".to_string(),
                            ));
                        }
                    }
                }
                let expected = ty.as_ref().map(|t| apply_subst(t, &self.subst));
                let ity = match init {
                    Some(e) => self.check_store_operand(e, expected.as_ref())?,
                    None => None,
                };
                if let (Some(exp), Some(actual)) = (&expected, &ity) {
                    if !has_unresolved_marker(exp) && !has_unresolved_marker(actual) && !ty_compat(exp, actual) {
                        return Err(mismatch(format!("`{}` is declared `{}`, but its initializer is `{}`{}", name, Type::shown_apart(exp, &actual).0, Type::shown_apart(exp, &actual).1, conversion_hint(exp, actual, init.as_deref()))));
                    }
                }
                // `[T-Item-Value-Uninferable]` (D-0093): a generic item
                // bound with nothing to fix its type parameters — a
                // binding is never polymorphic, as `auto v = Vec::new();`
                // is never unbounded (`[Generic-Call-Uninferable]`).
                if expected.is_none() && ity.is_none() {
                    if let Some(ExprKind::Path(segs, targs)) = init.as_ref().map(|e| &e.kind) {
                        if targs.is_empty() {
                            let n = segs.join("::");
                            if self.items.fns.get(&n).is_some_and(|f| !f.type_params.is_empty()) {
                                return Err(crate::modres::named(
                                    "diag.cannot-infer-type-parameter",
                                    format!("nothing here fixes the type parameters of `{0}` as a value: write `{0}<…>`, or declare the binding's `fn(...)` type", n),
                                ));
                            }
                        }
                    }
                    // A variant of a generic enum whose type arguments
                    // nothing fixes (`auto o = None;`, `auto r = Ok(5);`).
                    let variant = match init.as_ref().map(|e| &strip_parens_tc(e).kind) {
                        Some(ExprKind::Path(segs, _)) => Some(segs.clone()),
                        Some(ExprKind::Call(callee, _)) => match &callee.kind {
                            ExprKind::Path(segs, _) => Some(segs.clone()),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(segs) = variant {
                        if let Some(en) = self.variant_enum(&segs) {
                            if self.items.enums.get(&en).is_some_and(|e| !e.type_params.is_empty()) {
                                return Err(crate::modres::named(
                                    "diag.cannot-infer-type-parameter",
                                    format!("nothing here fixes the type of `{}`: declare the binding's type (`Option<i32> x = None;`, `Result<i32, str> r = Ok(5);`)", segs.last().unwrap()),
                                ));
                            }
                        }
                    }
                }
                let final_ty = expected.or(ity);
                match &final_ty {
                    Some(t) => {
                        self.gamma.insert(name.clone(), t.clone());
                    }
                    None => {
                        self.gamma.remove(name);
                    }
                }
                self.declare(name, init.is_some());
                if let Some(v) = init.as_ref().and_then(|e| literal_value(e)) {
                    if !self.flow.unreachable {
                        self.flow.lits.insert(name.clone(), v);
                    }
                }
                self.closure_moves.remove(name);
                self.closure_arity.remove(name);
                self.closure_self_writes.remove(name);
                self.closure_copied.remove(name);
                self.closure_owning.remove(name);
                if let Some(e) = init {
                    let here = *self.block_stack.last().unwrap_or(&0);
                    self.check_escape_into(here, e)?;
                    if let ExprKind::Closure { is_move, params, captures, body, .. } = &e.kind {
                        self.closure_moves.insert(name.clone(), *is_move);
                        self.closure_arity.insert(name.clone(), params.len());
                        if *is_move && captures.iter().any(|c| crate::interp::closure_body_writes(body, c)) {
                            self.closure_self_writes.insert(name.clone());
                        }
                        if !*is_move || !captures.iter().any(|c| self.place_is_resource(c, &[])) {
                            self.closure_copied.insert(name.clone());
                        } else {
                            self.closure_owning.insert(name.clone());
                        }
                    }
                    // `auto d = c;` moves the closure, and its arity with it.
                    if let ExprKind::Path(segs, _) = &e.kind {
                        if let Some(n) = segs.first().filter(|_| segs.len() == 1).and_then(|c| self.closure_arity.get(c)).copied() {
                            self.closure_arity.insert(name.clone(), n);
                        }
                    }
                }
                if final_ty.as_ref().map_or(false, is_borrow_ty) {
                    let fact = init.as_ref().and_then(|e| self.deriv_facts_of(e));
                    let facts: Vec<DerivKey> = match fact {
                        Some(f) => vec![f],
                        None => init.as_deref().map(|e| self.contained_ref_facts(e)).unwrap_or_default(),
                    };
                    self.set_deriv(name, facts);
                    self.tier_inherit(name, init.as_deref())?;
                    let origin = init.as_ref().and_then(|e| self.referent_of(e));
                    self.ref_origin.insert(name.clone(), origin);
                } else if let Some(e) = init.as_deref().filter(|_| final_ty.as_ref().map_or(false, |t| holds_ref(self.items, t))) {
                    // D-0088: an `Option<ref<V>>` (or any value holding a
                    // reference) from a call borrows from its arguments.
                    let facts = self.contained_ref_facts(e);
                    self.set_deriv(name, facts);
                    self.tier_inherit(name, Some(e))?;
                } else if let Some(e) = init.as_ref().filter(|e| is_spawn_call(e)) {
                    let facts = self.spawn_facts_of(e);
                    self.set_deriv(name, facts);
                } else if let Some(e) = init.as_ref().filter(|e| is_borrowing_closure(e)) {
                    let facts = self.closure_facts_of(e);
                    self.set_deriv(name, facts);
                }
                Ok(None)
            }
            Stmt::Destructure { struct_name, fields, names, rest, init } => {
                // D-0118: a bitstruct is read field by field, not taken apart.
                if self.items.structs.get(struct_name).map_or(false, |s| s.bits.is_some()) {
                    return Err(mismatch(format!("`{}` is a `bitstruct`, which is not taken apart; read its fields (`x.f`) instead", struct_name.strip_prefix("std::").unwrap_or(struct_name))));
                }
                let sty = Type::Named(struct_name.clone(), Vec::new());
                let st = self.check_store_operand(init, Some(&sty)).unwrap_or(None);
                // CHG-0347: the parser takes `Name { f, … } = v;` by its
                // shape (spec/22 disambiguation (5)); a name that is no
                // struct is reported here.
                if !self.items.structs.contains_key(struct_name) {
                    let shown = struct_name.strip_prefix("std::").unwrap_or(struct_name);
                    if self.items.enums.contains_key(struct_name) {
                        return Err(mismatch(format!("`{}` is an enum; destructuring (`Name {{ f, … }} = v;`) takes a struct apart, and an enum's value is taken apart by `match`", shown)));
                    }
                    return Err(crate::modres::named("diag.unbound-name", format!("no struct named `{}` is declared here; destructuring (`Name {{ f, … }} = v;`) names the struct it takes apart", shown)));
                }
                // `[T-Let-Destructure]`: the value is a `struct_name` (the
                // check was missing: `coby` faulted at run time, `cobc`
                // stopped with an internal error).
                let shown = struct_name.strip_prefix("std::").unwrap_or(struct_name);
                let pattern = matches!(&init.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0].starts_with("__pattern"));
                match &st {
                    Some(t) if has_unresolved_marker(t) => {}
                    Some(Type::Named(n, _)) if n == struct_name => {}
                    Some(Type::Ref(inner, _)) if matches!(&**inner, Type::Named(n, _) if n == struct_name) => {
                        return Err(mismatch(if pattern {
                            format!("a struct pattern takes its `{}` by value (D-0224), and here the value is reached through a reference; bind it whole (`Some(q)`) and read its fields through it (`q.f`), or match the value itself rather than a reference to it", shown)
                        } else {
                            format!("destructuring takes a `{}` by value, and this is a reference to one; read its fields through it (`r.f`), or destructure the value itself", shown)
                        }));
                    }
                    Some(t) => return Err(mismatch(format!("{} names `{}`, but the value is a `{}`", if pattern { "the struct pattern" } else { "the destructuring" }, shown, t))),
                    None => {}
                }
                if let Some(sdecl) = self.items.structs.get(struct_name).cloned() {
                    if let Some(unknown) = names.iter().find(|n| !sdecl.fields.iter().any(|f| &&f.name == n)) {
                        return Err(mismatch(format!("`{}` has no field `{}`", shown, unknown)));
                    }
                    // A struct with its own destructor keeps its fields: the
                    // destructor would run on a struct they had left.
                    if self.items.fns.contains_key(&format!("{}::drop", struct_name)) {
                        return Err(crate::modres::named("diag.move-out-of-field", format!("`{}` has a destructor, which needs its fields, so it cannot be taken apart; move it whole, or `replace` a field", Type::Named(struct_name.clone(), Vec::new()))));
                    }
                    // `[Let-Destructure]` (D-0044): every field, each once — or
                    // some, then `..` (D-0104).
                    let mut named: Vec<&String> = names.iter().collect();
                    named.sort();
                    named.dedup();
                    if named.len() != names.len() {
                        return Err(mismatch(format!("a field of `{}` is named twice", struct_name)));
                    }
                    if !*rest && names.len() != sdecl.fields.len() {
                        let missing: Vec<&str> = sdecl.fields.iter().map(|f| f.name.as_str()).filter(|f| !names.iter().any(|n| n == f)).collect();
                        return Err(mismatch(format!("destructuring names every field of `{}`; `{}` {} missing — name {} or end with `..` for the rest (D-0104)", struct_name, missing.join("`, `"), if missing.len() == 1 { "is" } else { "are" }, if missing.len() == 1 { "it" } else { "them" })));
                    }
                    let mut binders: Vec<&String> = fields.iter().collect();
                    binders.sort();
                    binders.dedup();
                    if binders.len() != fields.len() {
                        return Err(mismatch("two fields are bound to one name".to_string()));
                    }
                    // The struct's own type arguments, from the value.
                    let targs: Vec<Type> = match &st {
                        Some(Type::Named(_, a)) => a.clone(),
                        _ => Vec::new(),
                    };
                    let tsub: HashMap<String, Type> = sdecl.type_params.iter().cloned().zip(targs).collect();
                    let decl_names: Vec<String> = sdecl.fields.iter().map(|f| f.name.clone()).collect();
                    for (field, binder) in crate::ast::destructure_pairs(names, fields, *rest, &decl_names) {
                        let Some(fd) = sdecl.fields.iter().find(|f| f.name == field) else {
                            return Err(mismatch(format!("`{}` has no field `{}`", struct_name, field)));
                        };
                        // A private field is not visible, named or not (`..`
                        // takes it apart too).
                        if !self.privileged && !field_visible(struct_name, fd.export, &self.module) {
                            return Err(crate::modres::named("diag.name-not-visible", format!("field `{}` of `{}` is not exported", field, struct_name)));
                        }
                        self.gamma.insert(binder.clone(), apply_subst(&apply_subst(&fd.ty, &tsub), &self.subst));
                        self.declare(&binder, true);
                    }
                } else {
                    for field in fields {
                        self.declare(field, true);
                    }
                }
                Ok(None)
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => {
                let t = self.check_expr(e, None)?;
                // `[Stmt-Result-Discarded]` (D-0099): an error it carries
                // would be lost without a word.
                if matches!(&t, Some(Type::Named(n, _)) if n == "std::Result") {
                    return Err(crate::modres::named(
                        "diag.result-discarded",
                        "this `Result` is discarded, so an error it carries would be lost: handle it (`?`, `match`), or discard it on purpose with `_ = …;`".to_string(),
                    ));
                }
                Ok(t)
            }
        }
    }

    fn lookup_local(&self, name: &str) -> Option<Type> {
        self.gamma.get(name).cloned()
    }

    // A binding in scope, whether or not this pass knows its type (a
    // closure's, a capture's), as `check_path` decides it.
    fn is_local(&self, name: &str) -> bool {
        self.lookup_local(name).is_some() || self.flow.is_tracked(name) || self.capture_names.contains(name)
    }

    // Thin wrapper (see `Body::current_line`'s doc comment): tags the
    // first untagged fault that passes through with *this* expression's
    // own line -- a no-op once a nested `check_expr` call has already
    // tagged it, so the innermost (most precise) line always wins.
    fn check_expr(&mut self, e: &Expr, expected: Option<&Type>) -> TResult {
        self.current_line = e.line;
        self.current_col = e.col;
        self.cur_temp_ok = std::mem::take(&mut self.arg_temp_ok);
        // D-0218: a field, element or `*r` read as a value (not a place
        // position, not the operand of `*`) through a shared reference.
        let exempt = std::mem::take(&mut self.read_exempt) || self.place_pos;
        if matches!(e.kind, ExprKind::Paren(_)) {
            self.read_exempt = exempt;
        }
        let value_read = !exempt && matches!(e.kind, ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_));
        let r = self.check_expr_inner(e, expected);
        let r = match r {
            Ok(Some(t)) if value_read => self.read_through_shared(e, t, expected),
            other => other,
        };
        match r {
            Err(d) if !d.contains('@') => Err(format!("{d}{}", crate::diagnostics::at(e.line, e.col))),
            other => other,
        }
    }

    // `[Read-Ref-Through-Shared]` and `[Read-Exclusive-Through-Shared]`
    // (spec/08, D-0218): what a shared reference reaches is read-only at
    // every depth. An exclusive reference or slice read out through one is
    // a shared one, formed by a shared borrow (the front end rewrites `e`
    // to `&*e`, or `&e[0..$]` for a slice, and checks again); a value that
    // holds an exclusive reference is not copied out through one.
    fn read_through_shared(&mut self, e: &Expr, t: Type, expected: Option<&Type>) -> TResult {
        if has_unresolved_marker(&t) || !self.crosses_shared(e) {
            return Ok(Some(t));
        }
        let slice = matches!(t, Type::Slice(..));
        match t {
            Type::Ref(inner, Mode::Exclusive) | Type::Slice(inner, Mode::Exclusive) => {
                if matches!(expected, Some(Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive))) {
                    return Err(crate::modres::named(
                        "diag.borrow-exceeds-source",
                        "this exclusive reference is read through a shared reference, so it is shared here (D-0218): what a shared reference reaches cannot be written, at any depth; pass the struct by `ref<_, exclusive>` to write through it".to_string(),
                    ));
                }
                if SILENT.with(|s| s.get() > 0) {
                    // `check_more` cannot rewrite the program any more.
                    return Err(crate::modres::named("diag.borrow-exceeds-source", "an exclusive reference read through a shared reference in a body checked after the program was built".to_string()));
                }
                note_rewrite(e, if slice { Rewrite::SharedSlice } else { Rewrite::SharedRef });
                Ok(Some(if slice { Type::Slice(inner, Mode::Shared) } else { Type::Ref(inner, Mode::Shared) }))
            }
            t if contains_fn(self.items, &t) => {
                // A `fn` type does not show its closure's captures: each
                // tool checks, where this is read or called, that none is
                // an exclusive reference (`[Read-Fn-Through-Shared]`).
                self.items.shared_fn_reads.lock().unwrap().insert(e as *const Expr as usize);
                self.items.any_shared_fn_reads.store(true, std::sync::atomic::Ordering::Relaxed);
                if holds_excl_ref(self.items, &t, 0) {
                    return Err(crate::modres::named(
                        "diag.borrow-exceeds-source",
                        format!("this copies a `{}`, which holds an exclusive reference, out through a shared reference (D-0218): the copy could write what the shared reference reaches; borrow it instead (`&e`), or pass the owner by `ref<_, exclusive>`", t),
                    ));
                }
                Ok(Some(t))
            }
            t if holds_excl_ref(self.items, &t, 0) => Err(crate::modres::named(
                "diag.borrow-exceeds-source",
                format!("this copies a `{}`, which holds an exclusive reference, out through a shared reference (D-0218): the copy could write what the shared reference reaches; borrow it instead (`&e`), or pass the owner by `ref<_, exclusive>`", t),
            )),
            t => Ok(Some(t)),
        }
    }

    // An operand whose type the rule fixes: an `if`/`while` condition
    // (`[T-If]`, `[T-While]`: `bool`), an index or a slice bound
    // (`[Index-Checked]`, `[Slice-Form]`: `usize`).
    fn check_expect(&mut self, e: &Expr, want: &Type) -> Result<(), String> {
        let t = self.check_expr(e, Some(want))?;
        match t {
            Some(t) if !has_unresolved_marker(&t) && !ty_compat(&t, want) => {
                // `if (x = 6)`: an assignment is `void`; `==` compares.
                let hint = if *want == Type::Bool && matches!(strip_parens_tc(e).kind, ExprKind::Assign(..)) { "; `=` assigns (its value is `void`) -- compare with `==`" } else { "" };
                Err(format!("{}{}", mismatch(format!("expected `{}`, found `{}`{}", want, t, hint)), crate::diagnostics::at(e.line, e.col)))
            }
            _ => Ok(()),
        }
    }

    fn check_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> TResult {
        let in_place = std::mem::replace(&mut self.place_pos, false);
        match &e.kind {
            ExprKind::IntLit(v, suffix) => {
                let at = e as *const Expr as usize;
                // D-0090: a literal marked by the generic pass, at a
                // floating-point instantiation of its `number` parameter.
                if suffix.is_none() && matches!(expected, Some(Type::F32 | Type::F64)) && self.items.number_literals.lock().unwrap().contains(&at) {
                    return Ok(expected.cloned());
                }
                let t = self.check_int_lit(*v, suffix.as_deref(), expected)?;
                if matches!(&t, Some(m) if is_marker(m)) {
                    self.items.number_literals.lock().unwrap().insert(at);
                }
                Ok(t)
            }
            ExprKind::FloatLit(v, suffix) => {
                let t = match suffix.as_deref() {
                    Some("f32") => Type::F32,
                    Some("f64") => Type::F64,
                    _ => match expected {
                        Some(Type::F32) => Type::F32,
                        Some(Type::F64) => Type::F64,
                        _ => Type::F64,
                    },
                };
                // `[Literal-Out-Of-Range]`: a literal whose value rounds
                // beyond the type's largest finite magnitude (`1e400`,
                // `1e39: f32`) is not a value of it.
                let inf = if t == Type::F32 { (*v as f32).is_infinite() } else { v.is_infinite() };
                if inf {
                    return Err(crate::modres::named("diag.literal-out-of-range", format!("this literal is beyond `{}`'s largest finite value", t)));
                }
                Ok(Some(t))
            }
            // `[T-Lit-Str]`: always `str`; takes no expected type.
            ExprKind::StrLit(_) => Ok(Some(Type::Str)),
            ExprKind::BoolLit(_) => Ok(Some(Type::Bool)),
            ExprKind::Unit => Ok(Some(Type::Void)),
            ExprKind::Paren(inner) => {
                // A parenthesized place is the same place: `(*r)[k]` and
                // `(*r).f` do not read `*r` whole.
                self.place_pos = in_place;
                self.check_expr(inner, expected)
            }
            ExprKind::Path(segs, targs) => {
                // D-0258: a static is never read or moved whole: it is
                // borrowed shared (`&NAME`) and changed by its operations.
                if !in_place && self.items.statics.contains_key(&segs.join("::")) {
                    return Err(crate::modres::named("diag.read-of-resource", format!("`{}` is a static: it is borrowed (`&{}`) and changed through its operations, never read or moved whole", segs.join("::"), segs.join("::"))));
                }
                let mut t = self.check_path(segs)?;
                if t.is_none() {
                    // `[T-Item-Generic]` (D-0093): a generic item as a
                    // value, instantiated by explicit type arguments or
                    // the expected fn type; `check_path` left it untyped.
                    t = self.generic_item_value(segs, targs, expected)?;
                }
                if !in_place {
                    self.flow_read(e)?;
                }
                Ok(t)
            }
            ExprKind::Unary(op, inner) => self.check_unary(*op, inner, expected),
            ExprKind::Binary(op, a, b) => self.check_binary(*op, a, b, e, expected),
            ExprKind::Borrow(m, inner) => {
                self.tier_elem_access(inner, m.clone())?;
                // D-0258 `[Ref-Form-Static]`: `&NAME` of a static is a shared
                // reference to one object with program extent; it never
                // ends, so it is never tracked. `&mut NAME` is refused: the
                // static is every thread's, changed only by its operations.
                if let ExprKind::Path(segs, _) = &strip_parens_tc(inner).kind {
                    if let Some(s) = self.items.statics.get(&segs.join("::")) {
                        if *m == Mode::Exclusive {
                            return Err(crate::modres::named("diag.write-through-shared", format!("`{}` is a static, shared by every thread: borrow it `&{}` and change it through its operations", s.name, s.name)));
                        }
                        return Ok(Some(Type::Ref(Box::new(s.ty.clone()), Mode::Shared)));
                    }
                }
                // D-0258 `[Ref-Form-Const]`: `&K` of a constant of plain data
                // written as a literal is a shared reference to one object
                // with program extent (a table with one address).
                if let ExprKind::Call(callee, args) = &strip_parens_tc(inner).kind {
                    if let (ExprKind::Path(segs, _), true) = (&callee.kind, args.is_empty()) {
                        if let Some(f) = self.items.fns.get(&segs.join("::")).filter(|f| f.is_const).cloned() {
                            let literal = f.body.tail.as_ref().map_or(false, |e| literal_tree(e));
                            if literal && self.plain_problem(&f.ret).is_none() {
                                if *m == Mode::Exclusive {
                                    return Err(crate::modres::named("diag.borrow-of-non-place", format!("`{}` is a constant: borrow it `&{}`; it cannot be written", f.name, f.name)));
                                }
                                self.items.const_addrs.lock().unwrap().insert(segs.join("::"));
                                return Ok(Some(Type::Ref(Box::new(f.ret.clone()), Mode::Shared)));
                            }
                        }
                    }
                }
                // D-0254: nor has a lane of a `simd` (C takes no vector
                // element's address).
                if let ExprKind::Index(base, _) = &strip_parens_tc(inner).kind {
                    let bt = self.static_place_type(base).map(|t| match t {
                        Type::Ref(t, _) | Type::Guard(t) => *t,
                        t => t,
                    });
                    if let Some(st @ Type::Simd(..)) = bt {
                        return Err(crate::modres::named(
                            "diag.borrow-of-non-place",
                            format!("a lane of a `{}` is not a place of its own: read it (`v[i]`), assign it (`v[i] = x`), or borrow the whole vector", st),
                        ));
                    }
                }
                // D-0118: a bit field has no address of its own.
                if let Some((_, _, sname)) = self.bitfield_of(inner) {
                    let fname = match &strip_parens_tc(inner).kind { ExprKind::Field(_, f) => f.clone(), _ => String::new() };
                    return Err(crate::modres::named(
                        "diag.borrow-of-non-place",
                        format!("field `{}` of `bitstruct {}` is a run of bits, not a place: read it (`x.{}`), assign it (`x.{} = v`), or borrow the whole `{}`", fname, sname.strip_prefix("std::").unwrap_or(&sname), fname, fname, sname.strip_prefix("std::").unwrap_or(&sname)),
                    ));
                }
                // D-0073: as an argument of a call whose result is not a
                // reference, a call's result or an aggregate literal is
                // borrowed as a temporary that ends with the statement.
                let temp_ok = std::mem::take(&mut self.cur_temp_ok);
                // D-0103: a field or element of such a temporary, as an
                // argument, lives as long as the temporary does.
                if temp_ok && !is_temp_value(inner) && matches!(borrow_operand_kind(inner, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)), OperandKind::Temporary) {
                    self.place_pos = true;
                    let it = self.check_expr(inner, None)?;
                    self.items.temp_borrows.lock().unwrap().insert(e as *const Expr as usize);
                    return Ok(it.map(|t| Type::Ref(Box::new(t), m.clone())));
                }
                // A payload-less variant (`&None`, `&Wall`) is a value, not a
                // place: as an argument it is borrowed as a temporary.
                let variant_value = matches!(&strip_parens_tc(inner).kind, ExprKind::Path(segs, _) if !self.is_local(&segs[0]) && self.variant_enum(segs).is_some());
                if variant_value && !temp_ok {
                    return Err(crate::modres::named(
                        "diag.borrow-of-non-place",
                        "a variant is a value, which can be borrowed only as an argument of a call whose result is not a reference; elsewhere, bind it first".to_string(),
                    ));
                }
                if temp_ok && (is_temp_value(inner) || variant_value) {
                    // The referent type the argument's parameter expects,
                    // for a literal (D-0084).
                    let pointee = match expected {
                        Some(Type::Ref(t, _)) if !has_unresolved_marker(t) => Some((**t).clone()),
                        _ => None,
                    };
                    let it = self.check_expr(inner, pointee.as_ref())?;
                    if matches!(&it, Some(Type::Ref(..))) {
                        return Err("diag.borrow-of-non-place".to_string());
                    }
                    self.items.temp_borrows.lock().unwrap().insert(e as *const Expr as usize);
                    return Ok(it.map(|t| Type::Ref(Box::new(t), m.clone())));
                }
                // `[Ref-Form-Not-Place]` / `[Ref-Form-Temporary]` (spec/09 §2).
                match borrow_operand_kind(inner, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)) {
                    OperandKind::NonPlace if is_temp_value(inner) => {
                        return Err(crate::modres::named(
                            "diag.borrow-of-non-place",
                            "a temporary can be borrowed only as an argument of a call whose result is not a reference; elsewhere, bind it first".to_string(),
                        ))
                    }
                    OperandKind::NonPlace => {
                        return Err(crate::modres::named(
                            "diag.borrow-of-non-place",
                            "`&` borrows a place (a binding, a field, an element, `*r`); this is a value, which can be borrowed only as an argument of a call whose result is not a reference; elsewhere, bind it first".to_string(),
                        ))
                    }
                    OperandKind::Temporary => return Err(crate::modres::named("diag.borrow-of-temporary", "this is part of a temporary (a call's result or a literal), which ends with the statement; bind the value first, then borrow from the binding".to_string())),
                    OperandKind::Place => {}
                }
                // `[Borrow-Exceeds-Source]` (spec/08): an exclusive borrow
                // through a shared reference.
                if *m == Mode::Exclusive && self.crosses_shared(inner) {
                    return Err("diag.borrow-exceeds-source".to_string());
                }
                self.place_pos = true;
                let it = self.check_expr(inner, None)?;
                self.flow_borrow(inner, m)?;
                let r = self.referent_of(e);
                self.borrow_referents.insert(e as *const Expr as usize, r);
                Ok(it.map(|t| Type::Ref(Box::new(t), m.clone())))
            }
            ExprKind::SliceOf(m, base, lo, hi) if self.view_base(base).is_some() => {
                // D-0053: `&s[lo .. hi]` on a `String` (a place, borrowed
                // shared as `&s` would be) or on a `StringView` (any value)
                // is a `StringView`; `$` is the length in bytes. Shared
                // only: writing through a view could break its UTF-8.
                let on_string = self.view_base(base) == Some(true);
                // D-0135: as an argument, a `String` that is a temporary (a
                // call's result) or part of one may be viewed, as D-0103
                // lets it be sliced; it lives to the statement's end.
                let temp_ok = std::mem::take(&mut self.cur_temp_ok);
                if *m == Mode::Exclusive {
                    return Err("diag.type-mismatch".to_string());
                }
                if on_string {
                    let through_ref = self.returns_ref(base);
                    match if through_ref { OperandKind::Place } else { borrow_operand_kind(base, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)) } {
                        k @ (OperandKind::NonPlace | OperandKind::Temporary) if temp_ok => {
                            self.items.temp_borrows.lock().unwrap().insert(e as *const Expr as usize);
                            self.place_pos = matches!(k, OperandKind::Temporary);
                            self.check_expr(base, None)?;
                        }
                        OperandKind::NonPlace => return Err(slice_of_value_msg()),
                        OperandKind::Temporary => return Err(crate::modres::named("diag.borrow-of-temporary", "this is part of a temporary (a call's result or a literal), which ends with the statement; bind the value first, then borrow from the binding".to_string())),
                        OperandKind::Place => {
                            self.place_pos = true;
                            self.check_expr(base, None)?;
                            self.flow_borrow(base, m)?;
                        }
                    }
                } else {
                    self.check_expr(base, None)?;
                }
                self.check_expect(lo, &Type::Int(IntTy::Usize))?;
                self.check_expect(hi, &Type::Int(IntTy::Usize))?;
                let op = if on_string { crate::interp::ViewOp::SliceString } else { crate::interp::ViewOp::SliceView };
                self.items.view_ops.lock().unwrap().insert(e as *const Expr as usize, op);
                Ok(Some(Type::Named("std::StringView".to_string(), vec![])))
            }
            ExprKind::SliceOf(m, base, lo, hi) => {
                // D-0254: a `simd` is a value, not a run of places.
                if let Some(st @ Type::Simd(..)) = self.static_place_type(base).map(|t| match t {
                    Type::Ref(t, _) | Type::Guard(t) => *t,
                    t => t,
                }) {
                    return Err(crate::modres::named(
                        "diag.borrow-of-non-place",
                        format!("a `{}` is not sliced: its lanes are not places of their own; read them with `v[i]`, or copy the vector into an array or a `Vec` with `simd_store`", st),
                    ));
                }
                // `[Slice-Form]` (D-0047): `&base[lo .. hi]` borrows `base`
                // as `&base` would, and is a slice of its elements. A base
                // that is a reference (`&f()[..]` for `f` returning one) is
                // the place it refers to, as `&f()[i]` is (D-0073).
                // D-0103: as an argument, a temporary (an array literal, a
                // call's result) may be sliced; it lives to the statement's end.
                let temp_ok = std::mem::take(&mut self.cur_temp_ok);
                let through_ref = self.returns_ref(base);
                match if through_ref { OperandKind::Place } else { borrow_operand_kind(base, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)) } {
                    OperandKind::NonPlace | OperandKind::Temporary if temp_ok => {
                        self.items.temp_borrows.lock().unwrap().insert(e as *const Expr as usize);
                    }
                    OperandKind::NonPlace => return Err(slice_of_value_msg()),
                    OperandKind::Temporary => return Err(crate::modres::named("diag.borrow-of-temporary", "this is part of a temporary (a call's result or a literal), which ends with the statement; bind the value first, then borrow from the binding".to_string())),
                    OperandKind::Place => {}
                }
                self.place_pos = true;
                let bt = self.check_expr(base, None)?;
                if *m == Mode::Exclusive
                    && (self.crosses_shared(base) || matches!(&bt, Some(Type::Ref(_, Mode::Shared)) | Some(Type::Slice(_, Mode::Shared))))
                {
                    return Err("diag.borrow-exceeds-source".to_string());
                }
                self.check_expect(lo, &Type::Int(IntTy::Usize))?;
                self.check_expect(hi, &Type::Int(IntTy::Usize))?;
                let elem = match &bt {
                    None => return Ok(None),
                    Some(t) if has_unresolved_marker(t) => return Ok(None),
                    Some(t) => match indexed_elem(t) {
                        Some(el) => el,
                        None => return Err("diag.type-mismatch".to_string()),
                    },
                };
                // Literal bounds on an array are checked now.
                let n = match &bt {
                    Some(Type::Array(_, n)) => Some(*n),
                    Some(Type::Ref(inner, _)) => match &**inner {
                        Type::Array(_, n) => Some(*n),
                        _ => None,
                    },
                    _ => None,
                };
                if let (Some(n), Some(a), Some(b)) = (n, literal_value(lo), literal_value(hi)) {
                    if a < 0 || b < a || (b as u128) > n {
                        return Err("diag.index-out-of-bounds".to_string());
                    }
                }
                // D-0070: the slice borrows its range of `base`.
                let src = match self.slice_source(base) {
                    Some((x, p)) if x.starts_with('*') && p.is_empty() => Some((x, p)),
                    _ => self.place_of(base),
                };
                if let Some((x, mut path)) = src {
                    path.push(range_elem(lo, hi));
                    self.flow_clash(&x, &path, m.clone(), "diag.aliasing-conflict")?;
                }
                Ok(Some(Type::Slice(Box::new(elem), m.clone())))
            }
            ExprKind::Dollar => Ok(Some(Type::Int(IntTy::Usize))),
            ExprKind::Deref(inner) => {
                // `*g` through a guard (`rule.conc.lock`) accesses the guarded
                // value in place: `g` itself, a resource, is not read as a
                // value (as `*r` reads the reference `r`); it must only be
                // initialized.
                let through_guard = matches!(self.static_place_type(inner), Some(Type::Guard(_)));
                if through_guard {
                    self.place_pos = true;
                    if let Some((x, _)) = self.place_of(inner) {
                        if self.flow.init.get(&x) != Some(&Tri::T) {
                            self.refute(&uninit_msg(&x))?;
                        }
                    }
                }
                self.read_exempt = true;
                let it = self.check_expr(inner, None)?;
                if matches!(&it, Some(t) if is_marker(t)) {
                    return Err(unbounded_msg("`*` (dereference)"));
                }
                if matches!(it, Some(Type::Rawptr(_))) && self.unsafe_depth == 0 {
                    return Err("diag.trusted-outside-unsafe".to_string());
                }
                let ty = match it {
                    Some(Type::Ref(t, _)) => Some(*t),
                    Some(Type::Guard(t)) => Some(*t),
                    Some(Type::Rawptr(t)) => Some(*t),
                    // `*e` of a value that is no reference, guard or raw
                    // pointer (`*3`): `[Deref]`'s premise fails statically.
                    Some(t) if !has_unresolved_marker(&t) && matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Array(..) | Type::Simd(..) | Type::Handle(_) | Type::Mutex(_) | Type::Void | Type::Named(..)) => {
                        return Err(mismatch(format!("`*` needs a reference, a guard or a raw pointer, but this is `{}`", t)));
                    }
                    _ => None,
                };
                // `*r` read in value position is a `[Read]` of the place `*r`
                // (D-0071): the discharge table refutes it when an overlapping
                // exclusive `deriv(·, *r.·)` is T, as it does for `r.f` (D-0219:
                // found by the model; only writes through `*r` were refuted).
                if !in_place {
                    self.flow_read(e)?;
                }
                Ok(ty)
            }
            ExprKind::Field(base, name) => {
                self.place_pos = true;
                let t = self.check_field(base, name)?;
                if !in_place {
                    self.flow_read(e)?;
                }
                Ok(t)
            }
            ExprKind::Index(base, idx) => {
                self.tier_elem_access(e, Mode::Shared)?;
                self.place_pos = true;
                let bt = self.check_expr(base, None)?;
                // D-0064: `g[i]` through a guard is `(*g)[i]`.
                let bt = match bt {
                    Some(Type::Guard(t)) => Some(*t),
                    other => other,
                };
                if matches!(&bt, Some(t) if is_marker(t)) {
                    return Err(unbounded_msg("indexing"));
                }
                if matches!(bt, Some(Type::Ref(..))) {
                    // `[Field-Access-Auto-Deref]`'s index analogue: the
                    // reference itself is read.
                    self.flow_read(base)?;
                }
                self.check_expect(idx, &Type::Int(IntTy::Usize))?;
                let arr = match &bt {
                    Some(Type::Array(t, n) | Type::Simd(t, n)) => Some(((**t).clone(), *n)),
                    Some(Type::Ref(inner, _)) => match &**inner {
                        Type::Array(t, n) | Type::Simd(t, n) => Some(((**t).clone(), *n)),
                        _ => None,
                    },
                    _ => None,
                };
                // D-0047: a `Vec` or a slice is indexed too (its length is
                // known only when the program runs).
                if arr.is_none() {
                    if let Some(t) = bt.as_ref().and_then(indexed_elem) {
                        // An element is read in place or copied; a resource
                        // cannot be moved out of a `Vec` or a slice.
                        if !in_place && is_resource_ty(self.items, &t) {
                            return Err(crate::modres::named(
                                "diag.move-out-of-field",
                                format!("this would move a `{}` out of an element, leaving a hole; take it out with `Vec::remove`/`Vec::pop`, exchange it with `replace(&mut v[i], x)`, copy it (`{}::clone(&v[i])`), or borrow it (`&v[i]`)", t, match &t { Type::Named(n, _) => n.strip_prefix("std::").unwrap_or(n).to_string(), _ => "T".to_string() }),
                            ));
                        }
                        if !in_place {
                            self.flow_read(e)?;
                        }
                        return Ok(Some(t));
                    }
                }
                // `[Index-Checked]`'s bound, `discharge: static,
                // disposition: rejected` when the index is a literal
                // (spec/14 §6's value-range row).
                if let (Some((_, n)), Some(v)) = (&arr, self.known_value(idx)) {
                    if v < 0 || (v as u128) >= *n {
                        return Err("diag.index-out-of-bounds".to_string());
                    }
                }
                // `[T-Index]`: an array, a `Vec` or a slice (or a reference
                // to one); anything else of a known type is not indexed.
                if arr.is_none() {
                    if let Some(t) = &bt {
                        if !has_unresolved_marker(t) && !matches!(t, Type::Never) {
                            return Err(mismatch(match t {
                                Type::Named(n, _) if n == "std::String" => "a `String` is not indexed; its bytes are `String::as_bytes(&s)[i]`, and text is reached through `String::view`".to_string(),
                                Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n == "std::String") => "a `String` is not indexed; its bytes are `String::as_bytes(s)[i]`, and text is reached through `String::view`".to_string(),
                                other => format!("a `{}` cannot be indexed; arrays, `Vec`s and slices can", other),
                            }));
                        }
                    }
                }
                if !in_place {
                    self.flow_read(e)?;
                }
                Ok(arr.map(|(t, _)| t))
            }
            ExprKind::Call(callee, args) => {
                let t = self.check_call(callee, args, expected)?;
                if matches!(t, Some(Type::Ref(..))) {
                    let r = self.referent_of(e);
                    self.borrow_referents.insert(e as *const Expr as usize, r);
                }
                Ok(t)
            }
            ExprKind::StructLit(segs, targs, fields) => self.check_struct_lit(segs, targs, fields, expected),
            ExprKind::ArrayLit(items) => {
                let mut ety = None;
                // D-0254: an array literal where a `simd` is expected is one,
                // lane by lane.
                let simd = match expected {
                    Some(Type::Simd(t, n)) => Some(((**t).clone(), *n)),
                    _ => None,
                };
                let elem_expected = match expected {
                    Some(Type::Array(t, _) | Type::Simd(t, _)) => Some((**t).clone()),
                    _ => None,
                };
                let mut unknown = false;
                for it in items {
                    let want = elem_expected.clone().or(ety.clone());
                    let t = self.check_store_operand(it, want.as_ref())?;
                    // `[T-Array]`: every element has one type.
                    if let (Some(t), Some(w)) = (&t, &want) {
                        if !has_unresolved_marker(t) && !has_unresolved_marker(w) && *t != Type::Never && *w != Type::Never && !ty_compat(t, w) {
                            return Err(mismatch(format!("an array's elements have one type: `{}`, and this one is `{}`", w, t)));
                        }
                    }
                    if t.is_none() {
                        unknown = true;
                    }
                    if ety.is_none() {
                        ety = t;
                    }
                }
                if let Some((t, n)) = simd {
                    if items.len() as u128 != n {
                        return Err(mismatch(format!("a `simd<{}, {}>` is written with {} lanes, and this literal has {}", t, n, n, items.len())));
                    }
                    return Ok(Some(Type::Simd(Box::new(t), n)));
                }
                // An element this pass cannot type (`join(h)`) takes the
                // expected element type; with none, the array's type is
                // unknown too, not `i32`'s.
                match (ety, elem_expected) {
                    (Some(t), _) => Ok(Some(Type::Array(Box::new(t), items.len() as u128))),
                    (None, Some(t)) => Ok(Some(Type::Array(Box::new(t), items.len() as u128))),
                    (None, None) if unknown => Ok(None),
                    (None, None) => Ok(Some(Type::Array(Box::new(Type::Int(IntTy::I32)), items.len() as u128))),
                }
            }
            // `[v; N]` (D-0072): v is read once and copied into every
            // element, so its type is plain: no resource, no reference.
            ExprKind::ArrayRepeat(it, n) => {
                let elem_expected = match expected {
                    Some(Type::Array(t, _) | Type::Simd(t, _)) => Some((**t).clone()),
                    _ => None,
                };
                // D-0254: `[x; N]` where a `simd<τ, N>` is expected: every lane `x`.
                if let Some(Type::Simd(lt, ln)) = expected {
                    if *ln != *n {
                        return Err(mismatch(format!("a `simd<{}, {}>` has {} lanes, and this is `[v; {}]`", lt, ln, ln, n)));
                    }
                    self.check_expect(it, lt)?;
                    return Ok(Some(Type::Simd(lt.clone(), *ln)));
                }
                let t = self.check_expr(it, elem_expected.as_ref())?;
                if let Some(t) = &t {
                    if !has_unresolved_marker(t) {
                        let what = if is_resource_ty(self.items, t) {
                            Some("is a resource")
                        } else if holds_ref(self.items, t) {
                            Some("is or holds a reference")
                        } else if contains_fn(self.items, t) {
                            Some("is or holds a function value")
                        } else {
                            None
                        };
                        if let Some(what) = what {
                            return Err(mismatch(format!("`[v; {}]` copies `v` into every element, but `{}` {}", n, t, what)));
                        }
                    }
                }
                Ok(t.or(elem_expected).map(|t| Type::Array(Box::new(t), *n)))
            }
            ExprKind::Assign(lhs, rhs) => {
                self.tier_assign_rules(lhs, rhs)?;
                // `[Write-Not-Exclusive]` (spec/08): a write through a
                // shared reference, direct (`*r = ..`) or by auto-deref
                // (`r.f = ..`).
                if self.crosses_shared(lhs) {
                    return Err(crate::modres::named("diag.write-through-shared", "this writes through a shared reference (`ref<T, shared>`, `&x`); writing needs an exclusive one (`ref<T, exclusive>`, `&mut x`)".to_string()));
                }
                // Target place first, then the value (D-0007); the
                // write's own checks and transfer come last.
                // `spec/22`: the left side is a place. A value there (a
                // literal, a call, a constant's use) is a static error,
                // not a run-time one.
                fn is_place(e: &Expr) -> bool {
                    match &e.kind {
                        ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) => true,
                        ExprKind::Paren(x) => is_place(x),
                        _ => false,
                    }
                }
                if !is_place(lhs) {
                    return Err(mismatch("the left side of `=` is a value -- a literal, a call's result, or a constant (which stands for its value) -- not a variable, a field, an element or `*r`, so it cannot be assigned".to_string()));
                }
                // A function (an item, not a binding) is a value too.
                if let ExprKind::Path(segs, _) = &strip_parens_tc(lhs).kind {
                    let n = segs.join("::");
                    if !(segs.len() == 1 && self.is_local(&segs[0])) && (self.items.fns.contains_key(&n) || self.items.externs.contains_key(&n)) {
                        return Err(mismatch(format!("`{}` is a function, not a variable, so it cannot be assigned; a variable of type `fn(…)` can be", n)));
                    }
                }
                // D-0033 (1): `x = e` for a whole binding `x` whose value
                // was moved away or destroyed gives it a new one, as the
                // first write to a `[Let-Uninit]` binding does.
                // A captured name is not tracked in the closure's body (it is
                // `*self.f_i`), so writing it starts no tracking there: its
                // reads after the write stay unrefuted, as before it.
                let reinit = match &lhs.kind {
                    ExprKind::Path(segs, _) if segs.len() == 1 && self.is_local(&segs[0]) && (self.flow.is_tracked(&segs[0]) || !self.capture_names.contains(&segs[0])) => Some(segs[0].clone()),
                    _ => None,
                };
                self.place_pos = true;
                self.reinit_target = reinit.clone();
                let lt = self.check_expr(lhs, None);
                // D-0118: a write into a bit field must fit its width.
                if let Some((w, _, sname)) = self.bitfield_of(lhs) {
                    let fname = match &strip_parens_tc(lhs).kind { ExprKind::Field(_, f) => f.clone(), _ => String::new() };
                    self.check_bitfield_literal(rhs, w, &format!("field `{}` of `{}`", fname, sname.strip_prefix("std::").unwrap_or(&sname)))?;
                }
                self.reinit_target = None;
                let lt = lt?;
                let held = match &strip_parens_tc(lhs).kind {
                    ExprKind::Deref(t) => match &strip_parens_tc(t).kind {
                        ExprKind::Path(segs, _) if segs.len() == 1 && segs[0].starts_with("__compound") => Some(segs[0].clone()),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(t) = &held {
                    self.compound_held.push(t.clone());
                }
                let rt = self.check_store_operand(rhs, lt.as_ref());
                if held.is_some() {
                    self.compound_held.pop();
                }
                let rt = rt?;
                // D-0281 step 4c: a write to an element of a `Vec` is an
                // exclusive access, against the live holders, as a borrow
                // of it is (D-0282).
                self.tier_elem_access(lhs, Mode::Exclusive)?;
                if let (Some(l), Some(r)) = (&lt, &rt) {
                    if !has_unresolved_marker(l) && !has_unresolved_marker(r) && !ty_compat(l, r) {
                        // `a = b = 3`: C's chained assignment.
                        if matches!(strip_parens_tc(rhs).kind, ExprKind::Assign(..)) {
                            return Err(mismatch("an assignment has no value, so `a = b = 3` does not assign `a`: write `b = 3; a = b;`".to_string()));
                        }
                        return Err(mismatch(format!("the place is `{}`, but the value assigned is `{}`", l, r)));
                    }
                }
                self.flow_write(lhs)?;
                if let Some(x) = &reinit {
                    if !self.flow.unreachable {
                        self.flow.valid.insert(x.clone(), Tri::T);
                    }
                }
                if let Some((root, _)) = self.place_of(lhs) {
                    if let Some(dest) = self.decl_block.get(&root).copied() {
                        self.check_escape_into(dest, rhs)?;
                    }
                }
                if let Some(name) = root_local_name(lhs) {
                    if self.flow.is_tracked(&name) && lt.as_ref().map_or(false, is_borrow_ty) {
                        let fact = self.deriv_facts_of(rhs);
                        let facts: Vec<DerivKey> = match fact {
                            Some(f) => vec![f],
                            None => self.contained_ref_facts(rhs),
                        };
                        self.set_deriv(&name, facts);
                        self.tier_inherit(&name, Some(rhs))?;
                        let origin = self.referent_of(rhs);
                        self.ref_origin.insert(name, origin);
                    } else if self.flow.is_tracked(&name) && is_spawn_call(rhs) && matches!(strip_parens_tc(lhs).kind, ExprKind::Path(..)) {
                        let facts = self.spawn_facts_of(rhs);
                        self.set_deriv(&name, facts);
                    } else if self.flow.is_tracked(&name) && is_borrowing_closure(rhs) && matches!(strip_parens_tc(lhs).kind, ExprKind::Path(..)) {
                        let facts = self.closure_facts_of(rhs);
                        self.set_deriv(&name, facts);
                    }
                }
                Ok(Some(Type::Void))
            }
            ExprKind::Propagate(inner) => {
                // D-0273: a whole binding that holds a resource is used by
                // value, as an argument is: `x?` moves it.
                let whole_resource = matches!(self.place_of(inner), Some((x, p)) if p.is_empty() && self.place_is_resource(&x, &p));
                let it = if whole_resource { self.check_store_operand(inner, None)? } else { self.check_expr(inner, None)? };
                match &it {
                    Some(Type::Named(n, args)) if n == "std::Result" => {
                        // [Propagate-Err-Mismatch] (spec/18): the
                        // enclosing function's own return type must be
                        // Result<_, E'> with E' exactly this Result's own
                        // E -- checked only once E (args[1]) is actually
                        // known, never against a still-unresolved shape.
                        if let Some(err_ty) = args.get(1) {
                            let ret_is_matching_result = matches!(
                                &self.ret,
                                Type::Named(rn, rargs) if rn == "std::Result" && rargs.get(1) == Some(err_ty)
                            );
                            if !ret_is_matching_result {
                                return Err("diag.propagate-outside-fallible-context".to_string());
                            }
                        }
                        Ok(args.get(0).cloned())
                    }
                    // D-0130 `[Propagate-Option]` (spec/18): `?` on an
                    // `Option` gives its payload, or returns `None` from a
                    // function that itself returns an `Option`; anywhere else
                    // `Option::ok_or(o, e)?` names the error to return.
                    Some(Type::Named(n, args)) if n == "std::Option" => {
                        if matches!(&self.ret, Type::Named(rn, _) if rn == "std::Option") {
                            Ok(args.get(0).cloned())
                        } else {
                            Err(crate::modres::named(
                                "diag.propagate-outside-fallible-context",
                                "`?` on an `Option` returns `None`, so the function must return an `Option`; to return an error instead, write `Option::ok_or(o, e)?`".to_string(),
                            ))
                        }
                    }
                    // `[Propagate]`'s premise: the operand is a `Result` (or,
                    // by `[Propagate-Option]`, an `Option`).
                    Some(Type::Int(_) | Type::Bool | Type::F32 | Type::F64 | Type::Str) => Err("diag.type-mismatch".to_string()),
                    _ => Ok(None),
                }
            }
            ExprKind::Block(b) => self.check_nested_block(b, expected),
            ExprKind::Unsafe(b) => {
                self.unsafe_depth += 1;
                let r = self.check_nested_block(b, expected);
                self.unsafe_depth -= 1;
                r
            }
            ExprKind::If(c, t, f) => {
                self.check_expect(c, &Type::Bool)?;
                // Two successors; the branch states join afterwards.
                // D-0049: with no expected type, a branch that is only a
                // literal takes the other branch's type (checked first; a
                // literal has no effect on the flow state). The type is
                // recorded for the evaluator (`match_hints`, keyed by the
                // then-block).
                let before = self.flow.clone();
                let else_first = expected.is_none()
                    && is_literal_block(t)
                    && f.as_ref().map_or(false, |fe| !is_literal_branch(fe));
                // A branch checked against the other's type (the hint) that
                // comes out untyped (a generic variant such as `None`) has
                // that type: the hint, kept in `hinted`.
                let hinted: Option<Type>;
                let (tt, ft, after_t) = if else_first {
                    let ft = self.check_expr(f.as_ref().unwrap(), None)?;
                    let after_f = std::mem::replace(&mut self.flow, before);
                    let hint = ft.clone().filter(|x| *x != Type::Never && !has_unresolved_marker(x));
                    if let Some(h) = &hint {
                        self.items.match_hints.lock().unwrap().insert(t as *const Block as usize, h.clone());
                    }
                    let tt = self.check_nested_block(t, hint.as_ref())?;
                    let after_t = std::mem::replace(&mut self.flow, after_f);
                    hinted = hint;
                    (tt, ft, after_t)
                } else {
                    let tt = self.check_nested_block(t, expected)?;
                    let after_t = std::mem::replace(&mut self.flow, before);
                    let hint = if expected.is_none() {
                        tt.clone().filter(|x| *x != Type::Never && *x != Type::Void && !has_unresolved_marker(x))
                    } else {
                        None
                    };
                    if let (Some(h), Some(fe)) = (&hint, f) {
                        if is_literal_branch(fe) {
                            self.items.match_hints.lock().unwrap().insert(t as *const Block as usize, h.clone());
                        }
                    }
                    let ft = match f {
                        Some(fe) => self.check_expr(fe, expected.or(hint.as_ref()))?,
                        None => Some(Type::Void),
                    };
                    hinted = hint;
                    (tt, ft, after_t)
                };
                self.flow = after_t.join(&self.flow);
                let (tt, ft) = match (tt, ft, hinted) {
                    (None, b, Some(h)) if else_first => (Some(h), b),
                    (a, None, Some(h)) if !else_first && f.is_some() => (a, Some(h)),
                    (a, b, _) => (a, b),
                };
                match (tt, ft) {
                    (Some(a), Some(b)) if !has_unresolved_marker(&a) && !has_unresolved_marker(&b) => {
                        if f.is_none() {
                            // `[T-If]`: no `else` makes the `if` `()`, so the
                            // branch is `()` as well.
                            if !matches!(a, Type::Void | Type::Never) {
                                return Err(mismatch(format!("an `if` without `else` has no value, but this branch gives `{}`: add an `else`, or end the branch's last expression with `;`", a)));
                            }
                            Ok(Some(Type::Void))
                        } else if ty_compat(&a, &b) {
                            Ok(Some(a))
                        } else if a == Type::Never {
                            Ok(Some(b))
                        } else if b == Type::Never {
                            Ok(Some(a))
                        } else {
                            Err("diag.type-mismatch".to_string())
                        }
                    }
                    _ => Ok(None),
                }
            }
            ExprKind::While(c, b, step) => self.check_while(c, b, step.as_deref()),
            ExprKind::Match(scrut, arms) => self.check_match(e, scrut, arms, expected),
            ExprKind::Return(v) => {
                let rt = match &self.closure_ret {
                    Some(r) => r.clone(),
                    None => Some(self.ret.clone()),
                };
                // `[T-Return]`: the value's type is the enclosing function's
                // (or closure's) result type; a bare `return` gives `()`.
                let what = if self.closure_ret.is_some() { "closure" } else { "function" };
                match v {
                    Some(e) => {
                        self.check_escape_to_caller(e)?;
                        let t = self.check_store_operand(e, rt.as_ref())?;
                        if let (Some(t), Some(want)) = (&t, &rt) {
                            if !has_unresolved_marker(t) && !has_unresolved_marker(want) && *t != Type::Never && !ty_compat(t, want) {
                                return Err(mismatch(if matches!(want, Type::Void) {
                                    format!("this {} has no result, so `return` takes no value (this one gives `{}`)", what, t)
                                } else {
                                    format!("this `return` gives `{}`, but the {}'s result is `{}`", t, what, want)
                                }));
                            }
                        }
                    }
                    None => {
                        if let Some(want) = &rt {
                            if !matches!(want, Type::Void | Type::Never) && !has_unresolved_marker(want) {
                                return Err(mismatch(format!("a bare `return` gives no value, but the {}'s result is `{}`: write `return value;`", what, want)));
                            }
                        }
                    }
                }
                // An edge to the function exit: nothing follows here.
                self.flow.unreachable = true;
                Ok(Some(Type::Never))
            }
            ExprKind::Break | ExprKind::Continue => {
                if self.while_depth == 0 {
                    return Err("diag.break-outside-loop".to_string());
                }
                // An edge to the loop exit / the loop test.
                let st = self.flow.clone();
                if let Some(ctx) = self.loops.last_mut() {
                    if matches!(e.kind, ExprKind::Break) {
                        ctx.breaks.push(st);
                    } else {
                        ctx.continues.push(st);
                    }
                }
                self.flow.unreachable = true;
                Ok(Some(Type::Never))
            }
            ExprKind::Closure { is_move, captures, params, ret, body } => {
                let t = self.check_closure(*is_move, captures, params, ret.as_ref(), body, expected)?;
                // D-0073: the evaluators give the body's value this result type.
                if let Some(ft @ Type::Fn(..)) = &t {
                    if !has_unresolved_marker(ft) && self.subst.is_empty() {
                        self.items.closure_types.lock().unwrap().insert(e as *const Expr as usize, ft.clone());
                    }
                }
                Ok(t)
            }
        }
    }

    // `while (c) b`: a back edge from the end of the body (and every
    // `continue`) to the test; the exit is the test's false edge joined
    // with every `break`. The head state is iterated to the least fixed
    // point with refutations suppressed (a fact that is `F` on the
    // first pass may be `?` at the fixed point), then the body is walked
    // once more from the fixed head with refutations on. Type errors
    // surface on any pass.
    // A `for` loop's `step` is checked on the state after the body joined
    // with every `continue`'s, and its result is what reaches the head.
    fn check_while(&mut self, c: &Expr, b: &Block, step: Option<&Expr>) -> TResult {
        let entry = self.flow.clone();
        let mut head = entry.clone();
        // D-0175: `while (true)` leaves only by `break` (or `return`,
        // `fault`, …): with no `break` to it, nothing follows the loop and
        // its type is `Never`.
        let forever = matches!(c.kind, ExprKind::BoolLit(true));
        let outer_report = self.report;
        let mut exit;
        let mut iterations = 0;
        loop {
            self.report = false;
            self.flow = head.clone();
            let _ = self.check_expr(c, Some(&Type::Bool));
            let after_cond = self.flow.clone();
            self.loops.push(LoopCtx::default());
            self.while_depth += 1;
            let r = self.check_nested_block(b, None);
            self.while_depth -= 1;
            let ctx = self.loops.pop().unwrap_or_default();
            r?;
            let mut end = self.flow.clone();
            for st in &ctx.continues {
                end = end.join(st);
            }
            if let Some(s) = step {
                self.flow = end;
                self.check_stmt(&Stmt::Expr(Box::new(s.clone())))?;
                end = self.flow.clone();
            }
            self.report = outer_report;
            let new_head = entry.join(&end);
            exit = after_cond;
            if forever {
                exit.unreachable = true;
            }
            for st in &ctx.breaks {
                exit = exit.join(st);
            }
            iterations += 1;
            if new_head == head || iterations > 16 {
                break;
            }
            head = new_head;
        }
        if outer_report {
            self.flow = head;
            self.check_expect(c, &Type::Bool)?;
            let after_cond = self.flow.clone();
            self.loops.push(LoopCtx::default());
            self.while_depth += 1;
            let r = self.check_nested_block(b, None);
            self.while_depth -= 1;
            let ctx = self.loops.pop().unwrap_or_default();
            r?;
            if let Some(s) = step {
                let mut end = self.flow.clone();
                for st in &ctx.continues {
                    end = end.join(st);
                }
                self.flow = end;
                self.check_stmt(&Stmt::Expr(Box::new(s.clone())))?;
            }
            exit = after_cond;
            if forever {
                exit.unreachable = true;
            }
            for st in &ctx.breaks {
                exit = exit.join(st);
            }
        }
        let never = forever && exit.unreachable;
        self.flow = exit;
        Ok(Some(if never { Type::Never } else { Type::Void }))
    }

    fn check_int_lit(&self, v: u128, suffix: Option<&str>, expected: Option<&Type>) -> TResult {
        // D-0090: an unsuffixed literal where a `number` type parameter is
        // expected is of that type; its range is checked at each type the
        // function is used at.
        if suffix.is_none() {
            if let Some(m @ Type::Named(mn, _)) = expected.filter(|t| is_marker(t)) {
                if self.marker_bounds.get(mn).map_or(false, |b| b.implies(Bound::Number)) {
                    return Ok(Some(m.clone()));
                }
            }
        }
        let (it, defaulted) = IntTy::of_literal(suffix, expected);
        if !int_lit_fits(it, v) {
            return Err(int_lit_range_msg(&v.to_string(), it, defaulted));
        }
        Ok(Some(Type::Int(it)))
    }

    fn check_unary(&mut self, op: UnOp, inner: &Expr, expected: Option<&Type>) -> TResult {
        // The "most negative literal" case (`-2147483648`, ...): the
        // *positive* magnitude does not fit the type's positive range
        // (checked by `check_int_lit`/`int_lit_fits`, which know
        // nothing about the enclosing negation), but the negated value
        // is exactly `min(ty)`, a perfectly legal value. Checked here,
        // against the negated value, before ever calling `check_expr`
        // on the bare positive literal (which would reject it first).
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                // D-0090: `-1` where a `number` type parameter is expected.
                let at = inner as *const Expr as usize;
                if suf.is_none() {
                    if let Some(m @ Type::Named(mn, _)) = expected.filter(|t| is_marker(t)) {
                        if self.marker_bounds.get(mn).map_or(false, |b| b.implies(Bound::Number)) {
                            self.items.number_literals.lock().unwrap().insert(at);
                            return Ok(Some(m.clone()));
                        }
                    }
                    if matches!(expected, Some(Type::F32 | Type::F64)) && self.items.number_literals.lock().unwrap().contains(&at) {
                        return Ok(expected.cloned());
                    }
                }
                let (it, defaulted) = IntTy::of_literal(suf.as_deref(), expected);
                // `[T-Neg]`: unary minus needs a signed or float operand.
                if !it.signed() {
                    return Err(mismatch(format!("`-` needs a signed or floating-point operand, but this is `{}`", Type::Int(it))));
                }
                let bw = it.bitwidth(crate::value::ADDR_WIDTH);
                let neg: i128 = if bw >= 128 {
                    if *v == (1u128 << 127) {
                        i128::MIN
                    } else if *v < (1u128 << 127) {
                        -(*v as i128)
                    } else {
                        return Err(int_lit_range_msg(&format!("-{}", v), it, defaulted));
                    }
                } else {
                    -(*v as i128)
                };
                let (lo, hi) = int_range(it);
                if neg < lo || neg > hi {
                    return Err(int_lit_range_msg(&format!("-{}", v), it, defaulted));
                }
                return Ok(Some(Type::Int(it)));
            }
        }
        let it = self.check_expr(inner, expected)?;
        if let Some(m @ Type::Named(mn, _)) = it.as_ref().filter(|t| is_marker(t)) {
            // D-0090: `-x` needs `number`, `~x` `integer`.
            let (need, sym) = match op {
                UnOp::Neg => (Some(Bound::Number), "-"),
                UnOp::BitNot => (Some(Bound::Integer), "~"),
                _ => (None, "!"),
            };
            return match need {
                Some(b) if self.marker_bounds.get(mn).map_or(false, |have| have.implies(b)) => Ok(Some(m.clone())),
                Some(b) => Err(self.bound_msg(mn, sym, b)),
                None => Err(unbounded_msg("a unary operator")),
            };
        }
        // D-0254: `-` on a `simd` of a signed integer or a float, `~` on one of
        // an integer, lane by lane.
        if let Some(Type::Simd(lt, _)) = &it {
            let ok = match op {
                UnOp::Neg => matches!(&**lt, Type::F32 | Type::F64) || matches!(&**lt, Type::Int(t) if t.signed()),
                UnOp::BitNot => matches!(&**lt, Type::Int(_)),
                UnOp::Not => false,
            };
            if !ok {
                let sym = match op { UnOp::Neg => "-", UnOp::BitNot => "~", UnOp::Not => "!" };
                return Err(mismatch(format!("`{}` does not apply to a `{}`: `-` takes a simd of a signed integer or a float, `~` one of an integer; a mask is inverted with `~`", sym, it.as_ref().unwrap())));
            }
            return Ok(it);
        }
        match (op, &it) {
            (UnOp::Neg, Some(Type::Int(t))) if !t.signed() => Err(mismatch(format!("`-` needs a signed or floating-point operand, but this is `{}`", Type::Int(*t)))),
            // `[T-Neg]`: signed or float; `[T-Not]`: `~` on an integer.
            (UnOp::Neg, Some(t)) if !matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Never) && !has_unresolved_marker(t) => {
                Err(mismatch(format!("`-` needs a signed or floating-point operand, but this is `{}`", t)))
            }
            (UnOp::BitNot, Some(Type::Bool)) => Err(mismatch("`~` inverts an integer's bits; for a `bool` write `!`".to_string())),
            (UnOp::BitNot, Some(t)) if !matches!(t, Type::Int(_) | Type::Never) && !has_unresolved_marker(t) => {
                Err(mismatch(format!("`~` needs an integer, but this is `{}`", t)))
            }
            (UnOp::Not, Some(Type::Bool)) => Ok(Some(Type::Bool)),
            (UnOp::Not, Some(t)) if !matches!(t, Type::Bool) && !has_unresolved_marker(t) => {
                Err(mismatch(format!("`!` needs a `bool`, but this is `{}`", t)))
            }
            _ => Ok(it),
        }
    }

    // An operand whose type comes from the other operand
    // (`rule.type.expected`): an unsuffixed literal, or an enum literal
    // (`Some(…)`, `Ok(…)`) whose type arguments are left to context.
    fn takes_expected(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) if args.len() == 1 => {
                matches!(&callee.kind, ExprKind::Path(segs, targs) if targs.is_empty() && self.variant_enum(segs).is_some())
            }
            ExprKind::Paren(inner) => self.takes_expected(inner),
            _ => is_bare_num_literal(e),
        }
    }

    // D-0108: a comparison's operand, a `String` place borrowed shared as
    // `&e` would be; anything else is checked as a value.
    fn check_text_operand(&mut self, e: &Expr) -> TResult {
        let place = (matches!(self.static_place_type(e), Some(Type::Named(n, _)) if n == "std::String") || self.is_resource_key_place(e))
            && matches!(borrow_operand_kind(e, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..))) || self.returns_ref(b)), OperandKind::Place);
        if !place {
            return self.check_expr(e, None);
        }
        self.place_pos = true;
        let t = self.check_expr(e, None)?;
        self.flow_borrow(e, &Mode::Shared)?;
        Ok(t)
    }

    fn check_binary(&mut self, op: BinOp, a: &Expr, b: &Expr, whole: &Expr, expected: Option<&Type>) -> TResult {
        use BinOp::*;
        // D-0037: an operator on literal expressions only takes the type
        // its context expects, when that is a number.
        let numeric = expected.filter(|t| matches!(t, Type::Int(_) | Type::F32 | Type::F64));
        // D-0073: `==` would read a `String` (a resource) whole; name the
        // comparison that exists.
        let is_string = |e: &Expr| matches!(self.static_place_type(e), Some(Type::Named(n, _)) if n == "std::String");
        let is_text = |e: &Expr| is_string(e) || matches!(self.value_shape(e), Some(Type::Named(n, _)) if n == "std::String") || matches!(&strip_parens_tc(e).kind, ExprKind::StrLit(_));
        if op == Add && (is_text(a) || is_text(b)) {
            return Err(crate::modres::named("diag.read-of-resource", "`+` does not join text; append to a `String` with `String::append(&mut s, &t)` (or a `str`, a number), or build one with `sprintf(\"%s%s\", &s, &t)`".to_string()));
        }
        // `==` on a resource other than text (a `Vec`, a `HashMap`): the
        // comparison would read each side whole; say what to compare.
        // D-0271: a key type holding a `String` is compared where it is
        // (`[Cmp-Key]`), as a `String` is.
        if matches!(op, Eq | Ne) {
            for side in [a, b] {
                if let Some(t) = self.static_place_type(side) {
                    if !matches!(&t, Type::Named(n, _) if n == "std::String") && !self.named_key_type(&t) && is_resource_ty(self.items, &t) {
                        return Err(crate::modres::named(
                            "diag.read-of-resource",
                            format!("`{}` compares plain values, and a `{}` is a resource: compare what it holds -- its length and its elements, in a loop -- or a summary of it", if op == Eq { "==" } else { "!=" }, t),
                        ));
                    }
                }
            }
        }
        let literal_op = matches!(op, Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor | Shl | Shr) && is_literal_expr(whole);
        // A bare unsuffixed literal on the left takes its type from the
        // right operand (`rule.type.expected`: "the other operand's
        // determined type"); check the right side first in that case.
        // D-0254: where a `simd` is expected, an arithmetic operator's
        // operands are of that type (two literals included).
        let simd_expected = expected.filter(|t| matches!(t, Type::Simd(..))).filter(|_| matches!(op, Add | Sub | Mul | Div | BitAnd | BitOr | BitXor | Shl | Shr));
        let (at, bt) = if let Some(t) = simd_expected {
            let at = self.check_expr(a, Some(t))?;
            let bt = self.check_expr(b, Some(t))?;
            (at, bt)
        } else if let (Some(t), true) = (numeric, literal_op) {
            let at = self.check_expr(a, Some(t))?;
            let bt = if matches!(op, Shl | Shr) { self.check_expr(b, at.as_ref())? } else { self.check_expr(b, Some(t))? };
            (at, bt)
        } else if (self.takes_expected(a) || (is_literal_expr(a) && !matches!(op, Shl | Shr))) && !self.takes_expected(b) && !is_literal_expr(b) {
            // (D-0037: a literal expression on the left, `((1 << 40) - 1) & k`,
            // takes the right operand's type as a bare literal does.)
            let bt = self.check_expr(b, None)?;
            let at = self.check_expr(a, bt.as_ref())?;
            (at, bt)
        } else if matches!(op, Eq | Ne | Lt | Le | Gt | Ge) && (is_string(a) || is_string(b) || self.is_resource_key_place(a) || self.is_resource_key_place(b)) {
            // D-0108 `[Cmp-Text]`: a `String` place is borrowed shared for
            // the comparison, not read whole. D-0271 `[Cmp-Key]`: so is a
            // place of a key type that holds one.
            let at = self.check_text_operand(a)?;
            let bt = self.check_text_operand(b)?;
            (at, bt)
        } else {
            let at = self.check_expr(a, None)?;
            let bt = self.check_expr(b, at.as_ref())?;
            (at, bt)
        };
        let at = if at.is_none() { self.reguess(a, bt.as_ref()) } else { at };
        if matches!(&at, Some(t) if is_marker(t)) || matches!(&bt, Some(t) if is_marker(t)) {
            // `rule.type.kind`: an operator on a type parameter's value is
            // the bound's (D-0090); a parameter without one has none.
            return self.check_bounded_binary(op, at.as_ref(), bt.as_ref());
        }
        // `[T-Rawptr-Offset]` (spec/12 §5): `rawptr<τ> + isize : rawptr<τ>`,
        // a distinct rule from ordinary same-type arithmetic -- not an
        // "arithmetic type mismatch" at all.
        if op == Add {
            if let Some(Type::Rawptr(_)) = &at {
                if self.unsafe_depth == 0 {
                    return Err("diag.trusted-outside-unsafe".to_string());
                }
                return Ok(at);
            }
        }
        // D-0254: an operator on a `simd` works lane by lane.
        if matches!(&at, Some(Type::Simd(..))) || matches!(&bt, Some(Type::Simd(..))) {
            return simd_binary_type(op, at.as_ref(), bt.as_ref());
        }
        // D-0053: `==`/`!=` between a `StringView` and a `StringView` or a
        // `str` compares their text (`StringView::eq`, `StringView::eq_str`);
        // nothing else applies to a view.
        let is_view = |t: &Option<Type>| matches!(t, Some(Type::Named(n, _)) if n == "std::StringView");
        // D-0164: `==`/`!=` between a `String` and a `str` or a
        // `StringView`, either way round, compares their text: recorded
        // here and rewritten to `String::eq_str` / `String::eq_view` before
        // the program is checked again (as D-0162's literals are). Mixed
        // text types are still not ordered.
        let is_string = |t: &Option<Type>| matches!(t, Some(Type::Named(n, targs)) if n == "std::String" && targs.is_empty());
        if (is_string(&at) && (matches!(bt, Some(Type::Str)) || is_view(&bt))) || (is_string(&bt) && (matches!(at, Some(Type::Str)) || is_view(&at))) {
            let string_left = is_string(&at);
            let other_is_str = if string_left { matches!(bt, Some(Type::Str)) } else { matches!(at, Some(Type::Str)) };
            return match op {
                Eq | Ne => {
                    note_rewrite(whole, Rewrite::TextEq { string_left, other_is_str, negate: op == Ne });
                    Ok(Some(Type::Bool))
                }
                Lt | Le | Gt | Ge => Err(mismatch("`<`, `<=`, `>`, `>=` order two texts of one type; a `String` and a `str` or a `StringView` compare with `==` and `!=` only (D-0164): order two `String`s, or `String::as_view(&s)` and a view".to_string())),
                _ => Err(mismatch(format!("the operands are `{}` and `{}`, which cannot be combined", at.as_ref().unwrap(), bt.as_ref().unwrap()))),
            };
        }
        if is_view(&at) || is_view(&bt) {
            let view_left = is_view(&at);
            let other = if view_left { &bt } else { &at };
            let other_is_str = matches!(other, Some(Type::Str));
            if !(other_is_str || is_view(other)) {
                return Err("diag.type-mismatch".to_string());
            }
            // D-0108: two views are ordered by their bytes.
            let vo = match op {
                Eq | Ne => crate::interp::ViewOp::Eq { view_left, other_is_str, negate: op == Ne },
                Lt | Le | Gt | Ge if !other_is_str => crate::interp::ViewOp::Less { swap: matches!(op, Gt | Le), negate: matches!(op, Le | Ge) },
                Lt | Le | Gt | Ge => return Err(mismatch(TEXT_ORDER_HELP.to_string())),
                _ => return Err(mismatch("a `StringView` has `==`, `!=`, `<`, `<=`, `>`, `>=` only".to_string())),
            };
            self.items.view_ops.lock().unwrap().insert(whole as *const Expr as usize, vo);
            return Ok(Some(Type::Bool));
        }
        // `[Eq-Str]` and `[Cmp-Text]` are the only operators defined on
        // `str` (spec/12 §4): no arithmetic or logic.
        if (matches!(&at, Some(Type::Str)) || matches!(&bt, Some(Type::Str))) && !matches!(op, Eq | Ne | Lt | Le | Gt | Ge) {
            return Err(mismatch("`str` has only the comparisons `==`, `!=`, `<`, `<=`, `>`, `>=`".to_string()));
        }
        match op {
            Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor => {
                if let (Some(ta), Some(tb)) = (&at, &bt) {
                    if !has_unresolved_marker(ta) && !has_unresolved_marker(tb) {
                        if !ty_compat(ta, tb) {
                            return Err(mismatch(format!("the operands are `{}` and `{}`; convert one (`widen`, `narrow`, `to_float`)", ta, tb)));
                        }
                        // `[T-Arith]`: an integer type, or a float for
                        // `+ - * /` only.
                        let sym = match op { Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%", BitAnd => "&", BitOr => "|", _ => "^" };
                        match ta {
                            Type::Int(_) | Type::Never => {}
                            Type::F32 | Type::F64 if matches!(op, Add | Sub | Mul | Div) => {}
                            Type::F32 | Type::F64 if op == Rem => {
                                return Err(mismatch(format!("`%` is defined for integers only; a `{}` has `+ - * /` (a float remainder is `a - b * trunc(a / b)`)", ta)));
                            }
                            Type::F32 | Type::F64 => {
                                return Err(mismatch(format!("`{}` is defined for integers only; a float's bits are `reinterpret<u64>(x)` (or `u32` for `f32`)", sym)));
                            }
                            Type::Bool if matches!(op, BitAnd | BitOr | BitXor) => {
                                return Err(mismatch(format!("`{}` is defined for integers only; for `bool` write `&&`, `||` or `!=`", sym)));
                            }
                            other => {
                                return Err(mismatch(format!("`{}` is arithmetic, defined for numbers, not `{}`", sym, other)));
                            }
                        }
                        if let Type::Int(it) = ta {
                            self.check_literal_fold_overflow(op, *it, a, b)?;
                        }
                    }
                }
                Ok(at.or(bt))
            }
            Shl | Shr => {
                // `[T-Shift]`: the shifted operand is an integer.
                if let Some(t) = &at {
                    if !matches!(t, Type::Int(_) | Type::Never) && !has_unresolved_marker(t) {
                        return Err(mismatch(format!("`{}` shifts an integer's bits, and this is `{}`{}", if op == Shl { "<<" } else { ">>" }, t,
                            if matches!(t, Type::F32 | Type::F64) { "; a float's bits are `reinterpret<u64>(x)` (or `u32` for `f32`)" } else { "" })));
                    }
                }
                // `[T-Shift]`: the amount is a `u32` (a literal takes it).
                if let Some(Type::Int(k)) = &bt {
                    if *k != IntTy::U32 && !is_literal_expr(b) {
                        return Err(mismatch(format!("a shift amount is a `u32`, and this is `{}`: convert it with `{}<u32>(n)`", Type::Int(*k), if k.bitwidth(crate::value::ADDR_WIDTH) < 32 { "widen" } else { "narrow" })));
                    }
                }
                // `[Shift-Amount-Invalid]`, `discharge: static where constant`
                // (spec/14 §6's value-range row): a literal amount at or
                // beyond the operand type's bit width is rejected here.
                if let (Some(Type::Int(it)), Some(n)) = (&at, self.known_value(b)) {
                    if n < 0 || n >= it.bitwidth(crate::value::ADDR_WIDTH) as i128 {
                        return Err("diag.shift-amount-out-of-range".to_string());
                    }
                }
                Ok(at)
            }
            Eq | Ne | Lt | Le | Gt | Ge => {
                // `[Eq-Fn]` (spec/12 §4): no equality on function values —
                // a `fn` value, a closure, or an aggregate holding one
                // (`[Eq-Struct]` compares field by field).
                let callable_operand = |e: &Expr, t: &Option<Type>| {
                    t.as_ref().map_or(false, |t| contains_fn(&self.items, t))
                        || matches!(&e.kind, ExprKind::Closure { .. })
                        || matches!(&e.kind, ExprKind::Path(segs, _) if segs.len() == 1 && self.closure_moves.contains_key(&segs[0]))
                };
                if callable_operand(a, &at) || callable_operand(b, &bt) {
                    return Err("diag.type-mismatch".to_string());
                }
                // No comparison of slices (D-0047): whether two runs are
                // equal is element by element, which a program writes.
                if matches!(&at, Some(Type::Slice(..))) || matches!(&bt, Some(Type::Slice(..))) {
                    return Err("diag.type-mismatch".to_string());
                }
                // Nor of an aggregate holding one, a `StringView` included:
                // `[Eq-Struct]` would compare it field by field.
                for t in [&at, &bt].into_iter().flatten() {
                    if !has_unresolved_marker(t) && contains_slice(&self.items, t) {
                        return Err(mismatch(format!("`==` and `!=` compare a `{}` field by field, and it holds a `StringView` or a slice, which compares only on its own: compare the parts (`Option::unwrap(a) == b`, or a `match`)", t)));
                    }
                }
                if let (Some(ta), Some(tb)) = (&at, &bt) {
                    if !has_unresolved_marker(ta) && !has_unresolved_marker(tb) && !ty_compat(ta, tb) {
                        return Err(mismatch(format!("the operands are `{}` and `{}`, which cannot be compared{}", ta, tb, compare_hint(ta, tb))));
                    }
                    // `[T-Cmp]`: `< <= > >=` additionally need an ordered
                    // τ (D-0108: a number, `bool`, `str` or `String`; D-0271:
                    // a struct or enum key type, `[Cmp-Key]`).
                    if matches!(op, Lt | Le | Gt | Ge) && !has_unresolved_marker(ta) && !matches!(ta, Type::Never) && !self.bound_holds(Bound::Ordered, ta) {
                        return Err(mismatch(format!("`<`, `<=`, `>`, `>=` order numbers, `bool`, `str`, `String`, and a struct or enum of those; a `{}` is compared with `==` and `!=` only{}", ta, key_problem_text(self.items, ta))));
                    }
                    // D-0108 `[Cmp-Text]`: two `String`s compare by their
                    // bytes, each read where it is (neither is moved).
                    if !has_unresolved_marker(ta) && is_std_string(ta) {
                        let mut ops = self.items.view_ops.lock().unwrap();
                        // A `KeyCmp` set by another instantiation of a
                        // generic body compares text too; keep it.
                        if !matches!(ops.get(&(whole as *const Expr as usize)), Some(crate::interp::ViewOp::KeyCmp)) {
                            ops.insert(whole as *const Expr as usize, crate::interp::ViewOp::TextCmp);
                        }
                        return Ok(Some(Type::Bool));
                    }
                    // `[T-Cmp]`: "not resource" — whatever the operands'
                    // form (a call's result as much as a binding). D-0271: a
                    // key type holding a `String` is compared where it is.
                    if !has_unresolved_marker(ta) && !self.named_key_type(ta) && is_resource_ty(&self.items, ta) {
                        return Err(crate::modres::named(
                            "diag.read-of-resource",
                            format!("a comparison reads both operands whole, and `{}` is a resource; compare what it holds (an element at a time, `String::eq`, or through a view or slice)", ta),
                        ));
                    }
                }
                // D-0271 `[Cmp-Key]`: a struct or enum key type is ordered
                // field by field, as `Vec::sort` orders it; one that holds a
                // `String` is compared where it is, not read (as a `String`
                // is). `==` on one that holds no resource stays `[Eq-Struct]`'s
                // value comparison. Decided from whichever operand's type is
                // known: a bare variant (`lo < Lvl::Mid`) has none yet.
                let known = [&at, &bt].into_iter().flatten().find(|t| !has_unresolved_marker(t) && matches!(t, Type::Named(..)) && !is_std_string(t));
                if let Some(t) = known {
                    let ordering = matches!(op, Lt | Le | Gt | Ge);
                    if self.named_key_type(t) {
                        if ordering || is_resource_ty(self.items, t) {
                            self.items.view_ops.lock().unwrap().insert(whole as *const Expr as usize, crate::interp::ViewOp::KeyCmp);
                        }
                    } else if ordering && !matches!(t, Type::Named(n, _) if n == "std::StringView") {
                        return Err(mismatch(format!("`<`, `<=`, `>`, `>=` order numbers, `bool`, `str`, `String`, and a struct or enum of those; a `{}` is compared with `==` and `!=` only{}", t, key_problem_text(self.items, t))));
                    }
                }
                Ok(Some(Type::Bool))
            }
            And | Or => {
                // `[T-Logic]`: both operands `bool`.
                let sym = if op == And { "&&" } else { "||" };
                for t in [&at, &bt].into_iter().flatten() {
                    if !matches!(t, Type::Bool | Type::Never) && !has_unresolved_marker(t) {
                        return Err(mismatch(match t {
                            Type::Int(_) => format!("`{}` joins `bool`s, and this is `{}`; for a number write `n != 0`", sym, t),
                            other => format!("`{}` joins `bool`s, and this is `{}`", sym, other),
                        }));
                    }
                }
                Ok(Some(Type::Bool))
            }
        }
    }

    // literal-operand types couldn't be pinned down left-to-right (e.g.
    // `x + 1` where `x`'s own static type is unknown to this pass);
    // best-effort retry from the other side.
    fn reguess(&mut self, e: &Expr, hint: Option<&Type>) -> Option<Type> {
        self.check_expr(e, hint).ok().flatten()
    }

    // `rule.control.flow-analysis` §6's literal-operand constant-fold
    // refutation: when both operands are themselves literal expressions
    // (of the now-determined type `it`), the result is knowable without
    // running the program at all -- reject *statically* if it would
    // overflow/divide-by-zero/shift out of range, exactly as
    // `conf.i32-add-overflow-static` documents. A non-literal operand
    // (a binding, a call, ...) is left to the dynamic backstop, since
    // its value is not known here.
    // An operand's value when it is a literal, or a binding bound to one
    // with no intervening write (spec/14's value-range row).
    // A literal, or a literal-bound local (spec/14, CHG-0092). The latter
    // is a flow fact, so it is consulted only when refutations are
    // reported: on a loop's speculative passes the back edge has not yet
    // been joined in.
    fn known_value(&self, e: &Expr) -> Option<i128> {
        literal_value(e).or_else(|| match &strip_parens_tc(e).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 && self.report && !self.flow.unreachable => self.flow.lits.get(&segs[0]).copied(),
            _ => None,
        })
    }

    // An integer expression's value when it is made of known values
    // (literals, literal-bound locals), unary minus, `+ - * / %`, and
    // `widen`/`narrow` of one; None when it is not, or when the
    // arithmetic itself fails (that is another rule's to report).
    fn fold_int(&self, e: &Expr) -> Option<i128> {
        if let Some(v) = self.known_value(e) {
            return Some(v);
        }
        match &strip_parens_tc(e).kind {
            ExprKind::Unary(UnOp::Neg, a) => self.fold_int(a)?.checked_neg(),
            ExprKind::Binary(op, a, b) => {
                let (x, y) = (self.fold_int(a)?, self.fold_int(b)?);
                match op {
                    BinOp::Add => x.checked_add(y),
                    BinOp::Sub => x.checked_sub(y),
                    BinOp::Mul => x.checked_mul(y),
                    BinOp::Div if y != 0 => x.checked_div(y),
                    BinOp::Rem if y != 0 => x.checked_rem(y),
                    _ => None,
                }
            }
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, _) if segs.len() == 1 && (segs[0] == "widen" || segs[0] == "narrow") => self.fold_int(&args[0]),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_literal_fold_overflow(&self, op: BinOp, it: IntTy, a: &Expr, b: &Expr) -> Result<(), String> {
        let av = self.known_value(a);
        let bv = self.known_value(b);
        let (av, bv) = match (av, bv) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(()),
        };
        use crate::value::*;
        let r = match op {
            BinOp::Add => checked_add(it, av, bv).map(|_| ()),
            BinOp::Sub => checked_sub(it, av, bv).map(|_| ()),
            BinOp::Mul => checked_mul(it, av, bv).map(|_| ()),
            BinOp::Div => {
                if bv == 0 {
                    return Err("diag.div-by-zero".to_string());
                }
                checked_div(it, av, bv).map(|_| ())
            }
            BinOp::Rem => {
                if bv == 0 {
                    return Err("diag.div-by-zero".to_string());
                }
                checked_rem(it, av, bv).map(|_| ())
            }
            _ => return Ok(()),
        };
        let sym = match op { BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/", _ => "%" };
        r.map_err(|e| match e {
            ArithError::DivOverflow => "diag.div-overflow".to_string(),
            _ => {
                let (lo, hi) = int_range(it);
                let text = |e: &Expr, v: i128| match &strip_parens_tc(e).kind {
                    ExprKind::Path(segs, _) if segs.len() == 1 => format!("{} (here {})", shown_name(&segs[0]), v),
                    _ => v.to_string(),
                };
                crate::modres::named("diag.arith-overflow", format!("{} {} {} is outside `{}` ({} to {}); widen an operand first (`widen<T>(x)`) or use `wrapping_*`/`checked_*`", text(a, av), sym, text(b, bv), Type::Int(it), lo, hi))
            }
        })
    }

    fn check_field(&mut self, base: &Expr, name: &str) -> TResult {
        let bt = self.check_expr(base, None)?;
        // `[Field-Access-Auto-Deref]`: through a reference, and (D-0064)
        // through a guard, `g.f` being `(*g).f`.
        let bt = match bt {
            Some(Type::Ref(inner, _)) | Some(Type::Guard(inner)) => Some(*inner),
            other => other,
        };
        if matches!(&bt, Some(t) if is_marker(t)) {
            return Err(unbounded_msg("a field access"));
        }
        match bt {
            Some(Type::Named(sname, args)) => {
                if let Some(sdecl) = self.items.structs.get(&sname).cloned() {
                    let subst: HashMap<String, Type> = sdecl.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    match sdecl.fields.iter().find(|f| f.name == name) {
                        Some(fd) if !self.privileged && !field_visible(&sname, fd.export, &self.module) => {
                            Err(crate::modres::named("diag.name-not-visible", format!("field `{}` of `{}` is not exported", name, Type::Named(sname.clone(), Vec::new()))))
                        }
                        Some(fd) => Ok(Some(apply_subst(&fd.ty, &subst))),
                        None => Err(mismatch(format!("`{}` has no field `{}`{}", Type::Named(sname.clone(), args.clone()), name, self.method_hint(&sname, name)))),
                    }
                } else if self.items.enums.contains_key(&sname) {
                    // An enum value has no fields: its payload is reached
                    // by `match`.
                    Err(mismatch(format!("`{}` is an enum and has no field `{}`{}", Type::Named(sname.clone(), args.clone()), name, self.method_hint(&sname, name))))
                } else {
                    Ok(None) // closure capture struct or otherwise opaque; not modeled statically
                }
            }
            Some(t @ (Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Array(..) | Type::Slice(..) | Type::Fn(..) | Type::Handle(_) | Type::Mutex(_) | Type::Rawptr(_))) => {
                Err(mismatch(format!("a value of type `{}` has no field `{}`", t, name)))
            }
            _ => Ok(None),
        }
    }

    // CobaltC has no method-call syntax: `x.f(...)` calls a fn-valued
    // field. When `T::f` is a function, say how to call it.
    fn method_hint(&self, tname: &str, name: &str) -> String {
        let f = format!("{}::{}", tname, name);
        if self.items.fns.contains_key(&f) {
            format!("; there is no method-call syntax -- call the function as `{}(x, ...)`", f.strip_prefix("std::").unwrap_or(&f))
        } else {
            String::new()
        }
    }

    fn check_path(&mut self, segs: &[String]) -> TResult {
        if segs.len() == 1 {
            // `[Binding-Lookup]`'s `temporally-valid(a_x)`: refuted when
            // `valid(x) = F` (moved from, dropped) -- in every position,
            // since every use of the name goes through the lookup -- and
            // (D-0089) when `valid(x) = U`, moved on some path to here, as
            // a read that may be uninitialized is refuted (spec/11).
            let v = self.flow.valid.get(&segs[0]).copied();
            if matches!(v, Some(Tri::F) | Some(Tri::U)) && self.reinit_target.as_deref() != Some(segs[0].as_str()) {
                let n = shown_name(&segs[0]);
                let msg = if v == Some(Tri::F) {
                    format!("`{0}` was moved or dropped before this use, and holds no value here; give it a new value first (`{0} = …;`)", n)
                } else {
                    format!("`{0}` may have been moved or dropped before this use -- on some path to here, or in an earlier pass of the loop -- so it may hold no value; give it a new value on every path first (`{0} = …;`), or move a copy", n)
                };
                self.refute(&crate::modres::named("diag.stale-binding", msg))?;
            }
            if let Some(t) = self.lookup_local(&segs[0]) {
                return Ok(Some(t));
            }
            // Bound, but of a type this pass could not determine (a
            // `match` binder of an unresolved payload, ...): not unbound.
            if self.flow.is_tracked(&segs[0]) || self.capture_names.contains(&segs[0]) {
                return Ok(None);
            }
        }
        // D-0258: a static is a place of its type (`[Static-Use]`); as a
        // value it would be read or moved, which `check_expr` refuses.
        if let Some(s) = self.items.statics.get(&segs.join("::")) {
            return Ok(Some(s.ty.clone()));
        }
        if let Some(enum_name) = self.variant_enum(segs) {
            // `[T-Enum]`: a bare `Vi` is `Vi(())`, so a variant with a
            // payload written without one has a payload of the wrong type.
            if self.variant_arity(&enum_name, segs) == Some(true) && !self.items.fns.contains_key(&segs.join("::")) {
                let v = segs.last().cloned().unwrap_or_default();
                return Err(mismatch(format!("`{0}` carries a value; write `{0}(value)` (a variant without `(…)` is one that carries none)", v)));
            }
            // Payload-less variant; its own type params can't be known
            // from the name alone -- leave unresolved (opaque),
            // consistent with this pass's "don't guess" rule.
            return Ok(None);
        }
        let name = segs.join("::");
        if segs.len() == 1
            && !self.capture_names.contains(&segs[0])
            && !self.items.fns.contains_key(&name)
            && !self.items.externs.contains_key(&name)
            && !INTRINSIC_NAMES.contains(&segs[0].as_str())
        {
            // `[Binding-Lookup-Unbound]` / `[Resolve-Unbound]`: nothing in
            // any frame, no item, no variant -- a static fact.
            // A private function of the same name in another module (not
            // `std`'s): say so, rather than that none exists.
            let hidden = self.items.fns.iter().filter(|(k, f)| {
                !f.export && f.assoc_type.is_none() && !f.is_const && k.contains("::") && !k.starts_with("std::") && k.rsplit("::").next() == Some(segs[0].as_str())
            }).min_by(|a, b| a.0.cmp(b.0));
            // A function of `std` the module did not import.
            let std_key = format!("std::{}", segs[0]);
            let in_std = self.items.fns.get(&std_key).map_or(false, |f| f.export)
                || FLOAT_FNS.contains(&std_key.as_str())
                || FLOAT2_FNS.contains(&std_key.as_str())
                || ["std::printf", "std::sprintf", "std::eprintf", "std::assert"].contains(&std_key.as_str());
            return Err(crate::modres::named("diag.unbound-name", match hidden {
                Some((k, _)) => format!("nothing named `{}` is visible here; `{}` is not exported from its module", segs[0], k),
                None if in_std => format!("nothing named `{}` is declared here; the standard library's `{}` is brought in by `import std;`", segs[0], segs[0]),
                None => format!("nothing named `{}` is declared here", segs[0]),
            }));
        }
        // A built-in operation (`sizeof`, `widen`, `spawn`, …) named where
        // a value is wanted: it is only called, and has no value, so
        // `v[sizeof]` is rejected here (it reached `coby`'s evaluator, as
        // an unbound name, and `cobc`'s code generator, as an internal
        // error).
        if segs.len() == 1
            && INTRINSIC_NAMES.contains(&segs[0].as_str())
            && !self.capture_names.contains(&segs[0])
            && !self.items.fns.contains_key(&name)
            && !self.items.externs.contains_key(&name)
        {
            return Err(crate::modres::named("diag.unbound-name", format!("`{0}` is a built-in operation, only called (`{0}(…)`); it is not a value", segs[0])));
        }
        if segs.len() > 1 && !self.items.fns.contains_key(&name) && !self.items.externs.contains_key(&name) && !INTRINSIC_NAMES.contains(&name.as_str()) {
            // A qualified path can never be a local: it is an item, an
            // `Enum::Variant`, or `[Resolve-Unbound]`.
            let (prefix, last) = (segs[..segs.len() - 1].join("::"), &segs[segs.len() - 1]);
            let is_variant = self.items.enums.get(&prefix).map_or(false, |e| e.variants.iter().any(|v| &v.name == last));
            if !is_variant {
                return Err(crate::modres::named("diag.unbound-name", format!("nothing named `{}`", name.strip_prefix("std::").unwrap_or(&name))));
            }
        }
        if TRUSTED_STD_CALLS.contains(&name.as_str()) {
            return Err(crate::modres::named(
                "diag.trusted-outside-unsafe",
                format!("`{}` is only called, inside `unsafe {{ }}`, never used as a value", name.strip_prefix("std::").unwrap_or(&name)),
            ));
        }
        if let Some(f) = self.items.fns.get(&name) {
            // `[T-Item]`: a non-generic function item is a value of its
            // `fn(...) : τ` type -- and a function the program can call,
            // so its body is reached.
            if f.type_params.is_empty() {
                self.record_instantiation(&name, &[]);
                return Ok(Some(Type::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()))));
            }
            return Ok(None);
        }
        if self.items.externs.contains_key(&name) {
            // D-0252: an `extern fn` is called, inside `unsafe`, and is
            // never a value: a `fn` value's call needs no `unsafe`.
            return Err(crate::modres::named(
                "diag.trusted-outside-unsafe",
                format!("`{}` is a C function: it is only called, inside `unsafe {{ }}`, never used as a value", name.strip_prefix("std::").unwrap_or(&name)),
            ));
        }
        Ok(None)
    }

    // `[T-Item-Generic]` (D-0093): the type of a generic function item
    // used as a value, when this position instantiates it — explicit
    // type arguments (`id<i64>`), or the expected `fn(...)` type
    // (`fn(i32) : i32 f = id;`). `None` when the path is not a generic
    // item, or when neither is at hand (an argument position, whose call
    // instantiates it by its own rules, or a binding, which rejects it).
    fn generic_item_value(&mut self, segs: &[String], targs: &[Type], expected: Option<&Type>) -> TResult {
        let name = segs.join("::");
        let Some(f) = self.items.fns.get(&name).cloned() else { return Ok(None) };
        if f.type_params.is_empty() {
            return Ok(None);
        }
        let decl = Type::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()));
        if !targs.is_empty() {
            // `[Generic-Call-Arity]`'s analog: exactly one type argument
            // per parameter.
            if targs.len() != f.type_params.len() {
                return Err(mismatch(format!(
                    "`{}` takes {} type argument{}, and {} {} given",
                    name,
                    f.type_params.len(),
                    if f.type_params.len() == 1 { "" } else { "s" },
                    targs.len(),
                    if targs.len() == 1 { "is" } else { "are" }
                )));
            }
            let mut m = HashMap::default();
            for (p, a) in f.type_params.iter().zip(targs) {
                m.insert(p.clone(), apply_subst(a, &self.subst));
            }
            return Ok(Some(apply_subst(&decl, &m)));
        }
        if let Some(exp @ Type::Fn(eps, _)) = expected {
            if !has_unresolved_marker(exp) {
                if eps.len() != f.params.len() {
                    return Err(mismatch(format!(
                        "`{}` takes {} parameter{}, but `{}` is expected",
                        name,
                        f.params.len(),
                        if f.params.len() == 1 { "" } else { "s" },
                        exp
                    )));
                }
                let mut m = HashMap::default();
                unify_type_shape(&decl, exp, &mut m, &f.type_params);
                if f.type_params.iter().all(|p| m.contains_key(p)) {
                    let inst = apply_subst(&decl, &m);
                    if &inst == exp {
                        return Ok(Some(inst));
                    }
                    return Err(mismatch(format!("`{}` cannot have the type `{}`: it would be `{}`", name, exp, inst)));
                }
            }
        }
        Ok(None)
    }

    // D-0282: an access to `root.path` in `mode` (a `Vec`
    // element read or borrowed), against the live holders other than
    // those the access goes through.
    fn tier_access(&mut self, root: &str, path: &[PElem], mode: Mode, through: &HashSet<String>) -> Result<(), String> {
        let mut point = false;
        for (h, facts) in self.flow.deriv.iter() {
            if through.contains(h) || self.lookup_local(h).map_or(false, |t| self.tier_d_type(&t)) {
                continue;
            }
            if !self.compound_held.contains(h) && !tier_report::live_after(h, self.current_line, self.current_col) {
                continue;
            }
            for (k, v) in facts {
                if *v == Tri::F || k.root != root || permitted(mode.clone(), k.mode.clone()) {
                    continue;
                }
                match overlap3(&k.path, path) {
                    Ov::No => {}
                    Ov::Maybe => point = true,
                    // The hidden reference's fact may be its whole
                    // container (`&mut v[f(x)]`): which element is a value.
                    Ov::Yes if *v != Tri::T || self.compound_held.contains(h) => point = true,
                    Ov::Yes => {
                        let msg = format!("{} still borrows `{}`, so it cannot be {} here", holder_text(h), shown_place(&k.root, &k.path), if mode == Mode::Exclusive { "written or borrowed `mut`" } else { "read" });
                        return self.refute(&crate::modres::named("diag.aliasing-conflict", msg));
                    }
                }
            }
        }
        if point {
            let line = self.current_line;
            tier_report::with(|r| {
                r.point_checks += 1;
                r.events.push(format!("P\t{}\t{}\telement", line, root));
            });
            tier_report::left(format!("line {}: an element of `{}` that only values decide", line, shown_name(root)));
        }
        Ok(())
    }

    // An element of a `Vec` read (`v[i]`, `v[i].f`) or borrowed: what it
    // reaches, for `tier_access`.
    fn tier_elem_access(&mut self, e: &Expr, mode: Mode) -> Result<(), String> {
        if !tier_report::on() || !self.report || self.flow.unreachable {
            return Ok(());
        }
        let mut b: &Expr = e;
        loop {
            match &strip_parens_tc(b).kind {
                ExprKind::Field(x, _) => b = x,
                _ => break,
            }
        }
        if let Some((root, path)) = self.vec_element_root(b) {
            let mut through = HashSet::default();
            if let ExprKind::Index(base, _) = &strip_parens_tc(b).kind {
                let mut x: &Expr = base;
                loop {
                    match &strip_parens_tc(x).kind {
                        ExprKind::Field(y, _) | ExprKind::Deref(y) | ExprKind::Index(y, _) => x = y,
                        ExprKind::Path(segs, _) if segs.len() == 1 => {
                            through.insert(segs[0].clone());
                            break;
                        }
                        _ => break,
                    }
                }
            }
            return self.tier_access(&root, &path, mode, &through);
        }
        Ok(())
    }

    // D-0282: rules at an assignment. (1) Its place is found
    // before its value is computed (D-0007): a value that takes the place's
    // container exclusively (`v[0].x = grow(&mut v)`) may move the place,
    // which is a conflict decided here. (2) A place that holds a resource
    // is checked, when written, for the value it holds: a test of that
    // value (a discriminant), with no tracking state: a point check.
    fn tier_assign_rules(&mut self, lhs: &Expr, rhs: &Expr) -> Result<(), String> {
        if !tier_report::on() || !self.report || self.flow.unreachable {
            return Ok(());
        }
        let line = self.current_line;
        // The container the place is found in: its root's keys, extended by
        // the fields from the root to the root-most index (`p.cpu.r` of
        // `p.cpu.r[0]`); only a value taking that or a place around it
        // exclusively may move the element. Through a dereference, the
        // whole referent.
        let through_elem = {
            let mut b: &Expr = lhs;
            let mut elem = false;
            let mut fields: Vec<String> = Vec::new();
            loop {
                match &strip_parens_tc(b).kind {
                    ExprKind::Field(x, f) => {
                        fields.push(f.clone());
                        b = x;
                    }
                    ExprKind::Deref(x) => {
                        fields.clear();
                        b = x;
                    }
                    ExprKind::Index(x, _) => {
                        elem = true;
                        fields.clear();
                        b = x;
                    }
                    ExprKind::Call(c, xs) => {
                        // `*Vec::index_exclusive(&mut v, i) = …`
                        if matches!(&c.kind, ExprKind::Path(segs, _) if segs.last().map_or(false, |l| l.starts_with("index"))) && !xs.is_empty() {
                            elem = true;
                            fields.clear();
                            b = &xs[0];
                        } else {
                            break;
                        }
                    }
                    ExprKind::Borrow(_, x) => b = x,
                    _ => break,
                }
            }
            if elem {
                let ext: Vec<PElem> = fields.iter().rev().map(|f| PElem::Field(f.clone())).collect();
                let mut keys = self.inherited_facts(b, 0).unwrap_or_default();
                for (k, _) in keys.iter_mut() {
                    k.path.extend(ext.iter().cloned());
                }
                keys
            } else {
                Vec::new()
            }
        };
        if !through_elem.is_empty() {
            let mut excl: Vec<DerivKey> = Vec::new();
            let mut bs = Vec::new();
            excl_borrows_in(rhs, &mut bs);
            for b in &bs {
                if let Some(v) = self.inherited_facts(b, 0) {
                    excl.extend(v.into_iter().map(|(k, _)| k));
                }
            }
            for (k, _) in &through_elem {
                if excl.iter().any(|x| x.root == k.root && overlap3(&x.path, &k.path) != Ov::No) {
                    let msg = format!("the place is found before the value is computed, and the value takes `{}` exclusively, which may move the place (evaluation order, D-0007)", shown_name(&k.root));
                    return self.refute(&crate::modres::named("diag.stale-binding", msg));
                }
            }
        }
        if self.static_place_type(lhs).map_or(false, |t| is_resource_ty(self.items, &t)) {
            tier_report::with(|r| {
                r.point_checks += 1;
                r.events.push(format!("P\t{}\tresource place written: its value is tested", line));
            });
        }
        Ok(())
    }

    // D-0282: the call-site rule. Each argument that is or
    // holds a reference reaches places of this frame (its *whole* keys, in
    // its parameter's mode). The callee's access summary says what it does
    // through that parameter (its *projected* keys). A live holder of the
    // caller conflicts with what the callee does (use-time, D-0018); two
    // arguments conflict when what is done through one meets the other's
    // whole referent, which lives for the call. A certain conflict is
    // reported as the static pass reports one; one that depends on values
    // (an index, a range) is a point check.
    fn tier_call_rule(&mut self, callee: &Expr, args: &[Expr]) -> Result<(), String> {
        if !tier_report::on() || !self.report || self.flow.unreachable {
            return Ok(());
        }
        // An argument that reads a reference binding is a copy of it: a
        // later argument that assigns the binding leaves that copy with no
        // name, so what it reaches is the run's to check.
        for i in 1..args.len() {
            let mut assigned = Vec::new();
            visit_exprs(&args[i], &mut |e| {
                if let ExprKind::Assign(l, _) = &e.kind {
                    if let ExprKind::Path(segs, _) = &strip_parens_tc(l).kind {
                        if segs.len() == 1 {
                            assigned.push(segs[0].clone());
                        }
                    }
                }
            });
            let mut read = false;
            for a in &args[..i] {
                visit_exprs(a, &mut |e| {
                    if let ExprKind::Path(segs, _) = &e.kind {
                        read |= segs.len() == 1 && assigned.contains(&segs[0]);
                    }
                });
            }
            if let Some(x) = assigned.iter().find(|x| read && self.lookup_local(x).map_or(false, |t| holds_ref(self.items, &t))) {
                tier_report::left(format!("line {}: an argument assigns `{}` after an earlier one read it", self.current_line, x));
            }
        }
        // A reference argument is formed before the later arguments are
        // computed (D-0007). One of them that takes its root exclusively,
        // assigns into it or moves it may move what it reaches (`set(&mut
        // v[0], grow(&mut v))`): the run's stale-binding check decides.
        for i in 0..args.len().saturating_sub(1) {
            let Some(facts) = self.inherited_facts(&args[i], 0) else { continue };
            let roots: Vec<String> = facts.iter().map(|(k, _)| k.root.clone()).collect();
            if roots.is_empty() {
                continue;
            }
            let mut acts = false;
            for a in &args[i + 1..] {
                let mut bs = Vec::new();
                excl_borrows_in(a, &mut bs);
                for b in &bs {
                    if let Some(v) = self.inherited_facts(b, 0) {
                        acts |= v.iter().any(|(k, _)| roots.contains(&k.root));
                    }
                }
                visit_exprs(a, &mut |e| match &e.kind {
                    ExprKind::Assign(l, _) => acts |= root_local_name(l).map_or(false, |r| roots.contains(&r)),
                    ExprKind::Call(_, xs) => {
                        for x in xs {
                            if let ExprKind::Path(segs, _) = &strip_parens_tc(x).kind {
                                acts |= segs.len() == 1 && roots.contains(&segs[0]);
                            }
                        }
                    }
                    _ => {}
                });
            }
            if acts {
                tier_report::left(format!("line {}: a later argument acts on what an earlier reference argument reaches", self.current_line));
                break;
            }
        }
        // A call through a local `fn` value: whether it was emptied (moved
        // from) is a test of the value itself, a point check.
        if let ExprKind::Path(segs, _) = &callee.kind {
            if segs.len() == 1 && matches!(self.lookup_local(&segs[0]), Some(Type::Fn(..)) | Some(Type::Closure(_))) {
                let line = self.current_line;
                tier_report::with(|r| {
                    r.point_checks += 1;
                    r.events.push(format!("P\t{}\tfn value called: its state is tested", line));
                });
            }
        }
        // `drop(*r)` and the like: moved out through a reference.
        for a in args {
            if let ExprKind::Deref(inner) = &strip_parens_tc(a).kind {
                if let ExprKind::Path(segs, _) = &strip_parens_tc(inner).kind {
                    let moves = self.static_place_type(a).map_or(false, |t| is_resource_ty(self.items, &t) || matches!(&t, Type::Named(n, _) if n == "std::Atomic" || n == "std::Vec" || n == "std::String"));
                    if segs.len() == 1 && moves && matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..))) {
                        let msg = format!("`*{}` cannot be moved out: `{}` still refers to it", segs[0], segs[0]);
                        return self.refute(&format!("{}{}", crate::modres::named("diag.move-while-aliased", msg), crate::diagnostics::at(a.line, a.col)));
                    }
                }
            }
        }
        let Some(f) = self.resolve_fn_path(callee) else { return Ok(()) };
        let key = match &callee.kind {
            ExprKind::Path(segs, _) => segs.join("::"),
            _ => String::new(),
        };
        // The bindings a reference argument is formed through (`r` in
        // `&mut r.x`, `Vec::index_shared(&*r, 0)`, `r` itself): reaching
        // through them is not a conflict with them. A binding named only
        // in an index or a value argument is checked as any other holder.
        fn roots_in(e: &Expr, out: &mut HashSet<String>) {
            match &e.kind {
                ExprKind::Path(segs, _) if segs.len() == 1 => {
                    out.insert(segs[0].clone());
                }
                ExprKind::Paren(x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Borrow(_, x) | ExprKind::Index(x, _) | ExprKind::SliceOf(_, x, _, _) => roots_in(x, out),
                // A call on a reference (D-0011): its reference arguments,
                // not its values (`String::view(&s, h.lo, h.hi)` reaches
                // through `s`, not `h`).
                ExprKind::Call(_, xs) => {
                    for x in xs {
                        if matches!(&strip_parens_tc(x).kind, ExprKind::Borrow(..) | ExprKind::SliceOf(..) | ExprKind::Path(..) | ExprKind::Call(..)) {
                            roots_in(x, out);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut through: HashSet<String> = HashSet::default();
        for (p, a) in f.params.iter().zip(args.iter()) {
            // A type parameter's argument counts only when its value is
            // or holds a reference (`*r` given to `Vec::push` is a copy).
            let generic_ref = matches!(&p.ty, Type::Named(n, ta) if ta.is_empty() && f.type_params.iter().any(|tp| tp == n))
                && (matches!(&strip_parens_tc(a).kind, ExprKind::Borrow(..) | ExprKind::SliceOf(..)) || self.value_shape(a).or_else(|| self.static_place_type(a)).map_or(false, |t| is_borrow_ty(&t) || holds_ref(self.items, &t)));
            if is_borrow_ty(&p.ty) || holds_ref(self.items, &p.ty) || generic_ref {
                roots_in(a, &mut through);
            }
        }
        struct Access {
            idx: usize,
            holder: Option<String>,
            whole: Vec<DerivKey>,
            proj: Vec<DerivKey>,
            // The summary's field path of each `proj` key (D-0283's point
            // check extends the argument's place by it).
            fields: Vec<Vec<String>>,
            // The whole keys whose facts hold on some paths only (`?`): a
            // conflict through them is the run's to decide.
            unsure: Vec<(String, Vec<PElem>)>,
        }
        let sums = crate::tier::summaries(self.items);
        let mut accesses: Vec<Access> = Vec::new();
        for (i, (p, a)) in f.params.iter().zip(args.iter()).enumerate() {
            let mode = match &p.ty {
                Type::Ref(_, m) | Type::Slice(_, m) => m.clone(),
                t if holds_ref(self.items, t) => Mode::Shared,
                // A type parameter: the argument's own value, when it is
                // or holds a reference (a number is copied).
                Type::Named(n, ta) if ta.is_empty() && f.type_params.iter().any(|tp| tp == n) => match &strip_parens_tc(a).kind {
                    // `&x` given to `T b`: `T` is the borrow's type.
                    ExprKind::Borrow(m, _) | ExprKind::SliceOf(m, _, _, _) => m.clone(),
                    _ => {
                        let at = self.value_shape(a).or_else(|| self.static_place_type(a));
                        match at {
                            Some(t) if is_borrow_ty(&t) || holds_ref(self.items, &t) => Mode::Shared,
                            _ => continue,
                        }
                    }
                },
                _ => continue,
            };
            let a = strip_parens_tc(a);
            let holder = match &a.kind {
                ExprKind::Path(segs, _) if segs.len() == 1 && self.flow.is_tracked(&segs[0]) => Some(segs[0].clone()),
                _ => None,
            };
            let mut whole: Vec<DerivKey> = Vec::new();
            let mut unsure: Vec<(String, Vec<PElem>)> = Vec::new();
            if let Some(h) = &holder {
                let ht = self.lookup_local(h);
                if matches!(ht, Some(Type::Ref(..)) | Some(Type::Slice(..))) || ht.as_ref().map_or(false, |t| holds_ref(self.items, t)) {
                    whole.push(DerivKey { root: referent_root(h), path: Vec::new(), mode: mode.clone() });
                    if let Some(fs) = self.flow.deriv.get(h) {
                        whole.extend(fs.keys().map(|k| DerivKey { mode: mode.clone(), ..k.clone() }));
                        unsure.extend(fs.iter().filter(|(_, v)| **v == Tri::U).map(|(k, _)| (k.root.clone(), k.path.clone())));
                    }
                } else {
                    continue; // a value passed by copy or move
                }
            } else if let Some(v) = self.inherited_facts(a, 0) {
                for (k, t) in v.into_iter().filter(|(k, _)| k.root != "<tier D>") {
                    if t == Tri::U {
                        unsure.push((k.root.clone(), k.path.clone()));
                    }
                    whole.push(DerivKey { mode: mode.clone(), ..k });
                }
            }
            if whole.is_empty() {
                continue;
            }
            // What the callee does through this parameter.
            let (proj, fields) = match sums.param(&key, i).filter(|_| matches!(&p.ty, Type::Ref(..) | Type::Slice(..))) {
                Some(acc) => {
                    let shared_param = matches!(&p.ty, Type::Ref(_, Mode::Shared) | Type::Slice(_, Mode::Shared));
                    let mut out = Vec::new();
                    let mut fields = Vec::new();
                    for k in &whole {
                        for w in &acc.writes {
                            let mut q = k.clone();
                            // Through a shared parameter a write is interior
                            // mutability (`Atomic`, a guard): still shared.
                            q.mode = if shared_param { Mode::Shared } else { Mode::Exclusive };
                            q.path.extend(w.iter().map(|x| PElem::Field(x.clone())));
                            out.push(q);
                            fields.push(w.clone());
                        }
                        for r in &acc.reads {
                            let mut q = k.clone();
                            q.mode = Mode::Shared;
                            q.path.extend(r.iter().map(|x| PElem::Field(x.clone())));
                            out.push(q);
                            fields.push(r.clone());
                        }
                    }
                    (out, fields)
                }
                None => (whole.clone(), vec![Vec::new(); whole.len()]),
            };
            accesses.push(Access { idx: i, holder, whole, proj, fields, unsure });
        }
        let mut point_checks: Vec<String> = Vec::new();
        let site = (callee.line, callee.col);
        let mut pcs: Vec<crate::tier::PointCheck> = Vec::new();
        // What the callee does, against the caller's live holders. An
        // argument that is a visible borrow is checked against them where
        // it is formed (`rule.control.flow-analysis`), which refutes every
        // conflict this would, and names the borrow.
        for acc in &accesses {
            if matches!(&strip_parens_tc(&args[acc.idx]).kind, ExprKind::Borrow(..) | ExprKind::SliceOf(..)) {
                continue;
            }
            for (k, kf) in acc.proj.iter().zip(acc.fields.iter()) {
                for (h, facts) in self.flow.deriv.iter() {
                    let same_path = acc.holder.as_ref().map_or(false, |a| self.path_id(a) == self.path_id(h));
                    if Some(h) == acc.holder.as_ref() || same_path || through.contains(h) || accesses.iter().any(|x| x.holder.as_ref() == Some(h)) {
                        continue;
                    }
                    // A tier-D holder's conflicts are the run's to check.
                    if self.lookup_local(h).map_or(false, |t| self.tier_d_type(&t)) {
                        continue;
                    }
                    // A holder past its last use has ended (D-0111) -- as
                    // are the holders only it keeps alive: one rooted at
                    // its referent is checked through it.
                    if !tier_report::live_after(h, self.current_line, self.current_col) && !self.flow.deriv.iter().any(|(o, fs)| o != h && tier_report::live_after(o, self.current_line, self.current_col) && fs.keys().any(|k| k.root == referent_root(h))) {
                        continue;
                    }
                    for (hk, v) in facts {
                        if *v == Tri::F || hk.root != k.root || permitted(k.mode.clone(), hk.mode.clone()) {
                            continue;
                        }
                        let sure = *v == Tri::T && !acc.unsure.iter().any(|(r, p)| *r == k.root && k.path.starts_with(p));
                        let pc = || crate::tier::PointCheck { arg: acc.idx, fields: kf.clone(), mode: k.mode.clone(), other: crate::tier::PointOther::Holder(h.clone()), other_mode: hk.mode.clone(), callee: shown_callee(&key).to_string() };
                        match overlap3(&hk.path, &k.path) {
                            Ov::No => {}
                            Ov::Maybe => {
                                point_checks.push(format!("P\t{}\t{}\t{}", self.current_line, shown_name(&k.root), h));
                                pcs.push(pc());
                            }
                            Ov::Yes if !sure => {
                                point_checks.push(format!("P\t{}\t{}\t{} (on some paths)", self.current_line, shown_name(&k.root), h));
                                pcs.push(pc());
                            }
                            Ov::Yes => {
                                let msg = format!("`{}` {} `{}` through argument {} while {} still borrows `{}`; end that borrow first", shown_callee(&key), shown_access(&k.mode), shown_place(&k.root, &k.path), acc.idx + 1, holder_text(h), shown_place(&hk.root, &hk.path));
                                return self.refute(&crate::modres::named("diag.aliasing-conflict", msg));
                            }
                        }
                    }
                }
            }
        }
        // Between the arguments.
        for (ai, a) in accesses.iter().enumerate() {
            for b in accesses.iter().skip(ai + 1) {
                // `[Swap-Places]`: a place swapped with itself is no conflict.
                if (key == "std::swap" || key == "std::swap_places") && args.len() == 2 && same_place(&args[0], &args[1]) {
                    continue;
                }
                // One reference passed twice, or a copy of it beside it:
                // one path, which does not conflict with itself.
                if let (Some(x), Some(y)) = (&a.holder, &b.holder) {
                    let refs = matches!(self.lookup_local(x), Some(Type::Ref(..) | Type::Slice(..))) && matches!(self.lookup_local(y), Some(Type::Ref(..) | Type::Slice(..)));
                    if refs && self.path_id(x) == self.path_id(y) {
                        continue;
                    }
                }
                // Each pair: what the callee does through one argument (`x`,
                // with its summary fields), the other's whole referent (`y`).
                let pairs = a.proj.iter().zip(a.fields.iter()).flat_map(|(x, f)| b.whole.iter().map(move |y| (x, f, y, a.idx, b.idx))).chain(b.proj.iter().zip(b.fields.iter()).flat_map(|(y, f)| a.whole.iter().map(move |x| (y, f, x, b.idx, a.idx))));
                for (x, xf, y, xi, yi) in pairs {
                    if x.root != y.root || permitted(x.mode.clone(), y.mode.clone()) {
                        continue;
                    }
                    let is_unsure = |k: &DerivKey| a.unsure.iter().chain(b.unsure.iter()).any(|(r, p)| *r == k.root && k.path.starts_with(p));
                    let pc = crate::tier::PointCheck { arg: xi, fields: xf.clone(), mode: x.mode.clone(), other: crate::tier::PointOther::Arg(yi), other_mode: y.mode.clone(), callee: shown_callee(&key).to_string() };
                    // `[Swap-Places]`: the two places of a `swap`, of one
                    // type, are one place (no conflict) or disjoint; the
                    // values deciding which needs no check.
                    let swap = key == "std::swap" || key == "std::swap_places";
                    match overlap3(&x.path, &y.path) {
                        Ov::No => {}
                        Ov::Maybe if swap => {}
                        Ov::Yes if swap && (is_unsure(x) || is_unsure(y)) => {}
                        Ov::Maybe => {
                            point_checks.push(format!("P\t{}\t{}\targuments {} and {}", self.current_line, shown_name(&x.root), a.idx + 1, b.idx + 1));
                            pcs.push(pc);
                        }
                        Ov::Yes if is_unsure(x) || is_unsure(y) => {
                            point_checks.push(format!("P\t{}\t{}\targuments {} and {} (on some paths)", self.current_line, shown_name(&x.root), a.idx + 1, b.idx + 1));
                            pcs.push(pc);
                        }
                        Ov::Yes => {
                            let (through, other) = (xi, yi);
                            let msg = if x.mode == Mode::Exclusive {
                                format!("`{}` writes `{}` through argument {}, which argument {} also reaches", shown_callee(&key), shown_place(&x.root, &x.path), through + 1, other + 1)
                            } else {
                                format!("`{}` reads `{}` through argument {}, which argument {} reaches exclusively", shown_callee(&key), shown_place(&x.root, &x.path), through + 1, other + 1)
                            };
                            return self.refute(&crate::modres::named("diag.aliasing-conflict", msg));
                        }
                    }
                }
            }
        }
        for pc in pcs {
            crate::tier::record_point_check(self.items, site, pc);
        }
        if !point_checks.is_empty() {
            point_checks.sort();
            point_checks.dedup();
            let line = self.current_line;
            tier_report::with(|r| {
                r.point_checks += point_checks.len();
                r.events.extend(point_checks);
            });
            tier_report::left(format!("line {}: a call checked when it is made (D-0283)", line));
        }
        Ok(())
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr], expected: Option<&Type>) -> TResult {
        self.tier_call_rule(callee, args)?;
        // enum variant construction, `Vi(e)`
        if let ExprKind::Path(segs, targs) = &callee.kind {
            if args.len() == 1 {
                if let Some(enum_name) = self.variant_enum(segs) {
                    if let Some(e) = self.items.enums.get(&enum_name).cloned() {
                        let vi = e.variants.iter().position(|v| &v.name == segs.last().unwrap());
                        if let Some(vi) = vi {
                            // `[T-Enum]`: a payload-less `Vi` is written bare.
                            if self.variant_arity(&enum_name, segs) == Some(false) {
                                return Err("diag.type-mismatch".to_string());
                            }
                            if let Some(raw_payload) = &e.variants[vi].payload {
                                let mut m: HashMap<String, Type> = HashMap::default();
                                for (p, t) in e.type_params.iter().zip(targs.iter()) {
                                    m.insert(p.clone(), apply_subst(t, &self.subst));
                                }
                                // `rule.type.expected`: an expected type of this
                                // enum supplies the type arguments not written,
                                // so `Some(5000000000)` where `Option<u64>` is
                                // expected checks its payload as a u64.
                                if let Some(Type::Named(n, eargs)) = expected {
                                    if *n == enum_name && eargs.len() == e.type_params.len() {
                                        for (p, t) in e.type_params.iter().zip(eargs.iter()) {
                                            if !m.contains_key(p) && !has_unresolved_marker(t) {
                                                m.insert(p.clone(), t.clone());
                                            }
                                        }
                                    }
                                }
                                let remaining: HashSet<String> = e.type_params.iter().filter(|p| !m.contains_key(*p)).cloned().collect();
                                let hint = apply_subst(raw_payload, &m);
                                let at = if mentions_any(&hint, &remaining) {
                                    let saved = self.infer_poisoned;
                                    self.infer_poisoned = true;
                                    let r = self.check_store_operand(&args[0], None);
                                    self.infer_poisoned = saved;
                                    r?
                                } else if matches!(&hint, Type::Named(n, a) if n == "std::StringView" && a.is_empty()) && is_bare_str_literal(&args[0]) {
                                    // D-0225: `[Str-Literal-View]` through the
                                    // constructor: the literal is a view of
                                    // itself, rewritten before the program is
                                    // checked again.
                                    let mut lit: &Expr = &args[0];
                                    while let ExprKind::Paren(inner) = &lit.kind {
                                        lit = inner;
                                    }
                                    note_rewrite(lit, Rewrite::LiteralView);
                                    Some(hint.clone())
                                } else {
                                    let t = self.check_store_operand(&args[0], Some(&hint))?;
                                    // `[T-Enum]`: the payload has the variant's type.
                                    if let Some(t) = &t {
                                        if !has_unresolved_marker(t) && !has_unresolved_marker(&hint) && *t != Type::Never && !ty_compat(t, &hint) {
                                            return Err(mismatch(format!("`{}` holds a `{}`, but this is `{}`", segs.last().unwrap(), hint, t)));
                                        }
                                    }
                                    t
                                };
                                if let Some(at) = &at {
                                    unify_type_shape(raw_payload, at, &mut m, &e.type_params);
                                }
                                if e.type_params.iter().all(|p| m.contains_key(p)) {
                                    let targs_c: Vec<Type> = e.type_params.iter().map(|p| m[p].clone()).collect();
                                    return Ok(Some(Type::Named(enum_name, targs_c)));
                                }
                                return Ok(None);
                            }
                        }
                    }
                }
            }
        }
        // `[Resolve-Unqualified]`: a local binding comes before an item,
        // and an item before an intrinsic of the same name (spec/21 §0),
        // so a call through a local is typed as a call of its value.
        let local_callee = matches!(&callee.kind, ExprKind::Path(segs, _) if segs.len() == 1 && self.is_local(&segs[0]));
        if let (ExprKind::Path(segs, targs), false) = (&callee.kind, local_callee) {
            let name = segs.join("::");
            if let Some(f) = self.items.fns.get(&name).cloned() {
                if TRUSTED_STD_CALLS.contains(&name.as_str()) && self.unsafe_depth == 0 {
                    return Err(crate::modres::named(
                        "diag.trusted-outside-unsafe",
                        format!("`{}` reads through a raw pointer, so it is called inside `unsafe {{ }}`", name.strip_prefix("std::").unwrap_or(&name)),
                    ));
                }
                if f.name == "drop" && f.assoc_type.is_some() {
                    // spec/07 §1: a destructor is only ever run by
                    // `[Destroy]`; `drop(x)` is the intrinsic.
                    return Err("diag.direct-destructor-call".to_string());
                }
                return self.check_user_call(&f, targs, args, expected);
            }
            if let Some(ext) = self.items.externs.get(&name).cloned() {
                if self.unsafe_depth == 0 {
                    return Err("diag.trusted-outside-unsafe".to_string());
                }
                if args.len() != ext.params.len() {
                    return Err("diag.type-mismatch".to_string());
                }
                for (i, a) in args.iter().enumerate() {
                    let pty = ext.params.get(i).map(|p| p.ty.clone());
                    self.check_expr(a, pty.as_ref())?;
                }
                return Ok(Some(ext.ret.clone()));
            }
            // an unmodeled intrinsic (sizeof, widen, rawptr_of, spawn, ...):
            // still walk the arguments for their own internal correctness,
            // but don't claim to know the intrinsic's result type beyond
            // a few high-value, easy cases.
            // `std`'s own helpers for `String::append` and `parse`
            // (`rule.stdlib.text`): not names anywhere else.
            if STD_ONLY_INTRINSICS.contains(&name.as_str()) && self.module != "std" {
                return Err("diag.unbound-name".to_string());
            }
            const TRUSTED_INTRINSICS: &[&str] = &["reclaim", "release", "copy_raw", "deallocate", "text_write", "parse_check", "parse_value", "read_volatile", "write_volatile", "rawptr_from_address", "rawptr_address"];
            if TRUSTED_INTRINSICS.contains(&name.as_str()) && self.unsafe_depth == 0 {
                return Err("diag.trusted-outside-unsafe".to_string());
            }
            // A reclaimed object's references are checked by the run
            // against the record of raw storage (`[Reclaim]`), which the
            // static pass does not follow: the program is not decided
            // (D-0284). `std`'s own uses are its containers' bookkeeping.
            if name == "reclaim" && self.module != "std" && !self.module.starts_with("std::") {
                tier_report::left(format!("line {}: a reference to a reclaimed object is checked by the run", self.current_line));
            }
            // WI-15: an intrinsic of a fixed shape called in another
            // (`sizeof()`, `widen<i64>()`): rejected here, before both
            // tools' evaluation and lowering, which rely on the shape.
            if let Some(&(_, tmin, tmax, amin, amax, form)) = INTRINSIC_SHAPES.iter().find(|(ns, ..)| ns.contains(&name.as_str())) {
                if targs.len() < tmin || targs.len() > tmax || args.len() < amin || args.len() > amax {
                    return Err(crate::modres::named("diag.type-mismatch", format!("`{}` is written `{}{}`", name, name, form)));
                }
            }
            if name == "drop" && args.len() == 1 {
                if let Some(n) = root_local_name(&args[0]) {
                    if self.move_captures.contains_key(&n) {
                        // `drop(self.f_i)`: `[Destroy-Projection]`.
                        return Err("diag.move-out-of-field".to_string());
                    }
                }
                // `[Destroy]` on a place: `temporally-valid` (via the
                // lookup), `solitary` (`[Destroy-Not-Solitary]`), then
                // the binding is consumed.
                self.place_pos = true;
                self.check_expr(&args[0], None)?;
                if let Some((x, path)) = self.place_of(&args[0]) {
                    if path.is_empty() {
                        self.flow_consume(&x, "diag.destroy-while-aliased")?;
                    } else if self.place_is_resource(&x, &path) {
                        self.refute("diag.move-out-of-field")?;
                    }
                }
                return Ok(Some(Type::Void));
            }
            if name == "fault" {
                // `fault(d)` / `fault(d, message)` (spec/21 §0, D-0083): `d`
                // is not an expression but a diagnostic's name with `_` for
                // `-`, and it must name one that can happen at run time
                // (phase `dynamic` or `both` in spec/registry/diagnostics.md).
                let d = match args.first().map(|a| &a.kind) {
                    Some(ExprKind::Path(segs, _)) if segs.len() == 1 => segs[0].clone(),
                    _ => return Err(mismatch("`fault` takes a diagnostic's name, as in `fault(index_out_of_bounds)`, and optionally a `str` message".to_string())),
                };
                let id = format!("diag.{}", d.replace('_', "-"));
                let dynamic = crate::diagnostics::registry().get(&id).map_or(false, |i| matches!(i.phase.as_deref(), Some("dynamic") | Some("both")));
                if !dynamic {
                    return Err(crate::modres::named(
                        "diag.unbound-name",
                        format!("`{}` names no run-time fault; `fault` takes a dynamic diagnostic of the registry with `_` for `-`, such as `assert_failed`, `index_out_of_bounds`, `unwrap_failed` or `alloc_failure`", d),
                    ));
                }
                if args.len() > 2 {
                    return Err(mismatch("`fault` takes a diagnostic's name and at most one `str` message".to_string()));
                }
                if let Some(m) = args.get(1) {
                    let t = self.check_expr(m, Some(&Type::Str))?;
                    if !matches!(t, Some(Type::Str) | None) {
                        return Err(mismatch("`fault`'s message is a `str`".to_string()));
                    }
                }
                return Ok(Some(Type::Never));
            }
            if name == "spawn" && !self.flow.unreachable {
                // D-0213: lending `r` to the thread borrows `*r`.
                for k in self.spawn_lent_refs(args) {
                    self.flow_clash(&k.root, &k.path, k.mode.clone(), "diag.aliasing-conflict")?;
                }
            }
            if name == "spawn" {
                // `rule.conc.spawn`: the callee must be a fn value or a
                // `move` closure.
                if let Some(f0) = args.first() {
                    let borrowing = match &f0.kind {
                        ExprKind::Closure { is_move, .. } => !*is_move,
                        ExprKind::Path(segs, _) if segs.len() == 1 => self.closure_moves.get(&segs[0]) == Some(&false),
                        _ => false,
                    };
                    if borrowing {
                        return Err("diag.spawn-borrow-closure".to_string());
                    }
                }
            }
            if name == "join" && args.len() == 1 {
                // D-0071: the thread has finished; what it borrowed is free.
                if let ExprKind::Path(segs, _) = &strip_parens_tc(&args[0]).kind {
                    if segs.len() == 1 && !self.flow.unreachable {
                        self.flow.deriv.remove(&segs[0]);
                    }
                }
            }
            if name == "Mutex::new" && args.len() == 1 {
                // `Mutex::new<T>(T v) : mutex<T>` (spec/21 §0) is typed as
                // the generic call it is (`rule.fn.generic-call`): `T` from
                // an explicit argument, else the expected `mutex<τ>`, else
                // the argument's own type. The argument is checked with
                // inference live, so a nested `Vec::new()` that nothing
                // fixes is `[Generic-Call-Uninferable]`, not waved through.
                let hint = match (targs.first(), expected) {
                    (Some(t), _) => Some(apply_subst(t, &self.subst)),
                    (None, Some(Type::Mutex(i))) if !has_unresolved_marker(i) => Some((**i).clone()),
                    _ => None,
                };
                let t = self.check_store_operand(&args[0], hint.as_ref())?;
                return Ok(hint.or(t).map(|t| Type::Mutex(Box::new(t))));
            }
            if crate::each::NAMES.contains(&name.as_str()) {
                // `foreach` (D-0042): the expansion for the collection's
                // type, checked as the code it is. Its first argument is a
                // hidden binding; one of a type this pass does not know is
                // left to the dynamic evaluator.
                let t0 = match args.first().map(|a| &a.kind) {
                    Some(ExprKind::Path(segs, _)) if segs.len() == 1 => self.lookup_local(&segs[0]).map(|t| apply_subst(&t, &self.subst)),
                    _ => None,
                };
                let Some(t0) = t0.filter(|t| !has_unresolved_marker(t)) else {
                    return Ok(None);
                };
                let x = crate::each::expand(&name, &t0, args, callee.line, callee.col)?;
                let saved = self.privileged;
                self.privileged = true;
                let r = self.check_expr(&x, expected);
                self.privileged = saved;
                return r;
            }
            if name == "std::exit" {
                // `[Exit]` (D-0150): one `u8`, the program's exit status;
                // of type `never`, so it stands where any value is expected.
                if args.len() != 1 {
                    return Err(mismatch("`exit` takes one argument, the program's exit status (a `u8`)".to_string()));
                }
                self.check_expect(&args[0], &Type::Int(IntTy::U8))?;
                return Ok(Some(Type::Never));
            }
            if name == "std::print" {
                // `rule.stdlib.print` (spec/21 §2a): one argument, of a
                // printable type; a type parameter is checked at each
                // instantiation.
                if args.len() != 1 {
                    return Err("diag.type-mismatch".to_string());
                }
                let t = self.check_store_operand(&args[0], None)?;
                let printable = match &t {
                    None => true,
                    Some(t) if has_unresolved_marker(t) => true,
                    Some(t) => is_printable(t),
                };
                return if printable { Ok(Some(Type::Void)) } else { Err("diag.type-mismatch".to_string()) };
            }
            if name == "$assert_fail" {
                // `assert` after `modres` (D-0065): its message, if any,
                // `$fmt`'s text.
                if args.len() > 1 {
                    return Err("diag.type-mismatch".to_string());
                }
                if let Some(a) = args.first() {
                    self.check_expect(a, &Type::Str)?;
                }
                return Ok(Some(Type::Never));
            }
            if name == "std::assert" {
                // An `assert` `modres` did not rewrite: without a condition.
                return Err("diag.type-mismatch".to_string());
            }
            if name == "$fmt" {
                // `rule.stdlib.format` (spec/21 §2f, D-0038): `printf` and
                // `String::appendf` after `modres`. The format is a string
                // literal (`[Format-Invalid]` otherwise, or when it does not
                // parse), with one argument of its kind per specifier
                // (`[Format-Arg-Mismatch]`).
                let Some(ExprKind::StrLit(f)) = args.first().map(|a| &a.kind) else {
                    return Err(crate::modres::named("diag.format-invalid", "the format is a string literal, checked when the program is compiled".to_string()));
                };
                let pieces = crate::fmt::parse(f.as_bytes()).map_err(|m| crate::modres::named("diag.format-invalid", m))?;
                // One argument per specifier, and a `usize` before it for
                // each `*` (D-0100).
                let specs: Vec<crate::fmt::Kind> = crate::fmt::arg_kinds(&pieces);
                if specs.len() != args.len() - 1 {
                    return Err(mismatch(format!("the format takes {} argument{}, but {} follow{}", specs.len(), if specs.len() == 1 { "" } else { "s" }, args.len() - 1, if args.len() == 2 { "s" } else { "" })));
                }
                for (i, (k, a)) in specs.iter().zip(&args[1..]).enumerate() {
                    self.arg_temp_ok = true;
                    let t = self.check_store_operand(a, None);
                    self.arg_temp_ok = false;
                    let t = t?;
                    // A reference is formatted as the value it refers to
                    // (D-0042): `&i32` as an `i32`, `&mut String` as `&String`.
                    let t = match t {
                        Some(Type::Ref(inner, _)) if matches!(&*inner, Type::Named(n, _) if n == "std::String") => Some(Type::Ref(inner, Mode::Shared)),
                        Some(Type::Ref(inner, _)) if matches!(&*inner, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str) => Some(*inner),
                        other => other,
                    };
                    let is_enum = |t: &Type| match t {
                        Type::Named(n, _) => self.items.enums.contains_key(n),
                        Type::Ref(inner, _) => matches!(&**inner, Type::Named(n, _) if self.items.enums.contains_key(n)),
                        _ => false,
                    };
                    let ok = match &t {
                        None => true,
                        Some(t) if has_unresolved_marker(t) => true,
                        Some(Type::Never) => true,
                        // D-0100: a `*` width or precision is a `usize`.
                        Some(t) if *k == crate::fmt::Kind::Width => matches!(t, Type::Int(IntTy::Usize)),
                        // `%v`: any printable value (`rule.stdlib.print`), and
                        // an enum's variant name (D-0100).
                        Some(t) if *k == crate::fmt::Kind::Value => is_printable(t) || is_enum(t),
                        Some(Type::Int(_)) => *k == crate::fmt::Kind::Int,
                        Some(Type::F32 | Type::F64) => *k == crate::fmt::Kind::Float,
                        Some(Type::Str | Type::Bool) => *k == crate::fmt::Kind::Text,
                        Some(Type::Named(n, _)) if n == "std::StringView" => *k == crate::fmt::Kind::Text, // D-0053
                        Some(Type::Ref(inner, Mode::Shared)) => *k == crate::fmt::Kind::Text && matches!(&**inner, Type::Named(n, _) if n == "std::String"),
                        Some(_) => false,
                    };
                    if !ok {
                        let t = t.expect("a known type");
                        let why = if *k == crate::fmt::Kind::Width {
                            format!("a `*` width or precision takes a `usize`, and argument {} is `{}`", i + 1, t)
                        } else if *k == crate::fmt::Kind::Value {
                            format!("`%v` prints numbers, `bool`, text, an enum's variant name, and references to them; argument {} is `{}`", i + 1, t)
                        } else {
                            let takes = match k {
                                crate::fmt::Kind::Int => "an integer",
                                crate::fmt::Kind::Float => "a floating-point number",
                                _ => "text: a `str`, a `StringView`, a `bool` or a borrowed `String` (`&s`)",
                            };
                            let hint = match &t {
                                Type::Named(n, _) if n == "std::String" && *k == crate::fmt::Kind::Text => "; a `String` is passed by reference -- `&s`, or bind a `String` result first and pass `&name`",
                                Type::Named(n, _) if n == "std::String" => "; print text with `%s`",
                                _ => "",
                            };
                            format!("argument {} is `{}`, but its specifier takes {}{}", i + 1, t, takes, hint)
                        };
                        return Err(mismatch(why));
                    }
                }
                return Ok(Some(Type::Str));
            }
            if name == "min_value" || name == "max_value" {
                // `rule.arith.limits` (spec/06 §5a): no arguments, and the
                // type argument is written explicitly, as for the
                // conversions; it must be a number type.
                if !args.is_empty() {
                    return Err("diag.type-mismatch".to_string());
                }
                let Some(t) = targs.first().map(|t| apply_subst(t, &self.subst)) else {
                    return Err("diag.cannot-infer-type-parameter".to_string());
                };
                return match t {
                    // D-0051: a float's limits are its largest finite value
                    // and its negation.
                    Type::Int(_) | Type::F32 | Type::F64 => Ok(Some(t)),
                    _ if has_unresolved_marker(&t) => Ok(Some(t)),
                    _ => Err("diag.type-mismatch".to_string()),
                };
            }
            if name == "static_assert" {
                // `rule.module.static-assert` (D-0052): a constant `bool`
                // and an optional message, a string literal. Its value is
                // computed after this pass, per instantiation of a generic
                // body (`check_program`'s result); here it is typed and
                // recorded.
                if args.is_empty() {
                    return Err("diag.type-mismatch".to_string());
                }
                // `[Static-Assert-Not-Constant]` (D-0067): a condition that
                // is not a `bool`, or not a constant expression, has its
                // own diagnostic, which says what to use instead; an error
                // inside the condition is that error.
                let ct = self.check_expr(&args[0], Some(&Type::Bool))?;
                if matches!(&ct, Some(t) if !matches!(t, Type::Bool | Type::Never)) {
                    return Err("diag.static-assert-not-constant".to_string());
                }
                if !const_expr(self.items, &args[0]) {
                    return Err("diag.static-assert-not-constant".to_string());
                }
                // The condition first: what is wrong with it is what the
                // program most needs to hear. Then the message: a string
                // literal, or (D-0068) a format and its arguments, as
                // `assert`'s, each argument a constant expression.
                let mut message_fmt = None;
                let msg = match args.get(1).map(|m| &m.kind) {
                    None => None,
                    Some(ExprKind::StrLit(m)) if args.len() == 2 => Some(m.clone()),
                    Some(ExprKind::StrLit(_)) => {
                        if args[2..].iter().any(|a| !const_expr(self.items, a)) {
                            return Err("diag.static-assert-not-constant".to_string());
                        }
                        let line = args[1].line;
                        let col = args[1].col;
                        let fmt = Expr { kind: ExprKind::Call(Box::new(Expr { kind: ExprKind::Path(vec!["$fmt".to_string()], Vec::new()), line, col }), args[1..].to_vec()), line, col };
                        self.check_expr(&fmt, None)?;
                        message_fmt = Some(fmt);
                        None
                    }
                    Some(_) => return Err("diag.type-mismatch".to_string()),
                };
                // A generic body checked with its type parameters opaque
                // cannot compute it; each concrete instantiation does.
                if !self.subst.values().any(has_unresolved_marker) && self.report {
                    let mut sv: Vec<(&String, &Type)> = self.subst.iter().collect();
                    sv.sort_by(|a, b| a.0.cmp(b.0));
                    let key = (&args[0] as *const Expr as usize, format!("{:?}", sv));
                    self.static_asserts.push(StaticAssert { key, cond: args[0].clone(), subst: self.subst.clone(), line: args[0].line, message: msg, message_fmt });
                }
                return Ok(Some(Type::Void));
            }
            if !INTRINSIC_NAMES.contains(&name.as_str()) {
                // Not an item, not an intrinsic: resolve it as a name
                // first, so `[Resolve-Unbound]` is the static fact it is;
                // a bound closure or fn value then goes on as an
                // unmodeled callee.
                self.check_path(segs)?;
            }
            let saved = self.infer_poisoned;
            self.infer_poisoned = true;
            let mut r = Ok(());
            let mut arg_tys: Vec<Option<Type>> = Vec::with_capacity(args.len());
            // `[T-Alt]`: an unsuffixed literal operand of an alternative
            // operation takes its type from the other operand, as in a
            // binary expression; that operand is checked first.
            let alt = ALT_INTRINSICS.contains(&name.as_str()) && args.len() == 2;
            let lit_first = alt && is_bare_num_literal(&args[0]) && !is_bare_num_literal(&args[1]);
            let order: Vec<usize> = if lit_first { vec![1, 0] } else { (0..args.len()).collect() };
            let mut by_index: Vec<Option<Type>> = vec![None; args.len()];
            for &i in &order {
                let simd_two = SIMD_PAIR_INTRINSICS.contains(&name.as_str()) || alt;
                let hint = if alt && is_bare_num_literal(&args[i]) {
                    by_index[1 - i].clone().filter(|t| matches!(t, Type::Int(_)))
                } else if simd_two && i == 1 && matches!(&by_index[0], Some(Type::Simd(..))) {
                    // D-0254: the second operand of a lane-wise operation (a
                    // literal `[x; N]` included) is the first one's type.
                    by_index[0].clone()
                } else if (i == 0 && (name == "simd_min" || name == "simd_max")) || (i == 1 && name == "simd_select") {
                    // D-0254: with no vector operand before it, a literal
                    // takes the result's expected `simd` type.
                    expected.filter(|t| matches!(t, Type::Simd(..))).cloned()
                } else if name == "simd_select" && i == 2 {
                    by_index[1].clone().filter(|t| matches!(t, Type::Simd(..)))
                } else if name == "simd_store" && i == 2 {
                    // A literal vector stored into a slice of `τ` is a
                    // `simd<τ, N>` of its length.
                    let n = match &strip_parens_tc(&args[2]).kind {
                        ExprKind::ArrayLit(items) => Some(items.len() as u128),
                        ExprKind::ArrayRepeat(_, n) => Some(*n),
                        _ => None,
                    };
                    match (&by_index[0], n) {
                        (Some(Type::Slice(e, _)), Some(n)) => Some(Type::Simd(e.clone(), n)),
                        _ => None,
                    }
                } else if name == "simd_load" && i == 1 || name == "simd_store" && i == 1 {
                    Some(Type::Int(IntTy::Usize))
                } else if (name == "atomic_op" || name == "atomic_wait_op") && i == 1 {
                    Some(Type::Int(IntTy::U8))
                } else if name == "atomic_wait_op" && i == 2 {
                    Some(Type::Int(IntTy::U64))
                } else if name == "cpu_op" {
                    Some(Type::Int(IntTy::U8))
                } else if name == "atomic_op" && i >= 2 {
                    // D-0249, D-0253: the operands are the atomic's cell type.
                    by_index[0].as_ref().and_then(atomic_cell_ty)
                } else {
                    None
                };
                match self.check_store_operand(&args[i], hint.as_ref()) {
                    Ok(t) => by_index[i] = t,
                    Err(d) => {
                        r = Err(d);
                        break;
                    }
                }
            }
            arg_tys.extend(by_index);
            self.infer_poisoned = saved;
            r?;
            // D-0254: the lane-wise intrinsics, and the wrapping and
            // saturating ones on a `simd` of integers.
            if let Some(t) = simd_intrinsic_type(&name, targs, &arg_tys)? {
                return Ok(Some(t));
            }
            self.check_integer_intrinsic_operands(&name, targs, args, &arg_tys)?;
            // D-0091: `sqrt`, `floor`, `ceil`, `round`, `trunc` of an `f32`
            // or `f64`, of that type.
            if FLOAT_FNS.contains(&name.as_str()) {
                let name = name.trim_start_matches("std::");
                if args.len() != 1 {
                    return Err(mismatch(format!("`{}` takes one argument", name)));
                }
                return match arg_tys.first().cloned().flatten() {
                    Some(t @ (Type::F32 | Type::F64)) => Ok(Some(t)),
                    Some(t) if has_unresolved_marker(&t) => Err(mismatch(format!("`{}` takes an `f32` or `f64`; a type parameter's value is neither, whatever its bound", name))),
                    Some(t) => Err(mismatch(format!("`{}` takes an `f32` or `f64`, and this is `{}`; convert an integer with `to_float<f64>(…)`", name, t))),
                    None => Ok(None),
                };
            }
            // D-0101: `atan2(y, x)`, `powf(x, y)`: two operands of one float type.
            if FLOAT2_FNS.contains(&name.as_str()) {
                let name = name.trim_start_matches("std::");
                if args.len() != 2 {
                    return Err(mismatch(format!("`{}` takes two arguments", name)));
                }
                return match (arg_tys[0].clone(), arg_tys[1].clone()) {
                    (Some(a @ (Type::F32 | Type::F64)), Some(b)) if a == b => Ok(Some(a)),
                    (Some(a), Some(b)) if matches!(a, Type::F32 | Type::F64) && matches!(b, Type::F32 | Type::F64) => Err(mismatch(format!("`{}` takes two operands of one type, and these are `{}` and `{}`; a literal is `f64` unless suffixed (`1.0: f32`)", name, a, b))),
                    (Some(t), _) | (_, Some(t)) if !matches!(t, Type::F32 | Type::F64) && !has_unresolved_marker(&t) => Err(mismatch(format!("`{}` takes `f32` or `f64` operands, and this is `{}`; convert an integer with `to_float<f64>(…)`", name, t))),
                    _ => Ok(None),
                };
            }
            if alt {
                // `[T-Alt]`'s result: the operands' type τ, or `Option<τ>`
                // for the `checked_` family, so that a literal beside a
                // nested call (`wrapping_add(wrapping_mul(x, a), b)`) is
                // typed by it.
                let t = arg_tys.iter().flatten().find(|t| matches!(t, Type::Int(_))).cloned();
                return Ok(t.map(|t| if name.starts_with("checked_") { Type::Named("std::Option".to_string(), vec![t]) } else { t }));
            }
            // `[Spawn]`, `[Join]` (spec/19 §1): `spawn(f, …) : handle<R>` for
            // `f : fn(…) : R`, and `join(handle<R>) : R` (CHG-0093).
            if name == "spawn" && !args.is_empty() {
                // `[Spawn]`: the other arguments are the call's, typed
                // against the parameters of the function the thread runs,
                // as a call of it is: as many, each of its parameter's type
                // (a reference where it takes one), a generic one's type
                // parameters fixed by them.
                let callee: Option<(Vec<Param>, Vec<String>)> = match (self.resolve_fn_path(&args[0]), arg_tys.first()) {
                    (Some(f), _) => Some((f.params.clone(), f.type_params.clone())),
                    (None, Some(Some(Type::Fn(ps, _)))) => Some((ps.iter().enumerate().map(|(i, t)| Param { ty: t.clone(), name: format!("#{}", i + 1) }).collect(), Vec::new())),
                    _ => None,
                };
                if let Some((params, tps)) = callee {
                    if params.len() != args.len() - 1 {
                        return Err(mismatch(format!(
                            "`spawn(f, …)` passes its other arguments to `f`, which takes {} and is given {}",
                            params.len(),
                            args.len() - 1
                        )));
                    }
                    let mut m: HashMap<String, Type> = HashMap::default();
                    for (p, a) in params.iter().zip(arg_tys[1..].iter()) {
                        if let Some(a) = a {
                            unify_type_shape(&p.ty, a, &mut m, &tps);
                        }
                    }
                    let tset: HashSet<String> = tps.iter().cloned().collect();
                    self.check_arg_exact_types(&params, &args[1..], &arg_tys[1..], &m, &tset)?;
                }
                if let Some(Some(Type::Fn(_, r))) = arg_tys.first() {
                    if !has_unresolved_marker(r) {
                        return Ok(Some(Type::Handle(r.clone())));
                    }
                }
                // WI-15: what the thread runs is a function.
                if let Some(Some(t)) = arg_tys.first() {
                    if !matches!(t, Type::Fn(..) | Type::Closure(_) | Type::Never) && !has_unresolved_marker(t) {
                        return Err(crate::modres::named("diag.type-mismatch", format!("`spawn` runs a function: its first argument is a fn value or a `move` closure, not `{}`", t)));
                    }
                }
            }
            if name == "join" {
                if let Some(Some(Type::Handle(t))) = arg_tys.first() {
                    return Ok(Some((**t).clone()));
                }
            }
            // D-0204: `cancel(ref<handle<R>, shared>)`, and `std::try_join`'s
            // primitive `join_status(ref<handle<R>, shared>, u8 mode) : u64`.
            if name == "join_status" {
                if let Some(Some(Type::Ref(h, _))) = arg_tys.first() {
                    if matches!(**h, Type::Handle(_)) {
                        return Ok(Some(Type::Int(IntTy::U64)));
                    }
                }
            }
            if name == "cancel" {
                if let Some(Some(Type::Ref(h, _))) = arg_tys.first() {
                    if matches!(**h, Type::Handle(_)) {
                        return Ok(Some(Type::Void));
                    }
                }
            }
            // WI-15: the operand a thread or raw intrinsic takes, where its
            // type is known: `lock` a mutex's reference, `join` a handle,
            // `cancel` and `join_status` a handle's reference, `rawptr_of`
            // a reference (`spec/19` [Lock], [Join]; `spec/20` [Rawptr-Of]).
            if let Some(Some(t)) = arg_tys.first() {
                let fits = match name.as_str() {
                    "lock" => Some(matches!(t, Type::Ref(m, _) if matches!(**m, Type::Mutex(_)))),
                    "join" => Some(matches!(t, Type::Handle(_))),
                    "cancel" | "join_status" => Some(matches!(t, Type::Ref(h, _) if matches!(**h, Type::Handle(_)))),
                    "rawptr_of" => Some(matches!(t, Type::Ref(..))),
                    _ => None,
                };
                if fits == Some(false) && !matches!(t, Type::Never) && !has_unresolved_marker(t) {
                    let want = match name.as_str() {
                        "lock" => "a reference to a mutex: `lock(&m)`",
                        "join" => "the handle `spawn` gave",
                        "rawptr_of" => "a reference: `rawptr_of(&x)`",
                        _ => "a reference to the handle `spawn` gave",
                    };
                    return Err(crate::modres::named("diag.type-mismatch", format!("`{}` takes {}, not `{}`", name, want, t)));
                }
            }
            // D-0249 `rule.conc.atomic`, std-only: `atomic_op(ref<Atomic<T>, _>
            // a, u8 op, T x, T y) : T`, the value the operation found.
            if name == "atomic_op" {
                let t = arg_tys.first().cloned().flatten().as_ref().and_then(atomic_cell_ty);
                let Some(t) = t else {
                    return Err(mismatch("`atomic_op` takes a reference to an `Atomic` or an `AtomicPtr` first".to_string()));
                };
                for k in 2..4 {
                    if let Some(Some(vt)) = arg_tys.get(k) {
                        if !has_unresolved_marker(vt) && !has_unresolved_marker(&t) && !ty_compat(vt, &t) && !is_bare_num_literal(&args[k]) {
                            return Err(mismatch(format!("`atomic_op` on an `Atomic<{}>` takes `{}` operands, not `{}`", t, t, vt)));
                        }
                    }
                }
                return Ok(Some(t));
            }
            // D-0253, std-only: `atomic_wait_op(ref<Atomic<T>, _> a, u8 op, u64
            // seen) : u64`, the atomic's wake count; `cpu_op(u8 op)`, a fence
            // or a spin hint.
            if name == "atomic_wait_op" {
                let ok = matches!(arg_tys.first(), Some(Some(t)) if matches!(t, Type::Ref(a, _) if matches!(&**a, Type::Named(n, _) if n == "std::Atomic")));
                if !ok {
                    return Err(mismatch("`atomic_wait_op` takes a reference to an `Atomic` first".to_string()));
                }
                return Ok(Some(Type::Int(IntTy::U64)));
            }
            if name == "cpu_op" {
                return Ok(Some(Type::Void));
            }
            // `[Lock]` (spec/19 §2): `lock(ref<mutex<τ>, shared>) : guard<τ>`.
            if name == "lock" {
                if let Some(Some(Type::Ref(m, _))) = arg_tys.first() {
                    if let Type::Mutex(t) = &**m {
                        return Ok(Some(Type::Guard(t.clone())));
                    }
                }
            }
            // D-0123: a rotation keeps its operand's type.
            if name == "rotate_left" || name == "rotate_right" {
                if let Some(Some(t @ Type::Int(_))) = arg_tys.first() {
                    return Ok(Some(t.clone()));
                }
            }
            // D-0121: `read_volatile(p) : τ` and `write_volatile(p, v)` for
            // `p : rawptr<τ>`, τ plain (a resource crosses by `*p`, which
            // moves it).
            if name == "read_volatile" || name == "write_volatile" {
                let want = if name == "read_volatile" { 1 } else { 2 };
                if args.len() != want {
                    return Err(mismatch(format!("`{}` takes {} argument{}", name, want, if want == 1 { ": a raw pointer" } else { "s: a raw pointer and the value to write" })));
                }
                let Some(Some(Type::Rawptr(inner))) = arg_tys.first() else {
                    return Err(mismatch(format!("`{}` takes a `rawptr<T>` first", name)));
                };
                let t = (**inner).clone();
                if !has_unresolved_marker(&t) && is_resource_ty(self.items, &t) {
                    return Err(mismatch(format!("`{}` moves nothing: `{}` is a resource, which crosses raw memory by `*p` (moved in or out)", name, t)));
                }
                if name == "write_volatile" {
                    if let Some(Some(vt)) = arg_tys.get(1) {
                        if !has_unresolved_marker(vt) && !has_unresolved_marker(&t) && !ty_compat(vt, &t) && !is_bare_num_literal(&args[1]) {
                            return Err(mismatch(format!("`write_volatile` through a `rawptr<{}>` writes a `{}`, but the value is `{}`", t, t, vt)));
                        }
                    }
                    return Ok(Some(Type::Void));
                }
                return Ok(Some(t));
            }
            // D-0258: `rawptr_from_address<T>(a) : rawptr<T>` for `a : usize`,
            // and `rawptr_address(p) : usize` for `p : rawptr<T>`
            // (`[Rawptr-From-Address]`, `[Rawptr-Address]`; inside `unsafe`).
            if name == "rawptr_from_address" {
                if let Some(Some(t)) = arg_tys.first() {
                    if !has_unresolved_marker(t) && *t != Type::Int(IntTy::Usize) && !is_bare_num_literal(&args[0]) {
                        return Err(mismatch(format!("`rawptr_from_address` takes the address as a `usize`, not `{}`", t)));
                    }
                }
                return Ok(targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))));
            }
            if name == "rawptr_address" {
                if let Some(Some(t)) = arg_tys.first() {
                    if !has_unresolved_marker(t) && !matches!(t, Type::Rawptr(_) | Type::Never) {
                        return Err(mismatch(format!("`rawptr_address` takes a raw pointer, not `{}`", t)));
                    }
                }
                return Ok(Some(Type::Int(IntTy::Usize)));
            }
            // D-0116: `clone(&x) : T` for `x : T` with `T` clone.
            if name == "clone" {
                if args.len() != 1 {
                    return Err(mismatch("`clone` takes one argument, a reference: `clone(&x)`".to_string()));
                }
                let Some(Some(Type::Ref(inner, _))) = arg_tys.first() else {
                    return Err(mismatch("`clone` takes a reference to the value to copy: `clone(&x)`".to_string()));
                };
                let t = (**inner).clone();
                let is_bare_marker = matches!(&t, Type::Named(n, a) if a.is_empty() && n.starts_with('$'));
                if !is_bare_marker && has_unresolved_marker(&t) {
                    return Ok(Some(t));
                }
                if let Some(problem) = self.clone_problem(&t) {
                    return Err(mismatch(format!("`clone` needs its `T` to be `clone` ({}), and `{}` is not: {}", bound_members(Bound::Clone), t, problem)));
                }
                return Ok(Some(t));
            }
            return Ok(self.check_known_intrinsic(&name, targs, args));
        }
        // callee is itself an expression (closure value, etc.)
        let arity = match &callee.kind {
            ExprKind::Closure { params, .. } => Some(params.len()),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.closure_arity.get(&segs[0]).copied(),
            _ => None,
        };
        if arity.map_or(false, |n| n != args.len()) {
            return Err("diag.type-mismatch".to_string());
        }
        // `[Closure-Call]` of a binding holding a `move` closure that writes
        // a capture: `self` is an exclusive borrow of its place (D-0239),
        // an access the flow facts refute when another borrow of it lives
        // on every path (D-0245, `rule.control.flow-analysis`).
        if let ExprKind::Path(segs, _) = &callee.kind {
            if segs.len() == 1 && self.closure_self_writes.contains(&segs[0]) && self.flow.is_tracked(&segs[0]) {
                self.flow_clash_saying(&segs[0], &[], Mode::Exclusive, "diag.aliasing-conflict", Some("so it cannot be called here: the call changes what it captured"))?;
            }
        }
        let ct = self.check_expr(callee, None)?;
        if matches!(&ct, Some(t) if is_marker(t)) {
            return Err(unbounded_msg("a call"));
        }
        // Each argument at its parameter's type, as for a named function
        // (`rule.type.expected`): `f(5000000000)` for `f : fn(i64) : i64`.
        let ptys: Vec<Option<Type>> = match &ct {
            Some(Type::Fn(ps, _)) if ps.len() == args.len() => ps.iter().map(|t| Some(t.clone()).filter(|t| !has_unresolved_marker(t))).collect(),
            _ => vec![None; args.len()],
        };
        // And of it: `[T-Call]`'s premise, as `check_arg_types` makes it for
        // a named function (a bare literal takes the parameter's type; a
        // reference may weaken). Without it a closure or `fn` value took
        // an `i32` for its `i64`, a `bool` for an integer, an
        // `array<i32, 4>` for an `array<i32, 2>`, and the tools ran them
        // differently (or `cobc` emitted C that does not compile).
        for (i, (a, pt)) in args.iter().zip(ptys.iter()).enumerate() {
            let at = self.check_store_operand(a, pt.as_ref())?;
            if let (Some(pt), Some(at)) = (pt, at.as_ref()) {
                let literal_fits = matches!(pt, Type::Int(_) | Type::F32 | Type::F64);
                if *pt != *at && !weakens_to(at, pt) && !matches!(at, Type::Never) && !has_unresolved_marker(at) && !(literal_fits && is_bare_int_literal(a)) {
                    return Err(mismatch(format!("argument {} is `{}`, but the function value's parameter is `{}`{}", i + 1, at, pt, conversion_hint(pt, at, Some(a)))));
                }
            }
        }
        Ok(match ct {
            // `[T-Call]`: the callee is callable, with one argument per
            // parameter.
            Some(
                Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Array(..) | Type::Rawptr(_) | Type::Handle(_)
                | Type::Mutex(_) | Type::Guard(_),
            ) => return Err("diag.type-mismatch".to_string()),
            Some(Type::Named(n, _)) if !n.starts_with('$') => return Err("diag.type-mismatch".to_string()),
            Some(Type::Fn(ps, _)) if ps.len() != args.len() => return Err("diag.type-mismatch".to_string()),
            Some(Type::Fn(_, r)) => Some(*r),
            _ => None,
        })
    }

    // The operand and target types `rule.arith.alt` and
    // `rule.arith.convert` name (spec/06 §4, §5): an integer where the
    // rule says `τ integer`, a float where it says `τ ∈ {f32, f64}`, and
    // one `τ` for both operands of the alternative operations. Checked
    // per instantiation, so a generic body passing its `T` is rejected
    // here when `T` is not an integer; an operand still of an opaque
    // `T`, or of unknown type, is left alone.
    fn check_integer_intrinsic_operands(&self, name: &str, targs: &[Type], args: &[Expr], arg_tys: &[Option<Type>]) -> Result<(), String> {
        let opaque = |t: &Type| matches!(t, Type::Never) || has_unresolved_marker(t);
        let is_int = |t: &Type| matches!(t, Type::Int(_)) || opaque(t);
        let is_float = |t: &Type| matches!(t, Type::F32 | Type::F64) || opaque(t);
        let mismatch = || Err("diag.type-mismatch".to_string());
        match name {
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" | "checked_add"
            | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem" => {
                if arg_tys.iter().flatten().any(|t| !is_int(t)) {
                    return mismatch();
                }
                // A bare literal takes its type from the other operand.
                if let (Some(Some(Type::Int(a))), Some(Some(Type::Int(b)))) = (arg_tys.first(), arg_tys.get(1)) {
                    if a != b && !is_bare_num_literal(&args[0]) && !is_bare_num_literal(&args[1]) {
                        return mismatch();
                    }
                }
                Ok(())
            }
            // D-0123: an integer operand; a rotation's amount is a `u32`.
            "count_ones" | "leading_zeros" | "trailing_zeros" | "rotate_left" | "rotate_right" => {
                if let Some(Some(t)) = arg_tys.first() {
                    if !is_int(t) {
                        return Err(crate::modres::named("diag.type-mismatch", format!("`{}` counts or rotates the bits of an integer; `{}` is not one", name, t)));
                    }
                }
                if name.starts_with("rotate") {
                    if let Some(Some(k)) = arg_tys.get(1) {
                        if !opaque(k) && *k != Type::Int(IntTy::U32) && !is_bare_num_literal(&args[1]) {
                            return Err(crate::modres::named("diag.type-mismatch", format!("`{}`'s amount is a `u32`, not `{}`", name, k)));
                        }
                    }
                }
                Ok(())
            }
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" | "checked_narrow" => {
                // `[T-Convert]`'s kind premises (D-0051 adds floats to
                // `widen`, `reinterpret` and `to_float`'s operand).
                let num = |t: &Type| is_int(t) || is_float(t);
                let (src_ok, dst_ok): (&dyn Fn(&Type) -> bool, &dyn Fn(&Type) -> bool) = match name {
                    "widen" | "reinterpret" => (&num, &num),
                    "to_float" => (&num, &is_float),
                    "to_int" => (&is_float, &is_int),
                    _ => (&is_int, &is_int),
                };
                // What to write instead, for the mistakes a C programmer
                // makes most: a float through `narrow`, an integer
                // through `to_int`, a float result from `narrow`.
                let hint = |m: String| Err(format!("diag.type-mismatch{}{}", crate::diagnostics::DETAIL_SEP, m));
                if let Some(Some(a)) = arg_tys.first() {
                    if !src_ok(a) {
                        return hint(match (name, is_float(a)) {
                            ("narrow" | "narrow_wrapping", true) => format!("`{}` converts between integer types, and this is `{}`: a float becomes an integer with `to_int<T>(x)`", name, a),
                            ("to_int", false) if is_int(a) => format!("`to_int` converts a float, and this is `{}`: between integer types use `narrow<T>(x)` or `widen<T>(x)`", a),
                            _ => format!("`{}` converts a number, not `{}`", name, a),
                        });
                    }
                }
                if let Some(t) = targs.first().map(|t| apply_subst(t, &self.subst)) {
                    if !dst_ok(&t) {
                        return hint(match (name, is_float(&t)) {
                            ("narrow" | "narrow_wrapping", true) => format!("`{}` gives an integer type, not `{}`: to get a float use `to_float<{}>(x)`", name, t, t),
                            ("to_float", false) if is_int(&t) => format!("`to_float` gives `f32` or `f64`, not `{}`: to get an integer from a float use `to_int<{}>(x)`", t, t),
                            ("to_int", true) => format!("`to_int` gives an integer type, not `{}`: between floats use `widen<{}>(x)` or `to_float<{}>(x)`", t, t, t),
                            _ => format!("`{}<{}>`: `{}` is not a number type", name, t, t),
                        });
                    }
                    // The pair premises: `widen` only where every value fits
                    // (an integer into an integer, `f32` into `f64`),
                    // `reinterpret` only between two types of one width that
                    // differ in signedness or in being a float.
                    let bw = |t: &Type| match t {
                        Type::Int(i) => Some(i.bitwidth(crate::value::ADDR_WIDTH)),
                        Type::F32 => Some(32),
                        Type::F64 => Some(64),
                        _ => None,
                    };
                    if let Some(Some(src)) = arg_tys.first() {
                        let bad = match (name, src, &t) {
                            ("widen", Type::Int(si), Type::Int(di)) => {
                                let (sw, dw) = (si.bitwidth(crate::value::ADDR_WIDTH), di.bitwidth(crate::value::ADDR_WIDTH));
                                !match (si.signed(), di.signed()) {
                                    (x, y) if x == y => sw <= dw,
                                    (false, true) => sw < dw,
                                    _ => false,
                                }
                            }
                            ("widen", Type::F64, Type::F32) => true,
                            ("widen", Type::F32 | Type::F64, Type::F32 | Type::F64) => false,
                            ("widen", Type::Int(_), Type::F32 | Type::F64) | ("widen", Type::F32 | Type::F64, Type::Int(_)) => true,
                            ("reinterpret", Type::Int(si), Type::Int(di)) => bw(src) != bw(&t) || si.signed() == di.signed(),
                            ("reinterpret", Type::F32 | Type::F64, Type::F32 | Type::F64) => true,
                            ("reinterpret", Type::Int(_) | Type::F32 | Type::F64, Type::Int(_) | Type::F32 | Type::F64) => bw(src) != bw(&t),
                            _ => false,
                        };
                        if bad {
                            return hint(match (name, src, &t) {
                                ("widen", Type::Int(_), Type::Int(_)) => format!("`widen` converts only where every value fits, and not every `{}` fits in `{}`: use `narrow<{}>(x)` (checked) or `narrow_wrapping<{}>(x)`", src, t, t, t),
                                ("widen", Type::F64, Type::F32) => "`widen` cannot shrink `f64` to `f32`: use `to_float<f32>(x)`".to_string(),
                                ("widen", Type::Int(_), _) => format!("`widen` does not turn an integer into a float: use `to_float<{}>(x)`", t),
                                ("widen", _, Type::Int(_)) => format!("`widen` does not turn a float into an integer: use `to_int<{}>(x)` (checked)", t),
                                _ => format!("`reinterpret` keeps the bits, so `{}` and `{}` must have the same width and differ in signedness or in being a float", src, t),
                            });
                        }
                    }
                    // `[Narrow-Overflow]`, `[Float-To-Int-Invalid]`:
                    // static where the operand is constant.
                    if let (Type::Int(di), Some(e)) = (&t, args.first()) {
                        let (lo, hi) = int_range(*di);
                        if name == "narrow" {
                            if let Some(v) = self.fold_int(e) {
                                if v < lo || v > hi {
                                    return Err(crate::modres::named("diag.narrowing-overflow", format!("`narrow<{}>` of {} does not fit (`{}` is {} to {}); `narrow_wrapping<{}>` keeps the low bits", t, v, t, lo, hi, t)));
                                }
                            }
                        }
                        if name == "to_int" {
                            let lit = match &strip_parens_tc(e).kind {
                                ExprKind::FloatLit(x, _) => Some(*x),
                                ExprKind::Unary(UnOp::Neg, inner) => match &strip_parens_tc(inner).kind {
                                    ExprKind::FloatLit(x, _) => Some(-*x),
                                    _ => None,
                                },
                                _ => None,
                            };
                            if let Some(x) = lit {
                                let tr = x.trunc();
                                if !x.is_finite() || tr < lo as f64 || tr > hi as f64 {
                                    return Err(crate::modres::named("diag.narrowing-overflow", format!("`to_int<{}>` of {} does not fit (`{}` is {} to {})", t, x, t, lo, hi)));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // D-0118: `e` is `base.f` with `base` a `bitstruct` (directly or
    // through a reference): the field's width and the backing type.
    fn bitfield_of(&self, e: &Expr) -> Option<(u32, IntTy, String)> {
        let ExprKind::Field(base, f) = &strip_parens_tc(e).kind else { return None };
        let bt = match self.static_place_type(base)? {
            Type::Ref(t, _) | Type::Guard(t) => *t,
            t => t,
        };
        let Type::Named(sname, _) = bt else { return None };
        let sd = self.items.structs.get(&sname)?;
        let backing = sd.bits?;
        let fd = sd.fields.iter().find(|fd| &fd.name == f)?;
        Some((fd.width?, backing, sname))
    }

    // D-0118: a value written into a bit field of `width` bits must fit;
    // a literal that does not is refuted here (`[Bitfield-Overflow]`).
    fn check_bitfield_literal(&self, value: &Expr, width: u32, what: &str) -> Result<(), String> {
        if let Some(v) = literal_value(value).or_else(|| self.fold_int(value)) {
            let limit: i128 = if width >= 127 { i128::MAX } else { (1i128 << width) - 1 };
            if v < 0 || v > limit {
                return Err(crate::modres::named("diag.narrowing-overflow", format!("{} is {} bits wide, so it holds 0 to {}; {} does not fit", what, width, limit, v)));
            }
        }
        Ok(())
    }

    // D-0116: why `t` is not `clone`, or `None` when it is. A plain type is
    // copied by a read; a named resource type needs a `T::clone` — declared
    // (its own bounds must hold for `t`'s arguments) or derived
    // (`derive.rs`; then every field or payload must be `clone`). A type
    // reached again while its own fields are examined is taken as `clone`
    // (a recursive type through `Box`).
    fn clone_problem(&self, t: &Type) -> Option<String> {
        let mut seen = HashSet::default();
        self.clone_problem_in(t, &mut seen)
    }

    // D-0257: why `t` is not `plain` (bytes that mean the same anywhere),
    // or `None` when it is.
    fn plain_problem(&self, t: &Type) -> Option<String> {
        let mut seen = HashSet::default();
        self.plain_problem_in(t, &mut seen)
    }

    fn plain_problem_in(&self, t: &Type, seen: &mut HashSet<Type>) -> Option<String> {
        match t {
            Type::Named(n, a) if a.is_empty() && n.starts_with('$') => {
                if self.marker_bounds.get(n) == Some(&Bound::Plain) {
                    None
                } else {
                    Some(format!("type parameter `{}` is not declared `plain`", &n[1..]))
                }
            }
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Rawptr(_) | Type::Simd(..) | Type::Never => None,
            Type::Void => Some("`void` is no data".to_string()),
            Type::Str => Some("`str` names text the program holds, not bytes of its own".to_string()),
            Type::Ref(..) | Type::Slice(..) => Some("a reference reaches storage that is not its own".to_string()),
            Type::Fn(..) | Type::Closure(_) => Some("a function value is not data".to_string()),
            Type::Handle(_) => Some("a thread handle is not data".to_string()),
            Type::Mutex(_) => Some("a mutex is not plain data".to_string()),
            Type::Guard(_) => Some("a lock guard is not plain data".to_string()),
            Type::Array(e, _) => self.plain_problem_in(e, seen),
            Type::Named(n, args) => {
                if is_resource_ty(self.items, t) {
                    return Some(format!("`{}` is a resource", n.strip_prefix("std::").unwrap_or(n)));
                }
                if !seen.insert(t.clone()) {
                    return None;
                }
                if let Some(s) = self.items.structs.get(n) {
                    let subst: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    for fd in &s.fields {
                        let ft = apply_subst(&fd.ty, &subst);
                        if let Some(p) = self.plain_problem_in(&ft, seen) {
                            return Some(format!("field `{}` of `{}`: {}", fd.name, n.strip_prefix("std::").unwrap_or(n), p));
                        }
                    }
                    return None;
                }
                if let Some(e) = self.items.enums.get(n) {
                    let subst: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    for v in &e.variants {
                        if let Some(pt) = &v.payload {
                            let ft = apply_subst(pt, &subst);
                            if let Some(p) = self.plain_problem_in(&ft, seen) {
                                return Some(format!("variant `{}` of `{}`: {}", v.name, n.strip_prefix("std::").unwrap_or(n), p));
                            }
                        }
                    }
                    return None;
                }
                None
            }
        }
    }

    fn clone_problem_in(&self, t: &Type, seen: &mut HashSet<Type>) -> Option<String> {
        match t {
            Type::Named(n, a) if a.is_empty() && n.starts_with('$') => {
                if self.marker_bounds.get(n).map_or(false, |have| have.implies(Bound::Clone)) {
                    None
                } else {
                    Some(format!("type parameter `{}` is not declared `clone`", &n[1..]))
                }
            }
            // D-0218: `clone` reads through a shared reference, and an
            // exclusive reference copied out of one would write what it
            // reaches.
            Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive) => Some("an exclusive reference has no clone: a copy made through a shared reference could write what it reaches".to_string()),
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Never | Type::Ref(..) | Type::Rawptr(_) | Type::Slice(..) | Type::Simd(..) => None,
            Type::Array(e, _) => {
                if is_resource_ty(self.items, e) {
                    Some(format!("an array of `{}` (a resource) has no clone; a `Vec` of them does", e))
                } else {
                    self.clone_problem_in(e, seen)
                }
            }
            Type::Fn(..) | Type::Closure(_) => Some("a function value has no clone".to_string()),
            Type::Handle(_) => Some("a thread handle has no clone".to_string()),
            Type::Mutex(_) => Some("a mutex has no clone".to_string()),
            Type::Guard(_) => Some("a lock guard has no clone".to_string()),
            Type::Named(n, args) => {
                if !is_resource_ty(self.items, t) {
                    return if holds_excl_ref(self.items, t, 0) { Some(format!("`{}` holds an exclusive reference, which has no clone", n.strip_prefix("std::").unwrap_or(n))) } else { None };
                }
                if !seen.insert(t.clone()) {
                    return None;
                }
                let key = format!("{}::clone", n);
                let Some(f) = self.items.fns.get(&key) else {
                    let shown = n.strip_prefix("std::").unwrap_or(n);
                    return Some(format!("`{}` is a resource with no `{}::clone`", shown, shown));
                };
                if f.derived {
                    let params: Vec<String> = self.items.structs.get(n).map(|s| s.type_params.clone()).or_else(|| self.items.enums.get(n).map(|e| e.type_params.clone())).unwrap_or_default();
                    let m: HashMap<String, Type> = params.iter().cloned().zip(args.iter().cloned()).collect();
                    if let Some(s) = self.items.structs.get(n) {
                        for fd in &s.fields {
                            if let Some(p) = self.clone_problem_in(&apply_subst(&fd.ty, &m), seen) {
                                return Some(format!("its field `{}` is not: {}", fd.name, p));
                            }
                        }
                    } else if let Some(e) = self.items.enums.get(n) {
                        for v in &e.variants {
                            if let Some(pt) = &v.payload {
                                if let Some(p) = self.clone_problem_in(&apply_subst(pt, &m), seen) {
                                    return Some(format!("its variant `{}` holds a value that is not: {}", v.name, p));
                                }
                            }
                        }
                    }
                    None
                } else {
                    for ((tp, b), a) in f.type_params.iter().zip(f.type_bounds.iter()).zip(args.iter()) {
                        let Some(b) = b else { continue };
                        let bad = if *b == Bound::Clone {
                            self.clone_problem_in(a, seen)
                        } else if let Type::Named(an, aa) = a {
                            if aa.is_empty() && an.starts_with('$') {
                                (!self.marker_bounds.get(an).map_or(false, |have| have.implies(*b))).then(|| format!("type parameter `{}` is not declared `{}`", tp, b.name()))
                            } else {
                                (!self.bound_holds(*b, a)).then(|| format!("`{}` is not `{}`", a, b.name()))
                            }
                        } else {
                            (!self.bound_holds(*b, a)).then(|| format!("`{}` is not `{}`", a, b.name()))
                        };
                        if let Some(p) = bad {
                            let shown = n.strip_prefix("std::").unwrap_or(n);
                            return Some(format!("`{}::clone` needs its `{}` to be `{}`: {}", shown, tp, b.name(), p));
                        }
                    }
                    None
                }
            }
        }
    }

    fn check_known_intrinsic(&self, name: &str, targs: &[Type], _args: &[Expr]) -> Option<Type> {
        match name {
            "sizeof" | "alignof" => Some(Type::Int(IntTy::Usize)),
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                targs.first().map(|t| apply_subst(t, &self.subst))
            }
            // D-0122: the narrowed value, or `None`.
            "checked_narrow" => targs.first().map(|t| Type::Named("std::Option".to_string(), vec![apply_subst(t, &self.subst)])),
            // D-0123: a count of bits.
            "count_ones" | "leading_zeros" | "trailing_zeros" => Some(Type::Int(IntTy::U32)),
            "write_volatile" => Some(Type::Void),
            "rawptr_of" | "rawptr_from_address" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "rawptr_address" => Some(Type::Int(IntTy::Usize)),
            "reinterpret_ptr" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "dangling" | "null_rawptr" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "drop" | "release" | "copy_raw" | "deallocate" => Some(Type::Void),
            // `[Allocate]` (spec/21 §3b): `Result<rawptr<u8>, AllocError>`, so a
            // pointer bound by `Ok(p)` is typed before its first use (CHG-0321).
            "allocate" => Some(Type::Named("std::Result".to_string(), vec![Type::Rawptr(Box::new(Type::Int(IntTy::U8))), Type::Named("std::AllocError".to_string(), vec![])])),
            "str_len" | "slice_len" => Some(Type::Int(IntTy::Usize)),
            "str_byte" => Some(Type::Int(IntTy::U8)),
            "text_write" => Some(Type::Int(IntTy::Usize)),
            "parse_check" => Some(Type::Int(IntTy::Isize)),
            "parse_value" => targs.first().map(|t| apply_subst(t, &self.subst)),
            "append_native" => Some(Type::Void),
            "key_hash" => Some(Type::Int(IntTy::U64)),
            "swap_places" | "lend_places" => Some(Type::Void),
            "key_eq" | "key_less" | "key_less_at" => Some(Type::Bool),
            "str_ptr" => Some(Type::Rawptr(Box::new(Type::Int(IntTy::U8)))),
            // D-0134: a `str`'s bytes as a slice (`StringView::of`).
            "str_slice" => Some(Type::Slice(Box::new(Type::Int(IntTy::U8)), Mode::Shared)),
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" => {
                None // same type as its arguments; left unresolved rather than re-deriving arg types here
            }
            _ => None,
        }
    }

    fn check_user_call(&mut self, f: &std::sync::Arc<FnDecl>, targs: &[Type], args: &[Expr], expected: Option<&Type>) -> TResult {
        // `[T-Call]`: one argument per parameter.
        if args.len() != f.params.len() {
            return Err(mismatch(format!("`{}` takes {} argument{}, but {} {} given", fn_shown(f), f.params.len(), if f.params.len() == 1 { "" } else { "s" }, args.len(), if args.len() == 1 { "is" } else { "are" })));
        }
        let explicit: Vec<Type> = targs.iter().map(|t| apply_subst(t, &self.subst)).collect();
        // Type arguments a function does not take were ignored: a program's
        // own `parse` shadowing `std::parse` (D-0024) quietly ran the
        // wrong function for `parse<i64>(&s)`.
        if explicit.len() > f.type_params.len() {
            let std_key = format!("std::{}", f_key(f));
            let shadows = self.items.fns.get(&std_key).map_or(false, |g| !std::sync::Arc::ptr_eq(g, f) && !g.type_params.is_empty());
            let msg = if f.type_params.is_empty() {
                format!("`{}` takes no type arguments", fn_shown(f))
            } else {
                format!("`{}` takes {} type argument{}, but {} {} given", fn_shown(f), f.type_params.len(), if f.type_params.len() == 1 { "" } else { "s" }, explicit.len(), if explicit.len() == 1 { "is" } else { "are" })
            };
            let hint = if shadows { format!("; this is the program's own `{}`, which hides `{}` — write `{}<…>(…)` for the standard one", fn_shown(f), std_key, std_key) } else { String::new() };
            return Err(mismatch(format!("{}{}", msg, hint)));
        }
        let type_param_set: HashSet<String> = f.type_params.iter().cloned().collect();
        // D-0079: an unsuffixed literal argument whose parameter is not yet
        // known is not evidence for the type parameters. It is checked
        // after the other arguments and the expected type have fixed what
        // they can, typed by its parameter (a literal has no effects, so
        // the order is not observable).
        let deferred: Vec<bool> = args
            .iter()
            .enumerate()
            .map(|(i, a)| !f.type_params.is_empty() && (is_bare_num_literal(a) || is_bare_str_literal(a)) && f.params.get(i).map_or(false, |p| mentions_any(&apply_subst(&p.ty, &explicit_subst(f, &explicit)), &type_param_set)))
            .collect();
        let order: Vec<usize> = (0..args.len()).filter(|&i| !deferred[i]).chain((0..args.len()).filter(|&i| deferred[i])).collect();
        let mut by_index: Vec<Option<Type>> = vec![None; args.len()];
        let mut checked = vec![false; args.len()];
        for &i in &order {
            let a = &args[i];
            // A hint built from a declared param type that still mentions
            // one of `f`'s own type parameters (not fixed by an explicit
            // `<...>`) is not concrete -- passing it down as `expected`
            // would poison a nested generic call's own inference (e.g.
            // `Vec::push(&mut v, Vec::new())`: `item`'s declared type is
            // the bare `T`, not yet known when the second argument is
            // checked; `Vec::new()`'s own `T` must stay unresolved here
            // and be inferred afterward from the *first* argument's shape
            // instead, not from this poisoned hint).
            // The type parameters the arguments to the left have already
            // fixed count as known (`Vec::push(&mut v, Some(5000000000))`:
            // `v` fixes `T`, so the second argument expects `Option<u64>`).
            let mut known = explicit_subst(f, &explicit);
            for (j, t) in by_index.iter().enumerate() {
                if let (true, Some(p), Some(t)) = (checked[j], f.params.get(j), t) {
                    if !has_unresolved_marker(t) {
                        unify_type_shape(&p.ty, t, &mut known, &f.type_params);
                    }
                }
            }
            if deferred[i] {
                if let Some(exp) = expected.filter(|t| !has_unresolved_marker(t)) {
                    unify_type_shape(&f.ret, exp, &mut known, &f.type_params);
                }
            }
            let raw_hint = f.params.get(i).map(|p| apply_subst(&p.ty, &known));
            let poisoned = raw_hint.as_ref().map_or(true, |t| mentions_any(t, &type_param_set));
            let hint = raw_hint.filter(|_| !poisoned);
            let saved = self.infer_poisoned;
            if poisoned {
                self.infer_poisoned = true;
            }
            // D-0162: a text literal whose parameter, once the other
            // arguments and the expected type have fixed the type
            // parameters, is `String` is that `String` (`[Str-Literal-String]`
            // for a type parameter): recorded, and rewritten to
            // `String::from_str(L)` before the program is checked again.
            if deferred[i] && is_bare_str_literal(a) {
                let wants_string = matches!(&hint, Some(Type::Named(n, targs)) if n == "std::String" && targs.is_empty());
                note_literal_as_string(a, wants_string);
                if wants_string {
                    self.infer_poisoned = saved;
                    by_index[i] = Some(hint.clone().unwrap());
                    checked[i] = true;
                    continue;
                }
            }
            self.arg_temp_ok = !matches!(f.ret, Type::Ref(..));
            let r = self.check_store_operand(a, hint.as_ref());
            self.arg_temp_ok = false;
            self.infer_poisoned = saved;
            by_index[i] = r?;
            checked[i] = true;
        }
        let arg_types: Vec<Option<Type>> = by_index;
        if f.type_params.is_empty() {
            // D-0006 (no implicit conversion between distinct nominal
            // types): every parameter is already concrete here, so every
            // argument's own type must exactly match it -- [T-Call]'s
            // Γ ⊢ ei : τi, never actually enforced before (an i32
            // binding passed where i64 was declared silently "worked").
            self.check_arg_exact_types(&f.params, args, &arg_types, &HashMap::default(), &type_param_set)?;
            // Non-generic, but its body still needs to be reached by the
            // worklist to be checked at all -- `record_instantiation`
            // with zero type args does exactly that (and `checked`
            // memoizes so a frequently-called function is only checked
            // once).
            self.record_instantiation(&self.item_key(f), &[]);
            return Ok(Some(apply_subst(&f.ret, &HashMap::default())));
        }
        let mut m: HashMap<String, Type> = HashMap::default();
        for (i, tp) in f.type_params.iter().enumerate() {
            if let Some(t) = explicit.get(i) {
                m.insert(tp.clone(), t.clone());
            }
        }
        // Whether every type parameter is fixed. Not `m.len()`: unifying a
        // parameter such as `ref<String, exclusive>` also enters `String`
        // itself (`unify_type_shape` takes any bare name for a parameter).
        let all_fixed = |m: &HashMap<String, Type>| f.type_params.iter().all(|p| m.contains_key(p));
        if !all_fixed(&m) {
            for (i, p) in f.params.iter().enumerate() {
                if let Some(Some(at)) = arg_types.get(i) {
                    // A marker (`$T` of the enclosing generic body) is a
                    // perfectly good type argument for this pass.
                    unify_type_shape(&p.ty, at, &mut m, &f.type_params);
                }
            }
        }
        if !all_fixed(&m) {
            if let Some(exp) = expected {
                unify_type_shape(&f.ret, exp, &mut m, &f.type_params);
            }
        }
        // D-0006, same as the non-generic branch above: check every
        // parameter whose type is concrete *after* whatever substitution
        // is known so far (fully resolved or not) -- a param not
        // involving `T` at all must still exactly match its argument
        // even if `T` itself remains unresolved elsewhere in this call.
        self.check_arg_exact_types(&f.params, args, &arg_types, &m, &type_param_set)?;
        if all_fixed(&m) {
            let targs_c: Vec<Type> = f.type_params.iter().map(|p| m[p].clone()).collect();
            // `[Append-Not-Printable]`, `[Parse-Not-Numeric]` (spec/21
            // §2d): `T` is checked at the call, as `print`'s is.
            if let Some(t) = targs_c.first().filter(|t| !has_unresolved_marker(t)) {
                let is_std = |key: &str| self.items.fns.get(key).map_or(false, |g| std::sync::Arc::ptr_eq(g, f));
                let bad: Option<String> = if is_std("std::String::append") {
                    (!is_printable(t)).then(|| if matches!(t, Type::Named(n, _) if n == "std::String") {
                        "`String::append` takes a `String` by reference, not by value: `String::append(&mut out, &s)` (a call's result too: `&sprintf(...)`)".to_string()
                    } else {
                        match t {
                            Type::Ref(inner, _) if is_printable(inner) => format!("`String::append` takes a `String` by reference, and anything else it writes by value: pass `*r` for this `{}`", t),
                            _ => format!("`String::append` writes a number, a `bool`, a `str` or a `StringView`, or a `String` by reference (`&s`); `{}` is none of these", t),
                        }
                    })
                } else if is_std("std::String::parse") {
                    // D-0181: `bool` too.
                    (!matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool)).then(|| format!("`String::parse` reads a number or a `bool`; `{}` is neither", t))
                } else if is_std("std::Vec::sort") || is_std("std::Vec::binary_search") {
                    // D-0062: ordered by `key_less`, the key types' order;
                    // D-0181: a float too (a NaN after every other value).
                    (!is_sort_type(self.items, t)).then(|| format!("`Vec::{}` orders by the key types' own order (integers, `bool`, `str`, `String`, and structs and enums of them) or a float's; `{}` is not one{}, so pass the order: `Vec::{}_by`", f.name, t, key_problem_text(self.items, t), f.name))
                } else if is_std("std::PriorityQueue::new") {
                    // D-0137: ordered by `key_less`, as `Vec::sort` is.
                    (!is_sort_type(self.items, t)).then(|| format!("`PriorityQueue::new` orders by the key types' own order (integers, `bool`, `str`, `String`, and structs and enums of them) or a float's; `{}` is not one{}, so pass the order: `PriorityQueue::new_by`", t, key_problem_text(self.items, t)))
                } else if f.assoc_type.as_deref().map_or(false, |a| (a == "HashMap" || a == "HashSet") && is_std(&format!("std::{}::{}", a, f.name))) && f.type_params.iter().any(|p| p == "K") {
                    // `[Key-Not-Hashable]` (spec/21 §1a): `K` at every call
                    // (the `_str` lookups, D-0113, have no `K`: theirs is `String`).
                    let k = &targs_c[f.type_params.iter().position(|p| p == "K").unwrap()];
                    (!has_unresolved_marker(k) && !is_key_type(self.items, k)).then(|| format!("a `{}` key is an integer, `bool`, `str`, `String`, or a struct or enum of them; `{}` is not one{}", f.assoc_type.as_deref().unwrap_or("HashMap"), k, key_problem_text(self.items, k)))
                } else {
                    None
                };
                if let Some(msg) = bad {
                    return Err(mismatch(msg));
                }
            }
            // D-0090: each bounded type parameter's type satisfies its
            // bound; a caller's own type parameter, by a bound implying it.
            for ((tp, b), t) in f.type_params.iter().zip(f.type_bounds.iter()).zip(targs_c.iter()) {
                let Some(b) = b else { continue };
                let (ok, why) = match t {
                    Type::Named(n, a) if a.is_empty() && n.starts_with('$') => (self.marker_bounds.get(n).map_or(false, |have| have.implies(*b)), None),
                    t if *b == Bound::Clone => {
                        let p = self.clone_problem(t);
                        (p.is_none(), p)
                    }
                    t if *b == Bound::Plain => {
                        let p = self.plain_problem(t);
                        (p.is_none(), p)
                    }
                    t => (self.bound_holds(*b, t), None),
                };
                if !ok {
                    let what = match t {
                        Type::Named(n, a) if a.is_empty() && n.starts_with('$') => format!("type parameter `{}`{}", &n[1..], match self.marker_bounds.get(n) { Some(h) => format!(" (declared `{}`)", h.name()), None => " (declared without a bound)".to_string() }),
                        t => format!("`{}`", t),
                    };
                    return Err(mismatch(format!(
                        "`{}` needs its `{}` to be `{}` ({}), and {} is not{}",
                        f.name,
                        tp,
                        b.name(),
                        bound_members(*b),
                        what,
                        why.map(|w| format!(": {}", w)).unwrap_or_default()
                    )));
                }
            }
            self.record_instantiation(&self.item_key(f), &targs_c);
            Ok(Some(apply_subst(&f.ret, &m)))
        } else if self.infer_poisoned || expected.map_or(false, has_unresolved_marker) || arg_types.iter().any(|t| t.is_none()) {
            // Not decidable by this pass: an argument of a shape it does
            // not type (a closure capture, an intrinsic's result, ...)
            // could still fix the parameter.
            Ok(None)
        } else {
            // `[Generic-Call-Uninferable]` (spec/15 §5): no explicit
            // argument, no argument shape and no expected type fixes
            // some type parameter -- and nothing later ever will.
            let open: Vec<String> = f.type_params.iter().filter(|p| !m.contains_key(*p)).map(|p| format!("`{}`", p)).collect();
            let fname = match &f.assoc_type {
                Some(a) => format!("{}::{}", a, f.name),
                None => f.name.clone(),
            };
            let all: Vec<&str> = f.type_params.iter().map(|p| p.as_str()).collect();
            // The usual cause: a value passed where the parameter is a
            // reference, so its pattern cannot match (`Option::is_none(o)`
            // for `Option::is_none(&o)`). Say so.
            let hint = f.params.iter().zip(arg_types.iter()).enumerate().find_map(|(i, (p, a))| match (&p.ty, a) {
                (Type::Ref(_, m), Some(t)) if !matches!(t, Type::Ref(..) | Type::Slice(..)) => Some(format!(
                    "argument {} is `{}`, not a reference, but `{}` takes {}: pass `{}…`",
                    i + 1,
                    t,
                    fname,
                    if *m == Mode::Exclusive { "an exclusive reference there" } else { "a reference there" },
                    if *m == Mode::Exclusive { "&mut " } else { "&" }
                )),
                _ => None,
            });
            // Or an argument of another type altogether than its parameter
            // (`Option::unwrap(h)` with `h` a handle).
            let hint = hint.or_else(|| {
                f.params.iter().zip(arg_types.iter()).enumerate().find_map(|(i, (p, a))| match (&p.ty, a) {
                    (Type::Named(pn, _), Some(t)) if !type_param_set.contains(pn) && !has_unresolved_marker(t) && !matches!(t, Type::Named(an, _) if an == pn) => {
                        Some(format!("argument {} is `{}`, but `{}` takes `{}` there", i + 1, t, fname, p.ty))
                    }
                    _ => None,
                })
            });
            let advice = match hint {
                Some(h) => h,
                None => format!("write `{}<{}>(…)`, or use the result where its type is declared", fname, all.join(", ")),
            };
            Err(format!(
                "diag.cannot-infer-type-parameter{}nothing here fixes {} of `{}`: {}",
                crate::diagnostics::DETAIL_SEP,
                open.join(" and "),
                fname,
                advice
            ))
        }
    }

    // D-0006 ("no implicit conversion or subtyping anywhere"): a
    // concrete (already fully-substituted, not still mentioning one of
    // `f`'s own type parameters) parameter type must exactly nominal-
    // match its argument's own checked type. Skips a parameter whose
    // type still mentions an unresolved type parameter (nothing
    // concrete to compare yet -- generic inference, not this check,
    // handles that case) and an argument this pass could not itself
    // determine a type for (already reported elsewhere, or an
    // intrinsic/untyped shape outside this checker's narrowed scope).
    fn check_arg_exact_types(
        &self,
        params: &[Param],
        args: &[Expr],
        arg_types: &[Option<Type>],
        subst: &HashMap<String, Type>,
        type_param_set: &HashSet<String>,
    ) -> Result<(), String> {
        for (i, p) in params.iter().enumerate() {
            // A value where the parameter is a reference is wrong whatever
            // the type parameters are, and whatever the argument's own
            // unfixed parts (`Result::is_ok(Ok(1))`, `E` open): `[T-Call]`'s
            // τi is a `ref` and the argument is not one.
            if let Type::Ref(_, mode) = &p.ty {
                // Its type, or for one this pass leaves untyped (`Ok(1)`,
                // `E` open) its form: a literal or a constructor.
                let shown = match arg_types.get(i) {
                    Some(Some(at)) if !matches!(at, Type::Ref(..) | Type::Never) && !is_marker(at) => Some(format!("`{}`", at)),
                    Some(None) if args.get(i).map_or(false, |a| self.plainly_a_value(a)) => Some("a value".to_string()),
                    _ => None,
                };
                if let Some(shown) = shown {
                    return Err(mismatch(format!(
                        "argument {} (`{}`) is {}, not a reference, but the parameter is `{}`: pass `{}…`",
                        i + 1,
                        p.name,
                        shown,
                        apply_subst(&p.ty, subst),
                        if *mode == Mode::Exclusive { "&mut " } else { "&" }
                    )));
                }
            }
            let pty = apply_subst(&p.ty, subst);
            if mentions_any(&pty, type_param_set) {
                continue;
            }
            // A bare integer literal (`226`, `-5`) is not "already
            // typed" the way a binding is -- rule.arith.literal lets it
            // take on whatever concrete type context requires. When the
            // parameter's own type is only resolved *after* generic
            // inference (e.g. `Vec::push<T>`'s `x: T`, hinted before `T`
            // is known from the *other* argument, so it defaulted to
            // `i32`), the literal's pre-inference default type must not
            // be compared against the post-inference concrete type as if
            // it were a fixed mismatch -- exempt literals from this
            // check entirely; the arithmetic layer's own literal-range
            // checking already validates the value fits.
            // Only where a literal could take the parameter's type: a
            // number, or a type parameter of the body being checked
            // (`T: number`); `Option::unwrap(5: u64)` is a mismatch.
            let literal_fits = match &pty {
                Type::Int(_) | Type::F32 | Type::F64 => true,
                Type::Named(n, a) => a.is_empty() && n.starts_with('$'),
                _ => false,
            };
            if is_bare_int_literal(&args[i]) && literal_fits {
                continue;
            }
            if let Some(Some(at)) = arg_types.get(i) {
                if pty != *at && !weakens_to(at, &pty) {
                    return Err(mismatch(format!("argument {} (`{}`) is `{}`, but the parameter is `{}`{}", i + 1, p.name, at, pty, conversion_hint(&pty, at, args.get(i)))));
                }
            }
        }
        Ok(())
    }

    fn check_struct_lit(&mut self, segs: &[String], targs: &[Type], fields: &[(String, Expr)], expected: Option<&Type>) -> TResult {
        let name = segs.join("::");
        let sdecl = match self.items.structs.get(&name).cloned() {
            Some(s) => s,
            None => {
                for (_, e) in fields {
                    self.check_store_operand(e, None)?;
                }
                return Ok(None);
            }
        };
        let mut m: HashMap<String, Type> = HashMap::default();
        for (p, t) in sdecl.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), apply_subst(t, &self.subst));
        }
        // Every type parameter fixed? Not `m.len()`: a field such as
        // `String name` enters `String` too (`unify_type_shape`).
        let all_fixed = |m: &HashMap<String, Type>| sdecl.type_params.iter().all(|p| m.contains_key(p));
        if !all_fixed(&m) {
            if let Some(exp) = expected {
                let generic_self = Type::Named(name.clone(), sdecl.type_params.iter().map(|p| Type::Named(p.clone(), vec![])).collect());
                unify_type_shape(&generic_self, exp, &mut m, &sdecl.type_params);
            }
        }
        let mut seen = HashSet::default();
        for (fname, fexpr) in fields {
            if !seen.insert(fname.clone()) {
                return Err(mismatch(format!("field `{}` of `{}` is given twice", fname, Type::Named(name.clone(), Vec::new()))));
            }
            let idx = sdecl.fields.iter().position(|f| &f.name == fname);
            let idx = match idx {
                Some(i) => i,
                None => return Err(mismatch(format!("`{}` has no field `{}`", Type::Named(name.clone(), Vec::new()), fname))),
            };
            if !field_visible(&name, sdecl.fields[idx].export, &self.module) {
                return Err(crate::modres::named("diag.name-not-visible", format!("field `{}` of `{}` is not exported", fname, Type::Named(name.clone(), Vec::new()))));
            }
            let remaining: HashSet<String> = sdecl.type_params.iter().filter(|p| !m.contains_key(*p)).cloned().collect();
            let hint = apply_subst(&sdecl.fields[idx].ty, &m);
            let at = if mentions_any(&hint, &remaining) {
                let saved = self.infer_poisoned;
                self.infer_poisoned = true;
                let r = self.check_store_operand(fexpr, None);
                self.infer_poisoned = saved;
                let r = r?;
                // `[T-Struct]` by shape: `Vec<T>` is no array or String,
                // whatever `T` turns out to be.
                if let Some(t) = &r {
                    if *t != Type::Never && shape_conflict(&hint, t, &remaining) {
                        return Err(mismatch(format!("field `{}` of `{}` is `{}`, but the value is `{}`", fname, Type::Named(name.clone(), Vec::new()), hint, t)));
                    }
                }
                r
            } else {
                let t = self.check_store_operand(fexpr, Some(&hint))?;
                // `[T-Struct]`: the value has the field's type.
                if let Some(t) = &t {
                    if !has_unresolved_marker(t) && !has_unresolved_marker(&hint) && *t != Type::Never && !ty_compat(t, &hint) {
                        return Err(mismatch(format!("field `{}` of `{}` is `{}`, but the value is `{}`", fname, Type::Named(name.clone(), Vec::new()), hint, t)));
                    }
                }
                t
            };
            if let Some(at) = &at {
                if !has_unresolved_marker(at) {
                    unify_type_shape(&sdecl.fields[idx].ty, at, &mut m, &sdecl.type_params);
                }
            }
            // D-0118: a bit field's initial value must fit its width.
            if let Some(w) = sdecl.fields[idx].width {
                self.check_bitfield_literal(fexpr, w, &format!("field `{}` of `{}`", fname, name.strip_prefix("std::").unwrap_or(&name)))?;
            }
        }
        if seen.len() != sdecl.fields.len() {
            return Err("diag.type-mismatch".to_string());
        }
        if all_fixed(&m) {
            let targs_c: Vec<Type> = sdecl.type_params.iter().map(|p| m[p].clone()).collect();
            Ok(Some(Type::Named(name, targs_c)))
        } else {
            Ok(None)
        }
    }

    // Exhaustiveness (`rule.agg.match`) checked statically when the
    // scrutinee's enum is concretely known; otherwise left to the
    // existing dynamic `diag.non-exhaustive-match` fault.
    fn check_match(&mut self, whole: &Expr, scrut: &Expr, arms: &[Arm], expected: Option<&Type>) -> TResult {
        // The scrutinee is a place position (spec/13 §1): `[Match]` reads
        // its discriminant under `¬clash(a, shared)`, and a resource-
        // bearing enum matched by value is consumed.
        self.place_pos = true;
        let st = self.check_expr(scrut, None)?;
        // D-0218: what a shared reference reaches is read-only at every
        // depth. A scrutinee that is an exclusive reference read through a
        // shared one is a shared borrow (`&*e`), so its binders are shared;
        // a scrutinee matched by value through a shared reference cannot
        // bind a payload that holds an exclusive reference.
        let scrut_shared = self.crosses_shared(scrut);
        let st = match st {
            Some(Type::Ref(t, Mode::Exclusive)) if scrut_shared => {
                if SILENT.with(|s| s.get() > 0) {
                    return Err(crate::modres::named("diag.borrow-exceeds-source", "an exclusive reference read through a shared reference in a body checked after the program was built".to_string()));
                }
                note_rewrite(scrut, Rewrite::SharedRef);
                Some(Type::Ref(t, Mode::Shared))
            }
            other => other,
        };
        // D-0049: a resource binding matched by value is consumed only by
        // an arm that moves its payload out (below), not by the match.
        let mut consume_x: Option<String> = None;
        if let Some((x, path)) = self.deref_place_of(scrut).filter(|_| self.place_of(scrut).is_none()) {
            self.flow_clash(&x, &path, Mode::Shared, "diag.aliasing-conflict")?;
        }
        if let Some((x, path)) = self.place_of(scrut) {
            if self.flow.init.get(&x) != Some(&Tri::T) {
                self.refute(&uninit_msg(&x))?;
            }
            self.flow_clash(&x, &path, Mode::Shared, "diag.aliasing-conflict")?;
            if path.is_empty() && self.place_is_resource(&x, &[]) {
                consume_x = Some(x.clone());
            }
        }
        let enum_named = match &st {
            Some(Type::Named(n, args)) if self.items.enums.contains_key(n) => Some((n.clone(), args.clone())),
            Some(Type::Ref(inner, _)) => match &**inner {
                Type::Named(n, args) if self.items.enums.contains_key(n) => Some((n.clone(), args.clone())),
                _ => None,
            },
            _ => None,
        };
        // `[Match-By-Ref]` (D-0046): a scrutinee that is a reference,
        // `match (&e)` / `match (&mut e)` or a reference-typed binding,
        // binds each payload as a reference of that mode, into the enum.
        let by_ref: Option<Mode> = match &st {
            Some(Type::Ref(_, m)) => Some(m.clone()),
            _ => None,
        };
        // D-0056/D-0057: each arm's pattern, typed level by level: every
        // variant is one of the enum at its level, a nested pattern descends
        // only into a payload whose type is an enum, and a literal is of the
        // integer or `bool` type at its level (the scrutinee's, when the
        // pattern is only the literal). `binder_ty[i]` is the type of the
        // innermost payload arm i binds (before `by_ref`).
        let level0: Option<Type> = match (&enum_named, &st) {
            (Some((n, a)), _) => Some(Type::Named(n.clone(), a.clone())),
            (None, Some(t)) if matches!(t, Type::Int(_) | Type::Bool) || is_text_ty(t) => Some(t.clone()),
            _ => None,
        };
        // A reference to an integer, `bool` or text (D-0127) has no variants,
        // and is not itself a literal's type: `match (*r)` reads the value.
        if level0.is_none() && matches!(&st, Some(Type::Ref(inner, _)) if matches!(**inner, Type::Int(_) | Type::Bool) || is_text_ty(inner)) && arms.iter().any(|a| a.variant.is_some() || a.lit.is_some()) {
            return Err(format!("diag.type-mismatch{}a literal is matched against a reference, `{}`: `match (*r)` reads the value", crate::diagnostics::DETAIL_SEP, st.as_ref().unwrap()));
        }
        let mut binder_ty: Vec<Option<Type>> = Vec::new();
        // D-0109: the mode of the references each arm's pattern looks
        // through (the weaker, when there are several); its binder is a
        // reference of that mode.
        let mut crossed: Vec<Option<Mode>> = Vec::new();
        for arm in arms {
            crossed.push(None);
            // D-0058: a binder alone binds the whole scrutinee, of any type.
            if arm.variant.is_none() && arm.lit.is_none() && arm.binder.is_some() {
                binder_ty.push(match (&st, &by_ref) {
                    (Some(Type::Ref(inner, _)), Some(_)) => Some((**inner).clone()),
                    (t, _) => t.clone(),
                });
                continue;
            }
            let detail = |m: String| Err(format!("diag.type-mismatch{}{}", crate::diagnostics::DETAIL_SEP, m));
            let Some(t0) = level0.clone() else {
                // A scrutinee whose type this pass does not know; one it
                // does know and that is not an enum, an integer, `bool` or
                // text (D-0127) has no value a literal or variant names.
                if let (Some(t), true) = (&st, arm.lit.is_some() || arm.variant.is_some()) {
                    return match (arm.lit.as_deref(), &arm.variant) {
                        (Some(l), _) => detail(format!("the literal `{}` is matched against `{}`, which is not an integer, `bool` or text", literal_key(l), t)),
                        (None, Some(v)) => detail(format!("the pattern `{}` is matched against `{}`, which is not an enum", v, t)),
                        _ => Err("diag.type-mismatch".to_string()),
                    };
                }
                binder_ty.push(None);
                continue;
            };
            let mut cur: Option<Type> = Some(t0);
            let chain = arm.chain();
            for (k, vn) in chain.iter().enumerate() {
                // D-0109: a level below the first that is a reference to an
                // enum is matched through the reference.
                if k > 0 {
                    if let Some(Type::Ref(inner, m)) = cur.clone() {
                        if self.is_enum_ty(&inner) {
                            { let w = weaker_mode(crossed.last().unwrap().as_ref(), &m); *crossed.last_mut().unwrap() = Some(w); }
                            cur = Some(*inner);
                        }
                    }
                }
                let Some(level) = cur.clone().filter(|c| self.is_enum_ty(c)) else {
                    return match &cur {
                        Some(t) => detail(format!("the pattern `{}` is matched against `{}`, which is not an enum", vn, t)),
                        None => Err("diag.type-mismatch".to_string()),
                    };
                };
                match self.variant_payload(&level, vn) {
                    Some(p) => cur = p,
                    None => return detail(format!("`{}` is not a variant of `{}`", vn, level)),
                }
            }
            // D-0109: a literal below a variant compares through a
            // reference to an integer or `bool`.
            if arm.lit.is_some() && !chain.is_empty() {
                if let Some(Type::Ref(inner, m)) = cur.clone() {
                    if matches!(*inner, Type::Int(_) | Type::Bool) || is_text_ty(&inner) {
                        { let w = weaker_mode(crossed.last().unwrap().as_ref(), &m); *crossed.last_mut().unwrap() = Some(w); }
                        cur = Some(*inner);
                    }
                }
            }
            if let Some(l) = &arm.lit {
                let Some(lt) = cur.clone().filter(|c| matches!(c, Type::Int(_) | Type::Bool) || is_text_ty(c)) else {
                    return match &cur {
                        Some(t) => detail(format!("the literal `{}` is matched against `{}`, which is not an integer, `bool` or text", literal_key(l), t)),
                        None => Err("diag.type-mismatch".to_string()),
                    };
                };
                // D-0127: a text level takes a text literal and nothing else;
                // an integer or `bool` level takes no text literal.
                if is_text_ty(&lt) != matches!(l.kind, ExprKind::StrLit(_)) {
                    return detail(format!("the literal `{}` is not of type `{}`", literal_key(l), lt));
                }
                if is_text_ty(&lt) {
                    binder_ty.push(None);
                    continue;
                }
                // A constant in a pattern must fold to a literal (an integer
                // or `bool` constant expression, D-0058).
                let folded = match &l.kind {
                    ExprKind::IntLit(..) | ExprKind::BoolLit(_) => true,
                    ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(..)),
                    _ => false,
                };
                if !folded {
                    return Err("diag.type-mismatch".to_string());
                }
                match self.check_expr(l, Some(&lt))? {
                    Some(t) if t == lt => {}
                    _ => return Err("diag.type-mismatch".to_string()),
                }
            }
            binder_ty.push(if arm.binder.is_some() { cur } else { None });
        }
        // `[Match-Move-Through-Ref]` (spec/16 §4): a resource payload can
        // be bound only from a whole owned or temporary enum, never through
        // a reference or a projection (`base(a) ≠ None`).
        if by_ref.is_none() && scrut_shared {
            for (bt, c) in binder_ty.iter().zip(crossed.iter()) {
                let holds = match (bt, c) {
                    (Some(t), None) => holds_excl_ref(self.items, t, 0),
                    (Some(_), Some(Mode::Exclusive)) => true,
                    _ => false,
                };
                if holds {
                    return Err(crate::modres::named(
                        "diag.borrow-exceeds-source",
                        "this binds a payload that holds an exclusive reference by value, out of a value reached through a shared reference (D-0218): the copy could write what the shared reference reaches; match on the reference (`match (r)` or `match (&e)`) to bind it shared".to_string(),
                    ));
                }
            }
        }
        let through_ref_or_projection = by_ref.is_none() && matches!(&scrut.kind, ExprKind::Deref(_) | ExprKind::Field(..) | ExprKind::Index(..));
        if through_ref_or_projection {
            for bt in binder_ty.iter().zip(crossed.iter()).filter(|(_, c)| c.is_none()).filter_map(|(b, _)| b.as_ref()) {
                if is_resource_ty(self.items, bt) {
                    return Err(crate::modres::named("diag.move-out-of-field", format!("a `{}` payload cannot be moved out through a reference or out of part of a value; match on `&e` to borrow it, or `replace` the whole value first", bt)));
                }
            }
        }
        // `[Match-Non-Exhaustive]` and `[Match-Unreachable]` (D-0056): arms
        // are tried in order; an arm every value it matches already went to
        // an earlier arm is rejected, and the arms together must match
        // every value. The diagnostic names a pattern that is not covered.
        if let Some(t0) = &level0 {
            let chains: Vec<Vec<String>> = arms.iter().map(pattern_elems).collect();
            for k in 1..chains.len() {
                let tk = self.elems_type(t0, &chains[k]);
                if self.pattern_missing(&chains[k], tk, &chains[..k], false).is_none() {
                    // An earlier arm whose binder is capitalised most likely
                    // meant a variant that does not exist (`Ok(Directory)`):
                    // a name in a pattern binds, and matches every value.
                    let hint = arms[..k].iter().zip(chains[..k].iter()).find_map(|(a, ch)| {
                        let b = a.binder.as_ref()?;
                        if !b.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                            return None;
                        }
                        let inner = self.elems_type(t0, ch);
                        let variants = match &inner {
                            Some(Type::Named(n, _)) => self.items.enums.get(n).map(|e| e.variants.iter().map(|v| format!("`{}`", v.name)).collect::<Vec<_>>().join(", ")),
                            _ => None,
                        };
                        Some(match variants {
                            Some(vs) => format!("an earlier arm's `{}` is not a variant of `{}` but a name the pattern binds, so it matches every value; its variants are {}", b, inner.as_ref().unwrap(), vs),
                            None => format!("an earlier arm's `{}` is a name the pattern binds, not a variant, so it matches every value", b),
                        })
                    });
                    // Located at the arm (its body's line).
                    return Err(match hint {
                        Some(h) => format!("diag.unreachable-arm{}{}{}", crate::diagnostics::DETAIL_SEP, h, crate::diagnostics::at(arms[k].body.line, arms[k].body.col)),
                        None => format!("diag.unreachable-arm{}", crate::diagnostics::at(arms[k].body.line, arms[k].body.col)),
                    });
                }
            }
            if let Some(missing) = self.pattern_missing(&[], Some(t0.clone()), &chains, false) {
                return Err(format!("diag.non-exhaustive-match{}not covered: `{}`", crate::diagnostics::DETAIL_SEP, missing));
            }
        }
        let mut result: Option<Type> = None;
        let mut any_unknown = false;
        // Each arm starts from the state after the scrutinee; the arm
        // states join afterwards. The binder is bound in the arm's frame.
        let base = self.flow.clone();
        let mut joined: Option<Flow> = None;
        // `rule.type.expected`: "the type of the first arm's body for later
        // arms" -- when nothing outside the match fixes a type, the first
        // arm's does. Recorded for the evaluator too (`match_hints`), so a
        // literal in a later arm is typed the same way at run time.
        let mut arm_expected: Option<Type> = expected.cloned();
        // D-0049: an arm that is only a literal takes its type from the
        // first arm that is not; that arm is checked first (arms start
        // from the same state and join, so the order is otherwise free).
        let mut order: Vec<usize> = (0..arms.len()).collect();
        if expected.is_none() {
            if let Some(k) = arms.iter().position(|a| !is_literal_branch(&a.body)) {
                order.remove(k);
                order.insert(0, k);
            }
        }
        let arm_ix = order.clone();
        for (ai, arm) in order.iter().map(|&i| &arms[i]).enumerate() {
            let saved = self.gamma.clone();
            self.flow = base.clone();
            self.enter_scope();
            let cross = crossed[arm_ix[ai]].clone();
            if let (Some(bn), Some(pt)) = (&arm.binder, binder_ty[arm_ix[ai]].clone()) {
                if let (Some(x), None, None) = (&consume_x, &by_ref, &cross) {
                    if is_resource_ty(self.items, &pt) {
                        self.flow_consume(x, "diag.destroy-while-aliased")?;
                    }
                }
                let bt = match (&by_ref, &cross) {
                    (None, None) => pt,
                    (Some(m), None) | (None, Some(m)) => Type::Ref(Box::new(pt), m.clone()),
                    (Some(a), Some(b)) => Type::Ref(Box::new(pt), weaker_mode(Some(a), b)),
                };
                self.gamma.insert(bn.clone(), bt);
            }
            if let Some(bn) = &arm.binder {
                self.declare(bn, true);
                if cross.is_some() {
                    // D-0109: through a reference the pattern crossed, the
                    // binder borrows what that reference does.
                    let mut facts = self.contained_ref_facts(scrut);
                    if facts.is_empty() {
                        if let ExprKind::Path(segs, _) = &strip_parens_tc(scrut).kind {
                            if segs.len() == 1 {
                                facts = self.flow.deriv.get(&segs[0]).map(|m| m.keys().cloned().collect()).unwrap_or_default();
                            }
                        }
                    }
                    self.set_deriv(bn, facts);
                    self.tier_inherit(bn, Some(scrut))?;
                } else if let Some(m) = &by_ref {
                // D-0071: a binder of a match by reference is a reference
                // into the scrutinee's place for the arm.
                    let fact = self.match_referent(scrut).map(|(root, path)| DerivKey { root, path, mode: m.clone() });
                    self.set_deriv(bn, fact.into_iter().collect());
                    self.tier_inherit(bn, Some(scrut))?;
                } else if self.gamma.get(bn).map_or(false, |t| holds_ref(self.items, t)) {
                    // D-0088: a reference out of a call's `Option<ref<V>>`
                    // borrows from the call's reference arguments.
                    let facts = self.contained_ref_facts(scrut);
                    if !facts.is_empty() {
                        self.set_deriv(bn, facts);
                    }
                    self.tier_inherit(bn, Some(scrut))?;
                }
            }
            let at = self.check_store_operand(&arm.body, arm_expected.as_ref());
            let at = match at {
                Ok(t) => self.check_block_exit().map(|_| t),
                e => e,
            };
            self.exit_scope();
            self.gamma = saved;
            let at = at?;
            if ai == 0 && arm_expected.is_none() {
                if let Some(t) = &at {
                    if *t != Type::Never && !has_unresolved_marker(t) {
                        arm_expected = Some(t.clone());
                        self.items.match_hints.lock().unwrap().insert(whole as *const Expr as usize, t.clone());
                    }
                }
            }
            joined = Some(match joined {
                None => self.flow.clone(),
                Some(j) => j.join(&self.flow),
            });
            match at {
                Some(t) if !has_unresolved_marker(&t) => {
                    match &result {
                        None => result = Some(t),
                        Some(r) if *r == Type::Never => result = Some(t),
                        Some(r) if t == Type::Never || ty_compat(r, &t) => {}
                        Some(r) => return Err(mismatch(format!("one arm gives `{}` and another `{}`", r, t))),
                    }
                }
                _ => any_unknown = true,
            }
        }
        self.flow = joined.unwrap_or(base);
        if any_unknown {
            Ok(None)
        } else {
            Ok(result)
        }
    }

    // The place a match by reference looks into: `e` of `match (&e)` /
    // `match (&mut e)`, or the referent of `match (r)` for a reference
    // binding `r`.
    fn match_referent(&self, scrut: &Expr) -> Option<(String, Vec<PElem>)> {
        match &strip_parens_tc(scrut).kind {
            ExprKind::Borrow(_, inner) => self.borrowed_place(inner),
            ExprKind::Path(segs, _) if segs.len() == 1 && self.flow.is_tracked(&segs[0]) => match self.lookup_local(&segs[0])? {
                Type::Ref(..) => Some((referent_root(&segs[0]), Vec::new())),
                _ => None,
            },
            _ => None,
        }
    }

    // The payload type of variant `vn` of the enum type `t` (None for a
    // variant without one); None when `t` is not an enum with that variant.
    fn variant_payload(&self, t: &Type, vn: &str) -> Option<Option<Type>> {
        let Type::Named(n, args) = t else {
            return None;
        };
        let e = self.items.enums.get(n)?;
        let v = e.variants.iter().find(|v| v.name == vn)?;
        let sub: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
        Some(v.payload.as_ref().map(|p| apply_subst(p, &sub)))
    }

    fn is_enum_ty(&self, t: &Type) -> bool {
        matches!(t, Type::Named(n, _) if self.items.enums.contains_key(n))
    }

    // The type of the level after the pattern elements `elems` in `t0`
    // (the innermost payload's type); None after a literal.
    fn elems_type(&self, t0: &Type, elems: &[String]) -> Option<Type> {
        let mut t = Some(t0.clone());
        for e in elems {
            if e.starts_with('=') {
                return None;
            }
            // D-0109: a reference to an enum is matched through.
            let level = match t {
                Some(Type::Ref(inner, _)) if self.is_enum_ty(&inner) => Some(*inner),
                t => t,
            };
            let level = level.filter(|c| self.is_enum_ty(c))?;
            t = self.variant_payload(&level, e)?;
        }
        t
    }

    // D-0056/D-0057: a value whose pattern elements begin with `prefix`
    // (the next level being of type `t`) that none of `arms` matches,
    // written as a pattern; None when the arms match every such value. An
    // arm matches the values its elements begin. An enum level is split
    // into its variants, a `bool` level into `true` and `false`; an integer
    // level is covered only by an arm that stops above it (`_`, a binder).
    // `payload`: whether the last variant of `prefix` has a payload.
    fn pattern_missing(&self, prefix: &[String], t: Option<Type>, arms: &[Vec<String>], payload: bool) -> Option<String> {
        if arms.iter().any(|q| q.len() <= prefix.len() && prefix.starts_with(q)) {
            return None;
        }
        let extends = arms.iter().any(|q| q.len() > prefix.len() && q.starts_with(prefix));
        if !extends {
            return Some(render_pattern(prefix, payload));
        }
        // D-0109: below the first level, a reference to an enum (or a
        // `bool`) is split as its referent is.
        let t = match t {
            Some(Type::Ref(inner, _)) if !prefix.is_empty() && (self.is_enum_ty(&inner) || *inner == Type::Bool) => Some(*inner),
            t => t,
        };
        match &t {
            Some(level @ Type::Named(n, _)) if self.items.enums.contains_key(n) => {
                let e = self.items.enums.get(n)?.clone();
                for v in &e.variants {
                    let mut p = prefix.to_vec();
                    p.push(v.name.clone());
                    let pt = self.variant_payload(level, &v.name)?;
                    let has = pt.is_some();
                    if let Some(m) = self.pattern_missing(&p, pt, arms, has) {
                        return Some(m);
                    }
                }
                None
            }
            Some(Type::Bool) => {
                for b in ["=true", "=false"] {
                    let mut p = prefix.to_vec();
                    p.push(b.to_string());
                    if let Some(m) = self.pattern_missing(&p, None, arms, false) {
                        return Some(m);
                    }
                }
                None
            }
            _ => Some(render_pattern(prefix, payload)),
        }
    }

    // `diag.capture-list-mismatch` (`rule.fn.closure`, spec/15 §6): the
    // written capture list must name exactly the body's free-variable
    // set. A purely syntactic scan, independent of typing.
    fn check_closure(&mut self, is_move: bool, captures: &[String], params: &[Param], declared_ret: Option<&Type>, body: &Block, expected: Option<&Type>) -> TResult {
        // Free variables computed against only the *params* as bound --
        // the captures themselves must NOT be pre-excluded, since the
        // declared capture list is supposed to equal exactly this set
        // (bounding by the captures too would trivially make `free`
        // empty for any correctly-written closure and falsely flag
        // every one of them as a mismatch).
        let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut free: HashSet<String> = HashSet::default();
        collect_free_vars_block(body, &mut bound, &mut free);
        // Free *variables* (spec/15 §6: "names bound outside it and used
        // inside") -- an item named by a bare identifier (a function, an
        // extern, an intrinsic, an enum variant, a struct) is not one.
        free.retain(|n| !self.is_item_name(n));
        let declared: HashSet<String> = captures.iter().cloned().collect();
        if free != declared {
            return Err("diag.capture-list-mismatch".to_string());
        }
        // Forming the closure, in the enclosing body's flow: a `move`
        // capture of a resource binding consumes it (`[Closure-Form-
        // Move]` stores by move); a borrow capture is a borrow of the
        // binding in the mode the body derives (`[Closure-Form-Borrow]`),
        // checked against the live `deriv` facts like any other. Either
        // way the captured name must be temporally valid.
        for c in captures {
            self.flow.lits.remove(c);
            if !self.flow.is_tracked(c) {
                continue;
            }
            match self.flow.valid.get(c) {
                Some(Tri::F) => self.refute(&crate::modres::named("diag.stale-binding", format!("`{}` was moved or dropped before this closure captures it", shown_name(c))))?,
                Some(Tri::U) => self.refute(&crate::modres::named("diag.stale-binding", format!("`{}` may have been moved or dropped (on some path to here) before this closure captures it", shown_name(c))))?,
                _ => {}
            }
            if is_move {
                if self.place_is_resource(c, &[]) {
                    self.flow_consume(c, "diag.move-while-aliased")?;
                } else {
                    if self.flow.init.get(c) != Some(&Tri::T) {
                        self.refute(&uninit_msg(&c))?;
                    }
                    self.flow_clash(c, &[], Mode::Shared, "diag.aliasing-conflict")?;
                }
            } else {
                let mode = if crate::interp::closure_body_writes(body, c) { Mode::Exclusive } else { Mode::Shared };
                self.flow_clash(c, &[], mode, "diag.aliasing-conflict")?;
            }
        }
        // Body itself checked as a body of its own: parameters tracked,
        // captures opaque (inside, a captured name is `*self.f_i`,
        // spec/15 [Closure-Call] -- rooted at a dereference, so no fact
        // is ever `T` or `F` for it). Its type is the outer binding's: the
        // name denotes the captured place itself, whatever the capture
        // mode, so `u8 t = e + 10;` types `10` as `u8` there as outside.
        let outer_capture_types: HashMap<String, Option<Type>> =
            captures.iter().map(|c| (c.clone(), self.lookup_local(c))).collect();
        let saved_gamma = std::mem::take(&mut self.gamma);
        for (c, t) in &outer_capture_types {
            if let Some(t) = t {
                self.gamma.insert(c.clone(), t.clone());
            }
        }
        let saved_flow = std::mem::take(&mut self.flow);
        let saved_move_captures = std::mem::replace(&mut self.move_captures, if is_move { outer_capture_types } else { HashMap::default() });
        let saved_capture_names = std::mem::replace(&mut self.capture_names, captures.iter().cloned().collect());
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![Vec::new()]);
        let saved_loops = std::mem::take(&mut self.loops);
        let saved_depth = self.while_depth;
        let root = self.next_block;
        self.next_block += 1;
        let saved_blocks = std::mem::replace(&mut self.block_stack, vec![root]);
        let saved_decl = std::mem::take(&mut self.decl_block);
        let saved_origin = std::mem::take(&mut self.ref_origin);
        let saved_params = std::mem::replace(&mut self.params, params.iter().map(|p| p.name.clone()).collect());
        self.while_depth = 0;
        for p in params {
            self.gamma.insert(p.name.clone(), apply_subst(&p.ty, &self.subst));
            self.declare(&p.name, true);
        }
        // The result the closure is expected to give (`fn(…) : R` where it
        // is written), which its body is checked against.
        let exp_ret: Option<Type> = match expected {
            Some(Type::Fn(ps, r)) if ps.len() == params.len() && !has_unresolved_marker(r) => Some((**r).clone()),
            _ => None,
        };
        // D-0081: a written result type is what the body is checked
        // against, and must be the one the `fn` type it is used at says.
        let declared_ret = declared_ret.map(|t| apply_subst(t, &self.subst));
        if let (Some(d), Some(want)) = (&declared_ret, &exp_ret) {
            if !ty_compat(d, want) {
                let ptys: Vec<Type> = params.iter().map(|p| apply_subst(&p.ty, &self.subst)).collect();
                return Err(mismatch(format!("the closure's result is declared `{}`, but `{}` is expected", d, Type::Fn(ptys, Box::new(want.clone())))));
            }
        }
        let exp_ret = declared_ret.or(exp_ret);
        let saved_closure_ret = self.closure_ret.replace(exp_ret.clone());
        let r = self.check_nested_block(body, exp_ret.as_ref());
        self.closure_ret = saved_closure_ret;
        self.move_captures = saved_move_captures;
        self.capture_names = saved_capture_names;
        self.params = saved_params;
        self.ref_origin = saved_origin;
        self.decl_block = saved_decl;
        self.block_stack = saved_blocks;
        self.while_depth = saved_depth;
        self.loops = saved_loops;
        self.scopes = saved_scopes;
        self.flow = saved_flow;
        self.gamma = saved_gamma;
        let bt = r?;
        // A closure is a value of type `fn(P…) : R`, R its body's type
        // (D-0073): so a closure whose body gives another type than the
        // `fn` type it is used at is rejected here, not when it runs.
        let ptys: Vec<Type> = params.iter().map(|p| apply_subst(&p.ty, &self.subst)).collect();
        match (bt, exp_ret) {
            (Some(t), Some(want)) if !has_unresolved_marker(&t) && t != Type::Never && !ty_compat(&t, &want) => Err(mismatch(format!(
                "the closure's body gives `{}`, but `{}` is expected",
                t,
                Type::Fn(ptys, Box::new(want))
            ))),
            (Some(Type::Never), Some(want)) => Ok(Some(Type::Fn(ptys, Box::new(want)))),
            (Some(t), _) if !has_unresolved_marker(&t) && t != Type::Never && !ptys.iter().any(has_unresolved_marker) => Ok(Some(Type::Fn(ptys, Box::new(t)))),
            _ => Ok(None),
        }
    }
}

// D-0091: the floating-point functions, each of one `f32` or `f64`;
// D-0101 adds the logarithms, `exp` and the trigonometric functions.
// `std::math`'s since D-0136, realized natively and typed as an
// intrinsic is (`modres` enters them).
pub const FLOAT_FNS: &[&str] = &[
    "std::sqrt", "std::floor", "std::ceil", "std::round", "std::trunc", "std::ln", "std::exp", "std::log2", "std::log10", "std::sin", "std::cos", "std::tan",
    "std::asin", "std::acos", "std::atan", // D-0181
];

// D-0101: the floating-point functions of two operands of one type.
pub const FLOAT2_FNS: &[&str] = &["std::atan2", "std::powf"];

// `rule.arith.alt`'s operations, whose two operands share one type.
const ALT_INTRINSICS: &[&str] = &[
    "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub",
    "checked_mul", "checked_div", "checked_rem",
];

// D-0252: the functions of `std` called only inside `unsafe`, as the
// trusted intrinsics are (`spec/20` §1, `[Unsafe-Rejected]`): each reads
// through a `rawptr` its caller vouches for. Never a `fn` value, so no
// call of one escapes the check.
pub(crate) const TRUSTED_STD_CALLS: &[&str] = &["std::String::from_c", "std::Vec::from_c_bytes", "std::Vec::from_raw", "std::copy_to_raw"];

pub(crate) const INTRINSIC_NAMES: &[&str] = &[
    "drop", "clone", "sizeof", "alignof", "dangling", "null_rawptr", "checked_narrow", "count_ones", "leading_zeros", "trailing_zeros", "rotate_left", "rotate_right", "read_volatile", "write_volatile", "rawptr_of", "rawptr_from_address", "rawptr_address", "reclaim", "release", "copy_raw", "allocate", "deallocate",
    "lock", "spawn", "join", "join_status", "cancel", "std::Result::map_err", "std::print", "std::exit", "reinterpret_ptr", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float",
    "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul",
    "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem", "checked_neg", "fault", "stdout_write",
    "std::sqrt", "std::floor", "std::ceil", "std::round", "std::trunc", "std::ln", "std::exp", "std::log2", "std::log10", "std::sin", "std::cos", "std::tan",
    "std::asin", "std::acos", "std::atan", "std::atan2", "std::powf",
    "str_len", "str_byte", "str_ptr", "str_slice", "slice_len", "min_value", "max_value",
    "append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "key_less", "key_less_at", "swap_places", "lend_places", "atomic_op", "atomic_wait_op", "cpu_op", "simd_load", "simd_store", "simd_eq", "simd_ne", "simd_lt", "simd_le", "simd_gt", "simd_ge", "simd_select", "simd_any", "simd_all", "simd_min", "simd_max", "simd_sum", "simd_reduce_min", "simd_reduce_max", "$fmt", "$assert_fail",
    "$each_drain", "$each_len", "$each_at", "$each_at_mut", "$each_key", "$each_take", "$each_take_key",
    "static_assert", // D-0052
    "Mutex::new", // the one qualified intrinsic (`spec/19`; `mutex` is a built-in type, not a struct item)
];

// A reference or a slice (D-0047): a borrow for the escape and elision
// rules (`rule.temporal.elision`).
// D-0049: an argument `ref<τ, exclusive>` for a parameter `ref<τ,
// shared>` (a slice likewise) is passed as a shared reborrow.
fn weakens_to(arg: &Type, param: &Type) -> bool {
    match (arg, param) {
        (Type::Ref(a, Mode::Exclusive), Type::Ref(p, Mode::Shared)) | (Type::Slice(a, Mode::Exclusive), Type::Slice(p, Mode::Shared)) => a == p,
        _ => false,
    }
}

fn is_borrow_ty(t: &Type) -> bool {
    // D-0053: a `StringView` is a borrow as a slice is (it holds one).
    matches!(t, Type::Ref(..) | Type::Slice(..)) || matches!(t, Type::Named(n, _) if n == "std::StringView")
}

// The element type of what `x[i]` and `&x[lo .. hi]` index (D-0047): an
// array, a `Vec` or a slice, directly or through a reference.
fn indexed_elem(t: &Type) -> Option<Type> {
    match t {
        Type::Array(el, _) | Type::Slice(el, _) => Some((**el).clone()),
        Type::Named(n, args) if n == "std::Vec" => args.first().cloned(),
        Type::Ref(inner, _) => match &**inner {
            Type::Ref(..) => None,
            other => indexed_elem(other),
        },
        _ => None,
    }
}

// `rule.stdlib.print`'s printable types (`never` too: it is any type).
fn is_printable(t: &Type) -> bool {
    match t {
        Type::Str | Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Never => true,
        Type::Named(n, _) if n == "std::StringView" => true, // D-0053
        Type::Ref(inner, Mode::Shared) => matches!(&**inner, Type::Named(n, _) if n == "std::String"),
        _ => false,
    }
}

// Intrinsics only `std`'s own code may call: how it realizes
// `String::append<T>` and `String::parse<T>` (`rule.stdlib.text`), and
// `StringView::of` (D-0134).
// WI-15: the intrinsics of a fixed shape (`spec/21` §0's signatures): the
// fewest and the most type arguments a call takes (the most is the
// signature's type parameters; the fewest is one where nothing else can
// fix it), the fewest and the most arguments, and how it is written.
const INTRINSIC_SHAPES: &[(&[&str], usize, usize, usize, usize, &str)] = &[
    (&["sizeof", "alignof", "dangling", "null_rawptr"], 1, 1, 0, 0, "<T>()"),
    (&["min_value", "max_value"], 0, 1, 0, 0, "<T>()"),
    (&["widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int", "checked_narrow"], 1, 1, 1, 1, "<T>(x)"),
    (&["reinterpret_ptr", "reclaim"], 1, 1, 1, 1, "<T>(p)"),
    (&["rawptr_of"], 0, 1, 1, 1, "(r)"),
    (&["rawptr_from_address"], 1, 1, 1, 1, "<T>(a)"),
    (&["rawptr_address"], 0, 1, 1, 1, "(p)"),
    (&["count_ones", "leading_zeros", "trailing_zeros"], 0, 0, 1, 1, "(x)"),
    (&["checked_neg"], 0, 1, 1, 1, "(x)"),
    (&["rotate_left", "rotate_right"], 0, 0, 2, 2, "(x, k)"),
    (
        &["wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem"],
        0,
        1,
        2,
        2,
        "(a, b)",
    ),
    (&["str_len", "str_ptr", "slice_len"], 0, 0, 1, 1, "(s)"),
    (&["str_byte"], 0, 0, 2, 2, "(s, i)"),
    (&["drop"], 0, 1, 1, 1, "(x)"),
    (&["join", "cancel", "lock"], 0, 1, 1, 1, "(h)"),
    (&["allocate"], 0, 0, 2, 2, "(bytes, align)"),
    (&["deallocate"], 0, 0, 3, 3, "(p, bytes, align)"),
    (&["release"], 0, 0, 2, 2, "(p, n)"),
    (&["copy_raw"], 0, 0, 3, 3, "(dst, src, n)"),
    (&["join_status"], 0, 1, 2, 2, "(&h, mode)"),
    (&["Mutex::new"], 0, 1, 1, 1, "(x)"),
    (&["std::Result::map_err"], 0, 2, 2, 2, "(r, f)"),
    (&["spawn"], 0, 0, 1, usize::MAX, "(f, a…)"),
    (&["atomic_op"], 0, 0, 4, 4, "(a, op, x, y)"),
    (&["atomic_wait_op"], 0, 0, 3, 3, "(a, op, seen)"),
    (&["cpu_op"], 0, 0, 1, 1, "(op)"),
    // D-0254
    (&["simd_load"], 1, 1, 2, 2, "<V>(s, i)"),
    (&["simd_store"], 0, 0, 3, 3, "(s, i, v)"),
    (&["simd_eq", "simd_ne", "simd_lt", "simd_le", "simd_gt", "simd_ge", "simd_min", "simd_max"], 0, 0, 2, 2, "(a, b)"),
    (&["simd_select"], 0, 0, 3, 3, "(mask, a, b)"),
    (&["simd_any", "simd_all", "simd_sum", "simd_reduce_min", "simd_reduce_max"], 0, 0, 1, 1, "(v)"),
];

const STD_ONLY_INTRINSICS: &[&str] = &["str_slice", "append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "key_less", "key_less_at", "swap_places", "lend_places", "atomic_op", "atomic_wait_op", "cpu_op"];

// `rule.stdlib.hashmap`'s key types (spec/21 §1a, D-0041).
pub fn is_key_type(items: &Items, t: &Type) -> bool {
    key_type_problem(items, t, 0).is_none()
}

// D-0181: what `Vec::sort` and `PriorityQueue::new` order: a key type,
// or `f32`/`f64` (a NaN sorts after every other value).
fn is_sort_type(items: &Items, t: &Type) -> bool {
    matches!(t, Type::F32 | Type::F64) || is_key_type(items, t)
}

// Why `t` is not a key type (None when it is): an integer, `bool`, `str`,
// `String`, or (D-0110) a struct whose fields, or an enum whose payloads,
// are all key types. The reason names the part that is not.
fn key_type_problem(items: &Items, t: &Type, depth: usize) -> Option<String> {
    match t {
        Type::Int(_) | Type::Bool | Type::Str | Type::Never => None,
        Type::Named(n, _) if n == "std::String" => None,
        _ if depth > 16 => Some(format!("`{}` nests too deeply", t)),
        // D-0271: a `resource`-marked type is not a key type: its identity
        // is its own (`[Read-Resource-Rejected]` stands for it).
        Type::Named(n, _) if items.structs.get(n).map_or(false, |s| s.resource) || items.enums.get(n).map_or(false, |e| e.resource) => {
            Some("it is declared `resource`".to_string())
        }
        Type::Named(n, args) if items.structs.contains_key(n) => {
            let s = items.structs.get(n).unwrap();
            let sub: HashMap<String, Type> = s.type_params.iter().cloned().zip(args.iter().cloned()).collect();
            s.fields.iter().find_map(|f| {
                let ft = apply_subst(&f.ty, &sub);
                key_type_problem(items, &ft, depth + 1).map(|_| format!("its field `{}` is `{}`", f.name, ft))
            })
        }
        Type::Named(n, args) if items.enums.contains_key(n) => {
            let e = items.enums.get(n).unwrap();
            let sub: HashMap<String, Type> = e.type_params.iter().cloned().zip(args.iter().cloned()).collect();
            e.variants.iter().find_map(|v| {
                let pt = apply_subst(v.payload.as_ref()?, &sub);
                key_type_problem(items, &pt, depth + 1).map(|_| format!("its variant `{}` holds `{}`", v.name, pt))
            })
        }
        _ => Some(String::new()),
    }
}

// The message's tail naming why `t` is not a key type.
fn key_problem_text(items: &Items, t: &Type) -> String {
    match key_type_problem(items, t, 0) {
        Some(r) if !r.is_empty() => format!(": {}", r),
        _ => String::new(),
    }
}

impl<'a> Body<'a> {
    // The enum a variant path names: `E::V` (or `m::E::V`) by its prefix,
    // a bare `V` through the flat table (the resolver has already
    // rejected an ambiguous bare variant, `[Resolve-Ambiguous]`).
    // Whether the variant `segs` names has a payload: `Some(b)` when that
    // is certain -- a qualified `E::V`, or a bare `V` whose every enum
    // declaring a `V` agrees -- and `None` when enums disagree.
    fn variant_arity(&self, enum_name: &str, segs: &[String]) -> Option<bool> {
        let last = segs.last()?;
        let of = |e: &EnumDecl| e.variants.iter().find(|v| &v.name == last).map(|v| v.payload.is_some());
        if segs.len() >= 2 {
            return self.items.enums.get(enum_name).and_then(|e| of(e));
        }
        let mut seen = None;
        for e in self.items.enums.values() {
            if let Some(p) = of(e) {
                if seen.map_or(false, |s| s != p) {
                    return None;
                }
                seen = Some(p);
            }
        }
        seen
    }

    // Whether an expression is, by its form alone, a value and not a
    // reference: a literal, a struct or array literal, or a variant
    // constructed (`Ok(1)`, `None`) where no local has the name.
    fn plainly_a_value(&self, e: &Expr) -> bool {
        let variant = |segs: &[String]| self.variant_enum(segs).is_some() && !(segs.len() == 1 && self.lookup_local(&segs[0]).is_some());
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StructLit(..) | ExprKind::ArrayLit(_) | ExprKind::ArrayRepeat(..) => true,
            ExprKind::Paren(x) => self.plainly_a_value(x),
            ExprKind::Path(segs, _) => variant(segs),
            ExprKind::Call(callee, _) => matches!(&callee.kind, ExprKind::Path(segs, _) if variant(segs)),
            _ => false,
        }
    }

    fn variant_enum(&self, segs: &[String]) -> Option<String> {
        let last = segs.last()?;
        if segs.len() >= 2 {
            let key = segs[..segs.len() - 1].join("::");
            if let Some(e) = self.items.enums.get(&key) {
                return if e.variants.iter().any(|v| &v.name == last) { Some(key) } else { None };
            }
            // `m::V`: the enum of module `m` with that variant.
            if let Some(e) = self.items.module_variant_enum(&key, last) {
                return Some(e);
            }
        }
        self.items.enum_of_variant.get(last).cloned()
    }

    fn is_item_name(&self, n: &str) -> bool {
        self.items.fns.contains_key(n)
            || self.items.externs.contains_key(n)
            || self.items.statics.contains_key(n)
            || self.items.enum_of_variant.contains_key(n)
            || self.items.structs.contains_key(n)
            || self.items.enums.contains_key(n)
            || INTRINSIC_NAMES.contains(&n)
    }
}

fn f_key(f: &FnDecl) -> String {
    match &f.assoc_type {
        Some(t) => format!("{}::{}", t, f.name),
        None => f.name.clone(),
    }
}

fn explicit_subst(f: &FnDecl, explicit: &[Type]) -> HashMap<String, Type> {
    f.type_params.iter().cloned().zip(explicit.iter().cloned()).collect()
}

// A bare type parameter left opaque on purpose: `Ck::run` checks every
// generic body once with each `T` mapped to the marker `$T`, so
// `rule.type.kind`'s restriction (no arithmetic, comparison, field
// access, indexing, dereference or call on a bare `T`) can be checked
// as a static fact about the declaration (`diag.unbounded-type-
// parameter`). Every other check treats a marker as "unknown".
// `diag.unbounded-type-parameter` with what was done to the `T`.
// `&e[lo .. hi]` whose `e` is not a place: unlike `&f()`, a slice of a
// call's result or a literal is not a temporary argument (D-0073), even
// as an argument.
// `[Literal-Out-Of-Range]` for an integer literal: the value, the type
// it took, and -- when nothing fixed that type -- that it defaulted.
fn int_lit_range_msg(text: &str, it: IntTy, defaulted: bool) -> String {
    let (lo, hi) = int_range(it);
    let why = if defaulted {
        format!("; nothing here fixes its type, so it is `{}` -- write `{}: T` for the type you mean", Type::Int(it), text)
    } else {
        String::new()
    };
    crate::modres::named("diag.literal-out-of-range", format!("`{}` does not fit `{}` ({} to {}){}", text, Type::Int(it), lo, hi, why))
}

// `[Read-Uninit]` (definite assignment, spec/11): `x` may have no value
// here on some path.
fn uninit_msg(x: &str) -> String {
    let n = shown_name(x);
    crate::modres::named("diag.use-of-uninitialized", format!("`{0}` is read here, but on some path to here it was never given a value; give it one where it is declared (`… {0} = …;`) or on every path before this", n))
}

// What to write when a value of type `got` stands where `want` is
// required: the conversion, borrow or constructor that gives one.
fn conversion_hint(want: &Type, got: &Type, e: Option<&Expr>) -> String {
    let int_literal = e.map_or(false, |e| matches!(strip_parens_tc(e).kind, ExprKind::IntLit(..)));
    let place = e.map(|e| match &strip_parens_tc(e).kind {
        ExprKind::Path(segs, _) if segs.len() == 1 => shown_name(&segs[0]).to_string(),
        ExprKind::Borrow(_, inner) => match &strip_parens_tc(inner).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => shown_name(&segs[0]).to_string(),
            _ => "x".to_string(),
        },
        _ => "x".to_string(),
    });
    let x = place.unwrap_or_else(|| "x".to_string());
    match (want, got) {
        (Type::Int(w), Type::Int(g)) => {
            let (wl, wh) = int_range(*w);
            let (gl, gh) = int_range(*g);
            if wl <= gl && gh <= wh {
                format!("; convert it: `widen<{}>(…)`", Type::Int(*w))
            } else {
                format!("; convert it: `narrow<{0}>(…)` (checked: faults when the value does not fit) or `narrow_wrapping<{0}>(…)`", Type::Int(*w))
            }
        }
        (Type::F32 | Type::F64, Type::Int(_)) if int_literal => "; an integer literal is not a float -- write it with a point (`3.0`)".to_string(),
        (Type::F32 | Type::F64, Type::Int(_)) => format!("; convert it with `to_float<{}>(…)`", want),
        (Type::Int(_), Type::F32 | Type::F64) => format!("; convert it with `to_int<{}>(…)` (toward zero, checked)", want),
        (Type::F32, Type::F64) | (Type::F64, Type::F32) => format!("; convert it with `to_float<{}>(…)`", want),
        (Type::Bool, Type::Int(_)) => "; a number is not a `bool` -- compare it (`x != 0`)".to_string(),
        (Type::Named(n, _), Type::Str) if n == "std::String" => "; a `str` value becomes a `String` with `String::from_str(…)` (a literal is one already where a `String` is declared, D-0149)".to_string(),
        // D-0134: a literal is a view where one is expected; a `str` binding is not.
        (Type::Named(n, _), Type::Str) if n == "std::StringView" => format!("; view it with `StringView::of({})`", x),
        (Type::Named(n, _), Type::Ref(inner, _)) if n == "std::StringView" && matches!(&**inner, Type::Named(s, _) if s == "std::String") => {
            format!("; view the whole text: `&{}[0..$]`", x)
        }
        (Type::Ref(inner, m), t) if **inner == *t => format!("; pass a reference: `{}{}`", if *m == Mode::Exclusive { "&mut " } else { "&" }, x),
        (Type::Ref(inner, Mode::Exclusive), Type::Ref(ginner, Mode::Shared)) if inner == ginner => format!("; the parameter writes through it -- pass `&mut {}`", x),
        _ => String::new(),
    }
}

fn compare_hint(a: &Type, b: &Type) -> String {
    match (a, b) {
        (Type::Int(_), Type::Int(_)) => "; convert one side first (`widen<T>(…)` or `narrow<T>(…)`) so both have one type".to_string(),
        (Type::Int(_), Type::F32 | Type::F64) | (Type::F32 | Type::F64, Type::Int(_)) => "; convert one side first (`to_float<T>(…)` or `to_int<T>(…)`)".to_string(),
        _ => String::new(),
    }
}

fn slice_of_value_msg() -> String {
    crate::modres::named(
        "diag.borrow-of-non-place",
        "`&e[lo .. hi]` slices a place (a binding, a field, an element, `*r`); `e` here is a value (a call's result or a literal), which can be sliced only as an argument of a call whose result is not a reference; elsewhere, bind it first, then slice the binding".to_string(),
    )
}

// D-0258: the lane type of a static's atomic type: `Atomic<T>` gives `T`,
// `AtomicBool` gives `bool`; any other type is no static.
fn static_lane(t: &Type) -> Option<Type> {
    match t {
        Type::Named(n, a) if n == "std::Atomic" && a.len() == 1 => match &a[0] {
            Type::Int(it) if !matches!(it, IntTy::I128 | IntTy::U128) => Some(a[0].clone()),
            _ => None,
        },
        Type::Named(n, a) if n == "std::AtomicBool" && a.is_empty() => Some(Type::Bool),
        _ => None,
    }
}

// D-0258: an initializer written out as literals: a number, `bool`, a
// negated number, or an array or struct literal of those.
pub fn literal_tree(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..)),
        ExprKind::Paren(inner) => literal_tree(inner),
        ExprKind::ArrayLit(es) => es.iter().all(literal_tree),
        ExprKind::ArrayRepeat(x, _) => literal_tree(x),
        ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, x)| literal_tree(x)),
        _ => false,
    }
}

// D-0090: the types a bound admits, for messages.
fn bound_members(b: Bound) -> &'static str {
    match b {
        Bound::Eq | Bound::Ordered => "a number, `bool`, `str`, `String`, or a struct or enum of those",
        Bound::Number => "an integer or floating-point type",
        Bound::Integer => "an integer type",
        Bound::Clone => "a plain type, `String`, `Rc<T>`, `Box<T: clone>`, `Vec<T: clone>`, a hash table of them, a type with its own `T::clone`, or a struct or enum of `clone` types",
        Bound::Plain => "a number, `bool`, a raw pointer, a `simd`, or an array, struct or enum of those",
    }
}

fn unbounded_msg(what: &str) -> String {
    crate::modres::named(
        "diag.unbounded-type-parameter",
        format!("{} on a value of a type parameter: a generic `T` has no operations of its own; give it a bound (`T: eq`, `ordered`, `number` or `integer`, D-0090), or pass the operation in as a `fn` parameter, as `Vec::sort_by` takes `less`", what),
    )
}

fn marker_name(n: &str) -> String {
    format!("${}", n)
}

fn is_marker(t: &Type) -> bool {
    matches!(t, Type::Named(n, args) if args.is_empty() && n.starts_with('$'))
}

fn has_unresolved_marker(t: &Type) -> bool {
    match t {
        Type::Named(n, args) => (args.is_empty() && n.starts_with('$')) || args.iter().any(has_unresolved_marker),
        Type::Ref(inner, _) | Type::Slice(inner, _) | Type::Rawptr(inner) | Type::Array(inner, _) | Type::Handle(inner) | Type::Mutex(inner) | Type::Guard(inner) => {
            has_unresolved_marker(inner)
        }
        Type::Fn(ps, r) => ps.iter().any(has_unresolved_marker) || has_unresolved_marker(r),
        _ => false,
    }
}

// Does `ty` still mention one of the given (as yet unresolved) bare
// type-parameter names anywhere inside it?
fn is_bare_int_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::IntLit(..) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(..)),
        _ => false,
    }
}

// An unsuffixed integer or float literal, possibly negated or
// parenthesized: the operand whose type `rule.type.expected` takes from
// the other operand of the same operator.
fn is_bare_str_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::StrLit(_) => true,
        ExprKind::Paren(inner) => is_bare_str_literal(inner),
        _ => false,
    }
}

// D-0162: the text literals (by node address) that a generic call's
// parameter types as `String`, and those it types otherwise in another
// instantiation; only the first kind, never seen as the second, is
// rewritten (`take_literals_as_string`).
static LITERALS_AS_STRING: std::sync::Mutex<Option<(HashSet<usize>, HashSet<usize>)>> = std::sync::Mutex::new(None);

/// A front-end rewrite the checker asks for (D-0164), keyed by the node.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Rewrite {
    TextEq { string_left: bool, other_is_str: bool, negate: bool },
    // D-0218: an exclusive reference read through a shared one: `&*e`.
    SharedRef,
    // D-0218: the same for a slice: `&e[0..$]`.
    SharedSlice,
    // D-0225: a text literal where a variant's payload is a `StringView`:
    // `StringView::of(L)` (`[Str-Literal-View]` through a constructor).
    LiteralView,
}

static REWRITES: std::sync::Mutex<Option<HashMap<usize, Rewrite>>> = std::sync::Mutex::new(None);

fn note_rewrite(e: &Expr, r: Rewrite) {
    if SILENT.with(|s| s.get() > 0) {
        return;
    }
    let mut g = REWRITES.lock().unwrap_or_else(|p| p.into_inner());
    g.get_or_insert_with(Default::default).insert(e as *const Expr as usize, r);
}

/// The rewrites the last check asked for (and forgets them).
pub fn take_rewrites() -> HashMap<usize, Rewrite> {
    REWRITES.lock().unwrap_or_else(|p| p.into_inner()).take().unwrap_or_default()
}

fn note_literal_as_string(e: &Expr, as_string: bool) {
    if SILENT.with(|s| s.get() > 0) {
        return;
    }
    let mut e = e;
    while let ExprKind::Paren(inner) = &e.kind {
        e = inner;
    }
    let addr = e as *const Expr as usize;
    let mut g = LITERALS_AS_STRING.lock().unwrap_or_else(|p| p.into_inner());
    let (yes, no) = g.get_or_insert_with(Default::default);
    if as_string {
        yes.insert(addr);
    } else {
        no.insert(addr);
    }
}

/// Clears the record `check_program` keeps of D-0162's literals.
pub fn reset_literals_as_string() {
    *LITERALS_AS_STRING.lock().unwrap_or_else(|p| p.into_inner()) = None;
    *REWRITES.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// Whether some literal was typed `String` in one instantiation and
/// another type in another, in the check just made.
pub fn literal_disagreed() -> bool {
    let g = LITERALS_AS_STRING.lock().unwrap_or_else(|p| p.into_inner());
    match &*g {
        Some((yes, no)) => yes.intersection(no).next().is_some(),
        None => false,
    }
}

/// The addresses of the text literals the last check typed as `String`
/// through a generic parameter, and never otherwise.
pub fn take_literals_as_string() -> HashSet<usize> {
    let g = LITERALS_AS_STRING.lock().unwrap_or_else(|p| p.into_inner()).take();
    match g {
        Some((yes, no)) => yes.difference(&no).copied().collect(),
        None => HashSet::default(),
    }
}

fn is_bare_num_literal(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => is_bare_num_literal(inner),
        ExprKind::Paren(inner) => is_bare_num_literal(inner),
        _ => false,
    }
}

fn mentions_any(ty: &Type, names: &HashSet<String>) -> bool {
    match ty {
        Type::Named(n, args) => (args.is_empty() && names.contains(n)) || args.iter().any(|a| mentions_any(a, names)),
        Type::Ref(inner, _) | Type::Slice(inner, _) | Type::Rawptr(inner) | Type::Array(inner, _) | Type::Handle(inner) | Type::Mutex(inner) | Type::Guard(inner) => {
            mentions_any(inner, names)
        }
        Type::Fn(ps, r) => ps.iter().any(|p| mentions_any(p, names)) || mentions_any(r, names),
        _ => false,
    }
}

// The digits are a u128, so a magnitude up to 2^127 is representable
// only under an odd number of negations (i128's minimum), and one
// above 2^127 never is; `as i128` alone folded 2^127 ≤ v to a wrong
// negative value, and negating the minimum overflowed.
fn literal_value(e: &Expr) -> Option<i128> {
    fn go(e: &Expr, neg: bool) -> Option<i128> {
        match &e.kind {
            ExprKind::IntLit(v, _) if neg => (*v <= 1u128 << 127).then(|| (*v as i128).wrapping_neg()),
            ExprKind::IntLit(v, _) => i128::try_from(*v).ok(),
            ExprKind::Paren(inner) => go(inner, neg),
            ExprKind::Unary(UnOp::Neg, inner) => go(inner, !neg),
            _ => None,
        }
    }
    go(e, false)
}

fn int_range(t: IntTy) -> (i128, i128) {
    crate::value::int_min_max(t)
}

fn int_lit_fits(t: IntTy, v: u128) -> bool {
    if t == IntTy::U128 {
        return true; // every literal the lexer can produce is a u128
    }
    let (lo, hi) = int_range(t);
    if t.signed() {
        (v as i128) <= hi && (lo < 0)
    } else {
        v <= hi as u128
    }
}

// D-0111: for each statement of `b`, the bindings declared by a `let` of
// `b` (once only, and not a hidden `$` name) that no later statement nor
// the tail mentions and that no closure in `b` captures: they may end
// after that statement (`Body::end_ref_bindings` keeps those whose type
// is a reference, a slice or a `StringView`).
fn early_end_plan(b: &Block) -> (Vec<Vec<(String, usize)>>, Vec<HashSet<String>>, HashSet<String>) {
    let n = b.stmts.len();
    let mut plan: Vec<Vec<(String, usize)>> = vec![Vec::new(); n];
    let mentions: Vec<HashSet<String>> = b.stmts.iter().map(stmt_mentions).collect();
    let mut tail = HashSet::default();
    if let Some(e) = &b.tail {
        collect_free_vars_expr(e, &mut HashSet::default(), &mut tail);
    }
    let mut captured = HashSet::default();
    for s in &b.stmts {
        closure_captures_stmt(s, &mut captured);
    }
    if let Some(e) = &b.tail {
        closure_captures_expr(e, &mut captured);
    }
    let mut declared: HashMap<&str, (usize, usize)> = HashMap::default();
    for (i, s) in b.stmts.iter().enumerate() {
        if let Stmt::Let { name, .. } = s {
            declared.entry(name.as_str()).or_insert((i, 0)).1 += 1;
        }
    }
    for (name, (d, count)) in declared {
        if count != 1 || name.starts_with('$') || tail.contains(name) || captured.contains(name) {
            continue;
        }
        let last = (d + 1..n).filter(|&j| mentions[j].contains(name)).last().unwrap_or(d);
        if last + 1 < n || b.tail.is_some() {
            plan[last].push((name.to_string(), d));
        }
    }
    for p in plan.iter_mut() {
        p.sort();
    }
    (plan, mentions, tail)
}

// D-0275: a reference, slice or `StringView` whose referent holds no
// reference and no `fn` value: it can take another binding's path only by
// being rebound (`stmt_rebinds`), never through what it reaches.
fn inert_ref(items: &Items, t: &Type) -> bool {
    match t {
        Type::Ref(inner, _) | Type::Slice(inner, _) => !may_hold_ref(items, inner, 0),
        Type::Named(n, a) => n == "std::StringView" && a.is_empty(),
        _ => false,
    }
}

// Whether statement `s` may rebind `m`: it assigns `m` whole, borrows it
// whole exclusively, or a closure in it captures it (D-0275).
fn stmt_rebinds(s: &Stmt, m: &str) -> bool {
    let is_m = |e: &Expr| matches!(&strip_parens_tc(e).kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == m);
    let mut found = false;
    let mut look = |e: &Expr| match &e.kind {
        ExprKind::Assign(a, _) if is_m(a) => found = true,
        ExprKind::Borrow(Mode::Exclusive, a) if is_m(a) => found = true,
        ExprKind::Closure { captures, .. } if captures.iter().any(|c| c == m) => found = true,
        _ => {}
    };
    match s {
        Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => crate::consts::visit(e, &mut look),
        Stmt::Destructure { init, .. } => crate::consts::visit(init, &mut look),
        Stmt::Let { init: None, .. } => {}
    }
    found
}

// The names statement `s` mentions (bound inside it or not: a shadowing
// inner binding only makes the answer larger).
fn stmt_mentions(s: &Stmt) -> HashSet<String> {
    let mut free = HashSet::default();
    let mut bound = HashSet::default();
    match s {
        Stmt::Let { init, .. } => {
            if let Some(e) = init {
                collect_free_vars_expr(e, &mut bound, &mut free);
            }
        }
        Stmt::Destructure { init, .. } => collect_free_vars_expr(init, &mut bound, &mut free),
        Stmt::Expr(e) | Stmt::BlockLike(e) => collect_free_vars_expr(e, &mut bound, &mut free),
    }
    free
}

fn closure_captures_stmt(s: &Stmt, out: &mut HashSet<String>) {
    match s {
        Stmt::Let { init: Some(e), .. } => closure_captures_expr(e, out),
        Stmt::Let { init: None, .. } => {}
        Stmt::Destructure { init, .. } => closure_captures_expr(init, out),
        Stmt::Expr(e) | Stmt::BlockLike(e) => closure_captures_expr(e, out),
    }
}

// Every name a closure anywhere in `e` captures.
fn closure_captures_expr(e: &Expr, out: &mut HashSet<String>) {
    let block = |b: &Block, out: &mut HashSet<String>| {
        for s in &b.stmts {
            closure_captures_stmt(s, out);
        }
        if let Some(t) = &b.tail {
            closure_captures_expr(t, out);
        }
    };
    match &e.kind {
        ExprKind::Closure { captures, body, .. } => {
            out.extend(captures.iter().cloned());
            block(body, out);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            closure_captures_expr(a, out);
            closure_captures_expr(lo, out);
            closure_captures_expr(hi, out);
        }
        ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Paren(a) | ExprKind::Propagate(a) | ExprKind::Borrow(_, a) | ExprKind::ArrayRepeat(a, _) => closure_captures_expr(a, out),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            closure_captures_expr(a, out);
            closure_captures_expr(b, out);
        }
        ExprKind::Call(c, args) => {
            closure_captures_expr(c, out);
            args.iter().for_each(|a| closure_captures_expr(a, out));
        }
        ExprKind::StructLit(_, _, fields) => fields.iter().for_each(|(_, x)| closure_captures_expr(x, out)),
        ExprKind::ArrayLit(items) => items.iter().for_each(|x| closure_captures_expr(x, out)),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, out),
        ExprKind::If(c, t, f) => {
            closure_captures_expr(c, out);
            block(t, out);
            if let Some(f) = f {
                closure_captures_expr(f, out);
            }
        }
        ExprKind::While(c, b, step) => {
            closure_captures_expr(c, out);
            block(b, out);
            if let Some(s) = step {
                closure_captures_expr(s, out);
            }
        }
        ExprKind::Match(s, arms) => {
            closure_captures_expr(s, out);
            arms.iter().for_each(|a| closure_captures_expr(&a.body, out));
        }
        ExprKind::Return(Some(v)) => closure_captures_expr(v, out),
        _ => {}
    }
}

fn collect_free_vars_block(b: &Block, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    for s in &b.stmts {
        match s {
            Stmt::Let { name, init, .. } => {
                if let Some(e) = init {
                    collect_free_vars_expr(e, bound, free);
                }
                bound.insert(name.clone());
            }
            Stmt::Destructure { fields, init, .. } => {
                collect_free_vars_expr(init, bound, free);
                for field in fields {
                    bound.insert(field.clone());
                }
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => collect_free_vars_expr(e, bound, free),
        }
    }
    if let Some(e) = &b.tail {
        collect_free_vars_expr(e, bound, free);
    }
}

fn collect_free_vars_expr(e: &Expr, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::SliceOf(_, b, lo, hi) => {
            collect_free_vars_expr(b, bound, free);
            collect_free_vars_expr(lo, bound, free);
            collect_free_vars_expr(hi, bound, free);
        }
        ExprKind::Dollar => {}
        ExprKind::Path(segs, _) => {
            if segs.len() == 1 && !bound.contains(&segs[0]) {
                free.insert(segs[0].clone());
            }
        }
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit => {}
        ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Paren(a) | ExprKind::Propagate(a) => {
            collect_free_vars_expr(a, bound, free)
        }
        ExprKind::Borrow(_, a) => collect_free_vars_expr(a, bound, free),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            collect_free_vars_expr(a, bound, free);
            collect_free_vars_expr(b, bound, free);
        }
        ExprKind::Call(c, args) => {
            collect_free_vars_expr(c, bound, free);
            // `fault(name, …)`'s first argument is a diagnostic's name, not a
            // variable the closure uses (CHG-0324).
            let names_a_diag = matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "fault");
            for (i, a) in args.iter().enumerate() {
                if names_a_diag && i == 0 {
                    continue;
                }
                collect_free_vars_expr(a, bound, free);
            }
        }
        ExprKind::StructLit(_, _, fields) => {
            for (_, e) in fields {
                collect_free_vars_expr(e, bound, free);
            }
        }
        ExprKind::ArrayLit(items) => {
            for it in items {
                collect_free_vars_expr(it, bound, free);
            }
        }
        ExprKind::ArrayRepeat(it, _) => collect_free_vars_expr(it, bound, free),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => {
            let mut inner_bound = bound.clone();
            collect_free_vars_block(b, &mut inner_bound, free);
        }
        ExprKind::If(c, t, f) => {
            collect_free_vars_expr(c, bound, free);
            let mut ib = bound.clone();
            collect_free_vars_block(t, &mut ib, free);
            if let Some(f) = f {
                collect_free_vars_expr(f, bound, free);
            }
        }
        ExprKind::While(c, b, step) => {
            collect_free_vars_expr(c, bound, free);
            let mut ib = bound.clone();
            collect_free_vars_block(b, &mut ib, free);
            if let Some(s) = step {
                collect_free_vars_expr(s, bound, free);
            }
        }
        ExprKind::Match(s, arms) => {
            collect_free_vars_expr(s, bound, free);
            for a in arms {
                let mut ib = bound.clone();
                if let Some(b) = &a.binder {
                    ib.insert(b.clone());
                }
                collect_free_vars_expr(&a.body, &mut ib, free);
            }
        }
        ExprKind::Return(v) => {
            if let Some(e) = v {
                collect_free_vars_expr(e, bound, free);
            }
        }
        ExprKind::Break | ExprKind::Continue => {}
        ExprKind::Closure { captures, .. } => {
            // A nested closure's own capture list is itself the set of
            // outer names it needs -- those are free with respect to
            // *this* closure too, unless already bound here.
            for c in captures {
                if !bound.contains(c) {
                    free.insert(c.clone());
                }
            }
        }
    }
}

// Whether `e` is a constant expression (D-0036): literals, operators,
// other constants, struct, array and variant literals of constant
// expressions, their fields and elements (D-0066), the name of a
// function, and the pure intrinsics below.
fn const_expr(items: &Items, e: &Expr) -> bool {
    const PURE: &[&str] = &[
        "min_value", "max_value", "sizeof", "alignof", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int",
        "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul",
    ];
    let is_variant = |segs: &[String]| {
        let last = segs.last().map(|s| s.as_str()).unwrap_or("");
        if segs.len() >= 2 {
            let prefix = segs[..segs.len() - 1].join("::");
            if let Some(en) = items.enums.get(&prefix) {
                return en.variants.iter().any(|v| v.name == last);
            }
        }
        items.enum_of_variant.contains_key(last)
    };
    match &e.kind {
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) | ExprKind::Unit => true,
        ExprKind::Paren(x) | ExprKind::Unary(_, x) => const_expr(items, x),
        ExprKind::Binary(_, a, b) => const_expr(items, a) && const_expr(items, b),
        ExprKind::ArrayLit(es) => es.iter().all(|x| const_expr(items, x)),
        ExprKind::ArrayRepeat(x, _) => const_expr(items, x),
        ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, x)| const_expr(items, x)),
        // D-0066: a field of a constant struct, an element of a constant
        // array at a constant index.
        ExprKind::Field(x, _) => const_expr(items, x),
        ExprKind::Index(x, i) => const_expr(items, x) && const_expr(items, i),
        ExprKind::Path(segs, _) => is_variant(segs) || items.fns.get(&segs.join("::")).map_or(false, |f| !f.is_const),
        ExprKind::Call(c, args) => {
            let ExprKind::Path(segs, _) = &c.kind else { return false };
            let name = segs.join("::");
            let args_ok = args.iter().all(|x| const_expr(items, x));
            match items.fns.get(&name) {
                Some(f) => f.is_const && args.is_empty(),
                None => args_ok && (is_variant(segs) || PURE.contains(&name.as_str())),
            }
        }
        _ => false,
    }
}

fn is_borrowing_closure(e: &Expr) -> bool {
    matches!(&strip_parens_tc(e).kind, ExprKind::Closure { is_move: false, captures, .. } if !captures.is_empty())
}

fn is_spawn_call(e: &Expr) -> bool {
    match &strip_parens_tc(e).kind {
        ExprKind::Call(callee, _) => matches!(&callee.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "spawn"),
        _ => false,
    }
}

fn strip_parens_tc(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Paren(inner) => strip_parens_tc(inner),
        _ => e,
    }
}

// An arm's pattern as elements: its variants, outermost first, then its
// literal as `=` and the literal's value (D-0057).
fn pattern_elems(a: &Arm) -> Vec<String> {
    let mut e = a.chain();
    if let Some(l) = &a.lit {
        e.push(format!("={}", literal_key(l)));
    }
    e
}

// A pattern literal's value as text: `-3`, `97` for `b'a'`, `true`.
pub fn literal_key(l: &Expr) -> String {
    match &l.kind {
        ExprKind::IntLit(v, _) => v.to_string(),
        ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
            ExprKind::IntLit(v, _) => format!("-{}", v),
            _ => String::new(),
        },
        ExprKind::BoolLit(b) => b.to_string(),
        // D-0127: a text literal, quoted (distinct from every number).
        ExprKind::StrLit(t) => format!("{:?}", t),
        _ => String::new(),
    }
}

// `str`, `String` or `StringView`: a level a text literal is matched
// against (D-0127).
pub fn is_text_ty(t: &Type) -> bool {
    match t {
        Type::Str => true,
        Type::Named(n, _) => n == "std::String" || n == "std::StringView",
        _ => false,
    }
}

// `Ok(Some(_))` for the elements `Ok`, `Some` whose last variant has a
// payload; a literal element is written as its value (D-0056's
// missing-pattern detail).
fn render_pattern(chain: &[String], payload: bool) -> String {
    if chain.is_empty() {
        return "_".to_string();
    }
    let shown: Vec<&str> = chain.iter().map(|e| e.strip_prefix('=').unwrap_or(e)).collect();
    let mut s = shown.join("(");
    if payload {
        s.push_str("(_)");
    }
    for _ in 1..chain.len() {
        s.push(')');
    }
    s
}

// Every expression in `e`, `e` first (the bodies of closures too).
fn visit_exprs(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    fn block(b: &Block, f: &mut dyn FnMut(&Expr)) {
        for s in &b.stmts {
            match s {
                Stmt::Expr(x) | Stmt::BlockLike(x) => visit_exprs(x, f),
                Stmt::Let { init: Some(x), .. } | Stmt::Destructure { init: x, .. } => visit_exprs(x, f),
                Stmt::Let { .. } => {}
            }
        }
        if let Some(t) = &b.tail {
            visit_exprs(t, f);
        }
    }
    f(e);
    match &e.kind {
        ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Borrow(_, x) | ExprKind::Propagate(x) | ExprKind::ArrayRepeat(x, _) | ExprKind::Return(Some(x)) => visit_exprs(x, f),
        ExprKind::SliceOf(_, x, lo, hi) => {
            visit_exprs(x, f);
            visit_exprs(lo, f);
            visit_exprs(hi, f);
        }
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            visit_exprs(a, f);
            visit_exprs(b, f);
        }
        ExprKind::Call(c, xs) => {
            visit_exprs(c, f);
            for x in xs {
                visit_exprs(x, f);
            }
        }
        ExprKind::ArrayLit(xs) => {
            for x in xs {
                visit_exprs(x, f);
            }
        }
        ExprKind::StructLit(_, _, fs) => {
            for (_, x) in fs {
                visit_exprs(x, f);
            }
        }
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, f),
        ExprKind::If(c, t, e) => {
            visit_exprs(c, f);
            block(t, f);
            if let Some(e) = e {
                visit_exprs(e, f);
            }
        }
        ExprKind::While(c, b, step) => {
            visit_exprs(c, f);
            block(b, f);
            if let Some(x) = step {
                visit_exprs(x, f);
            }
        }
        ExprKind::Match(x, arms) => {
            visit_exprs(x, f);
            for a in arms {
                visit_exprs(&a.body, f);
            }
        }
        ExprKind::Closure { body, .. } => block(body, f),
        _ => {}
    }
}

// The exclusive borrows written in `e` (`&mut x`, `&mut x[a..b]`), for the
// D-0282 order rules.
fn excl_borrows_in(e: &Expr, out: &mut Vec<Expr>) {
    fn block_in(b: &Block, out: &mut Vec<Expr>) {
        for s in &b.stmts {
            match s {
                Stmt::Expr(x) | Stmt::BlockLike(x) => excl_borrows_in(x, out),
                Stmt::Let { init: Some(x), .. } | Stmt::Destructure { init: x, .. } => excl_borrows_in(x, out),
                _ => {}
            }
        }
        if let Some(t) = &b.tail {
            excl_borrows_in(t, out);
        }
    }
    match &e.kind {
        ExprKind::Borrow(Mode::Exclusive, _) => out.push(e.clone()),
        ExprKind::SliceOf(m, x, lo, hi) => {
            if *m == Mode::Exclusive {
                out.push(Expr { kind: ExprKind::Borrow(Mode::Exclusive, x.clone()), line: e.line, col: e.col });
            }
            excl_borrows_in(x, out);
            excl_borrows_in(lo, out);
            excl_borrows_in(hi, out);
        }
        ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Borrow(_, x) | ExprKind::Propagate(x) | ExprKind::ArrayRepeat(x, _) => excl_borrows_in(x, out),
        ExprKind::Return(Some(x)) => excl_borrows_in(x, out),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            excl_borrows_in(a, out);
            excl_borrows_in(b, out);
        }
        ExprKind::Call(c, xs) => {
            excl_borrows_in(c, out);
            for x in xs {
                excl_borrows_in(x, out);
            }
        }
        ExprKind::ArrayLit(xs) => {
            for x in xs {
                excl_borrows_in(x, out);
            }
        }
        ExprKind::StructLit(_, _, fs) => {
            for (_, x) in fs {
                excl_borrows_in(x, out);
            }
        }
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block_in(b, out),
        ExprKind::If(c, t, f) => {
            excl_borrows_in(c, out);
            block_in(t, out);
            if let Some(f) = f {
                excl_borrows_in(f, out);
            }
        }
        ExprKind::While(c, b, step) => {
            excl_borrows_in(c, out);
            block_in(b, out);
            if let Some(x) = step {
                excl_borrows_in(x, out);
            }
        }
        ExprKind::Match(x, arms) => {
            excl_borrows_in(x, out);
            for a in arms {
                excl_borrows_in(&a.body, out);
            }
        }
        _ => {}
    }
}
