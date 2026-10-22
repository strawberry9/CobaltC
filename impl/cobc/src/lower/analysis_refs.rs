// Per-function analyses: direct, reader and writer reference parameters.

use super::*;

impl<'a> Gen<'a> {
    // The *direct* reference parameters of `f`: a `ref<T, _>` with `T`
    // plain data, the function's only parameter that is or holds a
    // reference, which the body uses only as `r.f…` or `*r`, read or
    // assigned whole (`direct_uses`). Its accesses need no check: the
    // caller formed and checked the borrow, and while it lives no other
    // path can reach `*r` -- one would clash with `r` and fault at its
    // own access, the only place it could; `r` is the sole reference the
    // call was given, so no argument aliases it (whose clash would
    // otherwise be found at the first access meeting both, `spec/08`);
    // and nothing in the body forms a path from it, hands it on or keeps
    // it. `T` holds no reference or resource, so no slot or authority is
    // involved. Such a parameter gets no runtime binding at all.
    pub(super) fn direct_ref_params(&self, f: &FnDecl, subst: &HashMap<String, Type>) -> R<HashSet<String>> {
        if self.all_checks {
            return Ok(HashSet::new());
        }
        self.direct_refs_of(&f.params, &f.body, subst)
    }

    // `direct_ref_params` for a parameter list and body (a function's, or
    // a closure's that captures nothing). Besides a sole reference
    // parameter, any number of *shared* references to plain data qualify
    // together: two shared paths never clash, and an exclusive one to the
    // same storage could not have been formed while they live.
    pub(super) fn direct_refs_of(&self, params: &[Param], body: &Block, subst: &HashMap<String, Type>) -> R<HashSet<String>> {
        self.direct_refs_core(params, body, subst, true)
    }

    // The dual-body case (`lower_fn`, `private/direct-params-proposal.md`):
    // two or more reference parameters, at least one exclusive, every one
    // a `ref` or slice to plain data that `direct_refs_of`'s per-parameter
    // test admits. Each is then direct in the `__fast` body, which runs
    // only when the dispatcher has found every incoming path valid and no
    // two overlapping where one is exclusive (`cb_paths_disjoint`). Nothing
    // in such a body can change that: a direct parameter is never handed
    // on, stored or reborrowed, and the body holds no other reference.
    pub(super) fn dual_direct_params(&self, params: &[Param], body: &Block, subst: &HashMap<String, Type>) -> R<Option<Vec<String>>> {
        let tys: Vec<Type> = params.iter().map(|p| apply(&p.ty, subst)).collect();
        let refish: Vec<(&Param, &Type)> = params.iter().zip(&tys).filter(|(_, t)| self.has_refs(t) || self.holds_fn(t)).collect();
        if refish.len() < 2 || !refish.iter().any(|(_, t)| matches!(t, Type::Ref(_, Mode::Exclusive) | Type::Slice(_, Mode::Exclusive))) {
            return Ok(None);
        }
        for (_, t) in &refish {
            match t {
                Type::Ref(inner, _) | Type::Slice(inner, _) if self.plain_data(inner) && self.size_align(inner)?.0 > 0 && !matches!(&**inner, Type::Named(n, _) if n == "std::Vec") => {}
                _ => return Ok(None),
            }
        }
        let direct = self.direct_refs_core(params, body, subst, false)?;
        if refish.iter().all(|(p, _)| direct.contains(&p.name)) {
            Ok(Some(refish.iter().map(|(p, _)| p.name.clone()).collect()))
        } else {
            Ok(None)
        }
    }

