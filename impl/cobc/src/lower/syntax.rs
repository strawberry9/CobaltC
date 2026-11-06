// Syntax walks over the AST that the analyses share.

use super::*;

// Whether a body contains a loop (a `while`, which `for` and `foreach`
// are): where a field dual body's entry test is paid for.
pub(super) fn has_loop(b: &Block) -> bool {
    let mut found = false;
    visit_exprs(b, &mut |e| {
        if matches!(e.kind, ExprKind::While(..)) {
            found = true;
        }
    });
    found
}

// D-0189: the `Vec` fields among `fields` of the reference parameter `b`
// that the body uses only as a confined vector is used, or None if `b`
// itself is used any other way. Admitted: `b.f[i]` read or written
// (never borrowed); `&b.f` / `&mut b.f` as the first argument of
// `Vec::len`, `push`, `pop`, `reserve` or a `CONFINING_CALLS` call that no
// other argument mentions `b` in; and `b.g` for every other field, freely
// (its paths never overlap `b.f`). Not inside an index or the value
// assigned to `b.f[i]`: a call that may move or shorten a field there
// (any but `len` and the searches), whose element address is taken first.
// A field used any other way is left out; `b` named bare, borrowed whole,
// shadowed or captured rules the function out.
pub(super) fn field_confined_uses(body: &Block, b: &str, fields: &[String]) -> Option<(HashSet<String>, bool)> {
    struct W<'a> {
        b: &'a str,
        fields: &'a [String],
        used: HashSet<String>,
        bad_fields: HashSet<String>,
        out: bool,
        // Something under `b` other than a confined field's call argument
        // is borrowed (or matched, which may borrow): `b` then keeps its
        // path in the fast half.
        borrowed: bool,
    }
    // Whether `e` is a place rooted at `b` (`b.g`, `b.g[i].h`, `(*b).g`).
    fn rooted_on_b(w: &W, e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) => rooted_on_b(w, a),
            ExprKind::Deref(p) => path_name(strip_parens(p)) == Some(w.b),
            ExprKind::Path(..) => path_name(strip_parens(e)) == Some(w.b),
            _ => false,
        }
    }
    fn field_on_b<'e>(w: &W, e: &'e Expr) -> Option<&'e String> {
        let ExprKind::Field(base, f) = &strip_parens(e).kind else { return None };
        let base = match &strip_parens(base).kind {
            ExprKind::Deref(p) => strip_parens(p),
            _ => strip_parens(base),
        };
        (path_name(base) == Some(w.b)).then_some(f)
    }
    fn is_vec_field(w: &W, f: &str) -> bool {
        w.fields.iter().any(|x| x == f)
    }
    const READS: &[&str] = &["std::Vec::len", "Vec::len", "std::Vec::contains", "std::Vec::index_of", "std::Vec::binary_search"];
    fn call_name(c: &Expr) -> Option<String> {
        path_name_joined(c).map(|n| if n.starts_with("std::") { n } else { format!("std::{}", n) })
    }
    fn expr(w: &mut W, e: &Expr, guarded: bool) {
        if w.out {
            return;
        }
        if let Some(f) = field_on_b(w, e) {
            // `b.f` alone: a vector field used whole is out; another field
            // is used as any place is.
            if is_vec_field(w, f) {
                w.bad_fields.insert(f.clone());
            }
            return;
        }
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == w.b => w.out = true,
            ExprKind::Index(a, i) if field_on_b(w, a).map_or(false, |f| is_vec_field(w, f)) => {
                let f = field_on_b(w, a).unwrap().clone();
                w.used.insert(f);
                expr(w, i, true);
            }
            ExprKind::Assign(l, r) => {
                let target = match &strip_parens(l).kind {
                    ExprKind::Index(a, _) => field_on_b(w, a).map_or(false, |f| is_vec_field(w, f)),
                    _ => false,
                };
                expr(w, l, guarded);
                expr(w, r, guarded || target);
            }
            ExprKind::Borrow(_, x) => {
                if rooted_on_b(w, x) {
                    w.borrowed = true;
                }
                if let Some(f) = field_on_b(w, x) {
                    if is_vec_field(w, f) {
                        w.bad_fields.insert(f.clone());
                    }
                    return;
                }
                if let ExprKind::Index(a, i) = &strip_parens(x).kind {
                    if let Some(f) = field_on_b(w, a) {
                        if is_vec_field(w, f) {
                            w.bad_fields.insert(f.clone());
                        }
                        return expr(w, i, guarded);
                    }
                }
                expr(w, x, guarded);
            }
            ExprKind::Call(c, args) => {
                if let Some(n) = call_name(c) {
                    let admitted = n == "std::Vec::push" || n == "std::Vec::pop" || n == "std::Vec::reserve" || READS.contains(&n.as_str()) || CONFINING_CALLS.contains(&n.as_str());
                    if admitted {
                        if let Some(ExprKind::Borrow(_, x)) = args.first().map(|a| &strip_parens(a).kind) {
                            if let Some(f) = field_on_b(w, x).filter(|f| is_vec_field(w, f)).cloned() {
                                if guarded && !READS.contains(&n.as_str()) {
                                    w.bad_fields.insert(f.clone());
                                }
                                if args.iter().skip(1).any(|a| mentions(a, w.b)) {
                                    w.bad_fields.insert(f.clone());
                                }
                                w.used.insert(f);
                                for a in args.iter().skip(1) {
                                    expr(w, a, guarded);
                                }
                                return;
                            }
                        }
                    }
                }
                expr(w, c, guarded);
                for a in args {
                    expr(w, a, guarded);
                }
            }
            ExprKind::Closure { .. } => {
                if mentions(e, w.b) {
                    w.out = true;
                }
            }
            ExprKind::Field(a, _) | ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) | ExprKind::Return(Some(a)) => expr(w, a, guarded),
            ExprKind::Binary(_, a, c) | ExprKind::Index(a, c) => {
                expr(w, a, guarded);
                expr(w, c, guarded);
            }
            ExprKind::SliceOf(_, a, lo, hi) => {
                expr(w, a, guarded);
                expr(w, lo, guarded);
                expr(w, hi, guarded);
            }
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| expr(w, a, guarded)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| expr(w, a, guarded)),
            ExprKind::Block(bl) | ExprKind::Unsafe(bl) => block(w, bl, guarded),
            ExprKind::If(c, t, e2) => {
                expr(w, c, guarded);
                block(w, t, guarded);
                if let Some(e2) = e2 {
                    expr(w, e2, guarded);
                }
            }
            ExprKind::While(c, bl, st) => {
                expr(w, c, guarded);
                block(w, bl, guarded);
                if let Some(st) = st {
                    expr(w, st, guarded);
                }
            }
            ExprKind::Match(sc, arms) => {
                if rooted_on_b(w, sc) {
                    w.borrowed = true;
                }
                expr(w, sc, guarded);
                for a in arms {
                    if a.binder.as_deref() == Some(w.b) {
                        w.out = true;
                    }
                    expr(w, &a.body, guarded);
                }
            }
            _ => {
                // Anything else naming `b` that this walk does not know.
                if mentions(e, w.b) && !matches!(e.kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_)) {
                    w.out = true;
                }
            }
        }
    }
    fn block(w: &mut W, b: &Block, guarded: bool) {
        for s in &b.stmts {
            match s {
                Stmt::Let { name, init, .. } => {
                    if name == w.b {
                        w.out = true;
                    }
                    if let Some(e) = init {
                        expr(w, e, guarded);
                    }
                }
                Stmt::Destructure { names, init, .. } => {
                    if names.iter().any(|n| n == w.b) {
                        w.out = true;
                    }
                    expr(w, init, guarded);
                }
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(w, e, guarded),
            }
        }
        if let Some(t) = &b.tail {
            expr(w, t, guarded);
        }
    }
    let mut w = W { b, fields, used: HashSet::new(), bad_fields: HashSet::new(), out: false, borrowed: false };
    block(&mut w, body, false);
    if w.out {
        return None;
    }
    let used: HashSet<String> = w.used.difference(&w.bad_fields).cloned().collect();
    // Every field of `b` but the confined ones must be plain for `b` to
    // go unchecked: a vector field used otherwise is still reached
    // through `b`'s path.
    Some((used.clone(), !w.borrowed && w.bad_fields.is_empty() && fields.iter().all(|f| used.contains(f) || !mentions_field(body, b, f))))
}

