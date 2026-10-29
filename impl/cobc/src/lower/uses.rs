// Syntax walks: how a body uses a name (reads, writes, views, derefs).

use super::*;

// `StringView::trim(x)` (or `trim_start`, `trim_end`) of a bare name `x`:
// which, and `x`.
pub(super) fn trim_of(e: &Expr) -> Option<(String, String)> {
    let ExprKind::Call(c, args) = &strip_parens(e).kind else { return None };
    let ExprKind::Path(segs, t) = &c.kind else { return None };
    if !t.is_empty() || args.len() != 1 {
        return None;
    }
    let k = segs.join("::");
    let k = if k.starts_with("std::") { k } else { format!("std::{}", k) };
    if !TRIMS.contains(&k.as_str()) {
        return None;
    }
    Some((k, path_name(&args[0])?.to_string()))
}

// `StringView::of(L)` for a text literal `L` (`views.rs` writes a literal
// where a `StringView` is expected so): the literal.
pub(super) fn literal_view_arg(e: &Expr) -> Option<&Expr> {
    match &strip_parens(e).kind {
        ExprKind::Call(callee, args) if args.len() == 1 && matches!(&strip_parens(&args[0]).kind, ExprKind::StrLit(_)) && matches!(&callee.kind, ExprKind::Path(segs, t) if t.is_empty() && segs.join("::") == "std::StringView::of") => Some(&args[0]),
        _ => None,
    }
}

pub(super) fn as_view_arg(e: &Expr) -> Option<&Expr> {
    match &strip_parens(e).kind {
        ExprKind::Call(callee, args) if args.len() == 1 && matches!(&callee.kind, ExprKind::Path(segs, t) if t.is_empty() && matches!(segs.join("::").as_str(), "String::as_view" | "std::String::as_view")) => Some(&args[0]),
        _ => None,
    }
}

// An argument whose evaluation runs nothing that could write a place: a
// literal, a binding or a field of one, `*x`, a shared borrow of such a
// place, or `String::as_view` of one.
pub(super) fn inert_view_arg(e: &Expr) -> bool {
    fn place(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Path(segs, t) => segs.len() == 1 && t.is_empty(),
            ExprKind::Field(a, _) | ExprKind::Paren(a) => place(a),
            ExprKind::Deref(a) => matches!(&strip_parens(a).kind, ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty()),
            _ => false,
        }
    }
    match &strip_parens(e).kind {
        ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) => true,
        ExprKind::Borrow(Mode::Shared, x) => place(x),
        _ => place(e) || as_view_arg(e).map_or(false, inert_view_arg),
    }
}

// A one-segment path's name.
pub(super) fn path_name(e: &Expr) -> Option<&str> {
    match &strip_parens(e).kind {
        ExprKind::Path(s, _) if s.len() == 1 => Some(s[0].as_str()),
        _ => None,
    }
}

// Every expression in `e`, closures' bodies included.
pub(super) fn visit_expr(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => visit_expr(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            visit_expr(a, f);
            visit_expr(b, f);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            visit_expr(a, f);
            visit_expr(lo, f);
            visit_expr(hi, f);
        }
        ExprKind::Call(c, args) => {
            visit_expr(c, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| visit_expr(a, f)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| visit_expr(a, f)),
        ExprKind::ArrayRepeat(a, _) => visit_expr(a, f),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => visit_exprs(b, f),
        ExprKind::If(c, t, e2) => {
            visit_expr(c, f);
            visit_exprs(t, f);
            if let Some(e2) = e2 {
                visit_expr(e2, f);
            }
        }
        ExprKind::While(c, b, step) => {
            visit_expr(c, f);
            visit_exprs(b, f);
            if let Some(st) = step {
                visit_expr(st, f);
            }
        }
        ExprKind::Match(sc, arms) => {
            visit_expr(sc, f);
            arms.iter().for_each(|a| visit_expr(&a.body, f));
        }
        ExprKind::Return(Some(a)) => visit_expr(a, f),
        ExprKind::Closure { body, .. } => visit_exprs(body, f),
        _ => {}
    }
}

// Every name a block binds, at any depth: `let`s, destructured fields,
// match binders, closures' parameters (a `foreach` or `if`-pattern binds
// through these). By name, as `pinned_names` works.
pub(super) fn bound_names(b: &Block) -> HashSet<String> {
    fn stmts(b: &Block, out: &mut HashSet<String>) {
        for st in &b.stmts {
            match st {
                Stmt::Let { name, .. } => {
                    out.insert(name.clone());
                }
                Stmt::Destructure { fields, .. } => out.extend(fields.iter().cloned()),
                _ => {}
            }
        }
    }
    let mut out = HashSet::new();
    stmts(b, &mut out);
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Block(b) | ExprKind::Unsafe(b) | ExprKind::While(_, b, _) => stmts(b, &mut out),
        ExprKind::If(_, t, _) => stmts(t, &mut out),
        ExprKind::Match(_, arms) => out.extend(arms.iter().filter_map(|a| a.binder.clone())),
        ExprKind::Closure { params, body, .. } => {
            out.extend(params.iter().map(|p| p.name.clone()));
            stmts(body, &mut out);
        }
        _ => {}
    });
    out
}