    pub(super) fn direct_refs_core(&self, params: &[Param], body: &Block, subst: &HashMap<String, Type>, gate: bool) -> R<HashSet<String>> {
        let mut out = HashSet::new();
        let tys: Vec<Type> = params.iter().map(|p| apply(&p.ty, subst)).collect();
        let refish: Vec<&Type> = tys.iter().filter(|t| self.has_refs(t) || self.holds_fn(t)).collect();
        let all_shared_plain = refish.iter().all(|t| matches!(t, Type::Ref(inner, Mode::Shared) | Type::Slice(inner, Mode::Shared) if self.plain_data(inner)));
        if gate && refish.len() != 1 && !(refish.len() > 1 && all_shared_plain) {
            return Ok(out);
        }
        for (p, t) in params.iter().zip(&tys) {
            let (Type::Ref(inner, _) | Type::Slice(inner, _)) = t else { continue };
            // A reference to a `Vec` of plain elements qualifies when it is
            // only ever handed on whole: every access is then the callee's,
            // checked through the path the call was given.
            let vec_of_plain = matches!(t, Type::Ref(..)) && matches!(&**inner, Type::Named(n, a) if n == "std::Vec" && a.len() == 1 && self.plain_data(&a[0]));
            if vec_of_plain && only_handed_on(body, &p.name) {
            } else if matches!(t, Type::Ref(..)) && matches!(&**inner, Type::Named(n, _) if self.struct_decl(n).is_some()) && !self.has_refs(inner) && !self.holds_fn(inner) && box_rec_uses(body, &p.name) {
                // A struct reached through `Box`es (`stress/perf/tree-design.md`):
                // used only as `r.f` read, `r.f = e`, and `match (&r.f)` whose
                // binder goes only to `Box::get[_mut](b)` handed on. Direct, and
                // the lowering keeps the value checks a resource field needs.
                out.insert(p.name.clone());
                continue;
            } else if self.size_align(inner)?.0 == 0 || !self.plain_data(inner) {
                continue;
            }
            // A slice is used only through its elements and `slice_len`:
            // never handed on, so its stored path is never needed.
            if matches!(t, Type::Slice(..)) {
                if direct_uses(body, &p.name, &|_, _| false) {
                    out.insert(p.name.clone());
                }
                continue;
            }
            // `r` handed whole to a named function whose result is plain
            // data: the callee's path ends with the call, and it can give
            // nothing derived from `r` back. Not `spawn` (the thread keeps
            // it) and not a closure value.
            let callee_ok = |segs: &[String], _: usize| {
                let key = segs.join("::");
                let key = if self.items.fns.contains_key(&key) { key } else { format!("std::{}", key) };
                key != "spawn" && key != "std::spawn" && self.items.fns.get(&key).map_or(false, |g| matches!(g.ret, Type::Void) || self.plain_data(&g.ret))
            };
            if direct_uses(body, &p.name, &callee_ok) {
                out.insert(p.name.clone());
            }
        }
        Ok(out)
    }

    // Whether parameter `pos` of the non-generic function `key` is a
    // direct reference (`direct_ref_params`) handed on, if at all, only to
    // such parameters: then no use of the reference a call passes reads
    // its path, and the call may pass a bare address after the borrow's
    // check (`Fx::lower_user_call`). Recursion through a parameter answers
    // no (it would need a fixed point; no is safe).
    // A function's sole reference(-holding) parameter, of reference type,
    // that the body never assigns, borrows as a binding (`&r`) or
    // captures: no slot need hold its token -- with no other reference
    // into the function, nothing could clash with the path while the call
    // runs, and an unheld path no check counts (D-0018). Every access
    // through it stays checked, through the token it came with.
    // Several reference parameters qualify when every one is shared: two
    // shared paths never clash, and no exclusive path can be formed from
    // a shared one, so with no other reference into the function nothing
    // could clash with them either (`HashMap::get(m, k)` and the functions
    // it hands both on to).
    pub(super) fn unstored_ref_params(&self, f: &FnDecl, subst: &HashMap<String, Type>, direct: &HashSet<String>) -> R<HashSet<String>> {
        let mut out = HashSet::new();
        // D-0274: not in the bootstrap form.
        if self.bootstrap {
            return Ok(out);
        }
        let tys: Vec<Type> = f.params.iter().map(|p| apply(&p.ty, subst)).collect();
        let holding: Vec<&Type> = tys.iter().filter(|t| self.has_refs(t) || self.holds_fn(t)).collect();
        let all_shared = holding.iter().all(|t| matches!(t, Type::Ref(_, Mode::Shared)));
        if holding.is_empty() || (holding.len() > 1 && !all_shared) {
            return Ok(out);
        }
        let assigned = assigned_names(&f.body);
        for (p, t) in f.params.iter().zip(&tys) {
            if !matches!(t, Type::Ref(..)) || direct.contains(&p.name) || assigned.contains(&p.name) {
                continue;
            }
            let name = p.name.as_str();
            let is_r = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
            let mut ok = true;
            visit_exprs(&f.body, &mut |e| match &e.kind {
                ExprKind::Borrow(_, a) if is_r(a) => ok = false,
                ExprKind::Closure { body, captures, .. } if captures.iter().any(|c| c == name) || block_names(body, name) => ok = false,
                _ => {}
            });
            let mut shadow = false;
            visit_stmts(&f.body, &mut |st| {
                if let Stmt::Let { name: n, .. } = st {
                    if n == name {
                        shadow = true;
                    }
                }
            });
            if ok && !shadow {
                out.insert(p.name.clone());
            }
        }
        Ok(out)
    }