// Whether the body names `b.f` at all.
pub(super) fn mentions_field(body: &Block, b: &str, f: &str) -> bool {
    let mut found = false;
    visit_exprs(body, &mut |e| {
        if let ExprKind::Field(a, g) = &strip_parens(e).kind {
            let base = match &strip_parens(a).kind {
                ExprKind::Deref(p) => strip_parens(p),
                _ => strip_parens(a),
            };
            if g == f && path_name(base) == Some(b) {
                found = true;
            }
        }
    });
    found
}

// Whether a result type holds nothing a call could keep (D-0192's pure
// closures).
pub(super) fn plain_ret(g: &Gen, t: &Type) -> bool {
    matches!(t, Type::Void) || (g.plain_data(t) && !g.has_refs(t) && !g.holds_fn(t))
}

// Whether an arm's body uses `name` exactly once, and that use is the
// first thing it evaluates: `f(name, …)` or `f(g(name, …), …)`, `f` and
// `g` named (functions, or local closures).
pub(super) fn binder_used_once_first(body: &Expr, name: &str) -> bool {
    let is_name = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
    // The first thing evaluated: a block's first statement (or its value),
    // and in `x = x op e` (what `x op= e` is, D-0035), for a local `x`
    // other than the binder, `e`: reading or writing a local touches
    // nothing the binder's borrow reaches.
    let mut first = strip_parens(body);
    if let ExprKind::Block(b) = &first.kind {
        first = match (b.stmts.first(), &b.tail) {
            (Some(Stmt::Expr(e)) | Some(Stmt::BlockLike(e)), _) => strip_parens(e),
            (None, Some(t)) => strip_parens(t),
            _ => return false,
        };
    }
    let local = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] != name && t.is_empty());
    if let ExprKind::Assign(l, r) = &first.kind {
        if !local(l) {
            return false;
        }
        first = match &strip_parens(r).kind {
            ExprKind::Binary(_, x, e) if local(x) && path_name(x) == path_name(l) => strip_parens(e),
            _ => strip_parens(r),
        };
    }
    // A read through the binder (`*name`), first: no call to look into.
    if matches!(&first.kind, ExprKind::Deref(x) if is_name(x)) {
        let mut n = 0;
        let b = Block { stmts: Vec::new(), tail: Some(Box::new(body.clone())) };
        visit_exprs(&b, &mut |e| {
            if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
                n += 1;
            }
        });
        return n == 1;
    }
    let ExprKind::Call(callee, args) = &first.kind else { return false };
    // A named callee (a function, or a local closure, whose evaluation
    // only reads that local).
    if !matches!(&callee.kind, ExprKind::Path(..)) {
        return false;
    }
    // The other arguments are evaluated while the binder's value is
    // pending (D-0107): only literals and locals, which form no path and
    // so could not have met the binder's held path.
    let inert = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) | ExprKind::Unit) || local(e);
    let Some(a0) = args.first() else { return false };
    if !args[1..].iter().all(|x| inert(x)) {
        return false;
    }
    let first = is_name(a0)
        || matches!(&strip_parens(a0).kind, ExprKind::Call(g, ga) if matches!(&g.kind, ExprKind::Path(..)) && ga.first().map_or(false, |x| is_name(x)) && ga[1..].iter().all(|x| inert(x)));
    if !first {
        return false;
    }
    let mut n = 0;
    let b = Block { stmts: Vec::new(), tail: Some(Box::new(body.clone())) };
    visit_exprs(&b, &mut |e| {
        if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
            n += 1;
        }
    });
    n == 1
}