// `r` (a reference) used only to read through and to hand on to reader
// parameters (`Gen::reader_param`): `r.f…`/`r[i]…`/`*r` read as values;
// `r` itself, or (`field_borrow`) `&r.f…`, as an argument a `c_ok`
// parameter takes. Never otherwise borrowed, sliced, assigned through,
// matched on, captured, rebound or used bare.
pub(super) fn reader_uses(b: &Block, r: &str, field_borrow: bool, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // A projection chain (fields, elements, `*`) down to `r`.
    fn rooted(e: &Expr, r: &str) -> bool {
        match &e.kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => is_r(a, r) || rooted(a, r),
            ExprKind::Paren(a) => rooted(a, r),
            _ => false,
        }
    }
    // The index expressions inside such a chain.
    fn chain_ok(e: &Expr, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        match &e.kind {
            ExprKind::Field(a, _) | ExprKind::Deref(a) | ExprKind::Paren(a) => is_r(a, r) || chain_ok(a, r, fb, c),
            ExprKind::Index(a, i) => (is_r(a, r) || chain_ok(a, r, fb, c)) && ok(i, r, fb, c),
            _ => false,
        }
    }
    fn handed(a: &Expr, r: &str, fb: bool) -> bool {
        is_r(a, r) || (fb && matches!(&a.kind, ExprKind::Borrow(Mode::Shared, x) if rooted(x, r) && !matches!(strip_parens(x).kind, ExprKind::Deref(_) | ExprKind::Index(..)))) || as_view_of(a, r)
    }
    // D-0200: `String::as_view(r)` handed as a `StringView` argument: the
    // callee's parameter decides (`c_ok`: a pure view keeps nothing).
    fn as_view_of(a: &Expr, r: &str) -> bool {
        // CHG-0237: of `r` itself, or of a `String` field of it (`&r.f…`,
        // fields only).
        matches!(&strip_parens(a).kind, ExprKind::Call(c, args) if args.len() == 1 && (is_r(&args[0], r) || matches!(&strip_parens(&args[0]).kind, ExprKind::Borrow(Mode::Shared, y) if field_only(y, r))) && matches!(&c.kind, ExprKind::Path(s, t) if t.is_empty() && matches!(s.join("::").as_str(), "String::as_view" | "std::String::as_view")))
    }
    // CHG-0237: a view of `r`'s text, or of a `String` field of it (`fb`),
    // narrowed by `StringView::sub`/`trim*`; the narrowing's other
    // arguments are judged as any (`o`).
    fn view_chain(e: &Expr, r: &str, fb: bool, o: &dyn Fn(&Expr) -> bool) -> bool {
        let ExprKind::Call(callee, args) = &strip_parens(e).kind else { return false };
        let ExprKind::Path(segs, t) = &callee.kind else { return false };
        if !t.is_empty() {
            return false;
        }
        match (segs.join("::").as_str(), args.as_slice()) {
            ("String::as_view" | "std::String::as_view", [x]) => is_r(x, r) || (fb && matches!(&strip_parens(x).kind, ExprKind::Borrow(Mode::Shared, y) if rooted(y, r) && field_only(y, r))),
            ("StringView::sub" | "std::StringView::sub", [v, lo, hi]) => view_chain(v, r, fb, o) && o(lo) && o(hi),
            ("StringView::trim" | "std::StringView::trim" | "StringView::trim_start" | "std::StringView::trim_start" | "StringView::trim_end" | "std::StringView::trim_end", [v]) => view_chain(v, r, fb, o),
            _ => false,
        }
    }
    // A chain of fields only, down to `r`.
    fn field_only(e: &Expr, r: &str) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) => is_r(a, r) || field_only(a, r),
            _ => false,
        }
    }
    fn ok(e: &Expr, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        let o = |x: &Expr| ok(x, r, fb, c);
        // CHG-0237: `String::from_view(V)` for a view chain `V` over `r`:
        // a copy; the view lives only inside the expression.
        if let ExprKind::Call(callee, args) = &e.kind {
            if let (ExprKind::Path(segs, t), [inner]) = (&callee.kind, args.as_slice()) {
                if t.is_empty() && matches!(segs.join("::").as_str(), "String::from_view" | "std::String::from_view") && view_chain(inner, r, fb, &o) {
                    return true;
                }
            }
        }
        // D-0200: `String::from_view(String::view(r, lo, hi))`: a copy of part
        // of `r`; the view lives only inside the expression and nothing of
        // `r` is kept (`lower_substring` makes it one call).
        if let ExprKind::Call(callee, args) = &e.kind {
            if let (ExprKind::Path(segs, t), [inner]) = (&callee.kind, args.as_slice()) {
                if t.is_empty() && matches!(segs.join("::").as_str(), "String::from_view" | "std::String::from_view") {
                    if let ExprKind::Call(vc, vargs) = &strip_parens(inner).kind {
                        if let (ExprKind::Path(vs, vt), [x, lo, hi]) = (&vc.kind, vargs.as_slice()) {
                            if vt.is_empty() && matches!(vs.join("::").as_str(), "String::view" | "std::String::view") && is_r(x, r) {
                                return o(lo) && o(hi);
                            }
                        }
                    }
                }
            }
        }
        match &e.kind {
            ExprKind::Path(..) => !is_r(e, r),
            // D-0200: a `foreach`'s length (`each.rs`): a read of `r`.
            ExprKind::Call(callee, args) if args.len() == 1 && is_r(&args[0], r) && matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "$each_len") => true,
            ExprKind::Call(callee, args) => {
                let segs = match &callee.kind {
                    ExprKind::Path(s, _) if !is_r(callee, r) => Some(s),
                    _ => None,
                };
                o(callee) && args.iter().enumerate().all(|(i, a)| if handed(a, r, fb) { segs.map_or(false, |s| c(s, i)) } else { o(a) })
            }
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) if rooted(e, r) => chain_ok(e, r, fb, c),
            ExprKind::Borrow(_, a) | ExprKind::SliceOf(_, a, _, _) if is_r(a, r) || rooted(a, r) => false,
            ExprKind::Assign(l, _) if is_r(l, r) || rooted(l, r) => false,
            // CHG-0237: `match (r)` on the reference itself: each arm's
            // binder is a shared alias of part of `r` (D-0046), used only as
            // `r` may be.
            ExprKind::Match(sc, arms) if is_r(sc, r) => arms.iter().all(|a| {
                ok(&a.body, r, fb, c) && a.binder.as_deref().map_or(true, |bn| bn != r && ok(&a.body, bn, fb, c))
            }),
            ExprKind::Match(sc, _) if is_r(sc, r) || rooted(sc, r) || matches!(&sc.kind, ExprKind::Borrow(_, x) if is_r(x, r) || rooted(x, r)) => false,
            ExprKind::Closure { .. } => !mentions(e, r),
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => o(a),
            ExprKind::Binary(_, a, x) | ExprKind::Index(a, x) | ExprKind::Assign(a, x) => o(a) && o(x),
            ExprKind::SliceOf(_, a, lo, hi) => o(a) && o(lo) && o(hi),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| o(a)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| o(a)),
            ExprKind::Block(bl) | ExprKind::Unsafe(bl) => block(bl, r, fb, c),
            ExprKind::If(cd, t, f) => o(cd) && block(t, r, fb, c) && f.as_ref().map_or(true, |f| o(f)),
            ExprKind::While(cd, bl, st) => o(cd) && block(bl, r, fb, c) && st.as_ref().map_or(true, |s| o(s)),
            ExprKind::Match(sc, arms) => o(sc) && arms.iter().all(|a| a.binder.as_deref() != Some(r) && o(&a.body)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| o(v)),
            _ => true,
        }
    }
    // D-0200: `let x = &r.f…;` (shared), or a `foreach` step's element
    // `let x = $each_at(r, i, n);` / `$each_take(r, …)` (`each.rs`): a
    // shared alias of part of `r`, the other arguments evaluated as any
    // are. `x` must itself be used only so in the rest of its block.
    fn alias_of(e: &Expr, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Borrow(Mode::Shared, x) => rooted(x, r) && !matches!(strip_parens(x).kind, ExprKind::Deref(_)) && chain_ok(x, r, fb, c),
            ExprKind::Call(callee, args) => {
                matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && (s[0] == "$each_at" || s[0] == "$each_take"))
                    && args.first().map_or(false, |a| is_r(a, r))
                    && args[1..].iter().all(|a| ok(a, r, fb, c))
            }
            _ => false,
        }
    }
    fn block(b: &Block, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        b.stmts.iter().enumerate().all(|(i, s)| match s {
            Stmt::Let { name, init: Some(e), .. } if name != r && alias_of(e, r, fb, c) => {
                let rest = Block { stmts: b.stmts[i + 1..].to_vec(), tail: b.tail.clone() };
                block(&rest, name, fb, c)
            }
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r, fb, c)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && !is_r(init, r) && ok(init, r, fb, c),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r, fb, c),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r, fb, c))
    }
    block(b, r, field_borrow, c_ok)
}

// D-0200: `r` (an exclusive reference) used only to read and write
// through and to lend on (`Gen::writer_param`): `r.f…`/`r[i]…`/`*r` read
// as values or assigned where `assign_ok` allows; `r`, `&mut *r`, `&*r`,
// `&r.f…` or `&mut r.f…` as an argument a `c_ok` parameter takes, no
// other argument of that call naming `r`. Never otherwise borrowed,
// sliced, matched on, captured, rebound, returned or used bare.
pub(super) fn writer_uses(b: &Block, r: &str, c_ok: &dyn Fn(&[String], usize) -> bool, assign_ok: &dyn Fn(&Expr) -> bool) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    fn rooted(e: &Expr, r: &str) -> bool {
        match &e.kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => is_r(a, r) || rooted(a, r),
            ExprKind::Paren(a) => rooted(a, r),
            _ => false,
        }
    }
    struct C<'c> {
        r: &'c str,
        c: &'c dyn Fn(&[String], usize) -> bool,
        a: &'c dyn Fn(&Expr) -> bool,
    }
    impl C<'_> {
        // The index expressions inside a chain down to `r`.
        fn chain_ok(&self, e: &Expr) -> bool {
            match &e.kind {
                ExprKind::Field(a, _) | ExprKind::Deref(a) | ExprKind::Paren(a) => is_r(a, self.r) || self.chain_ok(a),
                ExprKind::Index(a, i) => (is_r(a, self.r) || self.chain_ok(a)) && self.ok(i),
                _ => false,
            }
        }
        fn handed(&self, a: &Expr) -> bool {
            let r = self.r;
            is_r(a, r)
                || matches!(&strip_parens(a).kind, ExprKind::Borrow(_, x) if matches!(&strip_parens(x).kind, ExprKind::Deref(y) if is_r(y, r)))
                || matches!(&strip_parens(a).kind, ExprKind::Borrow(_, x) if rooted(x, r) && matches!(strip_parens(x).kind, ExprKind::Field(..)) && self.field_chain(x))
        }
        // `r.f.g…` alone (no element, no `*`): what a field borrow names.
        fn field_chain(&self, e: &Expr) -> bool {
            match &strip_parens(e).kind {
                ExprKind::Field(a, _) => is_r(a, self.r) || self.field_chain(a),
                _ => false,
            }
        }
        fn ok(&self, e: &Expr) -> bool {
            let r = self.r;
            match &e.kind {
                ExprKind::Path(..) => !is_r(e, r),
                ExprKind::Call(callee, args) => {
                    let segs = match &callee.kind {
                        ExprKind::Path(s, _) if !is_r(callee, r) => Some(s),
                        _ => None,
                    };
                    let lent: Vec<usize> = (0..args.len()).filter(|&i| self.handed(&args[i])).collect();
                    if lent.len() > 1 {
                        return false;
                    }
                    self.ok(callee) && args.iter().enumerate().all(|(i, a)| if lent.contains(&i) { segs.map_or(false, |s| (self.c)(s, i)) } else { self.ok(a) })
                }
                ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) if rooted(e, r) => self.chain_ok(e),
                ExprKind::Assign(l, v) if rooted(l, r) => (self.a)(l) && self.chain_ok(l) && self.ok(v),
                ExprKind::Assign(l, _) if is_r(l, r) => false,
                ExprKind::Borrow(_, a) | ExprKind::SliceOf(_, a, _, _) if is_r(a, r) || rooted(a, r) => false,
                // `match (&r.f…)`: a shared view of part of `r` for the
                // match, no arm naming `r`, each binder only read
                // (`reader_uses`, lent to readers).
                ExprKind::Match(sc, arms) if matches!(&strip_parens(sc).kind, ExprKind::Borrow(Mode::Shared, x) if rooted(x, r) && self.field_chain(x)) => arms.iter().all(|a| {
                    !mentions(&a.body, r)
                        && a.lit.as_ref().map_or(true, |l| !mentions(l, r))
                        && a.binder.as_ref().map_or(true, |bn| {
                            let blk = Block { stmts: Vec::new(), tail: Some(a.body.clone()) };
                            reader_uses(&blk, bn, false, self.c)
                        })
                }),
                ExprKind::Match(sc, _) if is_r(sc, r) || rooted(sc, r) || matches!(&sc.kind, ExprKind::Borrow(_, x) if is_r(x, r) || rooted(x, r)) => false,
                ExprKind::Closure { .. } => !mentions(e, r),
                ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => self.ok(a),
                ExprKind::Binary(_, a, x) | ExprKind::Index(a, x) | ExprKind::Assign(a, x) => self.ok(a) && self.ok(x),
                ExprKind::SliceOf(_, a, lo, hi) => self.ok(a) && self.ok(lo) && self.ok(hi),
                ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| self.ok(a)),
                ExprKind::ArrayLit(es) => es.iter().all(|a| self.ok(a)),
                ExprKind::Block(bl) | ExprKind::Unsafe(bl) => self.block(bl),
                ExprKind::If(cd, t, f) => self.ok(cd) && self.block(t) && f.as_ref().map_or(true, |f| self.ok(f)),
                ExprKind::While(cd, bl, st) => self.ok(cd) && self.block(bl) && st.as_ref().map_or(true, |s| self.ok(s)),
                ExprKind::Match(sc, arms) => self.ok(sc) && arms.iter().all(|a| a.binder.as_deref() != Some(r) && self.ok(&a.body)),
                ExprKind::Return(v) => v.as_ref().map_or(true, |v| self.ok(v)),
                _ => true,
            }
        }
        fn block(&self, b: &Block) -> bool {
            let r = self.r;
            b.stmts.iter().all(|s| match s {
                // A compound assignment as the parser writes it: `let
                // __compoundN = &mut r.f…; *__compoundN = *__compoundN op e;`,
                // the place plain data (`assign_ok`), as `r.f… op= e` is.
                Stmt::Let { name, init: Some(e), .. } if name.starts_with("__compound") && matches!(&strip_parens(e).kind, ExprKind::Borrow(Mode::Exclusive, x) if rooted(x, r)) => {
                    let ExprKind::Borrow(_, x) = &strip_parens(e).kind else { unreachable!() };
                    (self.a)(x) && self.chain_ok(x)
                }
                Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| self.ok(e)),
                Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && !is_r(init, r) && self.ok(init),
                Stmt::Expr(e) | Stmt::BlockLike(e) => self.ok(e),
            }) && b.tail.as_ref().map_or(true, |e| self.ok(e))
        }
    }
    let c = C { r, c: c_ok, a: assign_ok };
    // A bare `r` handed on as the body's last expression or a `return`
    // would be the reference itself: `ok` refuses a bare `r` everywhere
    // but as a lent argument.
    c.block(b)
}

