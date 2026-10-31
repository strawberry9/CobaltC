// The scan behind `Gen::scan_vec_uses` (`confined_vecs`, `confining_table`).

use super::*;

#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    Value,
    Place,
}
struct Scan<'s, 'a> {
    g: &'s Gen<'a>,
    table: &'s HashMap<String, Vec<bool>>,
    declared: HashSet<String>,
    bad: HashSet<String>,
    // Every name the function binds (`bound_names`): a call by a
    // single name is to the function of that name only if no
    // binding of it (a closure, a `fn` value) could be called.
    bound: HashSet<String>,
    params: HashSet<String>,
}
const PUSH: &str = "std::Vec::push";
const POP: &str = "std::Vec::pop";
const RESERVE: &str = "std::Vec::reserve";
const LEN: &str = "std::Vec::len";
fn call_name(e: &Expr) -> Option<(String, &[Expr])> {
    match &e.kind {
        ExprKind::Call(callee, args) => match &callee.kind {
            ExprKind::Path(segs, _) => Some((segs.join("::"), args.as_slice())),
            _ => None,
        },
        _ => None,
    }
}
fn bare(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(&segs[0]),
        _ => None,
    }
}
// A vector passed as a reference argument: `&x`/`&mut x` (a
// local), or `p`/`&*p`/`&mut *p` (a reference parameter).
fn vec_arg(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Borrow(_, inner) => match &inner.kind {
            ExprKind::Deref(p) => bare(p),
            _ => bare(inner),
        },
        _ => bare(e),
    }
}
// `x[i]` (`[Index-Vec]`, D-0047): `*Vec::index_*(&x, i)` for a `Vec`
// `x` (which `bind_confined` checks); `x` a local or a reference
// parameter (`p[i]`, `(*p)[i]`), and the index.
fn index_expr_on_local(e: &Expr) -> Option<(&str, &Expr)> {
    let ExprKind::Index(a, i) = &e.kind else { return None };
    let mut a: &Expr = a;
    while let ExprKind::Paren(x) = &a.kind {
        a = x;
    }
    let x = match &a.kind {
        ExprKind::Deref(p) => bare(p)?,
        _ => bare(a)?,
    };
    Some((x, i))
}
// `Vec::index_*(&x, i)`: `x` and the index.
fn index_on_local(e: &Expr) -> Option<(&str, &Expr)> {
    let (name, args) = call_name(e)?;
    if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
        return None;
    }
    let [a0, a1] = args else { return None };
    Some((vec_arg(a0)?, a1))
}
// Whether `e` may move `x`'s buffer or shorten it, which an
// element's address taken before `e` runs would not survive: `x`
// handed to any call but as a shared borrow (`&x`, `&*x`) or to
// `Vec::len` / `Vec::index_shared` -- `push`, `pop`, `reserve`, or
// a helper that does one of them through a confining parameter.
fn pushes_onto(e: &Expr, x: &str) -> bool {
    let mut found = false;
    walk(e, &mut |e| {
        if let Some((name, args)) = call_name(e) {
            if name == LEN || name == "std::Vec::index_shared" {
                return;
            }
            for a in args {
                if vec_arg(a) == Some(x) && !matches!(&a.kind, ExprKind::Borrow(Mode::Shared, _)) {
                    found = true;
                }
            }
        }
    });
    found
}
// Every expression in `e`, closures' bodies included.
fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => walk(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            walk(a, f);
            walk(b, f);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            walk(a, f);
            walk(lo, f);
            walk(hi, f);
        }
        ExprKind::Call(c, args) => {
            walk(c, f);
            args.iter().for_each(|a| walk(a, f));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| walk(a, f)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| walk(a, f)),
        ExprKind::ArrayRepeat(a, _) => walk(a, f),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => walk_block(b, f),
        ExprKind::If(c, t, e2) => {
            walk(c, f);
            walk_block(t, f);
            if let Some(e2) = e2 {
                walk(e2, f);
            }
        }
        ExprKind::While(c, b, step) => {
            walk(c, f);
            walk_block(b, f);
            if let Some(st) = step {
                walk(st, f);
            }
        }
        ExprKind::Match(sc, arms) => {
            walk(sc, f);
            arms.iter().for_each(|a| walk(&a.body, f));
        }
        ExprKind::Return(Some(a)) => walk(a, f),
        ExprKind::Closure { body, .. } => walk_block(body, f),
        _ => {}
    }
}
fn walk_block(b: &Block, f: &mut dyn FnMut(&Expr)) {
    for st in &b.stmts {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => walk(e, f),
            Stmt::Destructure { init, .. } => walk(init, f),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        walk(t, f);
    }
}
impl Scan<'_, '_> {
    fn args_ctx(&self, name: &str) -> Ctx {
        // Arguments of a function or extern are lowered as values,
        // and so are those of the conversions and the integer
        // arithmetic intrinsics (`lower_intrinsic`); any other
        // intrinsic's may be places.
        const VALUE_INTRINSICS: &[&str] = &[
            "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul",
            "std::sqrt", "std::floor", "std::ceil", "std::round", "std::trunc", "std::ln", "std::exp", "std::log2", "std::log10", "std::sin",
            "std::cos", "std::tan", "std::asin", "std::acos", "std::atan", "std::atan2", "std::powf",
            "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem",
            // `printf`'s, `sprintf`'s, `assert`'s … formatted
            // arguments (`modres` makes the call `$fmt(…)`), each
            // lowered as a value; one written `&s` is still a place,
            // through the borrow (`Ctx::Place` below).
            "$fmt",
        ];
        let local = |n: &str| self.g.items.fns.contains_key(n) || self.g.items.externs.contains_key(n);
        if local(name) || VALUE_INTRINSICS.contains(&name) {
            Ctx::Value
        } else {
            Ctx::Place
        }
    }
    fn expr(&mut self, e: &Expr, ctx: Ctx) {
        match &e.kind {
            ExprKind::Path(segs, _) => {
                if segs.len() == 1 {
                    self.bad.insert(segs[0].clone());
                }
            }
            ExprKind::Deref(inner) => match index_on_local(inner) {
                Some((_, i)) if ctx == Ctx::Value => self.expr(i, Ctx::Value),
                _ => self.expr(inner, Ctx::Place),
            },
            ExprKind::Assign(l, r) => {
                match &l.kind {
                    ExprKind::Deref(inner) => match index_on_local(inner) {
                        Some((x, i)) if !pushes_onto(r, x) => self.expr(i, Ctx::Value),
                        _ => self.expr(l, Ctx::Place),
                    },
                    ExprKind::Index(..) => match index_expr_on_local(l) {
                        Some((x, i)) if !pushes_onto(r, x) => self.expr(i, Ctx::Value),
                        _ => self.expr(l, Ctx::Place),
                    },
                    // `x[i].f… = r` (and `x[i].f… op= r`): a field of
                    // the element written in place, as `x[i] = r` is.
                    ExprKind::Field(..) => match field_chain_root(l).and_then(index_expr_on_local) {
                        Some((x, i)) if !pushes_onto(r, x) => self.expr(i, Ctx::Value),
                        _ => self.expr(l, Ctx::Place),
                    },
                    _ => self.expr(l, Ctx::Place),
                }
                self.expr(r, Ctx::Value);
            }
            ExprKind::Call(callee, args) => self.call(e, callee, args, &[]),
            // `x[i].f…` read as a value: the element read, then
            // its fields; no path is formed.
            ExprKind::Field(..) if ctx == Ctx::Value && field_chain_root(e).map_or(false, |r| index_expr_on_local(r).is_some()) => {
                let (_, i) = index_expr_on_local(field_chain_root(e).unwrap()).unwrap();
                self.expr(i, Ctx::Value);
            }
            ExprKind::Borrow(_, a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) => self.expr(a, Ctx::Place),
            ExprKind::Index(a, i) => match index_expr_on_local(e) {
                Some((_, i)) if ctx == Ctx::Value => self.expr(i, Ctx::Value),
                _ => {
                    self.expr(a, Ctx::Place);
                    self.expr(i, Ctx::Value);
                }
            },
            ExprKind::SliceOf(_, a, lo, hi) => {
                self.expr(a, Ctx::Place);
                self.expr(lo, Ctx::Value);
                self.expr(hi, Ctx::Value);
            }
            ExprKind::Unary(_, a) => self.expr(a, Ctx::Value),
            ExprKind::Binary(_, a, b) => {
                self.expr(a, Ctx::Value);
                self.expr(b, Ctx::Value);
            }
            ExprKind::Paren(a) => self.expr(a, ctx),
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| self.expr(a, Ctx::Value)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a, Ctx::Value)),
            ExprKind::ArrayRepeat(a, _) => self.expr(a, Ctx::Value),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b, false),
            ExprKind::If(c, t, e2) => {
                self.expr(c, Ctx::Value);
                self.block(t, false);
                if let Some(e2) = e2 {
                    self.expr(e2, Ctx::Value);
                }
            }
            ExprKind::While(c, b, step) => {
                self.expr(c, Ctx::Value);
                self.block(b, false);
                if let Some(st) = step {
                    self.expr(st, Ctx::Value);
                }
            }
            ExprKind::Match(sc, arms) => {
                self.expr(sc, Ctx::Place);
                for a in arms {
                    if let Some(b) = &a.binder {
                        self.bad.insert(b.clone());
                    }
                    self.expr(&a.body, Ctx::Value);
                }
            }
            ExprKind::Return(Some(a)) => self.expr(a, Ctx::Value),
            ExprKind::Closure { captures, params, body, .. } => {
                // Nothing a closure names or captures is confined.
                self.bad.extend(captures.iter().cloned());
                self.bad.extend(params.iter().map(|p| p.name.clone()));
                let bad = &mut self.bad;
                walk_block(body, &mut |e| {
                    if let ExprKind::Path(segs, _) = &e.kind {
                        if segs.len() == 1 {
                            bad.insert(segs[0].clone());
                        }
                    }
                });
            }
            _ => {}
        }
    }
    // A call `e` (`callee(args)`), but for the arguments at `skip`,
    // which the caller has accounted for (moved-out locals).
    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Expr], skip: &[usize]) {
        if let Some((name, _)) = call_name(e) {
            if (name == PUSH || name == LEN || name == POP || name == RESERVE) && args.first().and_then(vec_arg).is_some() {
                args.iter().enumerate().skip(1).filter(|(j, _)| !skip.contains(j)).for_each(|(_, a)| self.expr(a, Ctx::Value));
                return;
            }
            // The calls whose bodies touch the elements only as
            // bytes and keep nothing (`CONFINING_CALLS`). As for
            // `push`, the other arguments are scanned as any use:
            // they are evaluated before the call, and one that
            // would hold a path into the vector through it (`&x`,
            // `&x[i]`) is a use that rules `x` out there.
            if CONFINING_CALLS.contains(&name.as_str()) && args.first().and_then(vec_arg).is_some() {
                args.iter().enumerate().skip(1).filter(|(j, _)| !skip.contains(j)).for_each(|(_, a)| self.expr(a, Ctx::Value));
                return;
            }
            // A name a binding could stand for: a call of
            // whatever it holds, so no rule below applies.
            let shadowed = !name.contains("::") && self.bound.contains(&name);
            // `&x[i]` / `&mut x[i]` for a pure direct
            // parameter (`Gen::pure_direct`), `x` named by no
            // other argument: the callee keeps no path, and
            // the element's address is all it is given
            // (`elem_access_place`'s confined case); only the
            // index is a use.
            let elem_arg = |j: usize, a: &Expr| -> Option<String> {
                let ExprKind::Borrow(_, place) = &a.kind else { return None };
                let (x, _) = index_expr_on_local(place)?;
                let f = self.g.items.fns.get(&name)?;
                let ok = matches!(f.params.get(j).map(|p| &p.ty), Some(Type::Ref(..)))
                    && self.g.pure_direct(&name, j)
                    && !args.iter().enumerate().any(|(k, b)| k != j && mentions(b, x));
                ok.then(|| x.to_string())
            };
            if !shadowed && args.iter().enumerate().any(|(j, a)| elem_arg(j, a).is_some()) {
                self.expr(callee, Ctx::Place);
                for (j, a) in args.iter().enumerate() {
                    match (elem_arg(j, a), &a.kind) {
                        (Some(_), ExprKind::Borrow(_, place)) => {
                            let (_, i) = index_expr_on_local(place).expect("an element argument");
                            self.expr(i, Ctx::Value);
                        }
                        _ if skip.contains(&j) => {}
                        _ => self.expr(a, self.args_ctx(&name)),
                    }
                }
                return;
            }
            if let Some(conf) = self.table.get(&name).filter(|_| !shadowed) {
                // A vector passed to a confining parameter,
                // and passed as no other argument of the
                // call: the parameters would alias. Other
                // uses in other arguments (reads, `len`,
                // even pushes, which move the buffer but
                // not the `Vec`) are over before the call
                // begins and leave no held path; they are
                // scanned like any other use.
                for (j, a) in args.iter().enumerate() {
                    let passed = conf.get(j) == Some(&true)
                        && vec_arg(a).map_or(false, |x| !args.iter().enumerate().any(|(k, b)| k != j && vec_arg(b) == Some(x)));
                    if !passed && !skip.contains(&j) {
                        self.expr(a, Ctx::Value);
                    }
                }
                return;
            }
            let c = self.args_ctx(&name);
            self.expr(callee, Ctx::Place);
            args.iter().enumerate().filter(|(j, _)| !skip.contains(j)).for_each(|(_, a)| self.expr(a, c));
        } else {
            self.expr(callee, Ctx::Place);
            args.iter().enumerate().filter(|(j, _)| !skip.contains(j)).for_each(|(_, a)| self.expr(a, Ctx::Value));
        }
    }
    // The function's result `whole`, at `e` within it: struct
    // literals' fields and calls' arguments that are bare names
    // `whole` mentions once are moved out, not used.
    fn tail_moves(&mut self, e: &Expr, whole: &Expr) {
        let moved = |a: &Expr| bare(a).map_or(false, |x| {
            let mut n = 0;
            walk(whole, &mut |w| {
                if matches!(&w.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == x) {
                    n += 1;
                }
                if let ExprKind::Closure { captures, .. } = &w.kind {
                    n += captures.iter().filter(|c| *c == x).count();
                }
            });
            n == 1
        });
        match &e.kind {
            ExprKind::StructLit(_, _, fs) => {
                for (_, fe) in fs {
                    if moved(fe) {
                        continue;
                    }
                    if matches!(&fe.kind, ExprKind::StructLit(..) | ExprKind::Call(..)) {
                        self.tail_moves(fe, whole);
                    } else {
                        self.expr(fe, Ctx::Value);
                    }
                }
            }
            ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Path(..)) => {
                self.expr(callee, Ctx::Place);
                let c = call_name(e).map_or(Ctx::Value, |(n, _)| self.args_ctx(&n));
                for a in args {
                    if moved(a) {
                        continue;
                    }
                    if matches!(&a.kind, ExprKind::StructLit(..) | ExprKind::Call(..)) {
                        self.tail_moves(a, whole);
                    } else {
                        self.expr(a, c);
                    }
                }
            }
            _ => self.expr(e, Ctx::Value),
        }
    }
    fn block(&mut self, b: &Block, fn_body: bool) {
        for (k, st) in b.stmts.iter().enumerate() {
            // A named reference to an element (`elem_ref_let`): only
            // the index is a use of the vector.
            if let Some((_, _, _, i)) = elem_ref_let(st, &b.stmts[k + 1..], b.tail.as_deref()) {
                self.expr(i, Ctx::Value);
                continue;
            }
            // A local moved into a call as its last use in the
            // function body (`overwrite(&mut h.v, v);`), as moving
            // it out as the result is: nothing of the function
            // reaches it afterwards.
            if fn_body {
                if let Stmt::Expr(e) = st {
                    if let ExprKind::Call(callee, args) = &e.kind {
                        let later = |x: &str| {
                            b.stmts[k + 1..].iter().any(|s2| match s2 {
                                Stmt::Let { name, init, .. } => name == x || init.as_ref().map_or(false, |e| mentions(e, x)),
                                Stmt::Destructure { fields, init, .. } => fields.iter().any(|f| f == x) || mentions(init, x),
                                Stmt::Expr(e) | Stmt::BlockLike(e) => mentions(e, x),
                            }) || b.tail.as_ref().map_or(false, |t| mentions(t, x))
                        };
                        let moved: Vec<usize> = (0..args.len())
                            .filter(|&j| match bare(&args[j]) {
                                Some(x) => !later(x) && !mentions(callee, x) && !args.iter().enumerate().any(|(l, o)| l != j && mentions(o, x)),
                                None => false,
                            })
                            .collect();
                        if !moved.is_empty() {
                            // The other arguments as in any call: a
                            // vector passed to a confining parameter
                            // beside them stays confined.
                            self.call(e, callee, args, &moved);
                            continue;
                        }
                    }
                }
            }
            match st {
                // A `foreach (x in &v)` over a local whose body
                // reads `x` only as `*x` (`Fx::foreach_alias`): the
                // loop's holder `$c` borrows `v` shared, and its uses
                // become bounds-checked element reads of `v`, so the
                // loop leaves `v` confined. Any other use of `v` in
                // the body is scanned as usual; a push to `v` there
                // is already rejected statically (the holder lives).
                Stmt::Let { name, init: Some(e), .. } if foreach_reads_only(name, e, b) => {
                    self.declared.insert(name.clone());
                    // Over a field (`&h.v`): `h` is used as usual.
                    if let ExprKind::Borrow(_, inner) = &e.kind {
                        if !matches!(&inner.kind, ExprKind::Path(..)) {
                            self.expr(inner, Ctx::Place);
                        }
                    }
                }
                // `foreach (x in p)`, `p` a parameter (a reference:
                // the only kind `confining_table` considers), whose
                // body reads `x` only as `*x` or `x.f…`: the loop
                // over the borrow `p` is (`each.rs`), its holder `p`
                // itself, which `foreach_alias` reads elements
                // through. `p` is not copied anywhere else.
                Stmt::Let { name, init: Some(e), .. } if foreach_param_reads_only(name, e, b, &self.params) => {
                    self.declared.insert(name.clone());
                }
                Stmt::Let { name, init: Some(e), .. } => {
                    self.declared.insert(name.clone());
                    self.expr(e, Ctx::Value);
                }
                Stmt::Let { name, init: None, .. } => {
                    self.bad.insert(name.clone());
                }
                Stmt::Destructure { fields, init, .. } => {
                    for field in fields {
                        self.bad.insert(field.clone());
                    }
                    self.expr(init, Ctx::Value);
                }
                Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e, Ctx::Value),
            }
        }
        match &b.tail {
            // The function's own result may move the local out:
            // nothing of the function runs after it.
            Some(t) if fn_body && matches!(&t.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty()) => {}
            // So may a struct built as the result (`String { .bytes
            // = v }`): its bare fields move in; the rest are scanned.
            // So may a struct or a call built as the result (`String {
            // .bytes = v }`, `Ok(String { .bytes = v })`): a bare name
            // there that the result names nowhere else moves in; the
            // rest is scanned.
            Some(t) if fn_body && matches!(&t.kind, ExprKind::StructLit(..) | ExprKind::Call(..)) => self.tail_moves(t, t),
            Some(t) => self.expr(t, Ctx::Value),
            None => {}
        }
    }
}

pub(super) fn scan(g: &Gen, body: &Block, params: &[String], table: &HashMap<String, Vec<bool>>, fn_body: bool) -> (HashSet<String>, HashSet<String>) {
    let bound: HashSet<String> = params.iter().cloned().chain(bound_names(body)).collect();
    let mut sc = Scan { g, table, declared: HashSet::new(), bad: HashSet::new(), bound, params: params.iter().cloned().collect() };
    sc.block(body, fn_body);
    (sc.declared, sc.bad)
}