// `*t = *t op e;` for the hidden `t`, with `e` a literal or a local
// (`lower_block_body`'s compound pair).
pub(super) fn compound_write_inert(s: &Stmt, t: &str) -> bool {
    let deref_t = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Deref(x) if path_name(x) == Some(t));
    let Stmt::Expr(e) = s else { return false };
    let ExprKind::Assign(l, r) = &e.kind else { return false };
    let ExprKind::Binary(_, a, rhs) = &strip_parens(r).kind else { return false };
    deref_t(l)
        && deref_t(a)
        && (matches!(&strip_parens(rhs).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_))
            || path_name(rhs).map_or(false, |n| n != t && !n.starts_with("__")))
}

// The locals of a function body that `bind_value` may hold by frame
// (`cb_frame_hold`) when of reference type: never assigned (`r = …`),
// never borrowed as a binding (`&r`), and mentioned in no closure (which
// could capture it).
pub(super) fn held_locals(b: &Block) -> HashSet<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    visit_stmts(b, &mut |st| {
        if let Stmt::Let { name, .. } = st {
            *count.entry(name.clone()).or_default() += 1;
        }
    });
    // A match arm's binder of reference type is such a local too.
    visit_exprs(b, &mut |e| {
        if let ExprKind::Match(_, arms) = &e.kind {
            for a in arms {
                if let Some(n) = &a.binder {
                    *count.entry(n.clone()).or_default() += 1;
                }
            }
        }
    });
    let assigned = assigned_names(b);
    let mut excluded: HashSet<String> = HashSet::new();
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Borrow(_, a) => {
            if let Some(n) = path_name(a) {
                excluded.insert(n.to_string());
            }
        }
        ExprKind::Closure { body, captures, .. } => {
            excluded.extend(captures.iter().cloned());
            visit_exprs(body, &mut |x| {
                if let ExprKind::Path(s, _) = &x.kind {
                    if s.len() == 1 {
                        excluded.insert(s[0].clone());
                    }
                }
            });
        }
        _ => {}
    });
    // The conditions are by name, so they cover every declaration of it.
    count.into_iter().filter(|(n, _)| !assigned.contains(n) && !excluded.contains(n) && !n.starts_with("__")).map(|(n, _)| n).collect()
}