// Whether `e` names `x` anywhere (a closure's capture included).
pub(super) fn mentions(e: &Expr, x: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |e| match &e.kind {
        ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == x => found = true,
        ExprKind::Closure { captures, .. } if captures.iter().any(|c| c == x) => found = true,
        _ => {}
    });
    found
}

// Every expression in `b`, closures' bodies included.
pub(super) fn visit_exprs(b: &Block, f: &mut dyn FnMut(&Expr)) {
    for st in &b.stmts {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => visit_expr(e, f),
            Stmt::Destructure { init, .. } => visit_expr(init, f),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        visit_expr(t, f);
    }
}

// A loop `while (x < E) body` (or a `for` with that condition and a
// `step`), where `x` is a local whose every access is proven
// (`unchecked`, as `plain` says), and the only write to `x` in `body` and
// `step` is one `x = x + 1` outside any nested loop, and nothing there
// declares another `x`: at that write `x` is unchanged since the test,
// so `x < E <= max`, and `x + 1` cannot overflow. The `+`'s address, for
// `lower_binary` to leave unchecked.
pub(super) fn loop_increment(c: &Expr, body: &Block, step: Option<&Expr>, plain: impl Fn(&str) -> bool) -> Option<usize> {
    let ExprKind::Binary(BinOp::Lt, a, _) = &c.kind else { return None };
    let ExprKind::Path(segs, _) = &a.kind else { return None };
    let [x] = segs.as_slice() else { return None };
    if !plain(x) {
        return None;
    }
    // Every write to `x`, with whether it is inside a nested loop; and
    // whether anything declares a name `x`.
    struct Scan<'a> {
        x: &'a str,
        writes: Vec<(&'a Expr, bool)>,
        declares: bool,
    }
    impl<'a> Scan<'a> {
        fn expr(&mut self, e: &'a Expr, nested: bool) {
            match &e.kind {
                ExprKind::Assign(l, r) => {
                    if matches!(&l.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == self.x) {
                        self.writes.push((e, nested));
                    }
                    self.expr(l, nested);
                    self.expr(r, nested);
                }
                ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => self.expr(a, nested),
                ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                    self.expr(a, nested);
                    self.expr(b, nested);
                }
                ExprKind::SliceOf(_, a, lo, hi) => {
                    self.expr(a, nested);
                    self.expr(lo, nested);
                    self.expr(hi, nested);
                }
                ExprKind::Call(c, args) => {
                    self.expr(c, nested);
                    args.iter().for_each(|a| self.expr(a, nested));
                }
                ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| self.expr(a, nested)),
                ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a, nested)),
                ExprKind::ArrayRepeat(a, _) => self.expr(a, nested),
                ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b, nested),
                ExprKind::If(c, t, f) => {
                    self.expr(c, nested);
                    self.block(t, nested);
                    if let Some(f) = f {
                        self.expr(f, nested);
                    }
                }
                ExprKind::While(c, b, st) => {
                    self.expr(c, true);
                    self.block(b, true);
                    if let Some(st) = st {
                        self.expr(st, true);
                    }
                }
                ExprKind::Match(sc, arms) => {
                    self.expr(sc, nested);
                    for a in arms {
                        if a.binder.as_deref() == Some(self.x) {
                            self.declares = true;
                        }
                        self.expr(&a.body, nested);
                    }
                }
                ExprKind::Return(Some(a)) => self.expr(a, nested),
                ExprKind::Closure { params, body, .. } => {
                    if params.iter().any(|p| p.name == self.x) {
                        self.declares = true;
                    }
                    self.block(body, true);
                }
                _ => {}
            }
        }
        fn block(&mut self, b: &'a Block, nested: bool) {
            for st in &b.stmts {
                match st {
                    Stmt::Let { name, init, .. } => {
                        if name == self.x {
                            self.declares = true;
                        }
                        if let Some(e) = init {
                            self.expr(e, nested);
                        }
                    }
                    Stmt::Destructure { fields, init, .. } => {
                        if fields.iter().any(|f| f == self.x) {
                            self.declares = true;
                        }
                        self.expr(init, nested);
                    }
                    Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e, nested),
                }
            }
            if let Some(t) = &b.tail {
                self.expr(t, nested);
            }
        }
    }
    let mut scan = Scan { x, writes: Vec::new(), declares: false };
    scan.block(body, false);
    if let Some(st) = step {
        scan.expr(st, false);
    }
    let ([(w, false)], false) = (scan.writes.as_slice(), scan.declares) else { return None };
    let ExprKind::Assign(_, rhs) = &w.kind else { return None };
    let ExprKind::Binary(BinOp::Add, p, one) = &rhs.kind else { return None };
    let is_x = matches!(&p.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == *x);
    let is_one = matches!(&one.kind, ExprKind::IntLit(1, _));
    (is_x && is_one).then(|| &**rhs as *const Expr as usize)
}

// The names a body assigns to as a whole (`x = …`): a binding of one of
// them may be given a new value after its old one moved away or ended
// (D-0033 (1)), so its frame is recorded (`Fx::note_frame`). By name,
// ignoring scopes, which only records more than needed.
pub(super) fn assigned_names(b: &Block) -> HashSet<String> {
    let mut out = HashSet::new();
    visit_exprs(b, &mut |e| {
        if let ExprKind::Assign(l, _) = &e.kind {
            if let ExprKind::Path(segs, _) = &l.kind {
                if segs.len() == 1 {
                    out.insert(segs[0].clone());
                }
            }
        }
    });
    out
}

// The names a body borrows (`&x…`, `&mut x…`, including `rawptr_of(&x)`),
// captures (a closure's capture list), or drops (`drop(x…)`): the
// bindings whose checks `rule.control.flow-analysis` may not prove, so
// they keep their runtime objects. By name, ignoring scopes, which only
// errs toward checking. A binding missed here would fail loudly: a
// borrow or drop of a place with no path is an internal error.
// An argument whose type can be read by probing it before its turn: a
// name, or a field, element or borrow of one. A literal would take its
// default type there, and a closure would be formed twice.
pub(super) fn probe_safe(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(..) => true,
        ExprKind::Paren(x) | ExprKind::Field(x, _) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) => probe_safe(x),
        ExprKind::Index(x, i) => probe_safe(x) && matches!(i.kind, ExprKind::Path(..)),
        _ => false,
    }
}

