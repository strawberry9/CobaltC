// Fx: values built in place (fresh pushes, inserts, sends) and finished calls.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // A literal that owns nothing: a variant without a payload (`None`), or
    // a struct literal of a struct with no destructor, not declared
    // `resource`, each field of which is plain data (no reference, no `fn`
    // value) or itself inert. Built in place, it needs no object: there is
    // nothing to destroy if a fault ends the statement first, and nothing
    // can reach it before the aggregate around it absorbs it.
    pub(super) fn inert_expr(&mut self, e: &Expr) -> R<bool> {
        // D-0274: not in the bootstrap form.
        if self.g.bootstrap {
            return Ok(false);
        }
        match &strip_parens(e).kind {
            ExprKind::Path(segs, _) => {
                if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
                    return Ok(false);
                }
                Ok(match self.variant_of(segs) {
                    Some((ename, vi)) => self.g.enum_decl(&ename).map_or(false, |ed| ed.variants[vi].payload.is_none()),
                    None => false,
                })
            }
            ExprKind::StructLit(segs, _, fields) => {
                let name = segs.join("::");
                let Some(sd) = self.g.struct_decl(&name) else { return Ok(false) };
                if sd.resource || sd.bits.is_some() || self.g.items.fns.contains_key(&format!("{}::drop", name)) {
                    return Ok(false);
                }
                // By each field's declared type (a generic one is not
                // judged: not inert).
                for (fname, fe) in fields {
                    let Some(fd) = sd.fields.iter().find(|f| &f.name == fname) else { return Ok(false) };
                    if !resolved(&fd.ty, &sd.type_params) || !sd.type_params.is_empty() {
                        return Ok(false);
                    }
                    let ft = fd.ty.clone();
                    let ok = if self.is_res(&ft) { self.inert_expr(fe)? } else { self.g.plain_data(&ft) };
                    if !ok {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    // An inert literal (`inert_expr`), or a variant whose payload is one or
    // is a `Box` of one (`inert_box_new`): destroying it runs no destructor.
    pub(super) fn inert_value(&mut self, e: &Expr) -> R<bool> {
        // D-0274: not in the bootstrap form.
        if self.g.bootstrap {
            return Ok(false);
        }
        if self.inert_expr(e)? {
            return Ok(true);
        }
        let ExprKind::Call(c, args) = &strip_parens(e).kind else { return Ok(false) };
        let ExprKind::Path(segs, _) = &c.kind else { return Ok(false) };
        if args.len() != 1 || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) || self.g.items.fns.contains_key(&segs.join("::")) {
            return Ok(false);
        }
        let Some((ename, vi)) = self.variant_of(segs) else { return Ok(false) };
        if !self.g.enum_decl(&ename).map_or(false, |ed| ed.variants[vi].payload.is_some()) {
            return Ok(false);
        }
        Ok(self.inert_expr(&args[0])? || self.inert_box_new(&args[0])?)
    }

    // `Box::new(x)` that `lower_box_new` builds natively, `x` inert: as a
    // variant's payload, a `Box` that needs no object (`objectless`).
    pub(super) fn inert_box_new(&mut self, e: &Expr) -> R<bool> {
        // D-0274: `Box::new` takes the general call path (`lower_box_new` is off).
        if self.g.bootstrap {
            return Ok(false);
        }
        let ExprKind::Call(c, args) = &strip_parens(e).kind else { return Ok(false) };
        let ExprKind::Path(segs, _) = &c.kind else { return Ok(false) };
        if segs.join("::") != "std::Box::new" || args.len() != 1 || !self.g.items.fns.get("std::Box::new").map_or(false, |f| in_prelude(f)) {
            return Ok(false);
        }
        let Ok(t) = self.probe_type(&args[0], None) else { return Ok(false) };
        if self.has_refs(&t) || self.holds_fn(&t) || matches!(t, Type::Closure(_) | Type::Never) {
            return Ok(false);
        }
        self.inert_expr(&args[0])
    }

    // `replace(&mut x, String::new())` or `replace(&mut x, Vec::new())`, `x`
    // an objectless local (a whole binding, no object) of type `elem`, a
    // resource holding no reference whose `_raw` push exists
    // (`native_vec_push`): `x`'s binding.
    pub(super) fn taken_objectless(&self, e: &Expr, elem: &Type) -> Option<Binding> {
        let ExprKind::Call(callee, rargs) = &strip_parens(e).kind else { return None };
        let ExprKind::Path(segs, targs) = &callee.kind else { return None };
        if !targs.is_empty() || rargs.len() != 2 || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) {
            return None;
        }
        let k = segs.join("::");
        let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        if k != "std::replace" || !self.g.items.fns.get(&k).map_or(false, |f| in_prelude(f)) {
            return None;
        }
        let ExprKind::Borrow(Mode::Exclusive, place) = &strip_parens(&rargs[0]).kind else { return None };
        let x = self.lookup(path_name(place)?)?;
        if x.ty != *elem || !self.objectless_c.contains(&x.c) || !matches!(x.kind, BindKind::Local) {
            return None;
        }
        let ExprKind::Call(nc, nargs) = &strip_parens(&rargs[1]).kind else { return None };
        let ExprKind::Path(ns, nt) = &nc.kind else { return None };
        let nk = ns.join("::");
        let nk = if self.g.items.fns.contains_key(&nk) { nk } else { format!("std::{}", nk) };
        let fresh = match nk.as_str() {
            "std::String::new" => matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty()),
            "std::Vec::new" => matches!(elem, Type::Named(n, _) if n == "std::Vec"),
            _ => false,
        };
        if !fresh || !nargs.is_empty() || !nt.is_empty() || !self.g.items.fns.get(&nk).map_or(false, |f| in_prelude(f)) {
            return None;
        }
        if !self.is_res(elem) || self.has_refs(elem) || self.holds_fn(elem) || !self.g.objectless_type(elem) {
            return None;
        }
        Some(x.clone())
    }

    // D-0200: `HashSet::insert(s, k)` of `String`s, `k` made with no object
    // (`lower_raw_string`): the native's `_raw` form
    // (`native_set_insert_string`), the set's argument evaluated first.
    pub(super) fn lower_set_insert_fresh(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::HashSet::insert" || !in_prelude(f) || args.len() != 2 {
            return Ok(None);
        }
        let string = Type::Named("std::String".to_string(), vec![]);
        let Ok(Type::Ref(st, Mode::Exclusive)) = self.probe_type(&args[0], None) else { return Ok(None) };
        if !matches!(&*st, Type::Named(n, a) if n == "std::HashSet" && a.len() == 1 && a[0] == string) {
            return Ok(None);
        }
        // Only a key `lower_raw_string` makes raw (its shapes, checked here
        // before anything is lowered).
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if !targs.is_empty() || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) || cargs.len() != 1 {
            return Ok(None);
        }
        let k = segs.join("::");
        let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        if !matches!(k.as_str(), "std::String::clone" | "std::String::from_str") || !self.g.items.fns.get(&k).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        let sv = self.lower_expr(&args[0], None)?;
        if matches!(sv.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let sv = self.into_temp(sv, line)?;
        let Some(kv) = self.lower_raw_string(&args[1])? else { return internal("a raw key that was not made") };
        if matches!(kv.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let cname = self.g.request_fn(name, vec![string])?;
        let t = self.g.fresh("t");
        self.at(line);
        self.emit(&format!("uint8_t {} = {}_raw({}, {});", t, cname, sv.c, kv.c));
        Ok(Some(V { c: t, ty: Type::Bool, obj: None }))
    }

    // D-0200: `Channel::send(ch, v)` of `String`s, `v` a copy made with no
    // object (`String::clone`/`from_str`, `lower_raw_string`): the native's
    // `_raw` form (`native_channel`), the channel's argument first.
    pub(super) fn lower_channel_send_fresh(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::Channel::send" || !in_prelude(f) || args.len() != 2 {
            return Ok(None);
        }
        let string = Type::Named("std::String".to_string(), vec![]);
        let Ok(Type::Ref(ct, Mode::Shared)) = self.probe_type(&args[0], None) else { return Ok(None) };
        if !matches!(&*ct, Type::Named(n, a) if n == "std::Channel" && a.len() == 1 && a[0] == string) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if !targs.is_empty() || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) || cargs.len() != 1 {
            return Ok(None);
        }
        let k = segs.join("::");
        let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        if !matches!(k.as_str(), "std::String::clone" | "std::String::from_str") || !self.g.items.fns.get(&k).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        let cv = self.lower_expr(&args[0], None)?;
        if matches!(cv.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let cv = self.into_temp(cv, line)?;
        let Some(vv) = self.lower_raw_string(&args[1])? else { return internal("a raw value that was not made") };
        if matches!(vv.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let rt = Type::Named("std::Result".to_string(), vec![Type::Void, string.clone()]);
        let cname = self.g.request_fn(name, vec![string])?;
        let rc = self.g.ctype(&rt)?;
        let t = self.g.fresh("t");
        self.at(line);
        self.emit(&format!("{} {} = {}_raw({}, {});", rc, t, cname, cv.c, vv.c));
        if self.is_res(&rt) {
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, t));
            return Ok(Some(V { c: t, ty: rt, obj: Some(o) }));
        }
        Ok(Some(V { c: t, ty: rt, obj: None }))
    }

    pub(super) fn lower_push_fresh(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::Vec::push" || !in_prelude(f) || args.len() != 2 {
            return Ok(None);
        }
        let Ok(Type::Ref(vt, Mode::Exclusive)) = self.probe_type(&args[0], None) else { return Ok(None) };
        let Type::Named(vn, ea) = &*vt else { return Ok(None) };
        let [elem] = ea.as_slice() else { return Ok(None) };
        if vn != "std::Vec" {
            return Ok(None);
        }
        // D-0200: `replace(&mut x, String::new())` / `Vec::new()` of an
        // objectless local `x` (`take`): its value moved out as bytes, `x`
        // left empty in place; neither has an object to make or end.
        // D-0200: an objectless local moved in (`push(v, x)`): its bytes go
        // into the cell with no object, as `read_place` would move it out
        // (stale if moved already; its moved flag set, or a `String` left
        // empty in place).
        if let Some(x) = path_name(&args[1]).and_then(|n| self.lookup(n)).filter(|x| {
            x.ty == *elem && self.objectless_c.contains(&x.c) && matches!(x.kind, BindKind::Local) && self.is_res(elem) && !self.has_refs(elem) && !self.holds_fn(elem)
        }) {
            let has_flag = self.moved_flag.contains_key(&x.c);
            let is_string = matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty());
            if (has_flag || is_string) && self.confined_local(&args[0]).is_none() && self.g.probing == 0 {
                let vv = self.lower_expr(&args[0], None)?;
                if matches!(vv.ty, Type::Never) {
                    return Ok(Some(never()));
                }
                let vv = self.into_temp(vv, line)?;
                self.objectless_use(&x.c, line);
                let sc = self.g.ctype(elem)?;
                let kt = self.g.fresh("k");
                self.emit(&format!("{} {} = {};", sc, kt, x.c));
                match self.moved_flag.get(&x.c).cloned() {
                    Some(fl) => self.emit(&format!("{} = 1;", fl)),
                    None => self.emit(&format!("{}.bytes.ptr = (uint8_t *)1; {}.bytes.len = 0; {}.bytes.cap = 0;", x.c, x.c, x.c)),
                }
                let pushn = self.g.request_fn(name, vec![elem.clone()])?;
                self.emit(&format!("{}_raw({}, {});", pushn, vv.c, kt));
                return Ok(Some(unit()));
            }
        }
        if let Some(x) = self.taken_objectless(&args[1], elem) {
            if self.confined_local(&args[0]).is_some() || self.g.request_fn(name, vec![elem.clone()]).is_err() {
                return Ok(None);
            }
            let vv = self.lower_expr(&args[0], None)?;
            if matches!(vv.ty, Type::Never) {
                return Ok(Some(never()));
            }
            let vv = self.into_temp(vv, line)?;
            self.objectless_use(&x.c, line);
            let sc = self.g.ctype(elem)?;
            let kt = self.g.fresh("k");
            self.emit(&format!("{} {} = {};", sc, kt, x.c));
            let init = self.g.empty_value_init(elem)?;
            self.emit(&init.replace("@", &x.c));
            let pushn = self.g.request_fn(name, vec![elem.clone()])?;
            self.emit(&format!("{}_raw({}, {});", pushn, vv.c, kt));
            return Ok(Some(unit()));
        }
        if !matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, _) = &callee.kind else { return Ok(None) };
        let prod = segs.join("::");
        if !matches!(prod.as_str(), "std::String::from_str" | "std::String::clone" | "std::String::from_view") || cargs.len() != 1 {
            return Ok(None);
        }
        if !self.g.items.fns.get(&prod).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        // A confined vector has its own path (`lower_confined_call`).
        if self.confined_local(&args[0]).is_some() {
            return Ok(None);
        }
        let pass = match &args[0].kind {
            ExprKind::Borrow(Mode::Exclusive, place) => path_name(place).and_then(|n| self.lookup(n)).filter(|b| {
                matches!(b.kind, BindKind::Local) && !b.root.is_empty() && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && !matches!(b.ty, Type::Ref(..))
            }),
            _ => None,
        };
        let vv = if let Some(b) = pass {
            let loc = self.loc(args[0].line);
            self.emit(&format!("cb_borrow_unminted({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
            V { c: format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root), ty: Type::Ref(Box::new(b.ty.clone()), Mode::Exclusive), obj: None }
        } else {
            let vv = self.lower_expr(&args[0], None)?;
            if matches!(vv.ty, Type::Never) {
                return Ok(Some(never()));
            }
            self.into_temp(vv, line)?
        };
        let a = self.lower_expr(&cargs[0], if prod == "std::String::from_str" { Some(&Type::Str) } else { None })?;
        if matches!(a.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let a = self.into_temp(a, line)?;
        let pname = self.g.request_fn(&prod, Vec::new())?;
        let sc = self.g.ctype(elem)?;
        let kt = self.g.fresh("k");
        self.emit(&format!("{} {} = {}_raw({});", sc, kt, pname, a.c));
        let pushn = self.g.request_fn(name, vec![elem.clone()])?;
        self.emit(&format!("{}_raw({}, {});", pushn, vv.c, kt));
        Ok(Some(unit()))
    }

    // Whether argument `j` of a call to `name` goes to a native map body
    // (`native_map_op`) that only checks through its path: the map, and
    // for `get`, `get_mut` and `contains` the key.
    // D-0200: `*Option::unwrap(HashMap::get(&m, k))` for a native map of
    // plain values: the lookup's `_acc` variant (`get_mut_acc_body`: the
    // value's address, its shared element access made, NULL for no entry),
    // `unwrap`'s fault for NULL, and the value read where it lies. The
    // reference the call would return lives only inside the expression,
    // read at once: its path, the `Option`'s object and the datum's
    // transfers make nothing a check could find.
    pub(super) fn unwrapped_map_get(&mut self, inner: &Expr, line: usize) -> R<Option<P>> {
        if self.g.bootstrap {
            return Ok(None);
        }
        let ExprKind::Call(uc, uargs) = &strip_parens(inner).kind else { return Ok(None) };
        let ExprKind::Path(us, ut) = &uc.kind else { return Ok(None) };
        let [garg] = uargs.as_slice() else { return Ok(None) };
        let ExprKind::Call(gc, gargs) = &strip_parens(garg).kind else { return Ok(None) };
        let ExprKind::Path(gs, gt) = &gc.kind else { return Ok(None) };
        if !ut.is_empty() || !gt.is_empty() || (us.len() == 1 && self.lookup(&us[0]).is_some()) || (gs.len() == 1 && self.lookup(&gs[0]).is_some()) {
            return Ok(None);
        }
        let key = |segs: &[String]| {
            let k = segs.join("::");
            if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) }
        };
        let (uk, gk) = (key(us), key(gs));
        if uk != "std::Option::unwrap" || gk != "std::HashMap::get" {
            return Ok(None);
        }
        let (Some(uf), Some(gf)) = (self.g.items.fns.get(&uk).cloned(), self.g.items.fns.get(&gk).cloned()) else { return Ok(None) };
        if !in_prelude(&uf) || !self.native_map_passthrough(&gk, &gf, gargs, 0) {
            return Ok(None);
        }
        let Ok(Type::Named(_, oa)) = self.probe_type(garg, None) else { return Ok(None) };
        let [Type::Ref(vt, Mode::Shared)] = oa.as_slice() else { return Ok(None) };
        if !self.g.plain_data(vt) {
            return Ok(None);
        }
        let vt = (**vt).clone();
        self.map_get_acc = Some(gargs.as_ptr() as usize);
        self.map_get_acc_out = None;
        let v = self.lower_expr(garg, None);
        self.map_get_acc = None;
        let v = v?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(P { c: v.c, ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None }));
        }
        let Some(pv) = self.map_get_acc_out.take() else { return internal("a native map lookup that was not made") };
        let m = "`Option::unwrap` found `None`";
        self.at(line);
        self.emit(&format!("if (!{}) cb_fault_msg(\"diag.unwrap-failed\", (const uint8_t *)\"{}\", {}u, CB_NOLOC, 0);", pv, m, m.len()));
        Ok(Some(P { c: format!("(*{})", pv), ty: vt, base: None, projs: Vec::new(), is_binding: false, raw: None }))
    }

    pub(super) fn native_map_passthrough(&mut self, name: &str, f: &FnDecl, args: &[Expr], j: usize) -> bool {
        let ops = ["std::HashMap::get", "std::HashMap::get_mut", "std::HashMap::contains", "std::HashMap::entry", "std::HashMap::insert"];
        if !ops.contains(&name) || !in_prelude(f) || !(j == 0 || (j == 1 && !matches!(name, "std::HashMap::entry" | "std::HashMap::insert"))) {
            return false;
        }
        let Some(a0) = args.first() else { return false };
        let Ok(Type::Ref(mt, _)) = self.probe_type(a0, None) else { return false };
        let Type::Named(hn, kv) = &*mt else { return false };
        let [k, v] = kv.as_slice() else { return false };
        let text = matches!(k, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if hn != "std::HashMap" || !(text || matches!(k, Type::Int(_) | Type::Bool)) {
            return false;
        }
        let Ok((vs, _)) = self.g.size_align(v) else { return false };
        if vs == 0 || self.has_refs(v) || self.holds_fn(v) {
            return false;
        }
        !matches!(name, "std::HashMap::entry" | "std::HashMap::insert") || self.g.plain_data(v)
    }

    // `&x` for a reader parameter (`Gen::reader_param`: the body only
    // reads through it and hands it on to readers, keeps nothing of it and
    // returns no reference): as for a native map operation's map
    // (`native_map_passthrough`), the borrow is checked and `x`'s root
    // passed; no path is minted. No other argument names `x`, so nothing
    // evaluated beside it forms a path that the call could meet.
    pub(super) fn reader_passthrough(&mut self, name: &str, f: &FnDecl, args: &[Expr], j: usize) -> bool {
        let Some(ExprKind::Borrow(Mode::Shared, place)) = args.get(j).map(|a| &a.kind) else { return false };
        let Some(x) = path_name(place) else { return false };
        if !(appended_text(name, f, j) || (matches!(f.params.get(j).map(|p| &p.ty), Some(Type::Ref(_, Mode::Shared))) && self.g.reader_param(name, j))) {
            return false;
        }
        !args.iter().enumerate().any(|(k, b)| k != j && mentions(b, x))
    }

    // `String::append(&mut x, y)` (`native_append_bytes`,
    // `native_append_string`): the helper's body uses `s`'s path only for
    // checks through it, keeps none of it and returns nothing, as a native
    // map operation's (`native_map_passthrough`). `y` is evaluated after
    // the borrow, so it must form no path to anything: a literal, a place
    // or a shared borrow of one (`inert_view_arg`), never naming `x`.
    pub(super) fn append_passthrough(&mut self, name: &str, f: &FnDecl, args: &[Expr], j: usize) -> bool {
        let appends = matches!(name, "std::String::append" | "std::String::append_str");
        if !appends || !in_prelude(f) || j != 0 || args.len() != 2 || !inert_view_arg(&args[1]) {
            return false;
        }
        let ExprKind::Borrow(_, place) = &strip_parens(&args[0]).kind else { return false };
        let Some(x) = path_name(place) else { return false };
        let mut named = false;
        let b = Block { stmts: Vec::new(), tail: Some(Box::new(args[1].clone())) };
        visit_exprs(&b, &mut |e| {
            if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == x) {
                named = true;
            }
        });
        if named {
            return false;
        }
        // `&t` of a `String` local: its type is the binding's (a probe of a
        // borrow of an objectless local has no path to borrow).
        if let ExprKind::Borrow(Mode::Shared, tp) = &strip_parens(&args[1]).kind {
            if let Some(b) = path_name(tp).and_then(|n| self.lookup(n)) {
                if matches!(&b.ty, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
                    return true;
                }
            }
        }
        match self.probe_type(&args[1], None) {
            Ok(Type::Str | Type::Int(_) | Type::F32 | Type::F64 | Type::Bool) => true,
            Ok(Type::Named(n, a)) => n == "std::StringView" && a.is_empty(),
            Ok(Type::Ref(inner, _)) => matches!(&*inner, Type::Named(n, a) if n == "std::String" && a.is_empty()),
            _ => false,
        }
    }

    // `HashMap::entry(m, k, x)` / `insert(m, k, x)`, natively for a
    // `String` key and a plain value (`native_map_op`), where `k` is
    // `String::from_str(s)` or `String::clone(r)` (`sprintf` is the
    // former) and `x` a literal or a local: the new `String` goes from the
    // copy to the map body with no object. Its object would be established,
    // sent, received, moved, sent and received again, then destroyed or
    // moved into the keys' cells (where an object ends at once, being
    // reference-free) -- with nothing in between that could see it (`x`
    // forms no path; a fault ends the program). So the `_raw` copy and the
    // `_rawk` body pass it as a value.
    pub(super) fn lower_map_fresh_key(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if !matches!(name, "std::HashMap::entry" | "std::HashMap::insert") || !in_prelude(f) || args.len() != 3 {
            return Ok(None);
        }
        let Ok(Type::Ref(mt, Mode::Exclusive)) = self.probe_type(&args[0], None) else { return Ok(None) };
        let Type::Named(hn, kv) = &*mt else { return Ok(None) };
        let [k, v] = kv.as_slice() else { return Ok(None) };
        if hn != "std::HashMap" || !matches!(k, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
            return Ok(None);
        }
        if self.g.size_align(v)?.0 == 0 || !self.g.plain_data(v) || self.has_refs(v) || self.holds_fn(v) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, _) = &callee.kind else { return Ok(None) };
        let prod = segs.join("::");
        if !matches!(prod.as_str(), "std::String::from_str" | "std::String::clone" | "std::String::from_view") || cargs.len() != 1 {
            return Ok(None);
        }
        if !self.g.items.fns.get(&prod).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        let inert = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_)) || path_name(e).map_or(false, |n| !n.starts_with("__"));
        if !inert(&args[2]) {
            return Ok(None);
        }
        let pass = match &args[0].kind {
            ExprKind::Borrow(Mode::Exclusive, place) => path_name(place).and_then(|n| self.lookup(n)).filter(|b| {
                matches!(b.kind, BindKind::Local) && !b.root.is_empty() && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && !matches!(b.ty, Type::Ref(..))
            }),
            _ => None,
        };
        let mv = if let Some(b) = pass {
            // As `native_map_passthrough` does for an ordinary call.
            let loc = self.loc(args[0].line);
            self.emit(&format!("cb_borrow_unminted({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
            V { c: format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root), ty: Type::Ref(Box::new(b.ty.clone()), Mode::Exclusive), obj: None }
        } else {
            let mv = self.lower_expr(&args[0], None)?;
            if matches!(mv.ty, Type::Never) {
                return Ok(Some(never()));
            }
            self.into_temp(mv, line)?
        };
        // D-0200: a bare view (`direct_refs`: a fused loop's piece, or one
        // trimmed from it) is its bytes alone; no check reads them, so it
        // is handed over as it is, with no object of its own.
        let bare = (prod == "std::String::from_view")
            .then(|| path_name(&cargs[0]).and_then(|n| self.lookup(n)).filter(|b| self.direct_refs.contains(&format!("{}.bytes", b.c))))
            .flatten();
        let a = if let Some(b) = bare {
            V { c: b.c, ty: b.ty, obj: None }
        } else {
            let a = self.lower_expr(&cargs[0], if prod == "std::String::from_str" { Some(&Type::Str) } else { None })?;
            if matches!(a.ty, Type::Never) {
                return Ok(Some(never()));
            }
            self.into_temp(a, line)?
        };
        let pname = self.g.request_fn(&prod, Vec::new())?;
        let sc = self.g.ctype(k)?;
        let kt = self.g.fresh("k");
        self.emit(&format!("{} {} = {}_raw({});", sc, kt, pname, a.c));
        let x = self.lower_expr(&args[2], Some(v))?;
        let x = self.into_temp(x, line)?;
        let ename = self.g.request_fn(name, vec![k.clone(), v.clone()])?;
        let ret = if name == "std::HashMap::entry" { Type::Ref(Box::new(v.clone()), Mode::Exclusive) } else { Type::Named("std::Option".to_string(), vec![v.clone()]) };
        if self.map_acc == Some(args.as_ptr() as usize) && name == "std::HashMap::entry" {
            self.map_acc = None;
            let vc = self.g.ctype(v)?;
            let pv = self.g.fresh("pv");
            self.emit(&format!("{} *{} = {}_rawk_acc({}, {}, {});", vc, pv, ename, mv.c, kt, x.c));
            self.map_acc_used = true;
            return Ok(Some(V { c: pv, ty: ret, obj: None }));
        }
        let call = format!("{}_rawk({}, {}, {})", ename, mv.c, kt, x.c);
        self.finish_call(call, ret, line).map(Some)
    }

    // Finishes a call whose C expression is built: the result's type
    // decides how it lands (void, reference, resource via the channel,
    // or a plain value).
    pub(super) fn finish_call(&mut self, call: String, ret: Type, line: usize) -> R<V> {
        if matches!(ret, Type::Void | Type::Never) {
            self.emit(&format!("{};", call));
            return Ok(if matches!(ret, Type::Never) { never() } else { unit() });
        }
        if matches!(ret, Type::Ref(..)) {
            let r = self.g.fresh("r");
            self.emit(&format!("cb_ref {} = {};", r, call));
            self.emit("cb_recv_ref();");
            return Ok(V { c: r, ty: ret, obj: None });
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&ret)?;
        let marked = self.raw_call.as_ref().map_or(false, |n| call.starts_with(&format!("{}{}(", n, ARRIVE_MARK)));
        if let Some(n) = self.raw_call.clone().filter(|n| marked || call.starts_with(&format!("{}(", n))) {
            // D-0196: into an objectless binding: the `__raw` form, whose
            // result has no object to receive (its `__arrive` twin where the
            // call asks for one, D-0219).
            self.raw_call = None;
            let rest = if marked { &call[n.len() + ARRIVE_MARK.len()..] } else { &call[n.len()..] };
            self.emit(&format!("{} {} = {}__raw{}{};", c, t, n, if marked { ARRIVE_MARK } else { "" }, rest));
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty: ret, obj: None });
        }
        self.emit(&format!("{} {} = {};", c, t, call));
        let obj = if self.is_res(&ret) {
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, t));
            Some(o)
        } else if self.has_refs(&ret) {
            let tid = self.tid(&ret)?;
            self.emit(&format!("cb_recv_datum(&{}, {}u);", t, tid));
            let loc = self.loc(line);
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
            Some(o)
        } else {
            None
        };
        Ok(V { c: t, ty: ret, obj })
    }
}
