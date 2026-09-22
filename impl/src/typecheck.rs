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
use std::collections::{HashMap, HashSet};

type TResult = Result<Option<Type>, String>;

// [Extern-Non-Ffi-Type] (spec/20 rule.trust.extern-call): `FfiType ::=
// integer types | f32 | f64 | bool | void | rawptr<tau>` -- every
// extern fn's parameter and return types must be one of these.
fn is_ffi_type(t: &Type) -> bool {
    matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Void | Type::Rawptr(_))
}

// The module part of an item's qualified key (`modres::qualify`): one
// trailing segment for `m::f`, two for an associated `m::T::name`.
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
fn recursive_type(items: &Items) -> Option<usize> {
    let mut decls: Vec<(&String, Vec<String>, usize)> = items.structs.iter().map(|(k, s)| (k, s.type_params.clone(), s.line)).collect();
    decls.extend(items.enums.iter().map(|(k, e)| (k, e.type_params.clone(), e.line)));
    decls.sort_by(|a, b| a.0.cmp(b.0));
    for (k, params, line) in decls {
        let t = Type::Named(k.clone(), params.iter().map(|p| Type::Named(p.clone(), Vec::new())).collect());
        if contains_by_value(items, &t, &mut Vec::new()) {
            return Some(line);
        }
    }
    None
}

pub fn check_program(items: &Items) -> Result<Vec<StaticAssert>, String> {
    if let Some(line) = recursive_type(items) {
        return Err(format!("diag.recursive-type@{}", line));
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

    // Whole-program static fact, checked unconditionally for every
    // declared extern fn regardless of whether it's ever called --
    // `[Extern-Non-Ffi-Type]` is `disposition: rejected` on the
    // declaration itself, the same shape as `rule.trust.unsafe`'s own
    // ahead-of-execution facts this pass already checks per call site,
    // but this one has no call site to hang the check on if the extern
    // is simply never used.
    for ext in items.externs.values() {
        for p in &ext.params {
            if !is_ffi_type(&p.ty) {
                return Err("diag.extern-non-ffi-type".to_string());
            }
        }
        if !is_ffi_type(&ext.ret) {
            return Err("diag.extern-non-ffi-type".to_string());
        }
    }
    // `rule.fn.program` `[Program-No-Main]`: exactly one non-generic
    // `fn main()` or `fn main() : u8` at the root, with no parameters
    // (an `extern` main lives in `externs`, so it is absent here too).
    // A root `main` of another shape is reported where it is declared.
    let main = match items.fns.get("main") {
        Some(m) if m.type_params.is_empty() && m.params.is_empty() && matches!(m.ret, Type::Void | Type::Int(IntTy::U8)) => m.clone(),
        Some(m) => return Err(format!("diag.no-main@{}", m.line)),
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
            }
        }
    }
    let mut ck = Ck {
        items,
        checked: HashSet::new(),
        worklist: vec![("main".to_string(), vec![])],
        max_steps: 20000,
        static_asserts: Vec::new(),
        static_assert_keys: HashSet::new(),
    };
    // `rule.fn.program`: a program is well-formed iff *every* item is
    // well-typed and every static rule is satisfied -- called or not.
    // Every non-generic function is checked as itself; every generic
    // body once with its type parameters opaque (`rule.type.kind`), in
    // addition to each concrete instantiation discovered from `main`.
    let mut keys: Vec<&String> = items.fns.keys().collect();
    keys.sort();
    for key in keys {
        let f = &items.fns[key];
        if f.type_params.is_empty() {
            ck.worklist.push((key.clone(), vec![]));
        } else {
            ck.worklist.push((key.clone(), f.type_params.iter().map(|t| Type::Named(marker_name(t), vec![])).collect()));
        }
    }
    let _ = main; // looked up fresh from the worklist below
    ck.run()?;
    // [Call-Multi-Ref-Return-Rejected] (spec/10 rule.temporal.elision):
    // a function returning a reference must have exactly one reference-
    // typed parameter. A declaration-level fact, checked for every
    // declared function whether or not it is called -- after the
    // bodies, so that a body's own `[Ref-Escape-Rejected]` (which
    // spec/conformance.md's `conf.reference-escape-rejected` names for
    // the zero-parameter shape) is what gets reported for it.
    for f in items.fns.values() {
        if is_borrow_ty(&f.ret) {
            let refs = f.params.iter().filter(|p| is_borrow_ty(&p.ty)).count();
            if refs != 1 {
                return Err("diag.lifetime-elision-ambiguous".to_string());
            }
        }
    }
    Ok(ck.static_asserts)
}

struct Ck<'a> {
    items: &'a Items,
    checked: HashSet<(String, Vec<Type>)>,
    worklist: Vec<(String, Vec<Type>)>,
    max_steps: u32,
    static_asserts: Vec<StaticAssert>,
    static_assert_keys: HashSet<(usize, String)>,
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
}

// Local per-function-body checking state.
struct Body<'a> {
    items: &'a Items,
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
    // The binding a whole-binding assignment is about to give a value
    // (D-0033 (1)): its lookup is not `[Binding-Lookup-Stale]`.
    reinit_target: Option<String>,
    ret: Type,
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
}