pub(super) fn pinned_names(b: &Block, direct: &dyn Fn(&[String], usize) -> bool) -> HashSet<String> {
    // The binding whose own storage a place is part of. Through `*` the
    // place is the referent's, not the reference variable's: `&*r`
    // borrows what `r` points to, and leaves `r` itself unborrowed.
    fn root(e: &Expr) -> Option<&str> {
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => Some(&segs[0]),
            ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Paren(b) => root(b),
            _ => None,
        }
    }
    fn expr(e: &Expr, out: &mut HashSet<String>, d: &dyn Fn(&[String], usize) -> bool) {
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Break | ExprKind::Continue | ExprKind::Dollar => {}
            ExprKind::Borrow(_, inner) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out, d);
            }
            ExprKind::SliceOf(_, inner, lo, hi) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out, d);
                expr(lo, out, d);
                expr(hi, out, d);
            }
            ExprKind::Call(c, args) => {
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(n) = args.first().and_then(root) {
                            out.insert(n.to_string());
                        }
                    }
                }
                expr(c, out, d);
                for (j, a) in args.iter().enumerate() {
                    // `&x`/`&mut x` passed to a pure direct parameter
                    // (`Gen::pure_direct_ref_arg`), no other argument of the
                    // call naming `x`: no path is minted on `x`, the callee
                    // keeps none, and nothing else reaches `x` while the
                    // call's arguments are evaluated, so it pins nothing.
                    if let (ExprKind::Path(segs, _), ExprKind::Borrow(_, inner)) = (&c.kind, &a.kind) {
                        if let ExprKind::Path(xs, _) = &inner.kind {
                            if let [x] = xs.as_slice() {
                                if d(segs, j) && !args.iter().enumerate().any(|(k, o)| k != j && mentions(o, x)) {
                                    continue;
                                }
                            }
                        }
                    }
                    expr(a, out, d);
                }
            }
            ExprKind::Closure { captures, body, .. } => {
                out.extend(captures.iter().cloned());
                block(body, out, d);
            }
            ExprKind::StructLit(_, _, fields) => fields.iter().for_each(|(_, f)| expr(f, out, d)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|x| expr(x, out, d)),
            ExprKind::ArrayRepeat(x, _) => expr(x, out, d),
            ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => expr(a, out, d),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                expr(a, out, d);
                expr(b, out, d);
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, out, d),
            ExprKind::If(c, t, f) => {
                expr(c, out, d);
                block(t, out, d);
                if let Some(f) = f {
                    expr(f, out, d);
                }
            }
            ExprKind::While(c, b, step) => {
                expr(c, out, d);
                block(b, out, d);
                if let Some(st) = step {
                    expr(st, out, d);
                }
            }
            ExprKind::Match(s, arms) => {
                expr(s, out, d);
                arms.iter().for_each(|a| expr(&a.body, out, d));
            }
            ExprKind::Return(r) => {
                if let Some(r) = r {
                    expr(r, out, d);
                }
            }
        }
    }
    fn block(b: &Block, out: &mut HashSet<String>, d: &dyn Fn(&[String], usize) -> bool) {
        for s in &b.stmts {
            match s {
                Stmt::Let { init, .. } => {
                    if let Some(e) = init {
                        expr(e, out, d);
                    }
                }
                Stmt::Destructure { init, .. } => expr(init, out, d),
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, out, d),
            }
        }
        if let Some(t) = &b.tail {
            expr(t, out, d);
        }
    }
    let mut out = HashSet::new();
    block(b, &mut out, direct);
    out
}

// `spec/15` §6: a capture is exclusive iff the body writes, exclusively
// borrows, drops, or moves a place rooted at it (the interpreter's
// `closure_body_writes`).
pub(super) fn body_writes(body: &Block, name: &str) -> bool {
    fn root_is(e: &Expr, name: &str) -> bool {
        match &e.kind {
            ExprKind::Path(segs, _) => segs.len() == 1 && segs[0] == name,
            ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Deref(b) => root_is(b, name),
            _ => false,
        }
    }
    fn scan(e: &Expr, name: &str, found: &mut bool) {
        match &e.kind {
            ExprKind::Assign(l, r) => {
                if root_is(l, name) {
                    *found = true;
                }
                scan(l, name, found);
                scan(r, name, found);
            }
            ExprKind::Borrow(Mode::Exclusive, inner) => {
                if root_is(inner, name) {
                    *found = true;
                }
                scan(inner, name, found);
            }
            ExprKind::SliceOf(m, inner, lo, hi) => {
                if *m == Mode::Exclusive && root_is(inner, name) {
                    *found = true;
                }
                scan(inner, name, found);
                scan(lo, name, found);
                scan(hi, name, found);
            }
            ExprKind::Borrow(_, inner) | ExprKind::Unary(_, inner) | ExprKind::Deref(inner) | ExprKind::Field(inner, _) | ExprKind::Paren(inner) | ExprKind::Propagate(inner) => scan(inner, name, found),
            ExprKind::Call(c, args) => {
                scan(c, name, found);
                for a in args {
                    scan(a, name, found);
                }
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(a0) = args.first() {
                            if root_is(a0, name) {
                                *found = true;
                            }
                        }
                    }
                }
            }
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                scan(a, name, found);
                scan(b, name, found);
            }
            ExprKind::StructLit(_, _, fields) => {
                for (_, f) in fields {
                    scan(f, name, found);
                }
            }
            ExprKind::ArrayLit(es) => {
                for x in es {
                    scan(x, name, found);
                }
            }
            ExprKind::ArrayRepeat(x, _) => scan(x, name, found),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => scan_block(b, name, found),
            ExprKind::If(c, t, e) => {
                scan(c, name, found);
                scan_block(t, name, found);
                if let Some(e) = e {
                    scan(e, name, found);
                }
            }
            ExprKind::While(c, b, step) => {
                scan(c, name, found);
                scan_block(b, name, found);
                if let Some(st) = step {
                    scan(st, name, found);
                }
            }
            ExprKind::Match(s, arms) => {
                scan(s, name, found);
                for a in arms {
                    scan(&a.body, name, found);
                }
            }
            ExprKind::Return(Some(x)) => scan(x, name, found),
            ExprKind::Closure { body, .. } => scan_block(body, name, found),
            _ => {}
        }
    }
    fn scan_block(b: &Block, name: &str, found: &mut bool) {
        for s in &b.stmts {
            match s {
                Stmt::Expr(e) | Stmt::BlockLike(e) => scan(e, name, found),
                Stmt::Let { init: Some(e), .. } => scan(e, name, found),
                Stmt::Destructure { init, .. } => scan(init, name, found),
                _ => {}
            }
        }
        if let Some(t) = &b.tail {
            scan(t, name, found);
        }
    }
    let mut found = false;
    scan_block(body, name, &mut found);
    found
}

// D-0219: a function whose result is a borrow formed in its own body (its
// value expression, through blocks, `unsafe` blocks and both arms of an
// `if`, is `&_m e`, and it has no `return`): the path it returns is new,
// so nothing can have been borrowed from it yet.
pub(super) fn returns_fresh_borrow(f: &FnDecl) -> bool {
    if !matches!(f.ret, Type::Ref(..) | Type::Slice(..)) {
        return false;
    }
    let mut ret = false;
    visit_exprs(&f.body, &mut |e: &Expr| {
        if matches!(e.kind, ExprKind::Return(_)) {
            ret = true;
        }
    });
    if ret {
        return false;
    }
    fn tail_fresh(b: &Block) -> bool {
        b.tail.as_deref().map_or(false, expr_fresh)
    }
    fn expr_fresh(e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Borrow(..) | ExprKind::SliceOf(..) => true,
            ExprKind::Block(b) | ExprKind::Unsafe(b) => tail_fresh(b),
            ExprKind::If(_, t, Some(e)) => tail_fresh(t) && expr_fresh(e),
            _ => false,
        }
    }
    tail_fresh(&f.body)
}

pub(super) fn strip_parens(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Paren(inner) => strip_parens(inner),
        _ => e,
    }
}

// Whether every use of the name `x` in `b` is `*x` read as a value
// (`Fx::foreach_alias`): not borrowed, assigned, passed as the reference,
// matched in place, captured, or declared again.
// `r` occurs in `b` only as the root of `r.f…` (fields, and indexes of
// array fields) or as `*r`, read or written whole, never under `&` or a
// slice, never handed on, matched on, captured, assigned or redeclared
// (`Gen::direct_ref_params`).
// `v` used in `b` only as `v.bytes[i]` read, `slice_len(v.bytes)`, or a
// whole argument of a call `callee_ok` accepts, named by no other argument
// of it (`Gen::pure_view`).
pub(super) fn view_uses(b: &Block, v: &str, callee_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
    view_uses_fused(b, v, callee_ok, &|_, _| None)
}