// The function an arm body's first call passes the binder to: `g` in
// `g(name, …)` as the outer call's first argument or as the outer call.
pub(super) fn binder_box_call(body: &Expr, name: &str) -> Option<String> {
    let is_name = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
    let mut first = strip_parens(body);
    if let ExprKind::Block(b) = &first.kind {
        first = match (b.stmts.first(), &b.tail) {
            (Some(Stmt::Expr(e)) | Some(Stmt::BlockLike(e)), _) => strip_parens(e),
            (None, Some(t)) => strip_parens(t),
            _ => return None,
        };
    }
    if let ExprKind::Assign(_, r) = &first.kind {
        first = match &strip_parens(r).kind {
            ExprKind::Binary(_, _, e) => strip_parens(e),
            _ => strip_parens(r),
        };
    }
    let ExprKind::Call(callee, args) = &first.kind else { return None };
    let ExprKind::Path(segs, _) = &callee.kind else { return None };
    if args.first().map_or(false, |a| is_name(a)) {
        return Some(segs.join("::"));
    }
    match args.first().map(|a| &strip_parens(a).kind) {
        Some(ExprKind::Call(g, ga)) if ga.first().map_or(false, |a| is_name(a)) => match &g.kind {
            ExprKind::Path(gs, _) => Some(gs.join("::")),
            _ => None,
        },
        _ => None,
    }
}

