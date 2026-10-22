// Fx: Vec indexing and calls with confined or checked-once arguments.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // `Vec::index_shared(&v, i)` / `Vec::index_exclusive(&mut v, i)` with the
    // borrow written in the call: `cb_borrow_check` makes the borrow's
    // check and the body's read check at once and mints no token (the
    // borrow would be unheld and end with the statement), then the
    // `…_pre` body does the rest. Any other first argument takes the
    // ordinary call.
    //
    // With `access` (`Some(write)`), the call is the operand of a `*`
    // that is read, or assigned a stateless value, at once
    // (`elem_access_place`), and a plain element type makes the result
    // the element's address rather than a reference: see
    // `cb_elem_access`. Any other element type returns the reference.
    // `[Index-Vec]` (D-0047): `e[k]` on a `Vec` is `*Vec::index_shared(&e, k)`,
    // or `*Vec::index_exclusive(&mut e, k)` where the place is written, with
    // `$` the `Vec`'s length (bound where the call's index is lowered). The
    // element is its own object, as `coby` has it: a borrow of it is not a
    // borrow of the whole `Vec`. `None` for anything else.
    pub(super) fn vec_index_call(&mut self, e: &Expr, write: bool) -> R<Option<std::rc::Rc<Expr>>> {
        let ExprKind::Index(base, idx) = &e.kind else { return Ok(None) };
        let key = (e as *const Expr as usize, write);
        if let Some(x) = self.vec_index_exprs.get(&key) {
            return Ok(Some(x.clone()));
        }
        // The base's type without reading it: a place's as a place
        // (reading a resource field as a value would move it); anything
        // else (a call's result) as the value it is.
        let is_place = {
            let mut x: &Expr = base;
            while let ExprKind::Paren(y) = &x.kind {
                x = y;
            }
            matches!(x.kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_))
        };
        // A confined parameter's `(*v)[k]` is lowered as its `v[k]`: the
        // base, parentheses stripped, is the dereference of the name, and
        // the place indexed is that dereference.
        let mut deref_place: Option<Expr> = None;
        let local_ty = match &strip_parens(base).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => match self.lookup(&segs[0]) {
                // A name a closure captured by borrow is `*self.f_i`: its
                // place is the captured value, not the reference.
                Some(b) => Some(match (&b.kind, b.ty) {
                    (BindKind::CaptureRef(_), Type::Ref(t, _)) => *t,
                    (_, t) => t,
                }),
                // A confined parameter (`lower_fn`'s mask) has no binding:
                // it is a reference to a `Vec` of the element type recorded.
                None => self.confined_params.get(&segs[0]).map(|(_, _, el)| {
                    Type::Ref(Box::new(Type::Named("std::Vec".to_string(), vec![el.clone()])), Mode::Shared)
                }),
            },
            // `(*v)[k]` on a confined parameter `v` (which has no binding,
            // so probing the place would not resolve the name): the type
            // is the `Vec` itself, and the place `*v`.
            ExprKind::Deref(inner) => match &strip_parens(inner).kind {
                ExprKind::Path(segs, _) if segs.len() == 1 && self.lookup(&segs[0]).is_none() => {
                    self.confined_params.get(&segs[0]).map(|(_, _, el)| {
                        deref_place = Some(Expr { kind: ExprKind::Deref(Box::new(strip_parens(inner).clone())), line: e.line, col: e.col });
                        Type::Named("std::Vec".to_string(), vec![el.clone()])
                    })
                }
                _ => None,
            },
            _ => None,
        };
        let bt = if let Some(t) = local_ty {
            t
        } else if is_place {
            self.probe_place_type(base)?
        } else {
            self.probe_type(base, None)?
        };
        let (through_ref, inner) = match &bt {
            Type::Ref(t, _) => (true, (**t).clone()),
            t => (false, t.clone()),
        };
        if !matches!(&inner, Type::Named(n, _) if n == "std::Vec") {
            return Ok(None);
        }
        let line = e.line;
        let place = if let Some(d) = deref_place { d } else if through_ref { Expr { kind: ExprKind::Deref(base.clone()), line, col: e.col } } else { (**base).clone() };
        let (fname, mode) = if write { ("index_exclusive", Mode::Exclusive) } else { ("index_shared", Mode::Shared) };
        let callee = Expr { kind: ExprKind::Path(vec!["std".to_string(), "Vec".to_string(), fname.to_string()], Vec::new()), line, col: e.col };
        let arg0 = Expr { kind: ExprKind::Borrow(mode, Box::new(place)), line, col: e.col };
        let call = Expr { kind: ExprKind::Call(Box::new(callee), vec![arg0, (**idx).clone()]), line, col: e.col };
        let x = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(call)), line, col: e.col });
        if let ExprKind::Deref(c) = &x.kind {
            if let ExprKind::Call(_, args) = &c.kind {
                self.index_sugar.insert(&args[0] as *const Expr as usize);
            }
        }
        self.vec_index_exprs.insert(key, x.clone());
        Ok(Some(x))
    }

    // `$` for the index argument of `Vec::index_*` (only `vec_index_call`
    // writes one there: `$` anywhere else is rejected statically).
    pub(super) fn lower_index_arg(&mut self, len: String, a: &Expr) -> R<V> {
        self.dollar.push(len);
        let r = self.lower_expr(a, Some(&Type::Int(IntTy::Usize)));
        self.dollar.pop();
        r
    }

    pub(super) fn lower_vec_index_pre(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize, access: Option<bool>) -> R<Option<V>> {
        if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
            return Ok(None);
        }
        let in_prelude = matches!(f.body.stmts.first(), Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) if e.line <= prelude::line_count());
        let (true, [a0, a1]) = (in_prelude, args) else { return Ok(None) };
        let ExprKind::Borrow(mode, inner) = &a0.kind else { return Ok(None) };
        // `e[k]` (`vec_index_call`): `[Index-Vec]` checks the borrow as an
        // access to element `k` (D-0070), after `k` is evaluated, as `coby`
        // does: a live slice of other elements is not in the way, the
        // `Vec` borrowed whole is. Any index will do.
        let sugar = self.index_sugar.contains(&(a0 as *const Expr as usize));
        // The index is evaluated after the borrow's check here, where the
        // ordinary call makes the read check after it; only an index that
        // cannot change the runtime's state keeps the two in agreement.
        // D-0274: in the bootstrap form only the sugar, which `[Index-Vec]`
        // needs; a stateless index is an optimisation.
        let stateless = !self.g.bootstrap && self.stateless(a1);
        if !stateless && !sugar {
            return Ok(None);
        }
        // D-0103: a `Vec` that is part of a temporary argument.
        self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(a0 as *const Expr as usize));
        let p = self.lower_place(inner);
        self.temp_root = false;
        let p = p?;
        let Some(base) = p.base.clone() else { return Ok(None) };
        let elem = match &p.ty {
            Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => a[0].clone(),
            _ => return Ok(None),
        };
        let loc = self.loc(a0.line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        if !sugar {
            self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
        } else if !stateless {
            // The index may read `$` or run code: the place is valid first.
            self.emit(&format!("cb_valid({}, {});", base, loc));
        }
        let i = self.lower_index_arg(format!("({}).len", p.c), a1)?;
        if matches!(i.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let it = self.into_temp(i, a1.line)?;
        if sugar {
            let mut projs = p.projs.clone();
            projs.push(format!("{{ CB_INDEX, {} }}", it.c));
            self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&projs), m, loc));
        }
        let cname = self.g.request_fn(name, vec![elem.clone()])?;
        let rmode = if name == "std::Vec::index_exclusive" { Mode::Exclusive } else { Mode::Shared };
        let plain = !self.is_res(&elem) && !self.has_refs(&elem) && !self.holds_fn(&elem) && !matches!(elem, Type::Closure(_));
        let reader_read = self.elem_reader && access == Some(false) && rmode == Mode::Shared && !self.has_refs(&elem) && !self.holds_fn(&elem) && !matches!(elem, Type::Closure(_) | Type::Ref(..));
        if let (Some(write), true) = (access, plain || reader_read) {
            // The `…_pre` body inline (its location, its bounds fault, the
            // element's address), then the access's checks in one call.
            let vec_c = self.g.ctype(&p.ty)?;
            let elem_c = self.g.ctype(&elem)?;
            let tid = self.tid(&elem)?;
            let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
            let rm = if rmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            let rm = self.elem_mode.unwrap_or(rm);
            self.at(line);
            self.emit(&format!("{} *{} = ({} *)&({});", vec_c, v, vec_c, p.c));
            self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
            self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
            let loc = self.loc(line);
            self.emit(&format!("cb_elem_access({}, sizeof(*{}), {}u, {}, {}, {});", e, e, tid, rm, write as u32, loc));
            return Ok(Some(V { c: e, ty: elem, obj: None }));
        }
        let r = self.g.fresh("r");
        if self.g.bootstrap {
            // D-0274: no native bodies, so no `…_pre` variant: its body inline.
            let vec_c = self.g.ctype(&p.ty)?;
            let elem_c = self.g.ctype(&elem)?;
            let tid = self.tid(&elem)?;
            let rm = if rmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
            self.at(line);
            self.emit(&format!("{} *{} = ({} *)&({});", vec_c, v, vec_c, p.c));
            self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
            self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
            self.emit(&format!("cb_ref {} = (cb_ref){{ (void *){}, cb_elem_borrow_here({}, {}u, {}) }};", r, e, e, tid, rm));
            return Ok(Some(V { c: r, ty: Type::Ref(Box::new(elem), rmode), obj: None }));
        }
        self.emit(&format!("cb_ref {} = {}_pre(((cb_ref){{ (void *)&({}), 0 }}), {});", r, cname, p.c, it.c));
        Ok(Some(V { c: r, ty: Type::Ref(Box::new(elem), rmode), obj: None }))
    }

    // `Vec::push(&mut x, e)` and `Vec::len(&x)` for a confined `x`
    // (`bind_confined`): the native bodies without their checks, and no
    // path formed for the borrow. `grow` is handed `x`'s root path.
    pub(super) fn lower_confined_call(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if CONFINING_CALLS.contains(&name) && in_prelude(f) {
            return self.lower_confining_call(name, f, args, line);
        }
        if (name != "std::Vec::push" && name != "std::Vec::len" && name != "std::Vec::pop" && name != "std::Vec::reserve") || !in_prelude(f) {
            return Ok(None);
        }
        if name == "std::Vec::len" {
            if let Some(vec) = args.first().and_then(|a| self.confined_field(a).map(|x| x.0).or_else(|| self.read_confined_vec(a).map(|x| x.0))) {
                let t = self.g.fresh("t");
                self.emit(&format!("uint64_t {} = ({}).len;", t, vec));
                return Ok(Some(V { c: t, ty: Type::Int(IntTy::Usize), obj: None }));
            }
        }
        let Some((vec, tok, elem)) = args.first().and_then(|a| self.confined_local(a)) else { return Ok(None) };
        if name == "std::Vec::reserve" {
            // The body, handed the root path as `grow` is: its checks
            // through it cannot fail (`bind_confined`).
            let v = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
            if matches!(v.ty, Type::Never) {
                return Ok(Some(never()));
            }
            let n = self.into_temp(v, args[1].line)?;
            let cname = self.g.request_fn(name, vec![elem])?;
            self.emit(&format!("{}(((cb_ref){{ (void *)&({}), {} }}), {});", cname, vec, tok, n.c));
            return Ok(Some(unit()));
        }
        if name == "std::Vec::pop" {
            let cname = self.g.request_fn(name, vec![elem.clone()])?;
            let opt = Type::Named("std::Option".to_string(), vec![elem]);
            let opt_c = self.g.ctype(&opt)?;
            let t = self.g.fresh("t");
            self.emit(&format!("{} {} = {}_nc(((cb_ref){{ (void *)&({}), {} }}));", opt_c, t, cname, vec, tok));
            return Ok(Some(V { c: t, ty: opt, obj: None }));
        }
        if name == "std::Vec::len" {
            let t = self.g.fresh("t");
            self.emit(&format!("uint64_t {} = ({}).len;", t, vec));
            return Ok(Some(V { c: t, ty: Type::Int(IntTy::Usize), obj: None }));
        }
        let v = self.lower_expr(&args[1], Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, args[1].line)?;
        let cname = self.g.request_fn(name, vec![elem])?;
        self.emit(&format!("{}_nc(((cb_ref){{ (void *)&({}), {} }}), {});", cname, vec, tok, x.c));
        Ok(Some(unit()))
    }

    // A `CONFINING_CALLS` call on a confined vector: no path is formed for
    // the vector argument, which is passed with its root path; the other
    // arguments are lowered as for any call, in order. With a native body
    // for the element type, its unchecked form (`nc_variant`); otherwise
    // the body itself, whose checks through the root cannot fail.
    pub(super) fn lower_confining_call(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        let Some((vec, tok, elem)) = args.first().and_then(|a| self.confined_local(a)) else { return Ok(None) };
        let subst = subst_of(&f.type_params, &[elem.clone()]);
        let mut cargs = vec![format!("((cb_ref){{ (void *)&({}), {} }})", vec, tok)];
        let key_native = matches!(name, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search") && matches!(elem, Type::Int(_) | Type::Bool);
        for (a, p) in args.iter().zip(&f.params).skip(1) {
            // A temporary key for a native search (`lower_user_call`'s
            // `native_key`): its value's address, no object made.
            if key_native {
                if let ExprKind::Borrow(Mode::Shared, inner) = &a.kind {
                    if is_place_expr(inner) && root_bound(inner, &|n| self.lookup(n).is_some()) {
                        // A place: its borrow's check, its address (a pure
                        // direct parameter, `pure_direct_in`).
                        let pl = self.lower_place(inner)?;
                        if let (Some(base), None) = (pl.base.clone(), &pl.raw) {
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_borrow_unminted({}, {}, CB_SHARED, {});", base, Self::projs_args(&pl.projs), loc));
                        }
                        let ad = self.g.fresh("ad");
                        self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                        cargs.push(format!("((cb_ref){{ {}, 0 }})", ad));
                        continue;
                    }
                    if !is_place_expr(inner) {
                        let v = self.lower_expr(inner, Some(&elem))?;
                        if matches!(v.ty, Type::Never) {
                            return Ok(Some(never()));
                        }
                        let kc = self.g.ctype(&v.ty)?;
                        let k = self.g.fresh("key");
                        self.emit(&format!("{} {} = {};", kc, k, v.c));
                        cargs.push(format!("((cb_ref){{ (void *)&{}, 0 }})", k));
                        continue;
                    }
                }
            }
            let want = apply(&p.ty, &subst);
            let v = self.lower_expr(a, Some(&want))?;
            if matches!(v.ty, Type::Never) {
                return Ok(Some(never()));
            }
            cargs.push(self.into_temp(v, a.line)?.c);
        }
        self.at_after_arg_calls(f, args, line);
        let (size, _) = self.g.size_align(&elem)?;
        let native = size > 0
            && self.g.plain_data(&elem)
            && (!matches!(name, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search" | "std::Vec::sort") || matches!(elem, Type::Int(_) | Type::Bool));
        let cname = self.g.request_fn(name, vec![elem])?;
        let callee = if native { format!("{}_nc", cname) } else { cname };
        let ret = apply(&f.ret, &subst);
        let call = format!("{}({})", callee, cargs.join(", "));
        if matches!(ret, Type::Void) {
            self.emit(&format!("{};", call));
            return Ok(Some(unit()));
        }
        let rc = self.g.ctype(&ret)?;
        let t = self.g.fresh("t");
        self.emit(&format!("{} {} = {};", rc, t, call));
        Ok(Some(V { c: t, ty: ret, obj: None }))
    }

    // `Vec::push(&mut x, e)` for a checked local `x` and a plain element
    // type, with `e` an expression that can neither move nor end `x` (no
    // call but a conversion, no assignment): `cb_borrow_check` makes the
    // borrow's check and the body's first read check at once, as for
    // `Vec::len`; the body's other checks are through that path with
    // nothing in between that could change their outcome, and the path,
    // unheld, would end with the statement unobserved (D-0018). Then the
    // native body without its checks (`_nc`), handed `x`'s root path, as
    // a confined `x`'s push is.
    pub(super) fn lower_push_checked_once(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::push" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0, a1] = args else { return Ok(None) };
        let ExprKind::Borrow(Mode::Exclusive, inner) = &a0.kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &inner.kind else { return self.lower_push_place_once(f, inner, a0, a1) };
        if segs.len() != 1 || !targs.is_empty() {
            return Ok(None);
        }
        let Some(b) = self.lookup(&segs[0]) else { return Ok(None) };
        if !matches!(b.kind, BindKind::Local) || b.root.is_empty() || self.direct_refs.contains(&b.c) {
            return Ok(None);
        }
        let Type::Named(n, a) = &b.ty else { return Ok(None) };
        if n != "std::Vec" || a.len() != 1 {
            return Ok(None);
        }
        let elem = a[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) || !self.inert(a1) {
            return Ok(None);
        }
        let loc = self.loc(a0.line);
        self.emit(&format!("cb_borrow_check({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
        let v = self.lower_expr(a1, Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, a1.line)?;
        let cname = self.g.request_fn(name, vec![elem])?;
        self.emit(&format!("{}_nc(((cb_ref){{ (void *)&({}), {} }}), {});", cname, b.c, b.root, x.c));
        Ok(Some(unit()))
    }

    // D-0209: a call of a function lowered with direct-parameter twins
    // (`Gen::lower_fn_direct`) whose every direct reference argument is a
    // borrow rooted at an owned local of the caller -- `&x`, `&mut x.f`,
    // `&mut v[i]`, never through a reference -- goes to `NAME__dir` itself,
    // the dispatcher's test made at the call site: the object is the
    // caller's own storage, which no other thread can reach (a reference to
    // it held elsewhere would have clashed with the borrow just formed, or
    // is shared and cannot release it), so `cb_direct_ok` would pass, and
    // the checked twin could fault nowhere the direct one does not
    // (`spec/08` §4 `[Standing-Local]` H5: the object is fresh storage, or
    // nothing can release it while the call runs).
    // D-0219: whether a call's reference arguments need no arrival test: at
    // most one argument is a reference or slice, and it is one whose path can
    // have no live descendant -- a borrow formed for the call, the result of
    // a function that returns a borrow it formed (`returns_fresh_borrow`), or
    // a direct parameter of a body entered only through an arrival test.
    // Every other path to the place would have clashed with its formation;
    // a descendant is what the cheap test does not see. Two reference
    // arguments are never vouched: a borrow in argument position is pending
    // (D-0107), so two of them, or a parameter beside a borrow made from it,
    // may overlap without a clash.
    pub(super) fn vouched_args(&self, f: &FnDecl, inst: &[Type], args: &[Expr]) -> bool {
        let sub = subst_of(&f.type_params, inst);
        let refs: Vec<&Expr> = args.iter().zip(&f.params).filter(|(_, p)| matches!(apply(&p.ty, &sub), Type::Ref(..) | Type::Slice(..))).map(|(a, _)| a).collect();
        match refs.as_slice() {
            [] => true,
            [a] => self.fresh_ref_arg(a),
            _ => false,
        }
    }

    pub(super) fn fresh_ref_arg(&self, a: &Expr) -> bool {
        match &strip_parens(a).kind {
            ExprKind::Borrow(..) | ExprKind::SliceOf(..) => true,
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                self.entry_tested && self.direct_params.contains(&segs[0]) && self.lookup(&segs[0]).map_or(false, |b| b.c == format!("p_{}", san(&segs[0])))
            }
            ExprKind::Call(callee, _) => match &strip_parens(callee).kind {
                ExprKind::Path(segs, _) => self.g.items.fns.get(&segs.join("::")).map_or(false, |f| returns_fresh_borrow(f)),
                _ => false,
            },
            _ => false,
        }
    }

    pub(super) fn direct_local_call(&mut self, name: &str, f: &FnDecl, inst: &[Type], mask: u64, args: &[Expr]) -> R<bool> {
        if self.g.all_checks || mask != 0 || !inst.is_empty() || !f.type_params.is_empty() || in_prelude(f) || name == "main" || args.len() != f.params.len() || self.raw_call.is_some() {
            return Ok(false);
        }
        let subst = HashMap::new();
        if self.g.dual_eligible(f, &subst)?.is_some() || self.g.field_dual_eligible(f, &subst)?.is_some() {
            return Ok(false);
        }
        let direct = self.g.direct_ref_params(f, &subst)?;
        if direct.is_empty() {
            return Ok(false);
        }
        let frozen = self.g.unstored_ref_params(f, &subst, &direct)?.into_iter().any(|n| f.params.iter().any(|p| p.name == n && matches!(p.ty, Type::Ref(_, Mode::Shared))));
        if frozen {
            return Ok(false);
        }
        let mut roots: Vec<String> = Vec::new();
        for (a, p) in args.iter().zip(&f.params) {
            let Type::Ref(_, pm) = &p.ty else { continue };
            if !direct.contains(&p.name) {
                continue;
            }
            let ExprKind::Borrow(am, inner) = &strip_parens(a).kind else { return Ok(false) };
            if am != pm || !self.owned_local_root(inner)? {
                return Ok(false);
            }
            let Some(r) = local_root_name(inner) else { return Ok(false) };
            roots.push(r);
        }
        // CHG-0342: the borrow's check is made where the borrow is, and
        // the direct body makes none at its entry (`spec/08` §4: the
        // arrival test). So no other argument may name a direct root: a
        // later argument that lends the same local to a thread
        // (`g(&mut x, spawn(f, &x))`) would form a lasting path the
        // direct body never sees, where the dispatcher's entry test
        // (`cb_direct_ok`) or the checked twin would find it.
        for (a, p) in args.iter().zip(&f.params) {
            let own = match (&p.ty, &strip_parens(a).kind) {
                (Type::Ref(..), ExprKind::Borrow(_, inner)) if direct.contains(&p.name) => local_root_name(inner),
                _ => None,
            };
            if roots.iter().any(|r| Some(r) != own.as_ref() && mentions(a, r)) {
                return Ok(false);
            }
        }
        Ok(true)
    }

}

/// The local `x` at the root of `x`, `x.f`, `x[i]`, … (`owned_local_root`'s shape).
pub(super) fn local_root_name(e: &Expr) -> Option<String> {
    let mut x = strip_parens(e);
    loop {
        match &x.kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) => x = strip_parens(a),
            ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty() => return Some(segs[0].clone()),
            _ => return None,
        }
    }
}

impl<'g, 'a> Fx<'g, 'a> {
    // Whether `e` is `x`, or fields and indexes (of a `Vec` or an array)
    // of it in any order, for `x` an owned local that is neither a
    // reference nor a slice, with no reference along the way.
    pub(super) fn owned_local_root(&mut self, e: &Expr) -> R<bool> {
        let mut x = strip_parens(e);
        loop {
            match &x.kind {
                ExprKind::Field(a, _) => {
                    let t = self.probe_type(strip_parens(a), None)?;
                    if !matches!(t, Type::Named(..) | Type::Array(..)) {
                        return Ok(false);
                    }
                    x = strip_parens(a);
                }
                ExprKind::Index(a, _) => {
                    let t = self.probe_type(strip_parens(a), None)?;
                    if !matches!(&t, Type::Named(n, _) if n == "std::Vec") && !matches!(t, Type::Array(..)) {
                        return Ok(false);
                    }
                    x = strip_parens(a);
                }
                ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty() => {
                    let Some(b) = self.lookup(&segs[0]) else { return Ok(false) };
                    return Ok(matches!(b.kind, BindKind::Local) && !matches!(b.ty, Type::Ref(..) | Type::Slice(..)));
                }
                _ => return Ok(false),
            }
        }
    }

    // D-0192: a call of a D-0188 dual-body function whose reference and
    // slice arguments are each rooted at a distinct owned local (`&mut x`,
    // `&x.f`, `&v[lo..hi]`; never through a reference, never an element of a
    // `Vec`), its other arguments naming none of them: they cannot alias, so
    // the dispatcher's test is the call site's to make. Each borrow's check
    // is made (`cb_borrow_unminted`, `lower_slice_of_as` unminted, no path)
    // and the bare form called (`lower_fn_dual`), which tests its slices for
    // an element with an object itself.
    pub(super) fn lower_dual_bare_call(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if self.g.all_checks || !f.type_params.is_empty() || args.len() != f.params.len() || name == "main" {
            return Ok(None);
        }
        let subst = HashMap::new();
        if self.g.dual_eligible(f, &subst)?.is_none() {
            return Ok(None);
        }
        // The owned local each reference argument is rooted at.
        let root_of = |fx: &Self, e: &Expr, allow_index: bool| -> Option<String> {
            let mut x = strip_parens(e);
            loop {
                match &x.kind {
                    ExprKind::Field(a, _) => x = strip_parens(a),
                    ExprKind::Index(a, _) if allow_index => x = strip_parens(a),
                    ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty() => {
                        let b = fx.lookup(&segs[0])?;
                        if !matches!(b.kind, BindKind::Local) || matches!(b.ty, Type::Ref(..) | Type::Slice(..)) {
                            return None;
                        }
                        return Some(segs[0].clone());
                    }
                    _ => return None,
                }
            }
        };
        let mut roots: Vec<String> = Vec::new();
        for (a, p) in args.iter().zip(&f.params) {
            match (&p.ty, &a.kind) {
                (Type::Ref(_, pm), ExprKind::Borrow(am, inner)) if pm == am => {
                    let Some(r) = root_of(self, inner, false) else { return Ok(None) };
                    roots.push(r);
                }
                (Type::Slice(_, pm), ExprKind::SliceOf(am, base, _, _)) if pm == am => {
                    let Some(r) = root_of(self, base, false) else { return Ok(None) };
                    roots.push(r);
                }
                (Type::Ref(..) | Type::Slice(..), _) => return Ok(None),
                _ => {}
            }
        }
        let distinct: HashSet<&String> = roots.iter().collect();
        if distinct.len() != roots.len() {
            return Ok(None);
        }
        for (a, p) in args.iter().zip(&f.params) {
            if !matches!(p.ty, Type::Ref(..) | Type::Slice(..)) && roots.iter().any(|r| mentions(a, r)) {
                return Ok(None);
            }
        }
        let mut cargs = Vec::new();
        for (a, p) in args.iter().zip(&f.params) {
            match (&p.ty, &a.kind) {
                (Type::Ref(_, pm), ExprKind::Borrow(_, inner)) => {
                    let pl = self.lower_place(inner)?;
                    let pl = self.auto_deref(pl)?;
                    if let Some(base) = &pl.base {
                        let loc = self.loc(a.line);
                        let m = if *pm == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                        self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&pl.projs), m, loc));
                    } else if pl.raw.is_some() {
                        return internal("a bare dual call's argument through a raw pointer");
                    }
                    let ad = self.g.fresh("ad");
                    self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                    cargs.push(format!("((cb_ref){{ {}, 0 }})", ad));
                }
                (Type::Slice(..), ExprKind::SliceOf(sm, base, lo, hi)) => {
                    let v = self.lower_slice_of_as(sm, base, lo, hi, a.line, false)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(Some(never()));
                    }
                    cargs.push(self.into_temp(v, a.line)?.c);
                }
                _ => {
                    let v = self.lower_expr(a, Some(&p.ty))?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(Some(never()));
                    }
                    cargs.push(self.into_temp(v, a.line)?.c);
                }
            }
        }
        let cname = self.g.request_fn(name, Vec::new())?;
        self.finish_call(format!("{}__bare({})", cname, cargs.join(", ")), f.ret.clone(), line).map(Some)
    }

    // D-0192: `Vec::swap`, `reverse`, `insert`, `remove`, `truncate`,
    // `clear`, `contains`, `index_of` and `binary_search` on `&mut x` / `&x`
    // for a local `x` that is not confined, as `lower_push_checked_once`
    // does `push`: the borrow's check (`cb_borrow_check`, the call's mode;
    // made where the borrow would be formed, before the other arguments)
    // and `x`'s root path handed to the native body, whose checks through
    // it are those through the borrow: the borrow would be the root's only
    // descendant the call adds, unheld, and no other argument names `x`
    // (an alias of it would have failed the check). No path minted.
    pub(super) fn lower_vec_checked_once(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if !ONCE_CALLS.contains(&name) || !in_prelude(f) {
            return Ok(None);
        }
        let Some(a0) = args.first() else { return Ok(None) };
        let ExprKind::Borrow(mode, inner) = &a0.kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &inner.kind else { return Ok(None) };
        if segs.len() != 1 || !targs.is_empty() {
            return Ok(None);
        }
        let Some(b) = self.lookup(&segs[0]) else { return Ok(None) };
        if !matches!(b.kind, BindKind::Local) || b.root.is_empty() || self.direct_refs.contains(&b.c) || self.confined.contains(&b.c) {
            return Ok(None);
        }
        let Type::Named(n, ta) = &b.ty else { return Ok(None) };
        if n != "std::Vec" || ta.len() != 1 {
            return Ok(None);
        }
        let elem = ta[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.movable_elem(&elem) {
            return Ok(None);
        }
        if matches!(name, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search") && self.g.search_ops(&elem).is_none() {
            return Ok(None);
        }
        if args.iter().skip(1).any(|a| mentions(a, &segs[0])) {
            return Ok(None);
        }
        let loc = self.loc(a0.line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        self.emit(&format!("cb_borrow_check({}, NULL, 0, {}, {});", b.root, m, loc));
        let subst = subst_of(&f.type_params, &[elem.clone()]);
        let ptypes: Vec<Type> = f.params.iter().skip(1).map(|p| apply(&p.ty, &subst)).collect();
        // A search's key, a place: its borrow's check and its address (the
        // key is pure direct, `pure_direct_in`, so a local lent only as one
        // may have no object).
        let key_place = matches!(name, "std::Vec::contains" | "std::Vec::index_of" | "std::Vec::binary_search")
            && matches!(&args[1].kind, ExprKind::Borrow(Mode::Shared, k) if is_place_expr(k) && root_bound(k, &|n| self.lookup(n).is_some()));
        let names = if key_place {
            let ExprKind::Borrow(_, k) = &args[1].kind else { unreachable!() };
            let pl = self.lower_place(k)?;
            if let (Some(base), None) = (pl.base.clone(), &pl.raw) {
                let kloc = self.loc(args[1].line);
                self.emit(&format!("cb_borrow_unminted({}, {}, CB_SHARED, {});", base, Self::projs_args(&pl.projs), kloc));
            }
            let ad = self.g.fresh("ad");
            self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
            vec![format!("((cb_ref){{ {}, 0 }})", ad)]
        } else {
            let Some(names) = self.lower_args(&ptypes, &args[1..])? else { return Ok(Some(never())) };
            names
        };
        self.at_after_arg_calls(f, args, line);
        let cname = format!("{}_once", self.g.request_fn(name, vec![elem])?);
        let mut cargs = vec![format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root)];
        cargs.extend(names);
        let ret = apply(&f.ret, &subst);
        self.finish_call(format!("{}({})", cname, cargs.join(", ")), ret, line).map(Some)
    }

    // `lower_push_checked_once` for `&mut p` with `p` a place other than a
    // bare local (`&mut s.bytes`): the same single check; the body inline.
    // `grow`, when needed, is handed a path formed for it then -- the
    // borrow the call would have formed, which nothing since could have
    // made fail -- unstamped, and ended at once.
    pub(super) fn lower_push_place_once(&mut self, f: &FnDecl, place: &Expr, a0: &Expr, a1: &Expr) -> R<Option<V>> {
        if !in_prelude(f) || !self.inert(a1) || !root_bound(place, &|n| self.lookup(n).is_some()) {
            return Ok(None);
        }
        let pt = self.probe_type(place, None)?;
        let Type::Named(n, a) = &pt else { return Ok(None) };
        if n != "std::Vec" || a.len() != 1 {
            return Ok(None);
        }
        let elem = a[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) {
            return Ok(None);
        }
        let pl = self.lower_place(place)?;
        let (Some(base), None) = (pl.base.clone(), &pl.raw) else { return Ok(None) };
        let loc = self.loc(a0.line);
        let projs = Self::projs_args(&pl.projs);
        self.emit(&format!("cb_borrow_check({}, {}, CB_EXCLUSIVE, {});", base, projs, loc));
        let v = self.lower_expr(a1, Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, a1.line)?;
        let vec_c = self.g.ctype(&pt)?;
        let grow = self.g.request_fn("std::Vec::grow", vec![elem])?;
        let vv = self.g.fresh("vv");
        self.emit(&format!("{} *{} = &({});", vec_c, vv, pl.c));
        self.emit(&format!(
            "if ({vv}->len == {vv}->cap) {{ uint64_t gt = cb_borrow_unstamped({base}, {projs}, CB_EXCLUSIVE, {loc}); {grow}(((cb_ref){{ (void *){vv}, gt }})); cb_call_done(gt); }}",
            vv = vv, base = base, projs = projs, loc = loc, grow = grow
        ));
        self.emit(&format!("{vv}->ptr[{vv}->len] = {x}; {vv}->len += 1;", vv = vv, x = x.c));
        Ok(Some(unit()))
    }

    // An expression whose evaluation moves, ends or rebinds nothing: the
    // places it reads, literals, operators, and the conversions.
    pub(super) fn inert(&self, e: &Expr) -> bool {
        const CONVERSIONS: &[&str] = &["str_byte", "str_len", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul"];
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Dollar => true,
            // A local's read, or a constant's; not a fn item or a variant
            // (a value built, but kept simple).
            ExprKind::Path(segs, targs) => segs.len() == 1 && targs.is_empty() && self.lookup(&segs[0]).map_or(false, |b| self.g.plain_data(&b.ty)),
            ExprKind::Unary(_, a) | ExprKind::Paren(a) | ExprKind::Deref(a) | ExprKind::Field(a, _) => self.inert(a),
            ExprKind::Binary(_, a, c) | ExprKind::Index(a, c) => self.inert(a) && self.inert(c),
            ExprKind::Call(c, args) => {
                matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && CONVERSIONS.contains(&segs[0].as_str()) && !self.g.items.fns.contains_key(&segs[0]))
                    && args.iter().all(|a| self.inert(a))
            }
            _ => false,
        }
    }

    // `Vec::len(&v)`: as `lower_vec_index_pre`, `cb_borrow_check` makes
    // the borrow's check and the body's read check at once; the length is
    // then read in place.
    pub(super) fn lower_vec_len_pre(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::len" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0] = args else { return Ok(None) };
        let ExprKind::Borrow(mode, inner) = &a0.kind else { return Ok(None) };
        let p = self.lower_place(inner)?;
        // A place with no path (under a raw pointer) takes the call; the
        // operand of `&` is a place, so lowering it again repeats nothing.
        let Some(base) = p.base.clone() else { return Ok(None) };
        if !matches!(&p.ty, Type::Named(n, a) if n == "std::Vec" && a.len() == 1) {
            return Ok(None);
        }
        let loc = self.loc(a0.line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
        let t = self.g.fresh("t");
        self.emit(&format!("uint64_t {} = ({}).len;", t, p.c));
        Ok(Some(V { c: t, ty: Type::Int(IntTy::Usize), obj: None }))
    }

    // `Vec::index_*(r, i)` with its arguments evaluated as for the call,
    // for a plain element type and an `access` (`elem_access_place`): the
    // native body inline (`native_vec_index`: its location, its checked
    // read of `len`, its bounds fault), then `cb_elem_access` in place of
    // the element borrow and the result's return. With the arguments
    // already evaluated, the checks keep the call's order whatever the
    // index is.
    pub(super) fn native_elem_access(&mut self, name: &str, f: &FnDecl, inst: &[Type], argv: &[V], write: bool, line: usize) -> R<Option<V>> {
        let rm = match name {
            "std::Vec::index_shared" => "CB_SHARED",
            "std::Vec::index_exclusive" => "CB_EXCLUSIVE",
            _ => return Ok(None),
        };
        let rm = self.elem_mode.unwrap_or(rm);
        let in_prelude = matches!(f.body.stmts.first(), Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) if e.line <= prelude::line_count());
        let ([elem], [r, i], true) = (inst, argv, in_prelude) else { return Ok(None) };
        if self.is_res(elem) || self.has_refs(elem) || self.holds_fn(elem) || matches!(elem, Type::Closure(_)) {
            return Ok(None);
        }
        let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let elem_c = self.g.ctype(elem)?;
        let tid = self.tid(elem)?;
        let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
        self.at(line);
        self.emit(&format!("cb_read({}.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);", r.c));
        self.emit(&format!("{} *{} = ({} *){}.p;", vec_c, v, vec_c, r.c));
        self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", i.c, v));
        // D-0222: the body's `_ = &mut (*v)[i .. i + 1];`, element i taken
        // exclusively through `v`, so a live shared path to the container
        // clashes.
        if name == "std::Vec::index_exclusive" {
            self.emit(&format!("cb_borrow_range_unminted({}.tok, NULL, 0, {}, {} + 1, CB_EXCLUSIVE, CB_NOLOC, 0);", r.c, i.c, i.c));
        }
        self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, i.c));
        let loc = self.loc(line);
        self.emit(&format!("cb_elem_access({}, sizeof(*{}), {}u, {}, {}, {});", e, e, tid, rm, write as u32, loc));
        Ok(Some(V { c: e, ty: elem.clone(), obj: None }))
    }

    // `*Vec::index_*(&v, i)` read, or assigned (`write`), at once, for a
    // plain element type: the element as an unchecked place, its checks
    // made by `cb_elem_access`. `None` for any other operand.
    pub(super) fn elem_access_place(&mut self, inner: &Expr, write: bool, line: usize) -> R<Option<P>> {
        let ExprKind::Call(callee, args) = &inner.kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
            return Ok(None);
        }
        let name = segs.join("::");
        if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
            return Ok(None);
        }
        let Some(f) = self.g.items.fns.get(&name).cloned() else { return Ok(None) };
        if let (true, [a0, a1]) = (in_prelude(&f), args.as_slice()) {
            let read = (!write && name == "std::Vec::index_shared").then(|| self.read_confined_vec(a0)).flatten().map(|(v, el)| (v, String::new(), el));
            if let Some((vec, _, elem)) = self.confined_elem_vec(a0).or(read) {
                // Confined (`bind_confined`, `confined_field`, a read of a
                // `read_confined_vec`): the body's location and
                // bounds fault, and the element's address; nothing else.
                let i = self.lower_index_arg(format!("({}).len", vec), a1)?;
                if matches!(i.ty, Type::Never) {
                    return Ok(Some(P { c: String::new(), ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None }));
                }
                let it = self.into_temp(i, a1.line)?;
                let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
                let elem_c = self.g.ctype(&elem)?;
                let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
                self.at(line);
                self.emit(&format!("{} *{} = &({});", vec_c, v, vec));
                self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
                self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
                return Ok(Some(P { c: format!("(*{})", e), ty: elem, base: None, projs: Vec::new(), is_binding: false, raw: None }));
            }
        }
        // CHG-0237: an element of a vector reached through a shared
        // reference parameter (`frozen_vec`), read as plain data or handed
        // to a reader parameter: its bounds fault and its address, nothing
        // else -- the parameter's path, held for the call, excludes every
        // path that could clash.
        if let (true, [a0, a1]) = (in_prelude(&f) && !write && name == "std::Vec::index_shared", args.as_slice()) {
            if let Some((vec, elem)) = self.frozen_vec(a0) {
                if (self.elem_reader && self.g.flat_value(&elem)) || self.g.plain_data(&elem) {
                    let i = self.lower_index_arg(format!("({}).len", vec), a1)?;
                    if matches!(i.ty, Type::Never) {
                        return Ok(Some(P { c: String::new(), ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None }));
                    }
                    let it = self.into_temp(i, a1.line)?;
                    let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
                    let elem_c = self.g.ctype(&elem)?;
                    let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
                    self.at(line);
                    self.emit(&format!("{} *{} = &({});", vec_c, v, vec));
                    self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
                    self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
                    return Ok(Some(P { c: format!("(*{})", e), ty: elem, base: None, projs: Vec::new(), is_binding: false, raw: None }));
                }
            }
        }
        let v = self.lower_user_call(&name, &f, targs, args, None, line, Some(write))?;
        match &v.ty {
            Type::Ref(pointee, _) => {
                let c = self.g.ctype(pointee)?;
                Ok(Some(P { c: format!("(*({} *){}.p)", c, v.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", v.c)), projs: Vec::new(), is_binding: false, raw: None }))
            }
            // A diverging argument: the caller emits nothing more.
            Type::Never => Ok(Some(P { c: String::new(), ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None })),
            _ => Ok(Some(P { c: format!("(*{})", v.c), ty: v.ty, base: None, projs: Vec::new(), is_binding: false, raw: None })),
        }
    }

    // An expression whose evaluation cannot change the runtime's state.
    // An integer conversion that cannot fault (`widen`, `narrow_wrapping`,
    // `reinterpret`, a `narrow` into a range that holds the operand's) is
    // as stateless as its operand.
    pub(super) fn stateless(&mut self, e: &Expr) -> bool {
        fn stateless(e: &Expr, conv: &mut dyn FnMut(&str, &Type, &Expr) -> bool) -> bool {
            match &e.kind {
                ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) | ExprKind::Unit | ExprKind::Path(..) => true,
                ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Field(x, _) => stateless(x, conv),
                ExprKind::Binary(_, l, r) => stateless(l, conv) && stateless(r, conv),
                ExprKind::Call(callee, args) => match (&callee.kind, args.as_slice()) {
                    (ExprKind::Path(segs, targs), [x]) if segs.len() == 1 && targs.len() == 1 => {
                        stateless(x, conv) && conv(&segs[0], &targs[0], x)
                    }
                    _ => false,
                },
                _ => false,
            }
        }
        let mut conv = |name: &str, dst: &Type, x: &Expr| -> bool {
            let intrinsic = self.lookup(name).is_none() && !self.g.items.fns.contains_key(name) && !self.g.items.externs.contains_key(name);
            let dst = self.sub(dst);
            match (intrinsic, name, dst) {
                (true, "widen" | "narrow_wrapping" | "reinterpret", Type::Int(_)) => matches!(self.probe_type(x, None), Ok(Type::Int(_))),
                (true, "narrow", Type::Int(d)) => matches!(self.probe_type(x, None), Ok(Type::Int(s)) if int_range_within(s, d)),
                _ => false,
            }
        };
        stateless(e, &mut conv)
    }

    // `[Guard-Deref]`: a guard is read in place, never moved (it is a
    // resource). Its storage holds the mutex interior's address and, in
    // the slot table, the lock path, so `*g` is `*r` for that reference.
    // `[Guard-Deref]` of a guard already lowered as a place: the guard is
    // read (it must be valid), and its lock path is the place's base.
    pub(super) fn guard_through(&mut self, p: P, line: usize) -> R<P> {
        if let Some(base) = &p.base {
            let loc = self.loc(line);
            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
        }
        let Type::Guard(pointee) = p.ty else { return internal("guard deref of a non-guard") };
        let c = self.g.ctype(&pointee)?;
        let r = self.g.fresh("r");
        self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", r, p.c, p.c));
        Ok(P { c: format!("(*({} *){}.p)", c, r), ty: *pointee, base: Some(format!("{}.tok", r)), projs: Vec::new(), is_binding: false, raw: None })
    }

    pub(super) fn guard_place(&mut self, inner: &Expr, line: usize) -> R<P> {
        let is_place = matches!(inner.kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) | ExprKind::Paren(..));
        let (slot, ty) = if is_place {
            let p = self.lower_place(inner)?;
            return self.guard_through(p, line);
        } else {
            let v = self.lower_expr(inner, None)?;
            let t = self.into_temp(v, line)?;
            (t.c, t.ty)
        };
        let Type::Guard(pointee) = ty else { return internal("guard deref of a non-guard") };
        let c = self.g.ctype(&pointee)?;
        let r = self.g.fresh("r");
        self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", r, slot, slot));
        Ok(P { c: format!("(*({} *){}.p)", c, r), ty: *pointee, base: Some(format!("{}.tok", r)), projs: Vec::new(), is_binding: false, raw: None })
    }

    // `[Extern-Call]` (`spec/20` §3): the arguments are values, evaluated
    // left to right; the result is a value of the declared FfiType (a
    // claim). The C function is the symbol named by the declaration,
    // resolved by the system linker against libc and libm. It is declared
    // under a private C name bound to that symbol with an assembler
    // label, so a CobaltC signature never collides with the C headers'
    // own declaration of the same function.
    pub(super) fn lower_extern_call(&mut self, name: &str, args: &[Expr], line: usize) -> R<V> {
        let ext = self.g.items.externs[name].clone();
        let cname = match self.g.externs_out.get(&ext.name) {
            Some(c) => c.clone(),
            None => {
                let cname = format!("cbx_{}", san(&ext.name));
                let ret = if matches!(ext.ret, Type::Void) { "void".to_string() } else { self.g.ctype(&ext.ret)? };
                let mut ps = Vec::new();
                for p in &ext.params {
                    ps.push(self.g.ctype(&p.ty)?);
                }
                let params = if ps.is_empty() { "void".to_string() } else { ps.join(", ") };
                writeln!(self.g.protos, "extern {} {}({}) __asm__(\"{}\");", ret, cname, params, ext.name).unwrap();
                self.g.externs_out.insert(ext.name.clone(), cname.clone());
                cname
            }
        };
        let mut argv = Vec::new();
        for (p, a) in ext.params.iter().zip(args.iter()) {
            let v = self.lower_expr(a, Some(&p.ty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            argv.push(self.into_temp(v, line)?.c);
        }
        let call = format!("{}({})", cname, argv.join(", "));
        if matches!(ext.ret, Type::Void) {
            self.emit(&format!("{};", call));
            return Ok(unit());
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&ext.ret)?;
        self.emit(&format!("{} {} = {};", c, t, call));
        Ok(V { c: t, ty: ext.ret.clone(), obj: None })
    }
}