// `view_uses`, where `fused(bl, v)` judges a block that is a fused text
// loop over `v` itself (`Gen::fusion_plan`): `Some(verdict)` on its body.
pub(super) fn view_uses_fused(b: &Block, v: &str, callee_ok: &dyn Fn(&[String], usize) -> bool, fused: &dyn Fn(&Block, &str) -> Option<bool>) -> bool {
    fn is_v(e: &Expr, v: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && t.is_empty() && p[0] == v)
    }
    fn bytes_of(e: &Expr, v: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Field(a, f) if f == "bytes" && is_v(a, v))
    }
    fn ex(e: &Expr, v: &str, ok: &dyn Fn(&[String], usize) -> bool, fu: &dyn Fn(&Block, &str) -> Option<bool>) -> bool {
        let x = |e: &Expr| ex(e, v, ok, fu);
        match &e.kind {
            ExprKind::Path(..) => !is_v(e, v),
            ExprKind::Index(a, i) if bytes_of(a, v) => x(i),
            ExprKind::Call(c, args) => {
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.len() == 1 && segs[0] == "slice_len" && args.len() == 1 && bytes_of(&args[0], v) {
                        return true;
                    }
                    return x(c)
                        && args.iter().enumerate().all(|(j, a)| {
                            if is_v(a, v) {
                                ok(segs, j) && !args.iter().enumerate().any(|(k, o)| k != j && mentions(o, v))
                            } else {
                                x(a)
                            }
                        });
                }
                x(c) && args.iter().all(|a| x(a))
            }
            ExprKind::Field(a, _) => !is_v(a, v) && !bytes_of(a, v) && x(a),
            // `&*f(…)` / `&mut *f(…)` (a compound assignment's target, as the
            // parser writes it): a reborrow of the call's result, whose
            // arguments are evaluated as any are.
            ExprKind::Borrow(_, a) if matches!(&strip_parens(a).kind, ExprKind::Deref(inner) if matches!(strip_parens(inner).kind, ExprKind::Call(..))) => x(a),
            ExprKind::Borrow(_, a) | ExprKind::SliceOf(_, a, _, _) if mentions(a, v) => false,
            ExprKind::Closure { .. } => !mentions(e, v),
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => x(a),
            // CHG-0237: `v == t`, `v < t`, … : a comparison reads the text
            // and keeps nothing of it (`[View-Eq]`, `[Cmp-Text]`).
            ExprKind::Binary(BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, a, b) if is_v(a, v) || is_v(b, v) => (is_v(a, v) || x(a)) && (is_v(b, v) || x(b)),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => x(a) && x(b),
            // A target that is `*f(…)` (`*HashMap::entry(…) += 1`): the
            // call's arguments, evaluated as any are; any other target
            // must not name `v`.
            ExprKind::Assign(a, b) => (if matches!(&strip_parens(a).kind, ExprKind::Deref(inner) if matches!(strip_parens(inner).kind, ExprKind::Call(..))) { x(a) } else { !mentions(a, v) }) && x(b),
            ExprKind::SliceOf(_, a, lo, hi) => x(a) && x(lo) && x(hi),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| x(a)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| x(a)),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => fu(b, v).unwrap_or_else(|| blk(b, v, ok, fu)),
            ExprKind::If(c, t, f) => x(c) && blk(t, v, ok, fu) && f.as_ref().map_or(true, |f| x(f)),
            ExprKind::While(c, b, st) => x(c) && blk(b, v, ok, fu) && st.as_ref().map_or(true, |s| x(s)),
            ExprKind::Match(sc, arms) => !mentions(sc, v) && arms.iter().all(|a| a.binder.as_deref() != Some(v) && x(&a.body)),
            ExprKind::Return(r) => r.as_ref().map_or(true, |r| x(r)),
            _ => true,
        }
    }
    fn blk(b: &Block, v: &str, ok: &dyn Fn(&[String], usize) -> bool, fu: &dyn Fn(&Block, &str) -> Option<bool>) -> bool {
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != v && init.as_ref().map_or(true, |e| ex(e, v, ok, fu)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == v) && !mentions(init, v),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ex(e, v, ok, fu),
        }) && b.tail.as_ref().map_or(true, |e| ex(e, v, ok, fu))
    }
    blk(b, v, callee_ok, fused)
}

pub(super) fn direct_uses(b: &Block, r: &str, callee_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&e.kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // Whether `r` is named anywhere in `e`, closures' bodies included.
    fn mentions(e: &Expr, r: &str) -> bool {
        let m = |x: &Expr| mentions(x, r);
        match &e.kind {
            ExprKind::Path(p, _) => p.len() == 1 && p[0] == r,
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => m(a),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => m(a) || m(b),
            ExprKind::SliceOf(_, a, lo, hi) => m(a) || m(lo) || m(hi),
            ExprKind::Call(c, args) => m(c) || args.iter().any(m),
            ExprKind::StructLit(_, _, fs) => fs.iter().any(|(_, a)| m(a)),
            ExprKind::ArrayLit(es) => es.iter().any(m),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block_mentions(b, r),
            ExprKind::If(c, t, f) => m(c) || block_mentions(t, r) || f.as_ref().map_or(false, |f| m(f)),
            ExprKind::While(c, b, st) => m(c) || block_mentions(b, r) || st.as_ref().map_or(false, |x| m(x)),
            ExprKind::Match(sc, arms) => m(sc) || arms.iter().any(|a| m(&a.body)),
            ExprKind::Return(v) => v.as_ref().map_or(false, |v| m(v)),
            ExprKind::Closure { body, .. } => block_mentions(body, r),
            _ => false,
        }
    }
    fn block_mentions(b: &Block, r: &str) -> bool {
        b.stmts.iter().any(|s| match s {
            Stmt::Let { name, init, .. } => name == r || init.as_ref().map_or(false, |e| mentions(e, r)),
            Stmt::Destructure { fields, init, .. } => fields.iter().any(|f| f == r) || mentions(init, r),
            Stmt::Expr(e) | Stmt::BlockLike(e) => mentions(e, r),
        }) || b.tail.as_ref().map_or(false, |e| mentions(e, r))
    }
    // `proj`: `e` is the base of a field or index projection, or the
    // operand of `*`; `lent`: `e` is under `&` or a slice.
    fn ok(e: &Expr, r: &str, proj: bool, lent: bool, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
        let ok = |e: &Expr, r: &str, proj: bool, lent: bool| ok(e, r, proj, lent, c_ok);
        let block_ok = |b: &Block, r: &str| block_ok(b, r, c_ok);
        match &e.kind {
            // `clone(&p)`: for the plain data a direct parameter reaches,
            // the read of `p` it lowers to.
            ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Path(segs, t) if segs.len() == 1 && segs[0] == "clone" && t.is_empty()) && args.len() == 1 && matches!(&args[0].kind, ExprKind::Borrow(Mode::Shared, _)) => {
                let ExprKind::Borrow(_, place) = &args[0].kind else { unreachable!() };
                ok(place, r, false, false)
            }
            // `slice_len(r)`: the length the slice carries.
            ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "slice_len") && args.len() == 1 && is_r(&args[0], r) => true,
            // D-0254: `simd_load(r, i)`, `simd_store(r, i, v)`: elements of
            // the slice reached in place, as `r[i]` reaches one.
            ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && (segs[0] == "simd_load" || segs[0] == "simd_store")) && args.first().map_or(false, |a| is_r(a, r)) => {
                args[1..].iter().all(|a| ok(a, r, false, false))
            }
            // `f(…, r, …)`: handed on whole (`Gen::direct_ref_params`).
            ExprKind::Call(c, args)
                if matches!(&c.kind, ExprKind::Path(segs, _) if args.iter().any(|a| is_r(a, r)) && args.iter().enumerate().all(|(i, a)| !is_r(a, r) || c_ok(segs, i))) =>
            {
                args.iter().all(|a| is_r(a, r) || ok(a, r, false, false))
            }
            ExprKind::Path(..) => !is_r(e, r) || (proj && !lent),
            ExprKind::Field(a, _) => ok(a, r, true, lent),
            ExprKind::Index(a, i) => ok(a, r, true, lent) && ok(i, r, false, false),
            ExprKind::Deref(a) => ok(a, r, true, lent),
            ExprKind::Borrow(_, a) => ok(a, r, false, true),
            ExprKind::SliceOf(_, a, lo, hi) => ok(a, r, false, true) && ok(lo, r, false, false) && ok(hi, r, false, false),
            ExprKind::Paren(a) => ok(a, r, proj, lent),
            ExprKind::Unary(_, a) | ExprKind::Propagate(a) => ok(a, r, false, lent),
            ExprKind::Binary(_, a, c) => ok(a, r, false, false) && ok(c, r, false, false),
            ExprKind::Assign(l, v) => ok(l, r, false, false) && ok(v, r, false, false),
            ExprKind::Call(c, args) => ok(c, r, false, false) && args.iter().all(|a| ok(a, r, false, false)),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| ok(a, r, false, false)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| ok(a, r, false, false)),
            ExprKind::ArrayRepeat(a, _) => ok(a, r, false, false),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block_ok(b, r),
            ExprKind::If(c, t, f) => ok(c, r, false, false) && block_ok(t, r) && f.as_ref().map_or(true, |f| ok(f, r, false, false)),
            ExprKind::While(c, b, st) => ok(c, r, false, false) && block_ok(b, r) && st.as_ref().map_or(true, |s| ok(s, r, false, false)),
            ExprKind::Match(sc, arms) => !mentions(sc, r) && arms.iter().all(|a| a.binder.as_deref() != Some(r) && ok(&a.body, r, false, false)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| ok(v, r, false, false)),
            ExprKind::Closure { .. } => !mentions(e, r),
            _ => !mentions(e, r),
        }
    }
    fn block_ok(b: &Block, r: &str, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
        let ok = |e: &Expr, r: &str, proj: bool, lent: bool| ok(e, r, proj, lent, c_ok);
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r, false, false)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && ok(init, r, false, false),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r, false, false),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r, false, false))
    }
    block_ok(b, r, callee_ok)
}