impl<'a> Ck<'a> {
    fn run(&mut self) -> Result<(), String> {
        let mut steps = 0u32;
        while let Some((name, targs)) = self.worklist.pop() {
            steps += 1;
            if steps > self.max_steps {
                break; // safety valve against a pathological/unbounded instantiation family
            }
            let key = (name.clone(), targs.clone());
            if self.checked.contains(&key) {
                continue;
            }
            self.checked.insert(key);
            let f = match self.items.fns.get(&name) {
                Some(f) => f.clone(),
                None => continue, // extern or unresolved; nothing to type-check
            };
            if f.type_params.len() != targs.len() {
                continue; // can't build a concrete substitution; skip (conservative)
            }
            let subst: HashMap<String, Type> = f.type_params.iter().cloned().zip(targs.iter().cloned()).collect();
            let mut gamma = HashMap::new();
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
                block_parent: HashMap::new(),
                decl_block: HashMap::new(),
                ref_origin: HashMap::new(),
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                borrow_referents: HashMap::new(),
                infer_poisoned: false,
                privileged: false,
                closure_moves: HashMap::new(),
                closure_arity: HashMap::new(),
                move_captures: HashMap::new(),
                capture_names: HashSet::new(),
                loops: Vec::new(),
                report: true,
                place_pos: false,
                reinit_target: None,
                ret: ret.clone(),
                subst,
                new_instantiations: Vec::new(),
                static_asserts: Vec::new(),
                unsafe_depth: 0,
                while_depth: 0,
                current_line: 0,
            };
            // Entry state (spec/14 §6): parameters `valid = T`, `init = T`.
            for p in &f.params {
                body.declare(&p.name, true);
            }
            let bt = match body.check_fn_body(&f.body, &ret) {
                Ok(bt) => bt,
                Err(d) => {
                    if std::env::var("COBALTC_TRACE").is_ok() {
                        eprintln!("TRACE static reject in `{}` <{:?}>: {}", name, targs, d);
                    }
                    return Err(d);
                }
            };
            if let Some(bt) = &bt {
                if !ty_compat(bt, &ret) {
                    return Err(format!("diag.type-mismatch@{}", body.current_line));
                }
            }
            self.worklist.extend(body.new_instantiations);
            for a in body.static_asserts {
                if self.static_assert_keys.insert(a.key.clone()) {
                    self.static_asserts.push(a);
                }
            }
        }
        Ok(())
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

fn unify_type_shape(decl_ty: &Type, actual: &Type, m: &mut HashMap<String, Type>) {
    match (decl_ty, actual) {
        (Type::Named(n, args), _) if args.is_empty() => {
            m.entry(n.clone()).or_insert_with(|| actual.clone());
        }
        (Type::Ref(d, _), Type::Ref(a, _)) | (Type::Slice(d, _), Type::Slice(a, _)) => unify_type_shape(d, a, m),
        (Type::Rawptr(d), Type::Rawptr(a)) => unify_type_shape(d, a, m),
        (Type::Array(d, _), Type::Array(a, _)) => unify_type_shape(d, a, m),
        (Type::Handle(d), Type::Handle(a)) => unify_type_shape(d, a, m),
        (Type::Mutex(d), Type::Mutex(a)) => unify_type_shape(d, a, m),
        (Type::Guard(d), Type::Guard(a)) => unify_type_shape(d, a, m),
        (Type::Named(_, dargs), Type::Named(_, aargs)) => {
            for (dp, ap) in dargs.iter().zip(aargs.iter()) {
                unify_type_shape(dp, ap, m);
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
            _ => return false,
        }
    }
    true
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
}

// A shadowed binding's facts, kept aside while the shadowing block is
// open and restored at its exit (facts belong to bindings, not names).
#[derive(Clone, Debug)]
struct SavedFacts {
    valid: Tri,
    init: Tri,
    own_deriv: Option<HashMap<DerivKey, Tri>>,
    rooted: Vec<(String, DerivKey, Tri)>,
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
        self.valid.insert(name.to_string(), Tri::T);
        self.init.insert(name.to_string(), if initialized { Tri::T } else { Tri::F });
        self.deriv.remove(name);
        self.clear_rooted(name);
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
        self.valid.remove(name);
        self.init.remove(name);
        self.deriv.remove(name);
        self.clear_rooted(name);
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
            let empty = HashMap::new();
            let fa = self.deriv.get(r).unwrap_or(&empty);
            let fb = other.deriv.get(r).unwrap_or(&empty);
            let mut keys: HashSet<&DerivKey> = fa.keys().collect();
            keys.extend(fb.keys());
            let mut merged = HashMap::new();
            for k in keys {
                if !out.valid.contains_key(&k.root) {
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
            ExprKind::StructLit(..) | ExprKind::ArrayLit(..) => {
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

fn root_local_name(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Path(segs, _) if segs.len() == 1 => Some(segs[0].clone()),
        ExprKind::Paren(inner) => root_local_name(inner),
        _ => None,
    }
}

impl<'a> Body<'a> {
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
        for s in &b.stmts {
            match self.check_stmt(s) {
                Ok(t) => last_stmt_ty = t,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            }
        }
        if r.is_ok() {
            r = match &b.tail {
                Some(e) => match self.check_escape_to_caller(e) {
                    Err(d) => Err(format!("{d}@{}", e.line)),
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
                ExprKind::Paren(p) => self.referent_of_place(&Expr { kind: ExprKind::Deref(p.clone()), line: e.line }),
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

    // The reference-valued sub-expressions whose value `e` stores: `e`
    // itself when it is a borrow or an elided call; the field, element
    // and payload initializers of an aggregate literal (a reference
    // stored into a field of an aggregate whose own value escapes
    // escapes with it); the results of a block, `if` or `match`.
    fn collect_stored_borrows<'e>(&self, e: &'e Expr, out: &mut Vec<&'e Expr>) {
        match &e.kind {
            ExprKind::Borrow(..) | ExprKind::SliceOf(..) => out.push(e),
            ExprKind::Paren(inner) => self.collect_stored_borrows(inner, out),
            ExprKind::Call(callee, args) => {
                if let ExprKind::Path(segs, _) = &callee.kind {
                    if args.len() == 1 && self.items.enum_of_variant.contains_key(segs.last().unwrap()) && !self.items.fns.contains_key(&segs.join("::")) {
                        self.collect_stored_borrows(&args[0], out);
                        return;
                    }
                }
                if self.referent_of(e).is_some() {
                    out.push(e);
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
        for b in bs {
            if let Some((br, _)) = self.referent_of(b) {
                if self.strictly_encloses(dest, br) {
                    return Err("diag.reference-escapes-scope".to_string());
                }
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
        for b in bs {
            if let Some((_, root)) = self.referent_of(b) {
                let ref_param = self.params.contains(&root) && self.lookup_local(&root).as_ref().map_or(false, is_borrow_ty);
                if !ref_param {
                    return Err("diag.reference-escapes-scope".to_string());
                }
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
                if !matches!(t, Type::Array(..)) {
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
        let mut t = self.lookup_local(x)?;
        for el in path {
            t = match (el, &t) {
                (PElem::Field(f), Type::Named(sname, args)) => {
                    let sd = self.items.structs.get(sname)?;
                    let subst: HashMap<String, Type> = sd.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    let fd = sd.fields.iter().find(|fd| &fd.name == f)?;
                    apply_subst(&fd.ty, &subst)
                }
                (PElem::Index(_), Type::Array(inner, _)) => (**inner).clone(),
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
                    Type::Ref(t, _) => *t,
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
                    Type::Ref(t, _) => *t,
                    t => t,
                };
                if let Some(t) = indexed_elem(&bt).filter(|_| !matches!(bt, Type::Array(..))) {
                    return Some(t);
                }
                match bt {
                    Type::Array(t, _) => Some(*t),
                    _ => None,
                }
            }
            ExprKind::Call(callee, _) => {
                let f = self.resolve_fn_path(callee)?;
                if f.type_params.is_empty() {
                    Some(f.ret.clone())
                } else if matches!(f.ret, Type::Ref(..)) {
                    Some(f.ret.clone()) // only its mode is ever consulted
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
            ExprKind::Deref(inner) => matches!(self.static_place_type(inner), Some(Type::Ref(_, Mode::Shared))),
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
        for facts in self.flow.deriv.values() {
            for (k, v) in facts {
                if *v == Tri::T && k.root == x && paths_overlap(&k.path, path) && !permitted(mode.clone(), k.mode.clone()) {
                    self.refute(diag)?;
                }
            }
        }
        Ok(())
    }

    // `solitary(a_x)`: refuted when some `deriv(·, x.·, ·) = T`.
    fn flow_solitary(&self, x: &str, diag: &str) -> Result<(), String> {
        for facts in self.flow.deriv.values() {
            for (k, v) in facts {
                if *v == Tri::T && k.root == x {
                    self.refute(diag)?;
                }
            }
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
            None => return Ok(()),
        };
        if self.flow.init.get(&x) != Some(&Tri::T) {
            self.refute("diag.use-of-uninitialized")?;
        }
        if self.place_is_resource(&x, &path) {
            self.refute("diag.read-of-resource")?;
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
        let (x, path) = match self.place_of(lhs) {
            Some(p) => p,
            None => return Ok(()),
        };
        if path.is_empty() {
            // D-0049: statically only where every value of the type owns
            // something; otherwise the value decides, at run time.
            if self.place_is_resource(&x, &[])
                && self.place_type(&x, &[]).map_or(false, |t| always_owns(self.items, &t))
                && self.flow.valid.get(&x) == Some(&Tri::T)
                && self.flow.init.get(&x) == Some(&Tri::T)
            {
                self.refute("diag.overwrite-of-live-resource")?;
            }
        } else if self.flow.init.get(&x) != Some(&Tri::T) {
            self.refute("diag.use-of-uninitialized")?;
        }
        self.flow_clash(&x, &path, Mode::Exclusive, "diag.aliasing-conflict")?;
        if path.is_empty() && !self.flow.unreachable {
            self.flow.init.insert(x, Tri::T);
        }
        Ok(())
    }

    // `[Borrow]` of a place: `¬clash(a0, m)`.
    fn flow_borrow(&mut self, inner: &Expr, mode: &Mode) -> Result<(), String> {
        if let Some((x, path)) = self.place_of(inner) {
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
        if let Some(name) = root_local_name(e) {
            if let Some(Some(t)) = self.move_captures.get(&name) {
                if is_resource_ty(self.items, t) {
                    return Err("diag.move-out-of-field".to_string());
                }
            }
        }
        if let Some((x, path)) = self.place_of(e) {
            if self.place_is_resource(&x, &path) {
                self.place_pos = true;
                let t = self.check_expr(e, expected)?;
                if path.is_empty() {
                    self.flow_consume(&x, "diag.move-while-aliased")?;
                } else {
                    self.refute("diag.move-out-of-field")?;
                }
                return Ok(t);
            }
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
            ExprKind::Borrow(m, inner) | ExprKind::SliceOf(m, inner, _, _) => self.place_of(inner).or_else(|| self.vec_element_root(inner)).map(|(root, path)| DerivKey { root, path, mode: m.clone() }),
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
                    ExprKind::Borrow(m, inner) | ExprKind::SliceOf(m, inner, _, _) => self.place_of(inner).map(|(root, path)| DerivKey { root, path, mode: m.clone() }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // `&v[i]` on a `Vec` is `Vec::index_shared(&v, i)` (`[Index-Vec]`), an
    // elided call on a borrow of `v`: derived from `v` as that call is.
    fn vec_element_root(&self, e: &Expr) -> Option<(String, Vec<PElem>)> {
        let ExprKind::Index(base, _) = &strip_parens_tc(e).kind else { return None };
        match self.static_place_type(base)? {
            Type::Named(n, _) if n == "std::Vec" => self.place_of(base),
            _ => None,
        }
    }

    // `auto r = e;` / `r = e;` for a reference-typed `r`: a visible
    // borrow (or elided call on one) sets that one `deriv` fact and
    // clears the others; anything else clears them all.
    fn set_deriv(&mut self, r: &str, fact: Option<DerivKey>) {
        if self.flow.unreachable {
            return;
        }
        self.flow.deriv.remove(r);
        if let Some(k) = fact {
            let mut m = HashMap::new();
            m.insert(k, Tri::T);
            self.flow.deriv.insert(r.to_string(), m);
        }
    }

    fn check_block(&mut self, b: &Block, expected: Option<&Type>) -> TResult {
        let mut last_stmt_ty: Option<Type> = None;
        for s in &b.stmts {
            last_stmt_ty = self.check_stmt(s)?;
        }
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

    // A nested block opens a scope: the bindings it declares end at its
    // exit (`[Block-Exit]`), for both `gamma` and the flow facts.
    fn check_nested_block(&mut self, b: &Block, expected: Option<&Type>) -> TResult {
        let saved_gamma = self.gamma.clone();
        self.enter_scope();
        let r = self.check_block(b, expected);
        self.exit_scope();
        self.gamma = saved_gamma;
        r
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
            Err(d) if !d.contains('@') => Err(format!("{d}@{}", self.current_line)),
            other => other,
        }
    }

    fn check_stmt_inner(&mut self, s: &Stmt) -> Result<Option<Type>, String> {
        match s {
            Stmt::Let { ty, name, init } => {
                let expected = ty.as_ref().map(|t| apply_subst(t, &self.subst));
                let ity = match init {
                    Some(e) => self.check_store_operand(e, expected.as_ref())?,
                    None => None,
                };
                if let (Some(exp), Some(actual)) = (&expected, &ity) {
                    if !has_unresolved_marker(exp) && !has_unresolved_marker(actual) && !ty_compat(exp, actual) {
                        return Err("diag.type-mismatch".to_string());
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
                self.closure_moves.remove(name);
                self.closure_arity.remove(name);
                if let Some(e) = init {
                    let here = *self.block_stack.last().unwrap_or(&0);
                    self.check_escape_into(here, e)?;
                    if let ExprKind::Closure { is_move, params, .. } = &e.kind {
                        self.closure_moves.insert(name.clone(), *is_move);
                        self.closure_arity.insert(name.clone(), params.len());
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
                    self.set_deriv(name, fact);
                    let origin = init.as_ref().and_then(|e| self.referent_of(e));
                    self.ref_origin.insert(name.clone(), origin);
                }
                Ok(None)
            }
            Stmt::Destructure { struct_name, fields, init } => {
                let sty = Type::Named(struct_name.clone(), Vec::new());
                let st = self.check_store_operand(init, Some(&sty)).unwrap_or(None);
                if let Some(sdecl) = self.items.structs.get(struct_name).cloned() {
                    // A struct with its own destructor keeps its fields: the
                    // destructor would run on a struct they had left.
                    if self.items.fns.contains_key(&format!("{}::drop", struct_name)) {
                        return Err("diag.move-out-of-field".to_string());
                    }
                    // `[Let-Destructure]` (D-0044): every field, each once.
                    let mut named: Vec<&String> = fields.iter().collect();
                    named.sort();
                    named.dedup();
                    if named.len() != fields.len() || fields.len() != sdecl.fields.len() {
                        return Err("diag.type-mismatch".to_string());
                    }
                    // The struct's own type arguments, from the value.
                    let targs: Vec<Type> = match &st {
                        Some(Type::Named(_, a)) => a.clone(),
                        _ => Vec::new(),
                    };
                    let tsub: HashMap<String, Type> = sdecl.type_params.iter().cloned().zip(targs).collect();
                    for field in fields {
                        let Some(fd) = sdecl.fields.iter().find(|f| &f.name == field) else {
                            return Err("diag.type-mismatch".to_string());
                        };
                        if !self.privileged && !field_visible(struct_name, fd.export, &self.module) {
                            return Err("diag.name-not-visible".to_string());
                        }
                        self.gamma.insert(field.clone(), apply_subst(&apply_subst(&fd.ty, &tsub), &self.subst));
                    }
                }
                for field in fields {
                    self.declare(field, true);
                }
                Ok(None)
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => self.check_expr(e, None),
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
        match self.check_expr_inner(e, expected) {
            Err(d) if !d.contains('@') => Err(format!("{d}@{}", e.line)),
            other => other,
        }
    }

    // An operand whose type the rule fixes: an `if`/`while` condition
    // (`[T-If]`, `[T-While]`: `bool`), an index or a slice bound
    // (`[Index-Checked]`, `[Slice-Form]`: `usize`).
    fn check_expect(&mut self, e: &Expr, want: &Type) -> Result<(), String> {
        let t = self.check_expr(e, Some(want))?;
        match t {
            Some(t) if !has_unresolved_marker(&t) && !ty_compat(&t, want) => Err(format!("diag.type-mismatch@{}", e.line)),
            _ => Ok(()),
        }
    }

    fn check_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> TResult {
        let in_place = std::mem::replace(&mut self.place_pos, false);
        match &e.kind {
            ExprKind::IntLit(v, suffix) => self.check_int_lit(*v, suffix.as_deref(), expected),
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
                    return Err("diag.literal-out-of-range".to_string());
                }
                Ok(Some(t))
            }
            // `[T-Lit-Str]`: always `str`; takes no expected type.
            ExprKind::StrLit(_) => Ok(Some(Type::Str)),
            ExprKind::BoolLit(_) => Ok(Some(Type::Bool)),
            ExprKind::Unit => Ok(Some(Type::Void)),
            ExprKind::Paren(inner) => self.check_expr(inner, expected),
            ExprKind::Path(segs, _targs) => {
                let t = self.check_path(segs)?;
                if !in_place {
                    self.flow_read(e)?;
                }
                Ok(t)
            }
            ExprKind::Unary(op, inner) => self.check_unary(*op, inner, expected),
            ExprKind::Binary(op, a, b) => self.check_binary(*op, a, b, e, expected),
            ExprKind::Borrow(m, inner) => {
                // `[Ref-Form-Not-Place]` / `[Ref-Form-Temporary]` (spec/09 §2).
                match borrow_operand_kind(inner, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..)))) {
                    OperandKind::NonPlace => return Err("diag.borrow-of-non-place".to_string()),
                    OperandKind::Temporary => return Err("diag.borrow-of-temporary".to_string()),
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
                if *m == Mode::Exclusive {
                    return Err("diag.type-mismatch".to_string());
                }
                if on_string {
                    match borrow_operand_kind(base, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..)))) {
                        OperandKind::NonPlace => return Err("diag.borrow-of-non-place".to_string()),
                        OperandKind::Temporary => return Err("diag.borrow-of-temporary".to_string()),
                        OperandKind::Place => {}
                    }
                    self.place_pos = true;
                    self.check_expr(base, None)?;
                    self.flow_borrow(base, m)?;
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
                // `[Slice-Form]` (D-0047): `&base[lo .. hi]` borrows `base`
                // as `&base` would, and is a slice of its elements.
                match borrow_operand_kind(base, &|b| matches!(self.static_place_type(b), Some(Type::Ref(..)))) {
                    OperandKind::NonPlace => return Err("diag.borrow-of-non-place".to_string()),
                    OperandKind::Temporary => return Err("diag.borrow-of-temporary".to_string()),
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
                self.flow_borrow(base, m)?;
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
                            self.refute("diag.use-of-uninitialized")?;
                        }
                    }
                }
                let it = self.check_expr(inner, None)?;
                if matches!(&it, Some(t) if is_marker(t)) {
                    return Err("diag.unbounded-type-parameter".to_string());
                }
                if matches!(it, Some(Type::Rawptr(_))) && self.unsafe_depth == 0 {
                    return Err("diag.trusted-outside-unsafe".to_string());
                }
                Ok(match it {
                    Some(Type::Ref(t, _)) => Some(*t),
                    Some(Type::Guard(t)) => Some(*t),
                    Some(Type::Rawptr(t)) => Some(*t),
                    _ => None,
                })
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
                self.place_pos = true;
                let bt = self.check_expr(base, None)?;
                if matches!(&bt, Some(t) if is_marker(t)) {
                    return Err("diag.unbounded-type-parameter".to_string());
                }
                if matches!(bt, Some(Type::Ref(..))) {
                    // `[Field-Access-Auto-Deref]`'s index analogue: the
                    // reference itself is read.
                    self.flow_read(base)?;
                }
                self.check_expect(idx, &Type::Int(IntTy::Usize))?;
                let arr = match &bt {
                    Some(Type::Array(t, n)) => Some(((**t).clone(), *n)),
                    Some(Type::Ref(inner, _)) => match &**inner {
                        Type::Array(t, n) => Some(((**t).clone(), *n)),
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
                            return Err("diag.move-out-of-field".to_string());
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
                if let (Some((_, n)), Some(v)) = (&arr, literal_value(idx)) {
                    if v < 0 || (v as u128) >= *n {
                        return Err("diag.index-out-of-bounds".to_string());
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
                let elem_expected = match expected {
                    Some(Type::Array(t, _)) => Some((**t).clone()),
                    _ => None,
                };
                let mut unknown = false;
                for it in items {
                    let t = self.check_store_operand(it, elem_expected.as_ref().or(ety.as_ref()))?;
                    if t.is_none() {
                        unknown = true;
                    }
                    if ety.is_none() {
                        ety = t;
                    }
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
            ExprKind::Assign(lhs, rhs) => {
                // `[Write-Not-Exclusive]` (spec/08): a write through a
                // shared reference, direct (`*r = ..`) or by auto-deref
                // (`r.f = ..`).
                if self.crosses_shared(lhs) {
                    return Err("diag.write-through-shared".to_string());
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
                    return Err("diag.type-mismatch".to_string());
                }
                // D-0033 (1): `x = e` for a whole binding `x` whose value
                // was moved away or destroyed gives it a new one, as the
                // first write to a `[Let-Uninit]` binding does.
                let reinit = match &lhs.kind {
                    ExprKind::Path(segs, _) if segs.len() == 1 && self.is_local(&segs[0]) => Some(segs[0].clone()),
                    _ => None,
                };
                self.place_pos = true;
                self.reinit_target = reinit.clone();
                let lt = self.check_expr(lhs, None);
                self.reinit_target = None;
                let lt = lt?;
                let rt = self.check_store_operand(rhs, lt.as_ref())?;
                if let (Some(l), Some(r)) = (&lt, &rt) {
                    if !has_unresolved_marker(l) && !has_unresolved_marker(r) && !ty_compat(l, r) {
                        return Err("diag.type-mismatch".to_string());
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
                        self.set_deriv(&name, fact);
                        let origin = self.referent_of(rhs);
                        self.ref_origin.insert(name, origin);
                    }
                }
                Ok(Some(Type::Void))
            }
            ExprKind::Propagate(inner) => {
                let it = self.check_expr(inner, None)?;
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
                let (tt, ft, after_t) = if else_first {
                    let ft = self.check_expr(f.as_ref().unwrap(), None)?;
                    let after_f = std::mem::replace(&mut self.flow, before);
                    let hint = ft.clone().filter(|x| *x != Type::Never && !has_unresolved_marker(x));
                    if let Some(h) = &hint {
                        self.items.match_hints.lock().unwrap().insert(t as *const Block as usize, h.clone());
                    }
                    let tt = self.check_nested_block(t, hint.as_ref())?;
                    let after_t = std::mem::replace(&mut self.flow, after_f);
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
                    (tt, ft, after_t)
                };
                self.flow = after_t.join(&self.flow);
                match (tt, ft) {
                    (Some(a), Some(b)) if !has_unresolved_marker(&a) && !has_unresolved_marker(&b) => {
                        if f.is_none() {
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
                let rt = self.ret.clone();
                match v {
                    Some(e) => {
                        self.check_escape_to_caller(e)?;
                        self.check_store_operand(e, Some(&rt))?;
                    }
                    None => {}
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
            ExprKind::Closure { is_move, captures, params, body } => self.check_closure(*is_move, captures, params, body),
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
            for st in &ctx.breaks {
                exit = exit.join(st);
            }
        }
        self.flow = exit;
        Ok(Some(Type::Void))
    }

    fn check_int_lit(&self, v: u128, suffix: Option<&str>, expected: Option<&Type>) -> TResult {
        let it = if let Some(s) = suffix {
            crate::ast::IntTy::from_str(s)
        } else {
            match expected {
                Some(Type::Int(t)) => Some(*t),
                _ => None,
            }
        };
        let it = it.unwrap_or(IntTy::I32);
        let (lo, hi) = int_range(it);
        if v > hi as u128 && !(lo < 0 && (v as i128) >= lo) {
            // value doesn't fit as unsigned magnitude; refine below
        }
        if !int_lit_fits(it, v) {
            return Err("diag.literal-out-of-range".to_string());
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
                let it = if let Some(s) = suf.as_deref() {
                    crate::ast::IntTy::from_str(s)
                } else {
                    match expected {
                        Some(Type::Int(t)) => Some(*t),
                        _ => None,
                    }
                }
                .unwrap_or(IntTy::I32);
                // `[T-Neg]`: unary minus needs a signed or float operand.
                if !it.signed() {
                    return Err("diag.type-mismatch".to_string());
                }
                let bw = it.bitwidth(crate::value::ADDR_WIDTH);
                let neg: i128 = if bw >= 128 {
                    if *v == (1u128 << 127) {
                        i128::MIN
                    } else if *v < (1u128 << 127) {
                        -(*v as i128)
                    } else {
                        return Err("diag.literal-out-of-range".to_string());
                    }
                } else {
                    -(*v as i128)
                };
                let (lo, hi) = int_range(it);
                if neg < lo || neg > hi {
                    return Err("diag.literal-out-of-range".to_string());
                }
                return Ok(Some(Type::Int(it)));
            }
        }
        let it = self.check_expr(inner, expected)?;
        if matches!(&it, Some(t) if is_marker(t)) {
            return Err("diag.unbounded-type-parameter".to_string());
        }
        match (op, &it) {
            (UnOp::Neg, Some(Type::Int(t))) if !t.signed() => Err("diag.type-mismatch".to_string()),
            (UnOp::Not, Some(Type::Bool)) => Ok(Some(Type::Bool)),
            (UnOp::Not, Some(t)) if !matches!(t, Type::Bool) && !has_unresolved_marker(t) => {
                Err("diag.type-mismatch".to_string())
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

    fn check_binary(&mut self, op: BinOp, a: &Expr, b: &Expr, whole: &Expr, expected: Option<&Type>) -> TResult {
        use BinOp::*;
        // D-0037: an operator on literal expressions only takes the type
        // its context expects, when that is a number.
        let numeric = expected.filter(|t| matches!(t, Type::Int(_) | Type::F32 | Type::F64));
        let literal_op = matches!(op, Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor | Shl | Shr) && is_literal_expr(whole);
        // A bare unsuffixed literal on the left takes its type from the
        // right operand (`rule.type.expected`: "the other operand's
        // determined type"); check the right side first in that case.
        let (at, bt) = if let (Some(t), true) = (numeric, literal_op) {
            let at = self.check_expr(a, Some(t))?;
            let bt = if matches!(op, Shl | Shr) { self.check_expr(b, at.as_ref())? } else { self.check_expr(b, Some(t))? };
            (at, bt)
        } else if self.takes_expected(a) && !self.takes_expected(b) {
            let bt = self.check_expr(b, None)?;
            let at = self.check_expr(a, bt.as_ref())?;
            (at, bt)
        } else {
            let at = self.check_expr(a, None)?;
            let bt = self.check_expr(b, at.as_ref())?;
            (at, bt)
        };
        let at = if at.is_none() { self.reguess(a, bt.as_ref()) } else { at };
        if matches!(&at, Some(t) if is_marker(t)) || matches!(&bt, Some(t) if is_marker(t)) {
            // `rule.type.kind`: no arithmetic, comparison or logic on a
            // bare type parameter.
            return Err("diag.unbounded-type-parameter".to_string());
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
        // D-0053: `==`/`!=` between a `StringView` and a `StringView` or a
        // `str` compares their text (`StringView::eq`, `StringView::eq_str`);
        // nothing else applies to a view.
        let is_view = |t: &Option<Type>| matches!(t, Some(Type::Named(n, _)) if n == "std::StringView");
        if is_view(&at) || is_view(&bt) {
            let view_left = is_view(&at);
            let other = if view_left { &bt } else { &at };
            let other_is_str = matches!(other, Some(Type::Str));
            if !matches!(op, Eq | Ne) || !(other_is_str || is_view(other)) {
                return Err("diag.type-mismatch".to_string());
            }
            self.items.view_ops.lock().unwrap().insert(whole as *const Expr as usize, crate::interp::ViewOp::Eq { view_left, other_is_str, negate: op == Ne });
            return Ok(Some(Type::Bool));
        }
        // `[Eq-Str]` is the only operator defined on `str` (spec/12 §4):
        // no ordering, arithmetic or logic.
        if (matches!(&at, Some(Type::Str)) || matches!(&bt, Some(Type::Str))) && !matches!(op, Eq | Ne) {
            return Err("diag.type-mismatch".to_string());
        }
        match op {
            Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor => {
                if let (Some(ta), Some(tb)) = (&at, &bt) {
                    if !has_unresolved_marker(ta) && !has_unresolved_marker(tb) {
                        if !ty_compat(ta, tb) {
                            return Err("diag.type-mismatch".to_string());
                        }
                        if let Type::Int(it) = ta {
                            self.check_literal_fold_overflow(op, *it, a, b)?;
                        }
                    }
                }
                Ok(at.or(bt))
            }
            Shl | Shr => {
                // `[Shift-Amount-Invalid]`, `discharge: static where constant`
                // (spec/14 §6's value-range row): a literal amount at or
                // beyond the operand type's bit width is rejected here.
                if let (Some(Type::Int(it)), Some(n)) = (&at, literal_value(b)) {
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
                if let (Some(ta), Some(tb)) = (&at, &bt) {
                    if !has_unresolved_marker(ta) && !has_unresolved_marker(tb) && !ty_compat(ta, tb) {
                        return Err("diag.type-mismatch".to_string());
                    }
                }
                let _ = whole;
                Ok(Some(Type::Bool))
            }
            And | Or => Ok(Some(Type::Bool)),
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
    fn check_literal_fold_overflow(&self, op: BinOp, it: IntTy, a: &Expr, b: &Expr) -> Result<(), String> {
        let av = literal_value(a);
        let bv = literal_value(b);
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
        r.map_err(|e| match e {
            ArithError::DivOverflow => "diag.div-overflow".to_string(),
            _ => "diag.arith-overflow".to_string(),
        })
    }

    fn check_field(&mut self, base: &Expr, name: &str) -> TResult {
        let bt = self.check_expr(base, None)?;
        let bt = match bt {
            Some(Type::Ref(inner, _)) => Some(*inner),
            other => other,
        };
        if matches!(&bt, Some(t) if is_marker(t)) {
            return Err("diag.unbounded-type-parameter".to_string());
        }
        match bt {
            Some(Type::Named(sname, args)) => {
                if let Some(sdecl) = self.items.structs.get(&sname).cloned() {
                    let subst: HashMap<String, Type> = sdecl.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    match sdecl.fields.iter().find(|f| f.name == name) {
                        Some(fd) if !self.privileged && !field_visible(&sname, fd.export, &self.module) => Err("diag.name-not-visible".to_string()),
                        Some(fd) => Ok(Some(apply_subst(&fd.ty, &subst))),
                        None => Err("diag.type-mismatch".to_string()),
                    }
                } else {
                    Ok(None) // closure capture struct or otherwise opaque; not modeled statically
                }
            }
            _ => Ok(None),
        }
    }

    fn check_path(&mut self, segs: &[String]) -> TResult {
        if segs.len() == 1 {
            // `[Binding-Lookup]`'s `temporally-valid(a_x)`: refuted when
            // `valid(x) = F` (moved from, dropped) -- in every position,
            // since every use of the name goes through the lookup.
            if self.flow.valid.get(&segs[0]) == Some(&Tri::F) && self.reinit_target.as_deref() != Some(segs[0].as_str()) {
                self.refute("diag.stale-binding")?;
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
        if let Some(enum_name) = self.variant_enum(segs) {
            // `[T-Enum]`: a bare `Vi` is `Vi(())`, so a variant with a
            // payload written without one has a payload of the wrong type.
            if self.variant_arity(&enum_name, segs) == Some(true) && !self.items.fns.contains_key(&segs.join("::")) {
                return Err("diag.type-mismatch".to_string());
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
            return Err("diag.unbound-name".to_string());
        }
        if segs.len() > 1 && !self.items.fns.contains_key(&name) && !self.items.externs.contains_key(&name) && !INTRINSIC_NAMES.contains(&name.as_str()) {
            // A qualified path can never be a local: it is an item, an
            // `Enum::Variant`, or `[Resolve-Unbound]`.
            let (prefix, last) = (segs[..segs.len() - 1].join("::"), &segs[segs.len() - 1]);
            let is_variant = self.items.enums.get(&prefix).map_or(false, |e| e.variants.iter().any(|v| &v.name == last));
            if !is_variant {
                return Err("diag.unbound-name".to_string());
            }
        }
        if let Some(f) = self.items.fns.get(&name) {
            // `[T-Item]`: a non-generic function item is a value of its
            // `fn(...) : τ` type.
            if f.type_params.is_empty() {
                return Ok(Some(Type::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()))));
            }
            return Ok(None);
        }
        if let Some(ext) = self.items.externs.get(&name) {
            return Ok(Some(Type::Fn(ext.params.iter().map(|p| p.ty.clone()).collect(), Box::new(ext.ret.clone()))));
        }
        Ok(None)
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr], expected: Option<&Type>) -> TResult {
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
                                let mut m: HashMap<String, Type> = HashMap::new();
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
                                } else {
                                    self.check_store_operand(&args[0], Some(&hint))?
                                };
                                if let Some(at) = &at {
                                    unify_type_shape(raw_payload, at, &mut m);
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
            const TRUSTED_INTRINSICS: &[&str] = &["reclaim", "release", "copy_raw", "deallocate", "text_write", "parse_check", "parse_value"];
            if TRUSTED_INTRINSICS.contains(&name.as_str()) && self.unsafe_depth == 0 {
                return Err("diag.trusted-outside-unsafe".to_string());
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
                // `fault(d)`, prelude-internal (spec/21 §0): `d` is one of a
                // fixed set of diagnostic names, not an expression.
                return Ok(Some(Type::Never));
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
                let x = crate::each::expand(&name, &t0, args, callee.line)?;
                let saved = self.privileged;
                self.privileged = true;
                let r = self.check_expr(&x, expected);
                self.privileged = saved;
                return r;
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
            if name == "$fmt" {
                // `rule.stdlib.format` (spec/21 §2f, D-0038): `printf` and
                // `String::appendf` after `modres`. The format is a string
                // literal (`[Format-Invalid]` otherwise, or when it does not
                // parse), with one argument of its kind per specifier
                // (`[Format-Arg-Mismatch]`).
                let Some(ExprKind::StrLit(f)) = args.first().map(|a| &a.kind) else {
                    return Err("diag.format-invalid".to_string());
                };
                let pieces = crate::fmt::parse(f.as_bytes()).map_err(|_| "diag.format-invalid".to_string())?;
                let specs: Vec<crate::fmt::Kind> = pieces
                    .iter()
                    .filter_map(|p| match p {
                        crate::fmt::Piece::Spec(s) => Some(s.kind()),
                        _ => None,
                    })
                    .collect();
                if specs.len() != args.len() - 1 {
                    return Err("diag.type-mismatch".to_string());
                }
                for (k, a) in specs.iter().zip(&args[1..]) {
                    let t = self.check_store_operand(a, None)?;
                    // A reference is formatted as the value it refers to
                    // (D-0042): `&i32` as an `i32`, `&mut String` as `&String`.
                    let t = match t {
                        Some(Type::Ref(inner, _)) if matches!(&*inner, Type::Named(n, _) if n == "std::String") => Some(Type::Ref(inner, Mode::Shared)),
                        Some(Type::Ref(inner, _)) if matches!(&*inner, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str) => Some(*inner),
                        other => other,
                    };
                    let ok = match &t {
                        None => true,
                        Some(t) if has_unresolved_marker(t) => true,
                        Some(Type::Never) => true,
                        // `%v`: any printable value (`rule.stdlib.print`).
                        Some(t) if *k == crate::fmt::Kind::Value => is_printable(t),
                        Some(Type::Int(_)) => *k == crate::fmt::Kind::Int,
                        Some(Type::F32 | Type::F64) => *k == crate::fmt::Kind::Float,
                        Some(Type::Str | Type::Bool) => *k == crate::fmt::Kind::Text,
                        Some(Type::Named(n, _)) if n == "std::StringView" => *k == crate::fmt::Kind::Text, // D-0053
                        Some(Type::Ref(inner, Mode::Shared)) => *k == crate::fmt::Kind::Text && matches!(&**inner, Type::Named(n, _) if n == "std::String"),
                        Some(_) => false,
                    };
                    if !ok {
                        return Err("diag.type-mismatch".to_string());
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
                if args.is_empty() || args.len() > 2 {
                    return Err("diag.type-mismatch".to_string());
                }
                self.check_expect(&args[0], &Type::Bool)?;
                if !const_expr(self.items, &args[0]) {
                    return Err("diag.const-not-constant".to_string());
                }
                let msg = match args.get(1).map(|m| &m.kind) {
                    None => None,
                    Some(ExprKind::StrLit(m)) => Some(m.clone()),
                    Some(_) => return Err("diag.type-mismatch".to_string()),
                };
                // A generic body checked with its type parameters opaque
                // cannot compute it; each concrete instantiation does.
                if !self.subst.values().any(has_unresolved_marker) && self.report {
                    let mut sv: Vec<(&String, &Type)> = self.subst.iter().collect();
                    sv.sort_by(|a, b| a.0.cmp(b.0));
                    let key = (&args[0] as *const Expr as usize, format!("{:?}", sv));
                    self.static_asserts.push(StaticAssert { key, cond: args[0].clone(), subst: self.subst.clone(), line: args[0].line, message: msg });
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
                let hint = if alt && is_bare_num_literal(&args[i]) {
                    by_index[1 - i].clone().filter(|t| matches!(t, Type::Int(_)))
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
            self.check_integer_intrinsic_operands(&name, targs, args, &arg_tys)?;
            if alt {
                // `[T-Alt]`'s result: the operands' type τ, or `Option<τ>`
                // for the `checked_` family, so that a literal beside a
                // nested call (`wrapping_add(wrapping_mul(x, a), b)`) is
                // typed by it.
                let t = arg_tys.iter().flatten().find(|t| matches!(t, Type::Int(_))).cloned();
                return Ok(t.map(|t| if name.starts_with("checked_") { Type::Named("std::Option".to_string(), vec![t]) } else { t }));
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
        let ct = self.check_expr(callee, None)?;
        if matches!(&ct, Some(t) if is_marker(t)) {
            return Err("diag.unbounded-type-parameter".to_string());
        }
        for a in args {
            self.check_store_operand(a, None)?;
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
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                // `[T-Convert]`'s kind premises (D-0051 adds floats to
                // `widen`, `reinterpret` and `to_float`'s operand).
                let num = |t: &Type| is_int(t) || is_float(t);
                let (src_ok, dst_ok): (&dyn Fn(&Type) -> bool, &dyn Fn(&Type) -> bool) = match name {
                    "widen" | "reinterpret" => (&num, &num),
                    "to_float" => (&num, &is_float),
                    "to_int" => (&is_float, &is_int),
                    _ => (&is_int, &is_int),
                };
                if let Some(Some(a)) = arg_tys.first() {
                    if !src_ok(a) {
                        return mismatch();
                    }
                }
                if let Some(t) = targs.first().map(|t| apply_subst(t, &self.subst)) {
                    if !dst_ok(&t) {
                        return mismatch();
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
                            return mismatch();
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_known_intrinsic(&self, name: &str, targs: &[Type], _args: &[Expr]) -> Option<Type> {
        match name {
            "sizeof" | "alignof" => Some(Type::Int(IntTy::Usize)),
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                targs.first().map(|t| apply_subst(t, &self.subst))
            }
            "rawptr_of" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "reinterpret_ptr" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "dangling" => targs.first().map(|t| Type::Rawptr(Box::new(apply_subst(t, &self.subst)))),
            "drop" | "release" | "copy_raw" | "deallocate" => Some(Type::Void),
            "str_len" | "slice_len" => Some(Type::Int(IntTy::Usize)),
            "str_byte" => Some(Type::Int(IntTy::U8)),
            "text_write" => Some(Type::Int(IntTy::Usize)),
            "parse_check" => Some(Type::Int(IntTy::Isize)),
            "parse_value" => targs.first().map(|t| apply_subst(t, &self.subst)),
            "append_native" => Some(Type::Void),
            "key_hash" => Some(Type::Int(IntTy::U64)),
            "swap_places" => Some(Type::Void),
            "key_eq" => Some(Type::Bool),
            "str_ptr" => Some(Type::Rawptr(Box::new(Type::Int(IntTy::U8)))),
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" => {
                None // same type as its arguments; left unresolved rather than re-deriving arg types here
            }
            _ => None,
        }
    }

    fn check_user_call(&mut self, f: &std::sync::Arc<FnDecl>, targs: &[Type], args: &[Expr], expected: Option<&Type>) -> TResult {
        // `[T-Call]`: one argument per parameter.
        if args.len() != f.params.len() {
            return Err("diag.type-mismatch".to_string());
        }
        let explicit: Vec<Type> = targs.iter().map(|t| apply_subst(t, &self.subst)).collect();
        let type_param_set: HashSet<String> = f.type_params.iter().cloned().collect();
        let mut arg_types: Vec<Option<Type>> = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
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
            for (j, t) in arg_types.iter().enumerate() {
                if let (Some(p), Some(t)) = (f.params.get(j), t) {
                    if !has_unresolved_marker(t) {
                        unify_type_shape(&p.ty, t, &mut known);
                    }
                }
            }
            let raw_hint = f.params.get(i).map(|p| apply_subst(&p.ty, &known));
            let poisoned = raw_hint.as_ref().map_or(true, |t| mentions_any(t, &type_param_set));
            let hint = raw_hint.filter(|_| !poisoned);
            let saved = self.infer_poisoned;
            if poisoned {
                self.infer_poisoned = true;
            }
            let r = self.check_store_operand(a, hint.as_ref());
            self.infer_poisoned = saved;
            arg_types.push(r?);
        }
        if f.type_params.is_empty() {
            // D-0006 (no implicit conversion between distinct nominal
            // types): every parameter is already concrete here, so every
            // argument's own type must exactly match it -- [T-Call]'s
            // Γ ⊢ ei : τi, never actually enforced before (an i32
            // binding passed where i64 was declared silently "worked").
            self.check_arg_exact_types(&f.params, args, &arg_types, &HashMap::new(), &type_param_set)?;
            // Non-generic, but its body still needs to be reached by the
            // worklist to be checked at all -- `record_instantiation`
            // with zero type args does exactly that (and `checked`
            // memoizes so a frequently-called function is only checked
            // once).
            self.record_instantiation(&f_key(f), &[]);
            return Ok(Some(apply_subst(&f.ret, &HashMap::new())));
        }
        let mut m: HashMap<String, Type> = HashMap::new();
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
                    unify_type_shape(&p.ty, at, &mut m);
                }
            }
        }
        if !all_fixed(&m) {
            if let Some(exp) = expected {
                unify_type_shape(&f.ret, exp, &mut m);
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
                let bad = if is_std("std::String::append") {
                    !is_printable(t)
                } else if is_std("std::parse") {
                    !matches!(t, Type::Int(_) | Type::F32 | Type::F64)
                } else if f.assoc_type.as_deref().map_or(false, |a| (a == "HashMap" || a == "HashSet") && is_std(&format!("std::{}::{}", a, f.name))) {
                    // `[Key-Not-Hashable]` (spec/21 §1a): `K` at every call.
                    !is_key_type(t)
                } else {
                    false
                };
                if bad {
                    return Err("diag.type-mismatch".to_string());
                }
            }
            self.record_instantiation(&f_key(f), &targs_c);
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
            Err("diag.cannot-infer-type-parameter".to_string())
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
            if is_bare_int_literal(&args[i]) {
                continue;
            }
            if let Some(Some(at)) = arg_types.get(i) {
                if pty != *at && !weakens_to(at, &pty) {
                    return Err("diag.type-mismatch".to_string());
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
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in sdecl.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), apply_subst(t, &self.subst));
        }
        // Every type parameter fixed? Not `m.len()`: a field such as
        // `String name` enters `String` too (`unify_type_shape`).
        let all_fixed = |m: &HashMap<String, Type>| sdecl.type_params.iter().all(|p| m.contains_key(p));
        if !all_fixed(&m) {
            if let Some(exp) = expected {
                let generic_self = Type::Named(name.clone(), sdecl.type_params.iter().map(|p| Type::Named(p.clone(), vec![])).collect());
                unify_type_shape(&generic_self, exp, &mut m);
            }
        }
        let mut seen = HashSet::new();
        for (fname, fexpr) in fields {
            seen.insert(fname.clone());
            let idx = sdecl.fields.iter().position(|f| &f.name == fname);
            let idx = match idx {
                Some(i) => i,
                None => return Err("diag.type-mismatch".to_string()),
            };
            if !field_visible(&name, sdecl.fields[idx].export, &self.module) {
                return Err("diag.name-not-visible".to_string());
            }
            let remaining: HashSet<String> = sdecl.type_params.iter().filter(|p| !m.contains_key(*p)).cloned().collect();
            let hint = apply_subst(&sdecl.fields[idx].ty, &m);
            let at = if mentions_any(&hint, &remaining) {
                let saved = self.infer_poisoned;
                self.infer_poisoned = true;
                let r = self.check_store_operand(fexpr, None);
                self.infer_poisoned = saved;
                r?
            } else {
                self.check_store_operand(fexpr, Some(&hint))?
            };
            if let Some(at) = &at {
                if !has_unresolved_marker(at) {
                    unify_type_shape(&sdecl.fields[idx].ty, at, &mut m);
                }
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
        // D-0049: a resource binding matched by value is consumed only by
        // an arm that moves its payload out (below), not by the match.
        let mut consume_x: Option<String> = None;
        if let Some((x, path)) = self.place_of(scrut) {
            if self.flow.init.get(&x) != Some(&Tri::T) {
                self.refute("diag.use-of-uninitialized")?;
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
            (None, Some(t @ (Type::Int(_) | Type::Bool))) => Some(t.clone()),
            _ => None,
        };
        // A reference to an integer or `bool` has no variants, and is not
        // itself a literal's type: `match (*r)` reads the value.
        if level0.is_none() && matches!(&st, Some(Type::Ref(inner, _)) if matches!(**inner, Type::Int(_) | Type::Bool)) && arms.iter().any(|a| a.variant.is_some() || a.lit.is_some()) {
            return Err("diag.type-mismatch".to_string());
        }
        let mut binder_ty: Vec<Option<Type>> = Vec::new();
        for arm in arms {
            let Some(t0) = level0.clone() else {
                // A scrutinee whose type this pass does not know.
                binder_ty.push(None);
                continue;
            };
            let mut cur: Option<Type> = Some(t0);
            for vn in arm.chain() {
                let Some(level) = cur.clone().filter(|c| self.is_enum_ty(c)) else {
                    return Err("diag.type-mismatch".to_string());
                };
                match self.variant_payload(&level, &vn) {
                    Some(p) => cur = p,
                    None => return Err("diag.type-mismatch".to_string()),
                }
            }
            if let Some(l) = &arm.lit {
                let Some(lt) = cur.clone().filter(|c| matches!(c, Type::Int(_) | Type::Bool)) else {
                    return Err("diag.type-mismatch".to_string());
                };
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
        let through_ref_or_projection = by_ref.is_none() && matches!(&scrut.kind, ExprKind::Deref(_) | ExprKind::Field(..) | ExprKind::Index(..));
        if through_ref_or_projection {
            for bt in binder_ty.iter().flatten() {
                if is_resource_ty(self.items, bt) {
                    return Err("diag.move-out-of-field".to_string());
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
                    // Located at the arm (its body's line).
                    return Err(format!("diag.unreachable-arm@{}", arms[k].body.line));
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
            if let (Some(bn), Some(pt)) = (&arm.binder, binder_ty[arm_ix[ai]].clone()) {
                if let (Some(x), None) = (&consume_x, &by_ref) {
                    if is_resource_ty(self.items, &pt) {
                        self.flow_consume(x, "diag.destroy-while-aliased")?;
                    }
                }
                let bt = match &by_ref {
                    Some(m) => Type::Ref(Box::new(pt), m.clone()),
                    None => pt,
                };
                self.gamma.insert(bn.clone(), bt);
            }
            if let Some(bn) = &arm.binder {
                self.declare(bn, true);
            }
            let at = self.check_store_operand(&arm.body, arm_expected.as_ref());
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
                        Some(_) => return Err("diag.type-mismatch".to_string()),
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
            let level = t.filter(|c| self.is_enum_ty(c))?;
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
    fn check_closure(&mut self, is_move: bool, captures: &[String], params: &[Param], body: &Block) -> TResult {
        // Free variables computed against only the *params* as bound --
        // the captures themselves must NOT be pre-excluded, since the
        // declared capture list is supposed to equal exactly this set
        // (bounding by the captures too would trivially make `free`
        // empty for any correctly-written closure and falsely flag
        // every one of them as a mismatch).
        let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut free: HashSet<String> = HashSet::new();
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
            if !self.flow.is_tracked(c) {
                continue;
            }
            if self.flow.valid.get(c) == Some(&Tri::F) {
                self.refute("diag.stale-binding")?;
            }
            if is_move {
                if self.place_is_resource(c, &[]) {
                    self.flow_consume(c, "diag.move-while-aliased")?;
                } else {
                    if self.flow.init.get(c) != Some(&Tri::T) {
                        self.refute("diag.use-of-uninitialized")?;
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
        // is ever `T` or `F` for it). This pass does not track the outer
        // binding's type into the closure either.
        let outer_capture_types: HashMap<String, Option<Type>> =
            captures.iter().map(|c| (c.clone(), self.lookup_local(c))).collect();
        let saved_gamma = std::mem::take(&mut self.gamma);
        let saved_flow = std::mem::take(&mut self.flow);
        let saved_move_captures = std::mem::replace(&mut self.move_captures, if is_move { outer_capture_types } else { HashMap::new() });
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
        let r = self.check_nested_block(body, None);
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
        r?;
        Ok(None)
    }
}

// `rule.arith.alt`'s operations, whose two operands share one type.
const ALT_INTRINSICS: &[&str] = &[
    "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub",
    "checked_mul", "checked_div", "checked_rem",
];

pub(crate) const INTRINSIC_NAMES: &[&str] = &[
    "drop", "sizeof", "alignof", "dangling", "rawptr_of", "reclaim", "release", "copy_raw", "allocate", "deallocate",
    "lock", "spawn", "join", "std::map_err", "std::print", "reinterpret_ptr", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float",
    "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul",
    "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem", "fault", "write",
    "str_len", "str_byte", "str_ptr", "slice_len", "min_value", "max_value",
    "append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "swap_places", "$fmt",
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
// `String::append<T>` and `parse<T>` (`rule.stdlib.text`).
const STD_ONLY_INTRINSICS: &[&str] = &["append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "swap_places"];

// `rule.stdlib.hashmap`'s key types (spec/21 §1a, D-0041).
fn is_key_type(t: &Type) -> bool {
    match t {
        Type::Int(_) | Type::Bool | Type::Str | Type::Never => true,
        Type::Named(n, _) => n == "std::String",
        _ => false,
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

    fn variant_enum(&self, segs: &[String]) -> Option<String> {
        let last = segs.last()?;
        if segs.len() >= 2 {
            let key = segs[..segs.len() - 1].join("::");
            if let Some(e) = self.items.enums.get(&key) {
                return if e.variants.iter().any(|v| &v.name == last) { Some(key) } else { None };
            }
        }
        self.items.enum_of_variant.get(last).cloned()
    }

    fn is_item_name(&self, n: &str) -> bool {
        self.items.fns.contains_key(n)
            || self.items.externs.contains_key(n)
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
fn is_bare_num_literal(e: &Expr) -> bool {
    match &e.kind {
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

fn literal_value(e: &Expr) -> Option<i128> {
    match &e.kind {
        ExprKind::IntLit(v, _) => Some(*v as i128),
        ExprKind::Paren(inner) => literal_value(inner),
        ExprKind::Unary(UnOp::Neg, inner) => literal_value(inner).map(|v| -v),
        _ => None,
    }
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
            for a in args {
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
// expressions, the name of a function, and the pure intrinsics below.
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
        ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, x)| const_expr(items, x)),
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
        _ => String::new(),
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
