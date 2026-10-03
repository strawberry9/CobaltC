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
    // The generic bodies (type parameters opaque) are pushed last, so
    // they are checked first: what they record (D-0090's literals) is in
    // place before any instantiation of them is checked.
    let mut generic = Vec::new();
    for key in keys {
        let f = &items.fns[key];
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
}

impl<'a> Ck<'a> {
    fn run(&mut self) -> Result<(), String> {
        // Every function is checked, and of the errors found the one
        // earliest in the source is reported (one without a location,
        // last): the order the worklist reaches functions in is not one a
        // reader can see.
        let mut errors: Vec<String> = Vec::new();
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
                arg_temp_ok: false,
                cur_temp_ok: false,
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
            let bt = match body.check_fn_body(&f.body, &ret) {
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
                    errors.push(format!("{}@{}", mismatch(format!("`{}` returns `{}`, but its body gives `{}`", name, ret, bt)), body.current_line));
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
        match errors.into_iter().enumerate().min_by_key(|(i, d)| (crate::diagnostics::split_location(d).1.unwrap_or(usize::MAX), *i)) {
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
            let empty = HashMap::new();
            let fa = self.deriv.get(r).unwrap_or(&empty);
            let fb = other.deriv.get(r).unwrap_or(&empty);
            let mut keys: HashSet<&DerivKey> = fa.keys().collect();
            keys.extend(fb.keys());
            let mut merged = HashMap::new();
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
                if !matches!(self.place_type(&x, &p)?, Type::Array(..)) {
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
        let mut holders: Vec<(&String, &DerivKey)> = Vec::new();
        for (h, facts) in self.flow.deriv.iter() {
            for (k, v) in facts {
                if *v == Tri::T && k.root == x && paths_overlap(&k.path, path) && !permitted(mode.clone(), k.mode.clone()) {
                    holders.push((h, k));
                }
            }
        }
        // The first holder by name, so the message is the same every run.
        if let Some((h, k)) = holders.into_iter().min_by(|a, b| a.0.cmp(b.0)) {
            let need = match (diag, &mode) {
                ("diag.aliasing-conflict", Mode::Exclusive) => "so it cannot be written or borrowed `mut` here".to_string(),
                ("diag.aliasing-conflict", Mode::Shared) => "so it cannot be read here".to_string(),
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
                && self.place_type(&x, &[]).map_or(false, |t| always_owns(self.items, &t))
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
                    Some(Type::Array(_, n)) if (*k as u64) < n as u64 => {}
                    _ => return Ok(()),
                },
                PElem::Index(None) | PElem::Range(_) => return Ok(()),
            }
        }
        let Some(ty) = self.place_type(x, path) else { return Ok(()) };
        if !always_owns(self.items, &ty) {
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
        // Tagged with the operand's line, as `check_expr` tags its own: a
        // refutation raised here (a move out of a field) is not inside one.
        let t = match self.check_store_operand_inner(e, expected) {
            Err(d) if !d.contains('@') => return Err(format!("{d}@{}", e.line)),
            other => other?,
        };
        // D-0071: a binding that holds borrows without being a reference
        // (a spawn's handle), stored elsewhere, takes them out of sight.
        if let ExprKind::Path(segs, _) = &strip_parens_tc(e).kind {
            if segs.len() == 1 && !self.flow.unreachable && self.flow.deriv.contains_key(&segs[0]) && !matches!(self.lookup_local(&segs[0]), Some(Type::Ref(..) | Type::Slice(..))) {
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
            if self.place_is_resource(&x, &path) {
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
                            let fields: Vec<String> = sd.fields.iter().map(|f| f.name.clone()).collect();
                            let rep = std::sync::Arc::new(Expr {
                                kind: ExprKind::Block(Block {
                                    stmts: vec![Stmt::Destructure { struct_name: sname.clone(), names: fields.clone(), fields, rest: false, init: base.clone() }],
                                    tail: Some(Box::new(Expr { kind: ExprKind::Path(vec![fname.clone()], vec![]), line })),
                                }),
                                line,
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
                return Err(format!("{}@{}", crate::modres::named("diag.move-out-of-field", MOVE_OUT_HELP.to_string()), e.line));
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
            ExprKind::SliceOf(m, inner, lo, hi) => self.borrowed_place(inner).map(|(root, mut path)| {
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
    fn borrowed_place(&self, inner: &Expr) -> Option<(String, Vec<PElem>)> {
        self.place_of(inner).or_else(|| self.vec_element_root(inner)).or_else(|| self.deref_place_of(inner))
    }

    // D-0071: `spawn(f, a1, …)` -- the new thread's parameters hold every
    // visible borrow among its arguments until `join`, so a handle bound
    // to it carries their `deriv` facts.
    fn spawn_facts_of(&self, e: &Expr) -> Vec<DerivKey> {
        let ExprKind::Call(_, args) = &strip_parens_tc(e).kind else { return Vec::new() };
        args.iter().skip(1).filter_map(|a| self.deriv_facts_of(a)).collect()
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
        let ExprKind::Index(base, _) = &strip_parens_tc(e).kind else { return None };
        match self.static_place_type(base)? {
            Type::Named(n, _) if n == "std::Vec" => self.place_of(base),
            _ => None,
        }
    }

    // `auto r = e;` / `r = e;` for a reference-typed `r`: a visible
    // borrow (or elided call on one) sets that one `deriv` fact and
    // clears the others; anything else clears them all.
    fn set_deriv(&mut self, r: &str, facts: Vec<DerivKey>) {
        if self.flow.unreachable {
            return;
        }
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
                m != n && self.lookup_local(m).map_or(false, |t| may_hold_ref(self.items, &t, 0)) && later(m)
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
                        return Err(mismatch(format!("`{}` is declared `{}`, but its initializer is `{}`{}", name, exp, actual, conversion_hint(exp, actual, init.as_deref()))));
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
                    let facts: Vec<DerivKey> = match fact {
                        Some(f) => vec![f],
                        None => init.as_deref().map(|e| self.contained_ref_facts(e)).unwrap_or_default(),
                    };
                    self.set_deriv(name, facts);
                    let origin = init.as_ref().and_then(|e| self.referent_of(e));
                    self.ref_origin.insert(name.clone(), origin);
                } else if let Some(e) = init.as_deref().filter(|_| final_ty.as_ref().map_or(false, |t| holds_ref(self.items, t))) {
                    // D-0088: an `Option<ref<V>>` (or any value holding a
                    // reference) from a call borrows from its arguments.
                    let facts = self.contained_ref_facts(e);
                    self.set_deriv(name, facts);
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
                if let Some(sdecl) = self.items.structs.get(struct_name).cloned() {
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
        self.cur_temp_ok = std::mem::take(&mut self.arg_temp_ok);
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
            Some(t) if !has_unresolved_marker(&t) && !ty_compat(&t, want) => {
                // `if (x = 6)`: an assignment is `void`; `==` compares.
                let hint = if *want == Type::Bool && matches!(strip_parens_tc(e).kind, ExprKind::Assign(..)) { "; `=` assigns (its value is `void`) -- compare with `==`" } else { "" };
                Err(format!("{}@{}", mismatch(format!("expected `{}`, found `{}`{}", want, t, hint)), e.line))
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
            ExprKind::Paren(inner) => self.check_expr(inner, expected),
            ExprKind::Path(segs, targs) => {
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
                if let Some((x, mut path)) = self.place_of(base) {
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
                let it = self.check_expr(inner, None)?;
                if matches!(&it, Some(t) if is_marker(t)) {
                    return Err(unbounded_msg("`*` (dereference)"));
                }
                if matches!(it, Some(Type::Rawptr(_))) && self.unsafe_depth == 0 {
                    return Err("diag.trusted-outside-unsafe".to_string());
                }
                Ok(match it {
                    Some(Type::Ref(t, _)) => Some(*t),
                    Some(Type::Guard(t)) => Some(*t),
                    Some(Type::Rawptr(t)) => Some(*t),
                    // `*e` of a value that is no reference, guard or raw
                    // pointer (`*3`): `[Deref]`'s premise fails statically.
                    Some(t) if !has_unresolved_marker(&t) && matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Array(..) | Type::Void | Type::Named(..)) => {
                        return Err(mismatch(format!("`*` needs a reference, a guard or a raw pointer, but this is `{}`", t)));
                    }
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
                let elem_expected = match expected {
                    Some(Type::Array(t, _)) => Some((**t).clone()),
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
                    Some(Type::Array(t, _)) => Some((**t).clone()),
                    _ => None,
                };
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
                let rt = self.check_store_operand(rhs, lt.as_ref())?;
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
        let place = matches!(self.static_place_type(e), Some(Type::Named(n, _)) if n == "std::String")
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
        let literal_op = matches!(op, Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor | Shl | Shr) && is_literal_expr(whole);
        // A bare unsuffixed literal on the left takes its type from the
        // right operand (`rule.type.expected`: "the other operand's
        // determined type"); check the right side first in that case.
        let (at, bt) = if let (Some(t), true) = (numeric, literal_op) {
            let at = self.check_expr(a, Some(t))?;
            let bt = if matches!(op, Shl | Shr) { self.check_expr(b, at.as_ref())? } else { self.check_expr(b, Some(t))? };
            (at, bt)
        } else if (self.takes_expected(a) || (is_literal_expr(a) && !matches!(op, Shl | Shr))) && !self.takes_expected(b) && !is_literal_expr(b) {
            // (D-0037: a literal expression on the left, `((1 << 40) - 1) & k`,
            // takes the right operand's type as a bare literal does.)
            let bt = self.check_expr(b, None)?;
            let at = self.check_expr(a, bt.as_ref())?;
            (at, bt)
        } else if matches!(op, Eq | Ne | Lt | Le | Gt | Ge) && (is_string(a) || is_string(b)) {
            // D-0108 `[Cmp-Text]`: a `String` place is borrowed shared for
            // the comparison, not read whole.
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
        // D-0053: `==`/`!=` between a `StringView` and a `StringView` or a
        // `str` compares their text (`StringView::eq`, `StringView::eq_str`);
        // nothing else applies to a view.
        let is_view = |t: &Option<Type>| matches!(t, Some(Type::Named(n, _)) if n == "std::StringView");
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
                if let (Some(ta), Some(tb)) = (&at, &bt) {
                    if !has_unresolved_marker(ta) && !has_unresolved_marker(tb) && !ty_compat(ta, tb) {
                        return Err(mismatch(format!("the operands are `{}` and `{}`, which cannot be compared{}", ta, tb, compare_hint(ta, tb))));
                    }
                    // `[T-Cmp]`: `< <= > >=` additionally need an ordered
                    // τ (D-0108: a number, `bool`, `str` or `String`).
                    if matches!(op, Lt | Le | Gt | Ge) && !has_unresolved_marker(ta) && !matches!(ta, Type::Never) && !Bound::Ordered.holds_for(ta) {
                        return Err(mismatch(format!("`<`, `<=`, `>`, `>=` order numbers, `bool`, `str` and `String`; a `{}` is compared with `==` and `!=` only", ta)));
                    }
                    // D-0108 `[Cmp-Text]`: two `String`s compare by their
                    // bytes, each read where it is (neither is moved).
                    if !has_unresolved_marker(ta) && is_std_string(ta) {
                        self.items.view_ops.lock().unwrap().insert(whole as *const Expr as usize, crate::interp::ViewOp::TextCmp);
                        return Ok(Some(Type::Bool));
                    }
                    // `[T-Cmp]`: "not resource" — whatever the operands'
                    // form (a call's result as much as a binding).
                    if !has_unresolved_marker(ta) && is_resource_ty(&self.items, ta) {
                        return Err(crate::modres::named(
                            "diag.read-of-resource",
                            format!("a comparison reads both operands whole, and `{}` is a resource; compare what it holds (an element at a time, `String::eq`, or through a view or slice)", ta),
                        ));
                    }
                }
                let _ = whole;
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
        if segs.len() > 1 && !self.items.fns.contains_key(&name) && !self.items.externs.contains_key(&name) && !INTRINSIC_NAMES.contains(&name.as_str()) {
            // A qualified path can never be a local: it is an item, an
            // `Enum::Variant`, or `[Resolve-Unbound]`.
            let (prefix, last) = (segs[..segs.len() - 1].join("::"), &segs[segs.len() - 1]);
            let is_variant = self.items.enums.get(&prefix).map_or(false, |e| e.variants.iter().any(|v| &v.name == last));
            if !is_variant {
                return Err(crate::modres::named("diag.unbound-name", format!("nothing named `{}`", name.strip_prefix("std::").unwrap_or(&name))));
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
            let mut m = HashMap::new();
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
                let mut m = HashMap::new();
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
            const TRUSTED_INTRINSICS: &[&str] = &["reclaim", "release", "copy_raw", "deallocate", "text_write", "parse_check", "parse_value", "read_volatile", "write_volatile"];
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
                        let fmt = Expr { kind: ExprKind::Call(Box::new(Expr { kind: ExprKind::Path(vec!["$fmt".to_string()], Vec::new()), line }), args[1..].to_vec()), line };
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
            if name == "spawn" {
                if let Some(Some(Type::Fn(_, r))) = arg_tys.first() {
                    if !has_unresolved_marker(r) {
                        return Ok(Some(Type::Handle(r.clone())));
                    }
                }
            }
            if name == "join" {
                if let Some(Some(Type::Handle(t))) = arg_tys.first() {
                    return Ok(Some((**t).clone()));
                }
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
        for (a, pt) in args.iter().zip(ptys.iter()) {
            self.check_store_operand(a, pt.as_ref())?;
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
        let mut seen = HashSet::new();
        self.clone_problem_in(t, &mut seen)
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
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Never | Type::Ref(..) | Type::Rawptr(_) | Type::Slice(..) => None,
            Type::Array(e, _) => {
                if is_resource_ty(self.items, e) {
                    Some(format!("an array of `{}` (a resource) has no clone; a `Vec` of them does", e))
                } else {
                    None
                }
            }
            Type::Fn(..) | Type::Closure(_) => Some("a function value has no clone".to_string()),
            Type::Handle(_) => Some("a thread handle has no clone".to_string()),
            Type::Mutex(_) => Some("a mutex has no clone".to_string()),
            Type::Guard(_) => Some("a lock guard has no clone".to_string()),
            Type::Named(n, args) => {
                if !is_resource_ty(self.items, t) {
                    return None;
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
                                (!b.holds_for(a)).then(|| format!("`{}` is not `{}`", a, b.name()))
                            }
                        } else {
                            (!b.holds_for(a)).then(|| format!("`{}` is not `{}`", a, b.name()))
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
            .map(|(i, a)| !f.type_params.is_empty() && is_bare_num_literal(a) && f.params.get(i).map_or(false, |p| mentions_any(&apply_subst(&p.ty, &explicit_subst(f, &explicit)), &type_param_set)))
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
            self.check_arg_exact_types(&f.params, args, &arg_types, &HashMap::new(), &type_param_set)?;
            // Non-generic, but its body still needs to be reached by the
            // worklist to be checked at all -- `record_instantiation`
            // with zero type args does exactly that (and `checked`
            // memoizes so a frequently-called function is only checked
            // once).
            self.record_instantiation(&self.item_key(f), &[]);
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
                        format!("`String::append` writes a number, `bool` or text (or a reference to one); `{}` is none of these", t)
                    })
                } else if is_std("std::String::parse") {
                    (!matches!(t, Type::Int(_) | Type::F32 | Type::F64)).then(|| format!("`String::parse` reads a number; `{}` is not a number type", t))
                } else if is_std("std::Vec::sort") || is_std("std::Vec::binary_search") {
                    // D-0062: ordered by `key_less`, the key types' order.
                    (!is_key_type(self.items, t)).then(|| format!("`Vec::{}` orders by the key types' own order (integers, `bool`, `str`, `String`, and structs and enums of them); `{}` is not one{}, so pass the order: `Vec::{}_by`", f.name, t, key_problem_text(self.items, t), f.name))
                } else if is_std("std::PriorityQueue::new") {
                    // D-0137: ordered by `key_less`, as `Vec::sort` is.
                    (!is_key_type(self.items, t)).then(|| format!("`PriorityQueue::new` orders by the key types' own order (integers, `bool`, `str`, `String`, and structs and enums of them); `{}` is not one{}, so pass the order: `PriorityQueue::new_by`", t, key_problem_text(self.items, t)))
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
                    t => (b.holds_for(t), None),
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
                unify_type_shape(&generic_self, exp, &mut m, &sdecl.type_params);
            }
        }
        let mut seen = HashSet::new();
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
                } else if let Some(m) = &by_ref {
                // D-0071: a binder of a match by reference is a reference
                // into the scrutinee's place for the arm.
                    let fact = self.match_referent(scrut).map(|(root, path)| DerivKey { root, path, mode: m.clone() });
                    self.set_deriv(bn, fact.into_iter().collect());
                } else if self.gamma.get(bn).map_or(false, |t| holds_ref(self.items, t)) {
                    // D-0088: a reference out of a call's `Option<ref<V>>`
                    // borrows from the call's reference arguments.
                    let facts = self.contained_ref_facts(scrut);
                    if !facts.is_empty() {
                        self.set_deriv(bn, facts);
                    }
                }
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
];

// D-0101: the floating-point functions of two operands of one type.
pub const FLOAT2_FNS: &[&str] = &["std::atan2", "std::powf"];

// `rule.arith.alt`'s operations, whose two operands share one type.
const ALT_INTRINSICS: &[&str] = &[
    "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub",
    "checked_mul", "checked_div", "checked_rem",
];

pub(crate) const INTRINSIC_NAMES: &[&str] = &[
    "drop", "clone", "sizeof", "alignof", "dangling", "checked_narrow", "count_ones", "leading_zeros", "trailing_zeros", "rotate_left", "rotate_right", "read_volatile", "write_volatile", "rawptr_of", "reclaim", "release", "copy_raw", "allocate", "deallocate",
    "lock", "spawn", "join", "std::Result::map_err", "std::print", "reinterpret_ptr", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float",
    "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul",
    "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem", "checked_neg", "fault", "stdout_write",
    "std::sqrt", "std::floor", "std::ceil", "std::round", "std::trunc", "std::ln", "std::exp", "std::log2", "std::log10", "std::sin", "std::cos", "std::tan",
    "std::atan2", "std::powf",
    "str_len", "str_byte", "str_ptr", "str_slice", "slice_len", "min_value", "max_value",
    "append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "key_less", "key_less_at", "swap_places", "$fmt", "$assert_fail",
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
const STD_ONLY_INTRINSICS: &[&str] = &["str_slice", "append_native", "text_write", "parse_check", "parse_value", "key_hash", "key_eq", "key_less", "key_less_at", "swap_places"];

// `rule.stdlib.hashmap`'s key types (spec/21 §1a, D-0041).
fn is_key_type(items: &Items, t: &Type) -> bool {
    key_type_problem(items, t, 0).is_none()
}

// Why `t` is not a key type (None when it is): an integer, `bool`, `str`,
// `String`, or (D-0110) a struct whose fields, or an enum whose payloads,
// are all key types. The reason names the part that is not.
fn key_type_problem(items: &Items, t: &Type, depth: usize) -> Option<String> {
    match t {
        Type::Int(_) | Type::Bool | Type::Str | Type::Never => None,
        Type::Named(n, _) if n == "std::String" => None,
        _ if depth > 16 => Some(format!("`{}` nests too deeply", t)),
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
        (Type::Named(n, _), Type::Str) if n == "std::String" => "; make a `String` from a literal with `String::from_str(\"…\")`".to_string(),
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

// D-0090: the types a bound admits, for messages.
fn bound_members(b: Bound) -> &'static str {
    match b {
        Bound::Eq | Bound::Ordered => "a number, `bool`, `str` or `String`",
        Bound::Number => "an integer or floating-point type",
        Bound::Integer => "an integer type",
        Bound::Clone => "a plain type, `String`, `Rc<T>`, `Box<T: clone>`, `Vec<T: clone>`, a hash table of them, a type with its own `T::clone`, or a struct or enum of `clone` types",
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
    let mut tail = HashSet::new();
    if let Some(e) = &b.tail {
        collect_free_vars_expr(e, &mut HashSet::new(), &mut tail);
    }
    let mut captured = HashSet::new();
    for s in &b.stmts {
        closure_captures_stmt(s, &mut captured);
    }
    if let Some(e) = &b.tail {
        closure_captures_expr(e, &mut captured);
    }
    let mut declared: HashMap<&str, (usize, usize)> = HashMap::new();
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

// The names statement `s` mentions (bound inside it or not: a shadowing
// inner binding only makes the answer larger).
fn stmt_mentions(s: &Stmt) -> HashSet<String> {
    let mut free = HashSet::new();
    let mut bound = HashSet::new();
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