// `r` is used in `b` only as (`Gen::direct_refs_of`, tree-design.md):
// - `r.f…` read as a value (fields of fields too),
// - `r.f… = e` with `e` not naming `r`,
// - the scrutinee `&r.f` / `&mut r.f` of a `match` (an `if`-pattern is
//   one), each of whose arms either binds nothing or uses its binder `b`
//   once, first, as `Box::get(b)` / `Box::get_mut(b)` handed as the first
//   argument of a named call whose other arguments are literals or
//   locals; the arms' bodies otherwise follow these same rules.
// Nothing else names `r`: it is never handed on, borrowed otherwise,
// dereferenced whole, captured, assigned or redeclared.
pub(super) fn box_rec_uses(b: &Block, r: &str) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // `r.f…`.
    fn field_of_r(e: &Expr, r: &str) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) => is_r(a, r) || field_of_r(a, r),
            _ => false,
        }
    }
    fn names(e: &Expr, r: &str) -> bool {
        let mut n = false;
        let blk = Block { stmts: Vec::new(), tail: Some(Box::new(e.clone())) };
        visit_exprs(&blk, &mut |x| {
            if is_r(x, r) {
                n = true;
            }
        });
        n
    }
    fn inert(e: &Expr) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_)) || path_name(e).map_or(false, |n| !n.starts_with("__"))
    }
    fn arm_ok(a: &Arm, r: &str) -> bool {
        match &a.binder {
            None => ok(&a.body, r),
            Some(bn) => {
                // `f(Box::get[_mut](b), inert…)`, possibly as `x = x op …`
                // (`binder_box_call`), `b` used once, first.
                let g = binder_box_call(&a.body, bn);
                if !(binder_used_once_first(&a.body, bn) && matches!(g.as_deref(), Some("std::Box::get" | "std::Box::get_mut"))) {
                    return false;
                }
                // The outer call: `r` appears nowhere in the arm.
                !names(&a.body, r)
            }
        }
    }
    fn ok(e: &Expr, r: &str) -> bool {
        let e = strip_parens(e);
        match &e.kind {
            _ if !names(e, r) => true,
            ExprKind::Field(..) if field_of_r(e, r) => true,
            ExprKind::Assign(l, v) if field_of_r(l, r) => !names(v, r) || ok(v, r) && !names(v, r),
            ExprKind::Match(sc, arms) => {
                let scrut_ok = match &strip_parens(sc).kind {
                    ExprKind::Borrow(_, pl) => field_of_r(pl, r),
                    _ => !names(sc, r),
                };
                scrut_ok && arms.iter().all(|a| arm_ok(a, r))
            }
            ExprKind::Binary(_, a, c) => ok(a, r) && ok(c, r),
            ExprKind::Unary(_, a) => ok(a, r),
            ExprKind::Assign(l, v) => !names(l, r) && ok(v, r),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, r),
            ExprKind::If(c, t, f) => ok(c, r) && block(t, r) && f.as_ref().map_or(true, |f| ok(f, r)),
            ExprKind::While(c, b, st) => ok(c, r) && block(b, r) && st.as_ref().map_or(true, |s| ok(s, r)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| ok(v, r)),
            _ => false,
        }
    }
    fn block(b: &Block, r: &str) -> bool {
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && !names(init, r),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r))
    }
    let _ = inert;
    block(b, r)
}

// `let $cN = &v;` opening a `foreach (x in &v)`'s expansion (`parse_foreach`)
// whose element binding `x` the body uses only as `*x` read as a value:
// the shape `Fx::foreach_alias` lowers to bounds-checked reads of `v`.
pub(super) fn foreach_reads_only(name: &str, init: &Expr, b: &Block) -> bool {
    if !name.starts_with("$c") {
        return false;
    }
    let ExprKind::Borrow(mode, inner) = &init.kind else { return false };
    let mutable = *mode == Mode::Exclusive;
    // A local, or a field of one or of a reference (`&h.v`, `&w.items`,
    // `&(*w).items`): the holder `$c` holds that place for the whole loop.
    let mut root: &Expr = inner;
    while let ExprKind::Field(b, _) | ExprKind::Paren(b) = &root.kind {
        root = b;
    }
    if !std::ptr::eq(root, &**inner) {
        if let ExprKind::Deref(p) = &root.kind {
            root = strip_parens(p);
        }
    }
    if !matches!(&root.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() && !segs[0].starts_with('$')) {
        return false;
    }
    let Some(tail) = &b.tail else { return false };
    let ExprKind::While(_, inner_b, _) = &tail.kind else { return false };
    let n = inner_b.stmts.len();
    if n < 2 {
        return false;
    }
    let (Stmt::Let { ty: None, name: x, init: Some(at), .. }, Stmt::BlockLike(body)) = (&inner_b.stmts[n - 2], &inner_b.stmts[n - 1]) else { return false };
    let ExprKind::Call(callee, args) = &at.kind else { return false };
    let holder_is = |e: &Expr| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == name);
    let at_fn = if mutable { "$each_at_mut" } else { "$each_at" };
    if !matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == at_fn) || args.len() != 3 || !holder_is(&args[0]) {
        return false;
    }
    let ExprKind::Block(body) = &body.kind else { return false };
    block_only_derefs(body, x, mutable)
}

// `foreach (x in p)` written without `&`, `p` one of `params`:
// `{ auto $e = p; auto $d = $each_drain($e, 1); for (…) { auto x = $each_take($d, $i, 1); body } }`
// with `body` reading `x` only as `*x` or `x.f…` (`block_only_derefs`).
pub(super) fn foreach_param_reads_only(name: &str, init: &Expr, b: &Block, params: &HashSet<String>) -> bool {
    if !name.starts_with("$e") {
        return false;
    }
    if !matches!(&init.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() && params.contains(&segs[0])) {
        return false;
    }
    let is = |e: &Expr, n: &str| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == n);
    let holder = b.stmts.iter().find_map(|st| match st {
        Stmt::Let { name: d, init: Some(di), .. } if d.starts_with("$d") => match &di.kind {
            ExprKind::Call(c, a) if is(c, "$each_drain") && a.first().map_or(false, |a0| is(a0, name)) => Some(d.clone()),
            _ => None,
        },
        _ => None,
    });
    let Some(holder) = holder else { return false };
    let Some(tail) = &b.tail else { return false };
    let ExprKind::While(_, inner_b, _) = &tail.kind else { return false };
    let [Stmt::Let { ty: None, name: x, init: Some(at), .. }, Stmt::BlockLike(body)] = inner_b.stmts.as_slice() else { return false };
    let ExprKind::Call(callee, args) = &at.kind else { return false };
    if !is(callee, "$each_take") || args.len() != 3 || !is(&args[0], &holder) {
        return false;
    }
    let ExprKind::Block(body) = &body.kind else { return false };
    block_only_derefs(body, x, false)
}

// `x` used only as `*x` read as a value; with `w`, also the whole place written,
// `*x = e` (which `*x op= e` is), or one of its fields, `x.f… = e`: `x` used
// only as `*x`, read or assigned.
pub(super) fn block_only_derefs(b: &Block, x: &str, w: bool) -> bool {
    stmts_only_derefs(&b.stmts, b.tail.as_deref(), x, w)
}

pub(super) fn stmts_only_derefs(stmts: &[Stmt], tail: Option<&Expr>, x: &str, w: bool) -> bool {
    stmts.iter().all(|s| match s {
        Stmt::Let { name, init, .. } => name != x && init.as_ref().map_or(true, |e| only_reads_w(e, x, false, w)),
        Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == x) && only_reads_w(init, x, true, w),
        Stmt::Expr(e) | Stmt::BlockLike(e) => only_reads_w(e, x, false, w),
    }) && tail.map_or(true, |e| only_reads_w(e, x, false, w))
}