    // Whether a function item is named anywhere but as a call's callee
    // (by its last segment: a coarse, safe answer).
    pub(super) fn used_as_value(&self, key: &str) -> bool {
        if self.fn_values.borrow().is_none() {
            let mut names = HashSet::new();
            for f in self.items.fns.values() {
                // A one-segment name bound in the function (a local, a
                // parameter, a binder) names that binding, never a function:
                // it shadows any (`std`'s locals called `total` or `len` made
                // every function of that name look used as a value).
                let bound: HashSet<String> = bound_names(&f.body).into_iter().chain(f.params.iter().map(|p| p.name.clone())).collect();
                let mut callees: HashSet<*const Expr> = HashSet::new();
                visit_exprs(&f.body, &mut |e| {
                    if let ExprKind::Call(c, _) = &e.kind {
                        if matches!(c.kind, ExprKind::Path(..)) {
                            callees.insert(&**c as *const Expr);
                        }
                    }
                });
                visit_exprs(&f.body, &mut |e| {
                    if let ExprKind::Path(segs, _) = &e.kind {
                        if !callees.contains(&(e as *const Expr)) && !(segs.len() == 1 && bound.contains(&segs[0])) {
                            if let Some(l) = segs.last() {
                                names.insert(l.clone());
                            }
                            // `Type::f`, which only such a path can name.
                            if segs.len() >= 2 {
                                names.insert(format!("{}::{}", segs[segs.len() - 2], segs[segs.len() - 1]));
                            }
                        }
                    }
                });
            }
            *self.fn_values.borrow_mut() = Some(names);
        }
        // An associated function (`Type::f`, the type's name capitalized)
        // is named by a path ending in `Type::f`, never by `f` alone (a
        // local called `len` is not `StringView::len`); any other function
        // by its last name, which errs toward "used".
        let segs: Vec<&str> = key.split("::").collect();
        if segs.len() >= 2 && segs[segs.len() - 2].starts_with(|c: char| c.is_ascii_uppercase()) {
            return self.fn_values.borrow().as_ref().unwrap().contains(&format!("{}::{}", segs[segs.len() - 2], segs[segs.len() - 1]));
        }
        let last = key.rsplit("::").next().unwrap_or(key);
        self.fn_values.borrow().as_ref().unwrap().contains(last)
    }