// `Option<ref<T>>`: reference data whose one reference is the payload.
pub(super) fn one_ref_option(t: &Type) -> bool {
    matches!(t, Type::Named(n, a) if n == "std::Option" && matches!(a.as_slice(), [Type::Ref(..)]))
}

// `match (HashMap::get_mut(…)) { Some(c) : *c op= x, None : … }` (or `_`
// for `None`), `x` a literal or a local other than `c`, the `None` arm
// binding nothing: nothing runs between the element access and the write
// but the read through `c` (`compound_write_inert`), as for
// `*HashMap::entry(…) op= x`.
pub(super) fn get_mut_acc_arms(scrut: &Expr, arms: &[Arm]) -> bool {
    let ExprKind::Call(callee, _) = &strip_parens(scrut).kind else { return false };
    let ExprKind::Path(segs, _) = &callee.kind else { return false };
    if !matches!(segs.join("::").as_str(), "HashMap::get_mut" | "std::HashMap::get_mut") || arms.len() != 2 {
        return false;
    }
    let some = arms.iter().filter(|a| a.variant.as_deref() == Some("Some")).count();
    let other_ok = arms.iter().all(|a| match a.variant.as_deref() {
        Some("Some") => a.binder.as_ref().map_or(false, |c| {
            let b = strip_parens(&a.body);
            let e = match &b.kind {
                ExprKind::Block(bl) => match (bl.stmts.as_slice(), &bl.tail) {
                    ([Stmt::Expr(x)], None) => &**x,
                    ([], Some(t)) => &**t,
                    _ => return false,
                },
                _ => b,
            };
            compound_write_inert(&Stmt::Expr(Box::new(e.clone())), c)
        }),
        Some("None") | None => a.binder.is_none() && a.nested.is_empty() && a.lit.is_none(),
        _ => false,
    });
    some == 1 && other_ok
}