pub(super) fn only_reads_w(e: &Expr, x: &str, place: bool, w: bool) -> bool {
    let only_reads = |e: &Expr, x: &str, place: bool| only_reads_w(e, x, place, w);
    let block_only_reads = |b: &Block, x: &str| block_only_derefs(b, x, w);
    let is_x = |e: &Expr| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == x);
    match &e.kind {
        ExprKind::Path(..) => !is_x(e),
        ExprKind::Deref(inner) if is_x(inner) => !place,
        ExprKind::Deref(a) | ExprKind::Paren(a) | ExprKind::Unary(_, a) | ExprKind::Propagate(a) => only_reads(a, x, place),
        // `x.f…` read as a value: `(*x).f…`, a field of the element read
        // (`Fx::alias_field_chain`).
        ExprKind::Field(..) if !place && field_chain_root(e).map_or(false, is_x) => true,
        ExprKind::Field(a, _) => only_reads(a, x, place),
        ExprKind::Borrow(_, a) => only_reads(a, x, true),
        ExprKind::Assign(l, r) if w && matches!(&strip_parens(l).kind, ExprKind::Deref(t) if is_x(t)) => only_reads(r, x, false),
        // `x.f… = e` (and `x.f… op= e`): a field of the element written in
        // place (`Fx::alias_field_chain`), as `*x = e` writes it whole.
        ExprKind::Assign(l, r) if w && field_chain_root(l).map_or(false, |t| is_x(t) || matches!(&strip_parens(t).kind, ExprKind::Deref(d) if is_x(d))) => {
            only_reads(r, x, false)
        }
        ExprKind::Assign(l, r) => only_reads(l, x, true) && only_reads(r, x, false),
        ExprKind::Index(a, i) => only_reads(a, x, true) && only_reads(i, x, false),
        ExprKind::SliceOf(_, a, lo, hi) => only_reads(a, x, true) && only_reads(lo, x, false) && only_reads(hi, x, false),
        ExprKind::Binary(_, a, b) => only_reads(a, x, false) && only_reads(b, x, false),
        ExprKind::Call(c, args) => {
            // `drop(e)` takes its operand in place position; every other
            // call takes values.
            let in_place = matches!(&c.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "drop");
            only_reads(c, x, false) && args.iter().all(|a| only_reads(a, x, in_place))
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| only_reads(a, x, false)),
        ExprKind::ArrayLit(es) => es.iter().all(|a| only_reads(a, x, false)),
        ExprKind::ArrayRepeat(a, _) => only_reads(a, x, false),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block_only_reads(b, x),
        ExprKind::If(c, t, f) => only_reads(c, x, false) && block_only_reads(t, x) && f.as_ref().map_or(true, |f| only_reads(f, x, false)),
        ExprKind::While(c, b, st) => only_reads(c, x, false) && block_only_reads(b, x) && st.as_ref().map_or(true, |s| only_reads(s, x, false)),
        ExprKind::Match(sc, arms) => only_reads(sc, x, true) && arms.iter().all(|a| a.binder.as_deref() != Some(x) && only_reads(&a.body, x, false)),
        ExprKind::Return(v) => v.as_ref().map_or(true, |v| only_reads(v, x, false)),
        ExprKind::Closure { .. } => false,
        _ => true,
    }
}


// `auto g = lock(m);` (D-0202 phase 4, `Fx::lower_bare_guard`) followed
// to the end of its block by code that uses `g` only as `*g` or `g.f…`,
// read or written (`block_only_derefs`), names nothing `m` names (the
// mutex is neither moved nor ended meanwhile), and cannot leave the block
// early (no `return`, `?`, or `break`/`continue` of an enclosing loop),
// so the unlock at the block's end always runs. The name, the mutex
// expression and the line.
pub(super) fn bare_guard_let<'e>(s: &'e Stmt, rest: &[Stmt], tail: Option<&Expr>) -> Option<(&'e str, &'e Expr, usize)> {
    let Stmt::Let { name, ty: None, init: Some(init), line, .. } = s else { return None };
    let ExprKind::Call(callee, args) = &init.kind else { return None };
    if !matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "lock") || args.len() != 1 {
        return None;
    }
    if !stmts_only_derefs(rest, tail, name, true) {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    visit_expr(&args[0], &mut |e| {
        if let ExprKind::Path(p, _) = &e.kind {
            if p.len() == 1 {
                names.push(p[0].clone());
            }
        }
    });
    let rest_block = Block { stmts: rest.to_vec(), tail: tail.map(|t| Box::new(t.clone())) };
    let mut ok = true;
    visit_exprs(&rest_block, &mut |e| match &e.kind {
        ExprKind::Path(p, _) if p.len() == 1 && names.contains(&p[0]) => ok = false,
        ExprKind::Return(_) | ExprKind::Propagate(_) => ok = false,
        _ => {}
    });
    if !ok || leaves_loop(&rest_block) {
        return None;
    }
    Some((name.as_str(), &args[0], *line))
}

// Each expression directly inside `e`, for the kinds no walker above
// treats specially (`leaves_loop`).
pub(super) fn visit_children(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => f(a),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            f(a);
            f(b);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            f(a);
            f(lo);
            f(hi);
        }
        ExprKind::Call(c, args) => {
            f(c);
            args.iter().for_each(|a| f(a));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| f(a)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| f(a)),
        ExprKind::Return(Some(v)) => f(v),
        _ => {}
    }
}

// Whether `b` has a `break` or `continue` outside every loop within it
// (one that leaves an enclosing loop).
pub(super) fn leaves_loop(b: &Block) -> bool {
    fn ex(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Break | ExprKind::Continue => true,
            ExprKind::While(c, _, _) => ex(c),
            ExprKind::Closure { .. } => false,
            ExprKind::Block(b) | ExprKind::Unsafe(b) => leaves_loop(b),
            ExprKind::If(c, t, f) => ex(c) || leaves_loop(t) || f.as_ref().map_or(false, |f| ex(f)),
            ExprKind::Match(sc, arms) => ex(sc) || arms.iter().any(|a| ex(&a.body)),
            _ => {
                let mut found = false;
                visit_children(e, &mut |c| found |= ex(c));
                found
            }
        }
    }
    b.stmts.iter().any(|st| match st {
        Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => ex(e),
        Stmt::Destructure { init, .. } => ex(init),
        Stmt::Let { init: None, .. } => false,
    }) || b.tail.as_ref().map_or(false, |t| ex(t))
}

// `ref<T, m> p = &m x[i];` (or `auto p = &m x[i];`), `x` a local or a
// reference parameter (`p[i]`, `(*p)[i]`), followed to the end of its
// block (`rest`, `tail`) by code that never names `x` and uses `p` only as
// `*p` or `p.f…`, read, or written when `m` is exclusive
// (`block_only_derefs`). For the rest of the block `p` is the element's
// address and is the only way to `x`: `p`, the borrow `&m x[i]` and the
// index `i`, for `Gen::scan_vec_uses` and `Fx::bind_elem_ref`.
pub(super) fn elem_ref_let<'e>(s: &'e Stmt, rest: &[Stmt], tail: Option<&Expr>) -> Option<(&'e str, &'e Expr, Mode, &'e Expr)> {
    let Stmt::Let { name, ty, init: Some(init), .. } = s else { return None };
    if !matches!(ty, None | Some(Type::Ref(..))) {
        return None;
    }
    let ExprKind::Borrow(mode, place) = &init.kind else { return None };
    let ExprKind::Index(a, i) = &strip_parens(place).kind else { return None };
    let bare = |e: &'e Expr| match &strip_parens(e).kind {
        ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(segs[0].as_str()),
        _ => None,
    };
    let x = match &strip_parens(a).kind {
        ExprKind::Deref(p) => bare(p)?,
        _ => bare(a)?,
    };
    if x == name || x.starts_with('$') || name.starts_with('$') {
        return None;
    }
    let w = *mode == Mode::Exclusive;
    let names_x = rest.iter().any(|st| match st {
        Stmt::Let { name: n, init, .. } => n == x || init.as_ref().map_or(false, |e| mentions(e, x)),
        Stmt::Destructure { fields, init, .. } => fields.iter().any(|f| f == x) || mentions(init, x),
        Stmt::Expr(e) | Stmt::BlockLike(e) => mentions(e, x),
    }) || tail.map_or(false, |t| mentions(t, x));
    (!names_x && stmts_only_derefs(rest, tail, name, w)).then_some((name.as_str(), &**place, mode.clone(), &**i))
}


// The root of a chain of field accesses (`a.b.c` -> `a`), through parentheses.
pub(super) fn field_chain_root(e: &Expr) -> Option<&Expr> {
    let mut a = e;
    while let ExprKind::Field(b, _) | ExprKind::Paren(b) = &a.kind {
        a = b;
    }
    if std::ptr::eq(a, e) {
        None
    } else {
        Some(a)
    }
}

// Argument `j` of a call being lowered: appended in order, or put in the
// slot a deferred literal held (D-0079).
pub(super) fn put_arg(argv: &mut Vec<V>, j: usize, v: V) {
    if j < argv.len() {
        argv[j] = v;
    } else {
        argv.push(v);
    }
}

// An unsuffixed numeric literal, possibly negated or parenthesized.
pub(super) fn is_bare_num_literal(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => is_bare_num_literal(inner),
        ExprKind::Paren(inner) => is_bare_num_literal(inner),
        _ => false,
    }
}

// D-0110: the referent of a key reference that is a struct or enum.
pub(super) fn composite_key(t: &Type) -> Option<Type> {
    match t {
        Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n != "std::String") => Some((**inner).clone()),
        _ => None,
    }
}

// D-0109: the mode of the references a pattern looks through: shared
// if any is.
pub(super) fn crossing_mode(crosses: &[(usize, Mode, Type)]) -> Option<Mode> {
    if crosses.is_empty() {
        None
    } else if crosses.iter().any(|c| c.1 == Mode::Shared) {
        Some(Mode::Shared)
    } else {
        Some(Mode::Exclusive)
    }
}

// D-0108: an expression `lower_place` reaches without a temporary: a
// binding, `*r`, or a field or element of one.
pub(super) fn is_place_syntax(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(segs, _) => segs.len() == 1,
        ExprKind::Deref(_) => true,
        ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Paren(b) => is_place_syntax(b),
        _ => false,
    }
}