    // Whether parameter `pos` of `key` is a *reader*: a shared reference
    // the body only reads through (`p.f…`, a value) and hands on, whole or
    // as `&p.f…`, to other readers (`reader_uses`), and the function's
    // result holds no reference. Nothing derived from the reference is then
    // kept, returned or stored, and nothing is written through it: a caller
    // that has proven every check on the referent for the call's duration
    // may pass it with token 0 (`cb_read`'s note in cbrt), and every check
    // through it is then the no-op the proof makes it. Recursion answers no.
    pub(super) fn reader_param(&self, key: &str, pos: usize) -> bool {
        // D-0274: no reader parameters in the bootstrap form.
        if self.bootstrap {
            return false;
        }
        let visiting = std::cell::RefCell::new(Vec::new());
        self.reader_param_in(key, pos, &visiting)
    }
    pub(super) fn reader_param_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        let id = (key.to_string(), pos);
        if let Some(&b) = self.reader_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        let Some(p) = f.params.get(pos) else { return false };
        // D-0200: the text searches, native (`native_text_search`): the
        // `String` is read after its borrow's check and nothing of it kept
        // (their bodies borrow a slice of it, which `reader_uses` refuses).
        if pos == 0 && in_prelude(f) && matches!(key, "std::String::find" | "std::String::contains" | "std::String::starts_with" | "std::String::ends_with") {
            return true;
        }
        // CHG-0237: `String::clone` copies the bytes it is lent into a new
        // `String` and keeps nothing of them.
        if pos == 0 && in_prelude(f) && key == "std::String::clone" {
            return true;
        }
        // The key of `HashMap::get`/`get_mut`/`contains`: the result's
        // reference, if any, is to a value in the map (`spec/21` §4),
        // never to anything of the key's, so only the body's uses decide.
        let map_key = pos == 1 && in_prelude(f) && matches!(key, "std::HashMap::get" | "std::HashMap::get_mut" | "std::HashMap::contains");
        if !matches!(p.ty, Type::Ref(_, Mode::Shared)) || (!map_key && (!resolved(&f.ret, &f.type_params) || self.has_refs(&f.ret) || self.holds_fn(&f.ret))) {
            self.reader_memo.borrow_mut().insert(id, false);
            return false;
        }
        visiting.borrow_mut().push(id.clone());
        let ok = reader_uses(&f.body, &p.name, true, &|segs: &[String], at: usize| {
            // The std-only intrinsics `key_hash(k)` and `key_eq(a, b)` read
            // their referents and give plain data: readers.
            if in_prelude(f) && matches!(segs, [s] if s == "key_hash" || s == "key_eq") {
                return true;
            }
            let k = segs.join("::");
            let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
            match self.items.fns.get(&k).and_then(|g| g.params.get(at)).map(|q| q.ty.clone()) {
                Some(Type::Ref(_, Mode::Shared)) => self.reader_param_in(&k, at, visiting),
                Some(Type::Named(n, a)) if n == "std::StringView" && a.is_empty() => self.pure_view(&k, at),
                _ => false,
            }
        });
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.reader_memo.borrow_mut().insert(id, ok);
        }
        ok
    }

    // D-0200: whether the block `b` is a `foreach` over `chars` or
    // `split_whitespace` that `Fx::lower_fused_text_loop` lowers fused, and
    // its parts. `local`: whether a one-segment name is a binding where the
    // block is (a local of that name would be what the call calls).
    pub(super) fn fusion_plan<'b>(&self, b: &'b Block, local: &dyn Fn(&str) -> bool) -> Option<FusionPlan<'b>> {
        if self.bootstrap {
            return None;
        }
        let [Stmt::Let { name: en, init: Some(coll), .. }, Stmt::Let { name: dn, init: Some(dr), .. }, Stmt::Let { name: inn, init: Some(_), .. }] = b.stmts.as_slice() else { return None };
        if !en.starts_with("$e") || !dn.starts_with("$d") || !inn.starts_with("$i") {
            return None;
        }
        let ExprKind::Call(dc, dargs) = &dr.kind else { return None };
        if !matches!(&dc.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "$each_drain") || !matches!(dargs.get(1).map(|a| &a.kind), Some(ExprKind::IntLit(1, _))) {
            return None;
        }
        let Some(tail) = &b.tail else { return None };
        let ExprKind::While(_, wb, Some(_)) = &tail.kind else { return None };
        let ([Stmt::Let { name: cname, init: Some(take), .. }, Stmt::BlockLike(be)], None) = (wb.stmts.as_slice(), &wb.tail) else { return None };
        if !matches!(&take.kind, ExprKind::Call(tc, _) if matches!(&tc.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "$each_take")) {
            return None;
        }
        let ExprKind::Block(body) = &be.kind else { return None };
        let ExprKind::Call(callee, args) = &strip_parens(coll).kind else { return None };
        let ExprKind::Path(segs, targs) = &callee.kind else { return None };
        if !targs.is_empty() || args.len() != 1 || (segs.len() == 1 && local(&segs[0])) {
            return None;
        }
        let k = segs.join("::");
        let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        let (on_string, chars) = match k.as_str() {
            "std::StringView::chars" => (false, true),
            "std::StringView::split_whitespace" => (false, false),
            "std::String::chars" => (true, true),
            "std::String::split_whitespace" => (true, false),
            _ => return None,
        };
        if !self.items.fns.get(&k).map_or(false, |f| in_prelude(f)) {
            return None;
        }
        // The body: `c` only as a pure view, or copied (`String::from_view`),
        // or trimmed into a local of its own (`let t = StringView::trim(c);`,
        // `trim_start`, `trim_end`, at the body's top level) that is itself
        // used only so; nothing the collection names.
        let derived: Vec<String> = body
            .stmts
            .iter()
            .filter_map(|st| match st {
                Stmt::Let { name, init: Some(e), .. } => trim_of(e).filter(|(_, x)| x == cname).map(|_| name.clone()),
                _ => None,
            })
            .collect();
        let pure = {
            let g = self;
            let ok = |segs: &[String], at: usize| -> bool {
                let k2 = segs.join("::");
                let k2 = if g.items.fns.contains_key(&k2) { k2 } else { format!("std::{}", k2) };
                let r = g.pure_view(&k2, at) || (k2 == "std::String::from_view" && g.items.fns.get(&k2).map_or(false, |f| in_prelude(f)));
                if std::env::var("COBC_DEBUG_FUSE").is_ok() {
                    eprintln!("FUSE ok {} {} = {}", k2, at, r);
                }
                r
            };
            let c_ok = |segs: &[String], at: usize| -> bool {
                let k2 = segs.join("::");
                let k2 = if g.items.fns.contains_key(&k2) { k2 } else { format!("std::{}", k2) };
                ok(segs, at) || (at == 0 && TRIMS.contains(&k2.as_str()) && !derived.is_empty())
            };
            // Each trimmed local, from just after its `let` (its own
            // declaration is no use of it).
            let after = |t: &str| -> Option<Block> {
                let at = body.stmts.iter().position(|st| matches!(st, Stmt::Let { name, .. } if name == t))?;
                if body.stmts[..at].iter().any(|st| matches!(st, Stmt::Let { name, .. } if name == t)) {
                    return None;
                }
                Some(Block { stmts: body.stmts[at + 1..].to_vec(), tail: body.tail.clone() })
            };
            let w_ok = view_uses(body, cname, &c_ok);
            let t_ok = derived.iter().all(|t| t != cname && after(t).map_or(false, |b| view_uses(&b, t, &ok)));
            if std::env::var("COBC_DEBUG_FUSE").is_ok() {
                eprintln!("FUSE parts w={} t={}", w_ok, t_ok);
                if std::env::var("COBC_DEBUG_FUSE_AST").is_ok() {
                    eprintln!("{:#?}", body);
                }
            }
            w_ok && t_ok
        };
        if std::env::var("COBC_DEBUG_FUSE").is_ok() {
            eprintln!("FUSE {} {} pure={} derived={:?}", k, cname, pure, derived);
        }
        if !pure {
            return None;
        }
        let mut named: Vec<String> = Vec::new();
        visit_expr(&args[0], &mut |x| {
            if let ExprKind::Path(s, _) = &x.kind {
                if s.len() == 1 {
                    named.push(s[0].clone());
                }
            }
        });
        let mut clash = false;
        visit_exprs(body, &mut |x| match &x.kind {
            ExprKind::Path(s, _) if s.len() == 1 && named.contains(&s[0]) => clash = true,
            ExprKind::Closure { captures, .. } if captures.iter().any(|c| named.contains(c)) => clash = true,
            _ => {}
        });
        if clash {
            return None;
        }
        Some(FusionPlan { source: &args[0], cname: cname.clone(), be, on_string, chars, derived })
    }

    // D-0200: the *quiet* container locals: `let x = …;` of a `Vec`,
    // `HashMap`, `HashSet`, `String` or `Queue`, declared once (not a
    // parameter), never moved, rebound, captured or matched by value, and
    // exclusively borrowed in the function's own code only as a call's
    // argument for a writer parameter (`writer_param`: nothing of the path
    // outlives the call, which holds it only while the callee runs, when
    // none of this code does) -- or by an element reference that needs no path
    // (`elem_ref_lets`). No held exclusive path to `x` then ever exists
    // where this function's code runs, and `x` is initialized where it is
    // declared, so its shared checks (`cb_read`, a shared
    // `cb_borrow_check` on its root) cannot fail. By initializer address.
    pub(super) fn quiet_locals(&self, body: &Block, params: &[String], elem_refs: &HashSet<usize>) -> HashSet<usize> {
        let mut decls: HashMap<String, Vec<usize>> = HashMap::new();
        visit_stmts(body, &mut |st| {
            if let Stmt::Let { name, ty, init: Some(e), .. } = st {
                let container = matches!(ty, Some(Type::Named(n, _)) if matches!(n.as_str(), "std::Vec" | "std::HashMap" | "std::HashSet" | "std::String" | "std::Queue" | "Vec" | "HashMap" | "HashSet" | "String" | "Queue"));
                decls.entry(name.clone()).or_default().push(if container { &**e as *const Expr as usize } else { 0 });
            } else if let Stmt::Let { name, .. } = st {
                decls.entry(name.clone()).or_default().push(0);
            } else if let Stmt::Destructure { fields, .. } = st {
                for f in fields {
                    decls.entry(f.clone()).or_default().push(0);
                }
            }
        });
        let mut out = HashSet::new();
        let bound = bound_names(body);
        for (x, ids) in decls {
            if ids.len() != 1 || ids[0] == 0 || params.contains(&x) {
                continue;
            }
            let mut ok = true;
            visit_exprs(body, &mut |e| {
                if let ExprKind::Closure { captures, body: cb, .. } = &e.kind {
                    if captures.iter().any(|c| c == &x) || block_names(cb, &x) {
                        ok = false;
                    }
                }
            });
            if ok && quiet_block(body, &x, &|segs: &[String], at: usize| {
                if segs.len() == 1 && bound.contains(&segs[0]) {
                    return false;
                }
                // A writer parameter: nothing of the path outlives the call.
                let k = segs.join("::");
                let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                matches!(self.items.fns.get(&k).and_then(|f| f.params.get(at)).map(|p| &p.ty), Some(Type::Ref(_, Mode::Exclusive))) && self.writer_param(&k, at)
            }, elem_refs) {
                out.insert(ids[0]);
            }
        }
        out
    }

    // D-0200: the `let`s binding a reference to an element (or a field of
    // one) of a local vector -- `ref<T, m> r = &x[i]…;` / `&mut x[i]…` --
    // that need no path: `x` is a local of the function (not a parameter,
    // whose aliases the caller made), named nowhere in the rest of the block
    // (so no new path to it can be formed while `r` lives: every alias it
    // already has is a held path, which the borrow's checks meet, and any
    // other would have to name it), and `r` is used there only as a reader
    // (shared) or a writer (exclusive, its referent's type written) is. By
    // the initializer's address.
    pub(super) fn elem_ref_lets(&self, body: &Block, params: &[String]) -> HashSet<usize> {
        let mut out = HashSet::new();
        let declared = bound_names(body);
        visit_blocks(body, &mut |blk| {
            for (i, st) in blk.stmts.iter().enumerate() {
                let Stmt::Let { name: r, ty, init: Some(e), .. } = st else { continue };
                let ExprKind::Borrow(mode, place) = &strip_parens(e).kind else { continue };
                if !matches!(strip_parens(place).kind, ExprKind::Index(..)) && !matches!(&strip_parens(place).kind, ExprKind::Field(a, _) if elem_chain_root(a).is_some()) {
                    continue;
                }
                let Some(x) = elem_chain_root(place) else { continue };
                if params.iter().any(|p| p == x) || !declared.contains(x) || x == r.as_str() {
                    continue;
                }
                let rest = Block { stmts: blk.stmts[i + 1..].to_vec(), tail: blk.tail.clone() };
                let mut named = false;
                visit_exprs(&rest, &mut |e| match &e.kind {
                    ExprKind::Path(s, _) if s.len() == 1 && s[0] == x => named = true,
                    ExprKind::Closure { captures, .. } if captures.iter().any(|c| c == x) => named = true,
                    _ => {}
                });
                let mut redeclared = false;
                visit_stmts(&rest, &mut |s| {
                    if matches!(s, Stmt::Let { name, .. } if name == r) {
                        redeclared = true;
                    }
                });
                if named || redeclared {
                    if std::env::var("COBC_DEBUG_ELEMREF").is_ok() {
                        eprintln!("ELEMREF {} over {}: named {} redeclared {}", r, x, named, redeclared);
                    }
                    continue;
                }
                let bound = bound_names(body);
                let c_ok = |segs: &[String], at: usize| -> bool {
                    if segs.len() == 1 && bound.contains(&segs[0]) {
                        return false;
                    }
                    let k = segs.join("::");
                    let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                    let res = match self.items.fns.get(&k).and_then(|g| g.params.get(at)).map(|q| q.ty.clone()) {
                        Some(Type::Ref(_, Mode::Exclusive)) => self.writer_param(&k, at),
                        _ => self.reads_only(&k, at),
                    };
                    if std::env::var("COBC_DEBUG_ELEMREF").is_ok() {
                        eprintln!("ELEMREF  callee {} {} = {}", k, at, res);
                    }
                    res
                };
                let ok = match mode {
                    Mode::Shared => reader_uses(&rest, r, true, &c_ok),
                    Mode::Exclusive => match ty {
                        Some(Type::Ref(pointee, Mode::Exclusive)) => {
                            let pointee = (**pointee).clone();
                            writer_uses(&rest, r, &c_ok, &|lhs: &Expr| self.plain_place(&pointee, lhs, r))
                        }
                        _ => false,
                    },
                };
                if std::env::var("COBC_DEBUG_ELEMREF").is_ok() {
                    eprintln!("ELEMREF {} over {} = {}", r, x, ok);
                }
                if ok {
                    out.insert(&**e as *const Expr as usize);
                }
            }
        });
        out
    }

    // D-0200: a value made only of scalars (integers, floats, `bool`s, and
    // structs of them): no text or other data a view or slice could reach
    // lies in it.
    pub(super) fn scalars_only(&self, t: &Type) -> bool {
        match t {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool => true,
            Type::Named(n, a) => match self.struct_decl(n) {
                Some(sd) => {
                    let sub = subst_of(&sd.type_params, a);
                    sd.fields.iter().all(|f| self.scalars_only(&apply(&f.ty, &sub)))
                }
                None => false,
            },
            _ => false,
        }
    }

    // D-0200: parameter `at` of `k` keeps nothing of what it is handed and
    // only reads it: a reader (`reader_param`), or a pure view.
    pub(super) fn reads_only(&self, k: &str, at: usize) -> bool {
        // A `std` item may be named without `std::` (the prelude's names).
        let std_k;
        let k = if self.items.fns.contains_key(k) {
            k
        } else {
            std_k = format!("std::{}", k);
            std_k.as_str()
        };
        match self.items.fns.get(k).and_then(|f| f.params.get(at)).map(|q| q.ty.clone()) {
            Some(Type::Ref(_, Mode::Shared)) => self.reader_param(k, at),
            Some(Type::Named(n, a)) if n == "std::StringView" && a.is_empty() => self.pure_view(k, at),
            _ => false,
        }
    }

    // D-0200: whether parameter `pos` of `key` is a *writer*: an exclusive
    // reference the body only reads and writes through and lends on --
    // `p.f…`/`p[i]…`/`*p` read as values; plain data assigned to such a
    // place (`writer_uses`, `plain_place`); `p` itself, `&mut *p`, or
    // `&p.f…`/`&mut p.f…` handed to a reader or writer parameter (or to
    // `swap_places`), no other argument of that call naming `p`. No
    // reference to anything `p` reaches is then kept, returned or stored,
    // and no two paths derived from `p` exist at once: every check through
    // it, and through what is derived from it, can fail only where a check
    // through `p` itself could, and a caller that has proven those for the
    // call's duration (an objectless local, `Gen::objectless_locals`) may
    // pass it with token 0. A resource assigned through it is not allowed:
    // its overwrite check (`cb_write`'s target) needs the target's
    // object. Recursion answers no.
    pub(super) fn writer_param(&self, key: &str, pos: usize) -> bool {
        let visiting = std::cell::RefCell::new(Vec::new());
        self.writer_param_in(key, pos, &visiting)
    }
    pub fn debug_lent(&self) {
        if let Ok(list) = std::env::var("COBC_DEBUG_LENT") {
            for k in list.split(',') {
                let k = k.trim();
                let n = self.items.fns.get(k).map_or(0, |f| f.params.len());
                let r: Vec<String> = (0..n).map(|i| format!("{}:r{}w{}", i, self.reader_param(k, i) as u8, self.writer_param(k, i) as u8)).collect();
                eprintln!("LENT {} {}", k, r.join(" "));
            }
        }
    }
    pub(super) fn writer_param_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        let id = (key.to_string(), pos);
        if let Some(&b) = self.writer_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        let Some(p) = f.params.get(pos) else { return false };
        // The insertions, native for their common forms: the container is
        // borrowed for the call and nothing of it kept (the result is the
        // value displaced, or whether the key was new).
        if pos == 0 && in_prelude(f) && matches!(key, "std::HashSet::insert" | "std::HashMap::insert") {
            return true;
        }
        let Type::Ref(pointee, Mode::Exclusive) = &p.ty else {
            self.writer_memo.borrow_mut().insert(id, false);
            return false;
        };
        let pointee = (**pointee).clone();
        visiting.borrow_mut().push(id.clone());
        let ok = writer_uses(
            &f.body,
            &p.name,
            &|segs: &[String], at: usize| {
                if segs.len() == 1 && segs[0] == "swap_places" {
                    return true;
                }
                let k = segs.join("::");
                let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                match self.items.fns.get(&k).and_then(|g| g.params.get(at)).map(|q| q.ty.clone()) {
                    Some(Type::Ref(_, Mode::Exclusive)) => self.writer_param_in(&k, at, visiting),
                    _ => self.reads_only(&k, at),
                }
            },
            &|lhs: &Expr| self.plain_place(&pointee, lhs, &p.name),
        );
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.writer_memo.borrow_mut().insert(id, ok);
        }
        ok
    }

    // The type of the place `e` -- a chain of fields, elements and `*` down
    // to the reference `r`, whose referent is of type `ty` -- is plain data
    // (no resource, no reference, no `fn`). A type parameter is not.
    pub(super) fn plain_place(&self, ty: &Type, e: &Expr, r: &str) -> bool {
        fn chain<'e>(e: &'e Expr, r: &str, out: &mut Vec<&'e Expr>) -> bool {
            match &e.kind {
                ExprKind::Paren(a) => chain(a, r, out),
                ExprKind::Path(s, t) => s.len() == 1 && s[0] == r && t.is_empty(),
                ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => {
                    out.push(e);
                    chain(a, r, out)
                }
                _ => false,
            }
        }
        let mut steps = Vec::new();
        if !chain(e, r, &mut steps) {
            return false;
        }
        let mut t = ty.clone();
        for st in steps.iter().rev() {
            t = match (&st.kind, &t) {
                (ExprKind::Field(_, name), Type::Named(n, args)) => {
                    let Some(sd) = self.struct_decl(n) else { return false };
                    let Some(fd) = sd.fields.iter().find(|fd| &fd.name == name) else { return false };
                    apply(&fd.ty, &subst_of(&sd.type_params, args))
                }
                (ExprKind::Index(..), Type::Array(el, _) | Type::Slice(el, _)) => (**el).clone(),
                (ExprKind::Index(..), Type::Named(n, a)) if n == "std::Vec" && a.len() == 1 => a[0].clone(),
                // `*r` of the reference itself: its referent, `ty`.
                (ExprKind::Deref(_), _) if std::ptr::eq(*st, *steps.last().unwrap()) => t.clone(),
                (ExprKind::Deref(_), Type::Ref(inner, _)) => (**inner).clone(),
                _ => return false,
            };
        }
        self.plain_data(&t) && !self.has_refs(&t) && !self.holds_fn(&t) && !matches!(t, Type::Named(ref n, ref a) if a.is_empty() && !n.contains("::") && self.struct_decl(n).is_none() && self.enum_decl(n).is_none())
    }

    // Whether parameter `pos` of the non-generic function `key`, a
    // `StringView`, is a *pure view*: the body reads it only as
    // `v.bytes[i]` and `slice_len(v.bytes)`, or hands it whole to a pure
    // view parameter, and the function's result is plain data, so nothing
    // derived from it outlives the call. Such a view is not sent: the
    // caller's copy (a place, or the call's temporary) holds its path
    // while the call runs, and the callee's reads through it are only
    // bounds-checked (`Fx::bind_param`, `lower_user_call`). Recursion
    // answers no (no fixed point; no is safe).
    pub(super) fn pure_view(&self, key: &str, pos: usize) -> bool {
        if self.bootstrap {
            return false;
        }
        let visiting = std::cell::RefCell::new(Vec::new());
        self.pure_view_in(key, pos, &visiting)
    }

    pub(super) fn pure_view_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        let id = (key.to_string(), pos);
        if let Some(&b) = self.view_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        // D-0200: `File::write_text`'s text, native (`native_file_write`):
        // its bytes read for the write, nothing of the view kept.
        if pos == 1 && key == "std::File::write_text" && in_prelude(f) {
            return true;
        }
        let is_view = |t: &Type| matches!(t, Type::Named(n, a) if n == "std::StringView" && a.is_empty());
        if !f.type_params.is_empty() || !f.params.get(pos).map_or(false, |p| is_view(&p.ty)) || self.used_as_value(key) {
            self.view_memo.borrow_mut().insert(id, false);
            return false;
        }
        // The result holds no reference (CHG-0237: a `String` too, not only
        // plain data), so nothing of the view can leave in it.
        if !(matches!(f.ret, Type::Void) || self.plain_data(&f.ret) || (resolved(&f.ret, &f.type_params) && !self.has_refs(&f.ret) && !self.holds_fn(&f.ret))) {
            self.view_memo.borrow_mut().insert(id, false);
            return false;
        }
        let name = f.params[pos].name.clone();
        let bound = bound_names(&f.body);
        visiting.borrow_mut().push(id.clone());
        let callee = |segs: &[String], at: usize| {
            if segs.len() == 1 && bound.contains(&segs[0]) {
                return false;
            }
            let k = segs.join("::");
            let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
            self.pure_view_in(&k, at, visiting)
        };
        // D-0200: a fused text loop over the view itself
        // (`Fx::lower_fused_text_loop`) reads it and keeps nothing; its body
        // is judged as any. Every name bound in the body or a parameter
        // counts as a local, as many as the lowering can see or more.
        let pnames: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
        let local = |n: &str| bound.contains(n) || pnames.iter().any(|p| p == n);
        let fused = |bl: &Block, v: &str| -> Option<bool> {
            let plan = self.fusion_plan(bl, &local)?;
            if !matches!(&strip_parens(plan.source).kind, ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() && s[0] == v) || plan.on_string || plan.cname == v {
                return None;
            }
            let ExprKind::Block(body) = &plan.be.kind else { return None };
            Some(view_uses(body, v, &callee))
        };
        let ok = view_uses_fused(&f.body, &name, &callee, &fused);
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.view_memo.borrow_mut().insert(id, ok);
        }
        ok
    }
}