// The `(h, f)` pairs, `h` a local declared once (`let`, with an
// initializer; no parameter, binder or capture of the name anywhere),
// whose field `h.f` is named only as `h.f[i]` (`Index`, or the
// `*Vec::index_*(&h.f, i)` it stands for) and `Vec::len(&h.f)`, and
// where `h` itself is named only as the base of such fields. Then no
// path to `h.f` or its elements is ever formed but through `h`'s root,
// nothing moves, grows, shrinks or replaces `h.f`'s buffer after `h` is
// declared, and an element address taken for `h.f[i] = e` survives `e`
// (`confined_field` checks the types). By name, across scopes; anything
// else -- every other use of `h.f`, any use of `h` whole, any mention in
// a closure -- rules the pair (or `h`) out. Other fields of `h` are used
// freely: their paths never overlap `h.f`.
pub(super) fn confined_fields(body: &Block, params: &[String]) -> HashSet<(String, String)> {
    struct St {
        lets: HashMap<String, usize>,
        bad_names: HashSet<String>,
        ok: HashSet<(String, String)>,
        bad: HashSet<(String, String)>,
    }
    fn field_of(e: &Expr) -> Option<(String, String)> {
        let ExprKind::Field(b, f) = &strip_parens(e).kind else { return None };
        Some((path_name(b)?.to_string(), f.clone()))
    }
    fn any_path(b: &Block, st: &mut St) {
        visit_exprs(b, &mut |e| {
            if let ExprKind::Path(s, _) = &e.kind {
                if s.len() == 1 {
                    st.bad_names.insert(s[0].clone());
                }
            }
        });
    }
    fn block(b: &Block, st: &mut St) {
        for s in &b.stmts {
            match s {
                Stmt::Let { name, init, .. } => {
                    *st.lets.entry(name.clone()).or_default() += 1;
                    match init {
                        Some(e) => expr(e, st),
                        None => {
                            st.bad_names.insert(name.clone());
                        }
                    }
                }
                Stmt::Destructure { names, init, .. } => {
                    st.bad_names.extend(names.iter().cloned());
                    expr(init, st);
                }
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, st),
            }
        }
        if let Some(t) = &b.tail {
            expr(t, st);
        }
    }
    // A borrow's place reaching `h.f[…]` (through fields, parentheses and
    // further indexing; `*Vec::index_*(&h.f, i)` likewise): a path to an
    // element would be formed, so the pair is out. The rest is scanned.
    fn borrowed(x: &Expr, st: &mut St) {
        match &x.kind {
            ExprKind::Field(b, _) if path_name(b).is_none() => borrowed(b, st),
            ExprKind::Paren(b) => borrowed(b, st),
            ExprKind::Index(a, i) => {
                match field_of(a) {
                    Some(hf) => {
                        st.bad.insert(hf);
                    }
                    None => borrowed(a, st),
                }
                expr(i, st);
            }
            ExprKind::Deref(c) => {
                if let ExprKind::Call(_, args) = &strip_parens(c).kind {
                    if let Some(hf) = args.first().and_then(|a0| match &strip_parens(a0).kind { ExprKind::Borrow(_, y) => field_of(y), _ => None }) {
                        st.bad.insert(hf);
                    }
                }
                expr(x, st);
            }
            _ => expr(x, st),
        }
    }
    fn expr(e: &Expr, st: &mut St) {
        if let ExprKind::Borrow(_, x) = &e.kind {
            return borrowed(x, st);
        }
        // The admitted forms.
        if let ExprKind::Index(a, i) = &e.kind {
            if let Some(hf) = field_of(a) {
                st.ok.insert(hf);
                return expr(i, st);
            }
        }
        if let ExprKind::Deref(c) = &e.kind {
            if let ExprKind::Call(callee, args) = &strip_parens(c).kind {
                let n = path_name_joined(callee);
                if matches!(n.as_deref(), Some("std::Vec::index_shared" | "std::Vec::index_exclusive")) {
                    if let [a0, i] = args.as_slice() {
                        if let Some(hf) = match &strip_parens(a0).kind { ExprKind::Borrow(_, x) => field_of(x), _ => None } {
                            st.ok.insert(hf);
                            return expr(i, st);
                        }
                    }
                }
            }
        }
        if let ExprKind::Call(callee, args) = &e.kind {
            if matches!(path_name_joined(callee).as_deref(), Some("Vec::len" | "std::Vec::len")) {
                if let [a0] = args.as_slice() {
                    if let Some(hf) = match &strip_parens(a0).kind { ExprKind::Borrow(_, x) => field_of(x), _ => None } {
                        st.ok.insert(hf);
                        return;
                    }
                }
            }
        }
        match &e.kind {
            ExprKind::Field(b, f) => match path_name(b) {
                Some(h) => {
                    st.bad.insert((h.to_string(), f.clone()));
                }
                None => expr(b, st),
            },
            ExprKind::Path(s, _) => {
                if s.len() == 1 {
                    st.bad_names.insert(s[0].clone());
                }
            }
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue | ExprKind::Return(None) => {}
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| expr(a, st)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| expr(a, st)),
            ExprKind::ArrayRepeat(a, _) | ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::Return(Some(a)) => expr(a, st),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                expr(a, st);
                expr(b, st);
            }
            ExprKind::SliceOf(_, a, lo, hi) => {
                expr(a, st);
                expr(lo, st);
                expr(hi, st);
            }
            ExprKind::Call(c, args) => {
                expr(c, st);
                args.iter().for_each(|a| expr(a, st));
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, st),
            ExprKind::If(c, t, e2) => {
                expr(c, st);
                block(t, st);
                if let Some(e2) = e2 {
                    expr(e2, st);
                }
            }
            ExprKind::While(c, b, step) => {
                expr(c, st);
                block(b, st);
                if let Some(x) = step {
                    expr(x, st);
                }
            }
            ExprKind::Match(sc, arms) => {
                expr(sc, st);
                for a in arms {
                    if let Some(b) = &a.binder {
                        st.bad_names.insert(b.clone());
                    }
                    if let Some(l) = &a.lit {
                        expr(l, st);
                    }
                    expr(&a.body, st);
                }
            }
            ExprKind::Closure { captures, params, body, .. } => {
                st.bad_names.extend(captures.iter().cloned());
                st.bad_names.extend(params.iter().map(|p| p.name.clone()));
                any_path(body, st);
            }
        }
    }
    let mut st = St { lets: HashMap::new(), bad_names: params.iter().cloned().collect(), ok: HashSet::new(), bad: HashSet::new() };
    block(body, &mut st);
    st.ok
        .iter()
        .filter(|(h, f)| st.lets.get(h) == Some(&1) && !st.bad_names.contains(h) && !st.bad.contains(&(h.clone(), f.clone())))
        .cloned()
        .collect()
}