// A place written as such: a binding, a field, an element, a dereference.
pub(super) fn is_place_expr(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(segs, t) => segs.len() == 1 && t.is_empty(),
        ExprKind::Field(a, _) | ExprKind::Paren(a) => is_place_expr(a),
        ExprKind::Index(a, _) => is_place_expr(a),
        ExprKind::Deref(_) => true,
        _ => false,
    }
}

// Whether the place's root name is a binding (not a constant or an item).
pub(super) fn root_bound(e: &Expr, bound: &dyn Fn(&str) -> bool) -> bool {
    match &e.kind {
        ExprKind::Path(segs, _) => segs.len() == 1 && bound(&segs[0]),
        ExprKind::Field(a, _) | ExprKind::Paren(a) | ExprKind::Index(a, _) => root_bound(a, bound),
        ExprKind::Deref(_) => true,
        _ => false,
    }
}

// `r` never the base of a projection or the operand of `*`: used, if at
// all, whole (`Gen::direct_refs_of`'s `Vec` case; `direct_uses` decides
// where a whole use may go).
pub(super) fn only_handed_on(b: &Block, r: &str) -> bool {
    let is_r = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty());
    let mut ok = true;
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) if is_r(a) => ok = false,
        ExprKind::SliceOf(_, a, _, _) if is_r(a) => ok = false,
        _ => {}
    });
    ok
}

// Whether `name` is named anywhere in `b` (closures' bodies included).
pub(super) fn block_names(b: &Block, name: &str) -> bool {
    let mut found = false;
    visit_exprs(b, &mut |e| {
        if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
            found = true;
        }
    });
    found
}

// Every statement in `b`, nested blocks' and closures' included.
// `Gen::quiet_locals`' walk: every mention of `x` in `b` a read through it
// (`x.f…`, `x[i]…`, as a value of a part or a place assigned), a shared
// borrow of it or of a part, or an exclusive borrow of it or of a part
// as a call's argument where `call_ok` (callee, position) allows, or the
// initializer of a proven element reference (`elem_refs`). Never bare
// (a move), never matched on by value, never declared again.
pub(super) fn quiet_block(b: &Block, x: &str, call_ok: &dyn Fn(&[String], usize) -> bool, elem_refs: &HashSet<usize>) -> bool {
    fn is_x(e: &Expr, x: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == x && t.is_empty())
    }
    fn rooted(e: &Expr, x: &str) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) => is_x(a, x) || rooted(a, x),
            _ => false,
        }
    }
    // A place rooted at `x` (or `x` itself): its index expressions.
    fn place_ok(e: &Expr, x: &str, c: &dyn Fn(&[String], usize) -> bool, er: &HashSet<usize>) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Path(..) => true,
            ExprKind::Field(a, _) => place_ok(a, x, c, er),
            ExprKind::Index(a, i) => place_ok(a, x, c, er) && ex(i, x, c, er),
            _ => false,
        }
    }
    fn ex(e: &Expr, x: &str, c: &dyn Fn(&[String], usize) -> bool, er: &HashSet<usize>) -> bool {
        let o = |a: &Expr| ex(a, x, c, er);
        match &e.kind {
            ExprKind::Path(..) => !is_x(e, x),
            ExprKind::Field(..) | ExprKind::Index(..) if rooted(e, x) => place_ok(e, x, c, er),
            ExprKind::Borrow(Mode::Shared, a) if is_x(a, x) || rooted(a, x) => place_ok(a, x, c, er),
            ExprKind::Borrow(_, a) | ExprKind::SliceOf(_, a, _, _) if is_x(a, x) || rooted(a, x) => false,
            ExprKind::Call(callee, args) => {
                let segs = match &callee.kind {
                    ExprKind::Path(s, _) => Some(s),
                    _ => None,
                };
                o(callee)
                    && args.iter().enumerate().all(|(i, a)| match &strip_parens(a).kind {
                        ExprKind::Borrow(Mode::Exclusive, p) if is_x(p, x) || rooted(p, x) => segs.map_or(false, |s| c(s, i)) && place_ok(p, x, c, er),
                        _ => o(a),
                    })
            }
            ExprKind::Assign(l, _) if is_x(l, x) => false,
            ExprKind::Assign(l, v) if rooted(l, x) => place_ok(l, x, c, er) && o(v),
            ExprKind::Match(sc, arms) => {
                !is_x(sc, x) && o(sc) && arms.iter().all(|a| a.binder.as_deref() != Some(x) && o(&a.body) && a.lit.as_ref().map_or(true, |l| o(l)))
            }
            ExprKind::Closure { .. } => !mentions(e, x),
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => o(a),
            ExprKind::Binary(_, a, b2) | ExprKind::Index(a, b2) | ExprKind::Assign(a, b2) => o(a) && o(b2),
            ExprKind::SliceOf(_, a, lo, hi) => o(a) && o(lo) && o(hi),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| o(a)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| o(a)),
            ExprKind::Block(bl) | ExprKind::Unsafe(bl) => blk(bl, x, c, er),
            ExprKind::If(cd, t, f) => o(cd) && blk(t, x, c, er) && f.as_ref().map_or(true, |f| o(f)),
            ExprKind::While(cd, bl, st) => o(cd) && blk(bl, x, c, er) && st.as_ref().map_or(true, |s| o(s)),
            ExprKind::Return(r) => r.as_ref().map_or(true, |r| o(r)),
            _ => true,
        }
    }
    fn blk(b: &Block, x: &str, c: &dyn Fn(&[String], usize) -> bool, er: &HashSet<usize>) -> bool {
        b.stmts.iter().all(|s| match s {
            // The local's own declaration: its initializer must not name it.
            Stmt::Let { name, init: Some(e), .. } if name == x => !mentions(e, x) && ex(e, x, c, er),
            Stmt::Let { name, .. } if name == x => false,
            // A proven element reference (`elem_ref_lets`): no path.
            Stmt::Let { init: Some(e), .. } if er.contains(&(&**e as *const Expr as usize)) => {
                let ExprKind::Borrow(_, p) = &strip_parens(e).kind else { return false };
                place_ok(p, x, c, er) || !mentions(e, x)
            }
            Stmt::Let { init, .. } => init.as_ref().map_or(true, |e| ex(e, x, c, er)),
            Stmt::Destructure { init, .. } => !is_x(init, x) && ex(init, x, c, er),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ex(e, x, c, er),
        }) && b.tail.as_ref().map_or(true, |e| ex(e, x, c, er))
    }
    blk(b, x, call_ok, elem_refs)
}

// `name`, declared by statement `at` of `blk`, declared nowhere after it
// and captured by no closure there.
pub(super) fn held_names_ok(blk: &Block, at: usize, name: &str) -> bool {
    let rest = Block { stmts: blk.stmts[at + 1..].to_vec(), tail: blk.tail.clone() };
    let mut ok = true;
    visit_stmts(&rest, &mut |s| {
        if matches!(s, Stmt::Let { name: n, .. } if n == name) {
            ok = false;
        }
    });
    visit_exprs(&rest, &mut |e| {
        if let ExprKind::Closure { captures, .. } = &e.kind {
            if captures.iter().any(|c| c == name) {
                ok = false;
            }
        }
    });
    ok
}

// Every block in `b` and `b` itself, closures' bodies included.
pub(super) fn visit_blocks(b: &Block, f: &mut dyn FnMut(&Block)) {
    f(b);
    let mut blocks: Vec<*const Block> = Vec::new();
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Block(x) | ExprKind::Unsafe(x) => blocks.push(x as *const Block),
        ExprKind::Closure { body, .. } => blocks.push(&**body as *const Block),
        ExprKind::If(_, x, _) | ExprKind::While(_, x, _) => blocks.push(x as *const Block),
        _ => {}
    });
    for x in blocks {
        f(unsafe { &*x });
    }
}

// `x[i]…` (elements, then fields, of elements of …) down to a one-segment
// name `x`: that name.
pub(super) fn elem_chain_root(e: &Expr) -> Option<&str> {
    match &strip_parens(e).kind {
        ExprKind::Index(a, _) => match &strip_parens(a).kind {
            ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => Some(s[0].as_str()),
            _ => elem_chain_root(a),
        },
        ExprKind::Field(a, _) => elem_chain_root(a),
        _ => None,
    }
}

pub(super) fn visit_stmts(b: &Block, f: &mut dyn FnMut(&Stmt)) {
    fn walk_b(b: &Block, f: &mut dyn FnMut(&Stmt)) {
        for st in &b.stmts {
            f(st);
        }
    }
    walk_b(b, f);
    let mut blocks: Vec<*const Block> = Vec::new();
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Block(x) | ExprKind::Unsafe(x) => blocks.push(x as *const Block),
        ExprKind::Closure { body, .. } => blocks.push(&**body as *const Block),
        ExprKind::If(_, x, _) | ExprKind::While(_, x, _) => blocks.push(x as *const Block),
        _ => {}
    });
    for x in blocks {
        walk_b(unsafe { &*x }, f);
    }
}