// The locals `x` (declared once, with an initializer; no parameter,
// binder or capture of the name) that no exclusive path can ever be held
// to, nor to any of their elements: `x` is named only as `x[i]` (read, or
// the target of a write -- a write is checked as usual), inside a shared
// borrow (`&x`, `&x[i]`, `&x[i].f`, however long it is held), or as
// `&mut x` handed straight to `Vec::push`, `pop`, `reserve` or `len`,
// whose paths end with their statement. Anything else -- `&mut x[i]`,
// `&mut x` elsewhere (`foreach (p in &mut x)`), moving or assigning `x`, a
// closure naming it -- rules `x` out. Then a read of an element or of the
// length meets only shared paths and the root, so it cannot clash
// (`Fx::read_confined_vec`).
pub(super) fn read_confined_vecs(body: &Block, params: &[String]) -> HashSet<String> {
    struct St {
        lets: HashMap<String, usize>,
        bad: HashSet<String>,
        seen: HashSet<String>,
    }
    fn any_path(b: &Block, st: &mut St) {
        visit_exprs(b, &mut |e| {
            if let ExprKind::Path(s, _) = &e.kind {
                if s.len() == 1 {
                    st.bad.insert(s[0].clone());
                }
            }
        });
    }
    fn bare(e: &Expr) -> Option<&str> {
        match &strip_parens(e).kind {
            ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => Some(&s[0]),
            _ => None,
        }
    }
    // A place under a shared borrow: its root, through fields, elements
    // and parentheses, is only read.
    fn shared_place(e: &Expr, st: &mut St) {
        match &e.kind {
            ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => {
                st.seen.insert(s[0].clone());
            }
            ExprKind::Field(b, _) | ExprKind::Paren(b) => shared_place(b, st),
            ExprKind::Index(a, i) => {
                shared_place(a, st);
                expr(i, st);
            }
            _ => expr(e, st),
        }
    }
    // A place under an exclusive borrow: its root is ruled out.
    fn exclusive_place(e: &Expr, st: &mut St) {
        match &e.kind {
            ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => {
                st.bad.insert(s[0].clone());
            }
            ExprKind::Field(b, _) | ExprKind::Paren(b) => exclusive_place(b, st),
            ExprKind::Index(a, i) => {
                exclusive_place(a, st);
                expr(i, st);
            }
            _ => expr(e, st),
        }
    }
    fn block(b: &Block, st: &mut St) {
        for s in &b.stmts {
            match s {
                Stmt::Let { name, init, .. } => {
                    *st.lets.entry(name.clone()).or_default() += 1;
                    match init {
                        Some(e) => expr(e, st),
                        None => {
                            st.bad.insert(name.clone());
                        }
                    }
                }
                Stmt::Destructure { names, init, .. } => {
                    st.bad.extend(names.iter().cloned());
                    expr(init, st);
                }
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, st),
            }
        }
        if let Some(t) = &b.tail {
            expr(t, st);
        }
    }
    fn expr(e: &Expr, st: &mut St) {
        match &e.kind {
            ExprKind::Index(a, i) if bare(a).is_some() => {
                st.seen.insert(bare(a).unwrap().to_string());
                expr(i, st);
            }
            ExprKind::Borrow(Mode::Shared, x) => shared_place(x, st),
            ExprKind::Borrow(Mode::Exclusive, x) => exclusive_place(x, st),
            ExprKind::Call(callee, args)
                if matches!(path_name_joined(callee).as_deref(), Some("Vec::push" | "std::Vec::push" | "Vec::pop" | "std::Vec::pop" | "Vec::reserve" | "std::Vec::reserve" | "Vec::len" | "std::Vec::len"))
                    && args.first().map_or(false, |a0| matches!(&strip_parens(a0).kind, ExprKind::Borrow(_, x) if bare(x).is_some())) =>
            {
                if let ExprKind::Borrow(_, x) = &strip_parens(&args[0]).kind {
                    st.seen.insert(bare(x).unwrap().to_string());
                }
                args[1..].iter().for_each(|a| expr(a, st));
            }
            ExprKind::Path(s, _) => {
                if s.len() == 1 {
                    st.bad.insert(s[0].clone());
                }
            }
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Dollar | ExprKind::Break | ExprKind::Continue | ExprKind::Return(None) => {}
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| expr(a, st)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| expr(a, st)),
            ExprKind::ArrayRepeat(a, _) | ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::Return(Some(a)) => expr(a, st),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                expr(a, st);
                expr(b, st);
            }
            ExprKind::SliceOf(_, a, lo, hi) => {
                // A slice of `x` holds a path to its elements; exclusive
                // or not, it is not one of the admitted forms.
                exclusive_place(a, st);
                expr(lo, st);
                expr(hi, st);
            }
            ExprKind::Call(c, args) => {
                expr(c, st);
                args.iter().for_each(|a| expr(a, st));
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, st),
            ExprKind::If(c, t, e2) => {
                expr(c, st);
                block(t, st);
                if let Some(e2) = e2 {
                    expr(e2, st);
                }
            }
            ExprKind::While(c, b, step) => {
                expr(c, st);
                block(b, st);
                if let Some(x) = step {
                    expr(x, st);
                }
            }
            ExprKind::Match(sc, arms) => {
                expr(sc, st);
                for a in arms {
                    if let Some(b) = &a.binder {
                        st.bad.insert(b.clone());
                    }
                    if let Some(l) = &a.lit {
                        expr(l, st);
                    }
                    expr(&a.body, st);
                }
            }
            ExprKind::Closure { captures, params, body, .. } => {
                st.bad.extend(captures.iter().cloned());
                st.bad.extend(params.iter().map(|p| p.name.clone()));
                any_path(body, st);
            }
        }
    }
    let mut st = St { lets: HashMap::new(), bad: params.iter().cloned().collect(), seen: HashSet::new() };
    block(body, &mut st);
    st.seen.iter().filter(|x| st.lets.get(*x) == Some(&1) && !st.bad.contains(*x)).cloned().collect()
}

// A path expression's segments joined (`Vec::len`), with no type arguments
// considered.
pub(super) fn path_name_joined(e: &Expr) -> Option<String> {
    match &strip_parens(e).kind {
        ExprKind::Path(s, _) => Some(s.join("::")),
        _ => None,
    }
}

// The text `String::append(s, &t)` appends: its `T` is then
// `ref<String, shared>`, and `append_native` hands it to the native
// `append_string`, which only reads through it and keeps nothing -- a
// reader, though the parameter is generic (`Gen::reader_param` asks for a
// `ref`).
pub(super) fn appended_text(name: &str, f: &FnDecl, j: usize) -> bool {
    name == "std::String::append" && j == 1 && in_prelude(f)
}
