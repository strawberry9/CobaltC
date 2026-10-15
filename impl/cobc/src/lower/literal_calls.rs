// Fx: calls with closure literals (sort_by, predicates, from_fn).

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // `Vec::sort_by(&mut x, [](ref<T, shared> a, ref<T, shared> b) : bool
    // { … })` for a plain `T`, the closure capturing nothing and using
    // `a` and `b` only as `*a`/`*b` or projection bases -- never lent,
    // stored or handed on (`direct_uses` with no callee). Each comparison's
    // element borrows are then unobservable: unheld, ended with their
    // statement, and against a vector the call holds exclusively. The
    // prelude's merge sort runs natively, in the same order of
    // comparisons (the closure may print or fault), calling the closure's
    // code with bare addresses; then the elements' cells are released and
    // the vector written, as `apply_order` and native `sort` do. The
    // closure value itself, never observed, is not formed.
    pub(super) fn lower_sort_by_literal(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::sort_by" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0, a1] = args else { return Ok(None) };
        // `&mut place`, or an exclusive `Vec` reference written bare (a
        // parameter `v`: the place `*v`).
        let deref_a0;
        let inner: &Expr = match &a0.kind {
            ExprKind::Borrow(Mode::Exclusive, inner) => inner,
            ExprKind::Path(..) if matches!(self.probe_type(a0, None)?, Type::Ref(_, Mode::Exclusive)) => {
                deref_a0 = Expr { kind: ExprKind::Deref(Box::new(a0.clone())), line: a0.line, col: a0.col };
                &deref_a0
            }
            _ => return Ok(None),
        };
        let ExprKind::Closure { is_move, captures, params, ret, body } = &a1.kind else { return Ok(None) };
        if !captures.is_empty() || params.len() != 2 {
            return Ok(None);
        }
        let Type::Named(n, targs) = self.probe_type(inner, None)? else { return Ok(None) };
        if n != "std::Vec" || targs.len() != 1 {
            return Ok(None);
        }
        let elem = targs[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) {
            return Ok(None);
        }
        for p in params {
            if !matches!(self.sub(&p.ty), Type::Ref(t, Mode::Shared) if *t == elem) || !direct_uses(body, &p.name, &|_, _| false) {
                return Ok(None);
            }
        }
        if self.g.direct_refs_of(params, body, &self.subst)?.len() != 2 {
            return Ok(None);
        }
        let p = self.lower_place(inner)?;
        let Some(base) = p.base.clone() else { return Ok(None) };
        let loc = self.loc(a0.line);
        self.emit(&format!("cb_borrow_check({}, {}, CB_EXCLUSIVE, {});", base, Self::projs_args(&p.projs), loc));
        // The closure's code alone (`lower_closure` without its value).
        let id = self.g.closure_id();
        let ptypes: Vec<Type> = params.iter().map(|p| self.sub(&p.ty)).collect();
        let cl = format!("cl_{}", id);
        self.g.closures.insert(id, ClosureInfo { fields: Vec::new(), is_move: *is_move, params: ptypes, ret: Type::Void, cname: cl.clone(), writes_self: false });
        let rt = self.lower_closure_body(id, params, ret.as_ref(), body)?;
        self.g.closures.get_mut(&id).unwrap().ret = rt.clone();
        if rt != Type::Bool {
            return internal("sort_by comparator not bool");
        }
        let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let elem_c = self.g.reg_ctype(&elem)?;
        let size = self.g.size_align(&elem)?.0;
        let helper = format!("sort_by_{}", id);
        let mut h = String::new();
        writeln!(h, "void {}({} *v)\n{{", helper, vec_c).unwrap();
        writeln!(h, "    uint64_t n = v->len;").unwrap();
        writeln!(h, "    if (n < 2) return;").unwrap();
        writeln!(h, "    uint64_t *a = (uint64_t *)malloc(n * 2 * sizeof(uint64_t));").unwrap();
        writeln!(h, "    {} *out = ({} *)malloc(n * sizeof({}));", elem_c, elem_c, elem_c).unwrap();
        writeln!(h, "    if (!a || !out) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(h, "    uint64_t *b = a + n;").unwrap();
        writeln!(h, "    for (uint64_t i = 0; i < n; i++) {{ a[i] = i; b[i] = 0; }}").unwrap();
        writeln!(h, "    uint64_t width = 1;").unwrap();
        writeln!(h, "    while (width < n) {{").unwrap();
        writeln!(h, "        uint64_t lo = 0;").unwrap();
        writeln!(h, "        while (lo < n) {{").unwrap();
        writeln!(h, "            uint64_t mid = n - lo > width ? lo + width : n;").unwrap();
        writeln!(h, "            uint64_t hi = n - mid > width ? mid + width : n;").unwrap();
        writeln!(h, "            uint64_t l = lo, r = mid, k = lo;").unwrap();
        writeln!(h, "            while (k < hi) {{").unwrap();
        writeln!(h, "                int take_left = l < mid && (r >= hi || !{}(((cb_ref){{ 0, 0 }}), ((cb_ref){{ (void *)&v->ptr[a[r]], 0 }}), ((cb_ref){{ (void *)&v->ptr[a[l]], 0 }})));", cl).unwrap();
        writeln!(h, "                if (take_left) b[k] = a[l++]; else b[k] = a[r++];").unwrap();
        writeln!(h, "                k++;").unwrap();
        writeln!(h, "            }}").unwrap();
        writeln!(h, "            lo = hi;").unwrap();
        writeln!(h, "        }}").unwrap();
        writeln!(h, "        uint64_t *t = a; a = b; b = t;").unwrap();
        writeln!(h, "        width = width > n ? n : width * 2;").unwrap();
        writeln!(h, "    }}").unwrap();
        writeln!(h, "    for (uint64_t i = 0; i < n; i++) out[i] = v->ptr[a[i]];").unwrap();
        writeln!(h, "    memcpy(v->ptr, out, n * sizeof({}));", elem_c).unwrap();
        writeln!(h, "    free(a < b ? a : b);").unwrap();
        writeln!(h, "    free(out);").unwrap();
        writeln!(h, "    cb_vec_drop_plain((uint8_t *)v->ptr, n, {}u);", size).unwrap();
        writeln!(h, "    cb_release((uint8_t *)v->ptr, n * {}u);", size).unwrap();
        writeln!(h, "}}\n").unwrap();
        if self.g.probing == 0 {
            writeln!(self.g.protos, "void {}({} *v);", helper, vec_c).unwrap();
            self.g.fns.push_str(&h);
        }
        self.emit(&format!("{}(&({}));", helper, p.c));
        Ok(Some(unit()))
    }

    // D-0189, D-0190, D-0191: `Vec::retain(v, p)`, `Vec::position(v, p)` and
    // `Vec::binary_search_by(v, key, less)` for a plain `T`, where the
    // predicate is a closure literal whose parameters are used only
    // directly (`direct_uses`), or a named function whose parameters are
    // pure direct (`Gen::pure_direct`): its code is called with bare element
    // addresses, in the body's order (`pred_source`). The body's checks are
    // kept: the vector's borrow (`cb_borrow_check`), each element's borrow
    // for the predicate (`cb_elem_access`, shared: it meets an element's
    // object, if any, as `index_shared` does), and, for `retain`, the
    // exclusive borrows of the two elements each move swaps, then one
    // release of the cells `truncate` pops.
    pub(super) fn lower_pred_literal(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        let retain = name == "std::Vec::retain";
        let search = name == "std::Vec::binary_search_by";
        if !(retain || search || name == "std::Vec::position") || !in_prelude(f) {
            return Ok(None);
        }
        let (a0, akey, ap) = match args {
            [a0, a1] if !search => (a0, None, a1),
            [a0, k, a2] if search => (a0, Some(k), a2),
            _ => return Ok(None),
        };
        let deref_a0;
        let inner: &Expr = match &a0.kind {
            ExprKind::Borrow(_, inner) => inner,
            ExprKind::Path(..) if matches!(self.probe_type(a0, None)?, Type::Ref(..)) => {
                deref_a0 = Expr { kind: ExprKind::Deref(Box::new(a0.clone())), line: a0.line, col: a0.col };
                &deref_a0
            }
            _ => return Ok(None),
        };
        let Type::Named(n, targs) = self.probe_type(inner, None)? else { return Ok(None) };
        if n != "std::Vec" || targs.len() != 1 {
            return Ok(None);
        }
        let elem = targs[0].clone();
        let size = self.g.size_align(&elem)?.0;
        if size == 0 || !self.g.movable_elem(&elem) {
            return Ok(None);
        }
        let res_elem = !self.g.plain_data(&elem);
        let arity = if search { 2 } else { 1 };
        let mut root = inner;
        loop {
            match &strip_parens(root).kind {
                ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => root = a,
                _ => break,
            }
        }
        let root_name = path_name(strip_parens(root)).map(|r| r.to_string());
        // A `fn` value in a local (D-0191): which function it holds is
        // known only when the call runs. Read once (a local: no effect),
        // its handle says whether it keeps nothing of its arguments
        // (`cb_fn_pure`): then the loop below, through its thunk; else the
        // prelude's body, called as the call would call it.
        let opaque = if self.pred_ok(ap, arity, &elem, root_name.as_deref())? {
            None
        } else {
            match &ap.kind {
                ExprKind::Path(segs, t) if segs.len() == 1 && t.is_empty() && self.lookup(&segs[0]).is_some() => {
                    let ft = self.probe_type(ap, None)?;
                    let shape = matches!(&ft, Type::Fn(ps, r) if ps.len() == arity && **r == Type::Bool
                        && ps.iter().all(|t| matches!(t, Type::Ref(x, Mode::Shared) if **x == elem)));
                    if !shape {
                        return Ok(None);
                    }
                    Some(ft)
                }
                _ => return Ok(None),
            }
        };
        if in_prelude(f) {
            self.at(line);
        }
        let p = self.lower_place(inner)?;
        let mode = if retain { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        if let Some(base) = p.base.clone() {
            let loc = self.loc(a0.line);
            self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&p.projs), mode, loc));
        }
        // The key, as the call's argument (a reference: its path formed,
        // as the body's binding of it holds one), before the predicate.
        let key = match akey {
            // For a literal or a named function, the key reaches only
            // parameters that read it (direct or pure direct): a place is
            // passed as its address after its borrow's check, a temporary as
            // the address of its value, no path minted (as `native_key`).
            Some(k) if opaque.is_none() => match &k.kind {
                ExprKind::Borrow(Mode::Shared, place) if is_place_expr(place) && root_bound(place, &|n| self.lookup(n).is_some()) => {
                    let pl = self.lower_place(place)?;
                    if let (Some(base), None) = (pl.base.clone(), &pl.raw) {
                        let loc = self.loc(k.line);
                        self.emit(&format!("cb_borrow_unminted({}, {}, CB_SHARED, {});", base, Self::projs_args(&pl.projs), loc));
                    }
                    let ad = self.g.fresh("ad");
                    self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                    Some(format!("((cb_ref){{ {}, 0 }})", ad))
                }
                ExprKind::Borrow(Mode::Shared, inner) if !is_place_expr(inner) && !res_elem => {
                    let v = self.lower_expr(inner, Some(&elem))?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(Some(never()));
                    }
                    let kc = self.g.ctype(&elem)?;
                    let kt = self.g.fresh("key");
                    self.emit(&format!("{} {} = {};", kc, kt, v.c));
                    Some(format!("((cb_ref){{ (void *)&{}, 0 }})", kt))
                }
                _ => {
                    let kv = self.lower_expr(k, Some(&Type::Ref(Box::new(elem.clone()), Mode::Shared)))?;
                    if matches!(kv.ty, Type::Never) {
                        return Ok(Some(never()));
                    }
                    Some(self.into_temp(kv, k.line)?.c)
                }
            },
            Some(k) => {
                let kv = self.lower_expr(k, Some(&Type::Ref(Box::new(elem.clone()), Mode::Shared)))?;
                if matches!(kv.ty, Type::Never) {
                    return Ok(Some(never()));
                }
                Some(self.into_temp(kv, k.line)?.c)
            }
            None => None,
        };
        let mut opaque_h: Option<String> = None;
        let (fname, selfr, env) = match &opaque {
            None => self.pred_source(ap)?,
            Some(ft) => {
                let hv = self.lower_expr(ap, Some(ft))?;
                let h = self.into_temp(hv, ap.line)?.c;
                opaque_h = Some(h.clone());
                (self.g.thunk(ft)?, Some("h".to_string()), None)
            }
        };
        let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let elem_c = self.g.reg_ctype(&elem)?;
        let tid = self.tid(&elem)?;
        self.g.uid += 1;
        let helper = format!("{}_{}", if retain { "retain" } else if search { "search_by" } else { "position" }, self.g.uid);
        let mut h = String::new();
        let mut envp = String::new();
        let mut enva = String::new();
        if let Some((cc, c, st)) = &env {
            envp = format!(", {} *c, uint64_t ct", cc);
            enva = format!(", &({}), {}", c, st);
        }
        if let Some(k) = &key {
            envp.push_str(", cb_ref k");
            enva.push_str(&format!(", {}", k));
        }
        if let Some(h) = &opaque_h {
            envp.push_str(", void *h");
            enva.push_str(&format!(", {}", h));
        }
        let call = |args: &[String]| -> String {
            let mut a: Vec<String> = selfr.iter().cloned().collect();
            a.extend(args.iter().cloned());
            format!("{}({})", fname, a.join(", "))
        };
        let el = |i: &str| format!("((cb_ref){{ (void *)&v->ptr[{}], 0 }})", i);
        // An element access that clashes faults at the call (as the
        // prelude's body would, its own lines having no location), not at
        // whatever line the predicate's body last recorded.
        let call_loc = self.loc(line);
        if retain {
            writeln!(h, "void {}({} *v{})\n{{", helper, vec_c, envp).unwrap();
            writeln!(h, "    uint64_t n = v->len, w = 0;").unwrap();
            writeln!(h, "    for (uint64_t i = 0; i < n; i++) {{").unwrap();
            writeln!(h, "        cb_elem_access(&v->ptr[i], sizeof(v->ptr[0]), {}u, CB_SHARED, 0u, {});", tid, call_loc).unwrap();
            writeln!(h, "        if ({}) {{", call(&[el("i")])).unwrap();
            writeln!(h, "            if (w != i) {{").unwrap();
            writeln!(h, "                cb_elem_access(&v->ptr[w], sizeof(v->ptr[0]), {}u, CB_EXCLUSIVE, 1u, {});", tid, call_loc).unwrap();
            writeln!(h, "                cb_elem_access(&v->ptr[i], sizeof(v->ptr[0]), {}u, CB_EXCLUSIVE, 1u, {});", tid, call_loc).unwrap();
            writeln!(h, "                {} t = v->ptr[w]; v->ptr[w] = v->ptr[i]; v->ptr[i] = t;", elem_c).unwrap();
            writeln!(h, "            }}").unwrap();
            writeln!(h, "            w++;").unwrap();
            writeln!(h, "        }}").unwrap();
            writeln!(h, "    }}").unwrap();
            if res_elem {
                // `truncate`'s pops: the elements not kept destroyed, last first.
                writeln!(h, "    if (w < n) {{ for (uint64_t k = n; k > w; k--) {{ v->len = k - 1; cb_drop_local((uint8_t *)&v->ptr[k - 1], {}u); }} cb_release_plain((uint8_t *)(v->ptr + w), (n - w) * {}u); v->len = w; }}", tid, size).unwrap();
            } else {
                writeln!(h, "    if (w < n) {{ cb_release_plain((uint8_t *)(v->ptr + w), (n - w) * {}u); v->len = w; }}", size).unwrap();
            }
            writeln!(h, "}}\n").unwrap();
        } else if search {
            // `binary_search_by`'s body: `less(v[mid], key)` halving, then
            // `!less(key, v[lo])` for a match, each element borrowed for
            // its call.
            let res = Type::Named("std::Result".to_string(), vec![Type::Int(IntTy::Usize), Type::Int(IntTy::Usize)]);
            let res_c = self.g.ctype(&res)?;
            writeln!(h, "{} {}({} *v{})\n{{", res_c, helper, vec_c, envp).unwrap();
            writeln!(h, "    {} r;", res_c).unwrap();
            writeln!(h, "    uint64_t lo = 0, hi = v->len;").unwrap();
            writeln!(h, "    while (lo < hi) {{").unwrap();
            writeln!(h, "        uint64_t mid = lo + (hi - lo) / 2;").unwrap();
            writeln!(h, "        cb_elem_access(&v->ptr[mid], sizeof(v->ptr[0]), {}u, CB_SHARED, 0u, {});", tid, call_loc).unwrap();
            writeln!(h, "        if ({}) lo = mid + 1; else hi = mid;", call(&[el("mid"), "k".to_string()])).unwrap();
            writeln!(h, "    }}").unwrap();
            writeln!(h, "    if (lo < v->len) {{").unwrap();
            writeln!(h, "        cb_elem_access(&v->ptr[lo], sizeof(v->ptr[0]), {}u, CB_SHARED, 0u, {});", tid, call_loc).unwrap();
            writeln!(h, "        if (!{}) {{ r.tag = 0u; r.u.v0 = lo; return r; }}", call(&["k".to_string(), el("lo")])).unwrap();
            writeln!(h, "    }}").unwrap();
            writeln!(h, "    r.tag = 1u;").unwrap();
            writeln!(h, "    r.u.v1 = lo;").unwrap();
            writeln!(h, "    return r;").unwrap();
            writeln!(h, "}}\n").unwrap();
        } else {
            let opt = Type::Named("std::Option".to_string(), vec![Type::Int(IntTy::Usize)]);
            let opt_c = self.g.ctype(&opt)?;
            writeln!(h, "{} {}({} *v{})\n{{", opt_c, helper, vec_c, envp).unwrap();
            writeln!(h, "    {} r;", opt_c).unwrap();
            writeln!(h, "    for (uint64_t i = 0; i < v->len; i++) {{").unwrap();
            writeln!(h, "        cb_elem_access(&v->ptr[i], sizeof(v->ptr[0]), {}u, CB_SHARED, 0u, {});", tid, call_loc).unwrap();
            writeln!(h, "        if ({}) {{ r.tag = 0u; r.u.v0 = i; return r; }}", call(&[el("i")])).unwrap();
            writeln!(h, "    }}").unwrap();
            writeln!(h, "    r.tag = 1u;").unwrap();
            writeln!(h, "    return r;").unwrap();
            writeln!(h, "}}\n").unwrap();
        }
        if self.g.probing == 0 {
            let proto = h.lines().next().expect("a signature").to_string();
            writeln!(self.g.protos, "{};", proto).unwrap();
            self.g.fns.push_str(&h);
        }
        let target = format!("{}(&({}){})", helper, p.c, enva);
        // The other branch for a `fn` value: the prelude's function, the
        // vector borrowed as the call borrows it (a direct one has no path).
        let fallback = match &opaque_h {
            Some(h) => {
                let tk = self.g.fresh("tk");
                let loc = self.loc(a0.line);
                let tok = match &p.base {
                    Some(base) => format!("cb_borrow({}, {}, {}, {})", base, Self::projs_args(&p.projs), mode, loc),
                    None => "0".to_string(),
                };
                let std_fn = self.g.request_fn(name, vec![elem.clone()])?;
                let mut fa = vec![format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tk)];
                if let Some(k) = &key {
                    fa.push(k.clone());
                }
                fa.push(h.clone());
                Some((format!("uint64_t {} = {};", tk, tok), format!("{}({})", std_fn, fa.join(", "))))
            }
            None => None,
        };
        if retain {
            match (&opaque_h, &fallback) {
                (Some(h), Some((pre, call))) => self.emit(&format!("if (cb_fn_pure({})) {}; else {{ {} {}; }}", h, target, pre, call)),
                _ => self.emit(&format!("{};", target)),
            }
            return Ok(Some(unit()));
        }
        let rty = if search {
            Type::Named("std::Result".to_string(), vec![Type::Int(IntTy::Usize), Type::Int(IntTy::Usize)])
        } else {
            Type::Named("std::Option".to_string(), vec![Type::Int(IntTy::Usize)])
        };
        let rc = self.g.ctype(&rty)?;
        let t = self.g.fresh("t");
        match (&opaque_h, &fallback) {
            (Some(h), Some((pre, call))) => {
                self.emit(&format!("{} {};", rc, t));
                self.emit(&format!("if (cb_fn_pure({})) {} = {}; else {{ {} {} = {}; }}", h, t, target, pre, t, call));
            }
            _ => self.emit(&format!("{} {} = {};", rc, t, target)),
        }
        Ok(Some(V { c: t, ty: rty, obj: None }))
    }

    // D-0192: `Vec::from_fn(n, [](usize i) : T { … })` for a plain `T`: the
    // closure's code called for each `i` in order (it may print, fault or
    // write what it captures), each result stored in place, the buffer grown
    // as the body's pushes grow it (4, then doubling: an allocation fails at
    // the same `i`). The vector is the call's result, an object of this
    // statement (`register_temp`).
    pub(super) fn lower_from_fn_literal(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::Vec::from_fn" || !in_prelude(f) {
            return Ok(None);
        }
        let [an, af] = args else { return Ok(None) };
        let ExprKind::Closure { params, .. } = &af.kind else { return Ok(None) };
        if params.len() != 1 || self.sub(&params[0].ty) != Type::Int(IntTy::Usize) {
            return Ok(None);
        }
        let Type::Fn(_, r) = self.probe_type(af, None).map(|t| match t {
            Type::Closure(id) => self.g.closures.get(&id).map(|i| Type::Fn(i.params.clone(), Box::new(i.ret.clone()))).unwrap_or(Type::Void),
            t => t,
        })? else { return Ok(None) };
        let elem = (*r).clone();
        if matches!(elem, Type::Void | Type::Never) || self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) {
            return Ok(None);
        }
        if in_prelude(f) {
            self.at(line);
        }
        let nv = self.lower_expr(an, Some(&Type::Int(IntTy::Usize)))?;
        if matches!(nv.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let n = self.into_temp(nv, an.line)?.c;
        let (fname, selfr, env, rt) = self.closure_source(af, None)?;
        if rt != elem {
            return internal("from_fn's closure result changed type");
        }
        let vec_ty = Type::Named("std::Vec".to_string(), vec![elem.clone()]);
        let vec_c = self.g.ctype(&vec_ty)?;
        let elem_c = self.g.ctype(&elem)?;
        let (size, align) = self.g.size_align(&elem)?;
        let selfa = match (&selfr, &env) {
            (Some(_), Some((_, c, st))) => format!("((cb_ref){{ (void *)&({}), {} }}), ", c, st),
            (Some(sr), None) => format!("{}, ", sr),
            (None, _) => String::new(),
        };
        let out = self.g.fresh("out");
        let i = self.g.fresh("i");
        self.emit(&format!("{} {}; {}.ptr = ({} *){}; {}.len = 0; {}.cap = 0;", vec_c, out, out, elem_c, align, out, out));
        self.emit(&format!("for (uint64_t {i} = 0; {i} < {n}; {i}++) {{", i = i, n = n));
        self.emit(&format!("    {} x = {}({}{});", elem_c, fname, selfa, i));
        self.emit(&format!("    if ({o}.len == {o}.cap) {{ uint64_t nc = {o}.cap == 0 ? 4 : {o}.cap * 2; uint64_t nb; if (__builtin_mul_overflow(nc, (uint64_t){s}u, &nb)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0); uint8_t *np = cb_reallocate_plain((uint8_t *){o}.ptr, {o}.cap * {s}u, nb, {a}u); if (!np) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0); {o}.ptr = ({e} *)np; {o}.cap = nc; }}", o = out, s = size, a = align, e = elem_c));
        self.emit(&format!("    {o}.ptr[{o}.len++] = x;", o = out));
        self.emit("}");
        self.register_temp(out, vec_ty, line).map(Some)
    }

    // Whether `a` is a predicate `lower_pred_literal` can call with bare
    // addresses: a closure literal of `arity` parameters, each
    // `ref<elem, shared>` and used only directly, capturing nothing of the
    // vector's root; or a named, non-generic function of that shape whose
    // parameters are pure direct, returning `bool`.
    pub(super) fn pred_ok(&mut self, a: &Expr, arity: usize, elem: &Type, root: Option<&str>) -> R<bool> {
        let ref_elem = |t: &Type| matches!(t, Type::Ref(x, Mode::Shared) if **x == *elem);
        match &a.kind {
            ExprKind::Closure { captures, params, body, .. } => {
                if params.len() != arity || root.map_or(false, |r| captures.iter().any(|c| c == r)) {
                    return Ok(false);
                }
                if self.g.plain_data(elem) {
                    for p in params {
                        if !ref_elem(&self.sub(&p.ty)) || !direct_uses(body, &p.name, &|_, _| false) {
                            return Ok(false);
                        }
                    }
                    return Ok(self.g.direct_refs_of(params, body, &self.subst)?.len() == arity);
                }
                // D-0192: a resource element (`String`): each parameter only
                // read through and handed to reader parameters (`reader_uses`,
                // as `foreach_alias` admits), so its address alone serves.
                let bound = bound_names(body);
                let g = &*self.g;
                Ok(params.iter().all(|p| {
                    ref_elem(&apply(&p.ty, &self.subst))
                        && reader_uses(body, &p.name, false, &|segs: &[String], at: usize| {
                            if segs.len() == 1 && bound.contains(&segs[0]) {
                                return false;
                            }
                            let k = segs.join("::");
                            g.reads_only(&k, at)
                        })
                }))
            }
            ExprKind::Path(segs, targs) if targs.is_empty() => {
                if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
                    return Ok(false);
                }
                let key = segs.join("::");
                let Some(g) = self.g.items.fns.get(&key).cloned() else { return Ok(false) };
                if in_prelude(&g) || !g.type_params.is_empty() || g.params.len() != arity || g.ret != Type::Bool {
                    return Ok(false);
                }
                Ok(g.params.iter().enumerate().all(|(j, p)| ref_elem(&p.ty) && self.g.pure_direct(&key, j)))
            }
            _ => Ok(false),
        }
    }

    // The predicate `pred_ok` accepted: the C function to call, its `self`
    // argument if it has one, and, for a capturing closure, the value built
    // here (as the call's argument would be: its C type, its C name, and
    // one exclusive borrow of it for the loop, which nothing else reaches
    // meanwhile; its body binds `self` and its parameters directly,
    // `pending_pred_closure`).
    pub(super) fn pred_source(&mut self, a: &Expr) -> R<(String, Option<String>, Option<(String, String, String)>)> {
        self.closure_source(a, Some(Type::Bool)).map(|(f, s, e, _)| (f, s, e))
    }

    // `pred_source` for a closure literal of any result type (`want`, if
    // given, checked): also the result type.
    pub(super) fn closure_source(&mut self, a: &Expr, want: Option<Type>) -> R<(String, Option<String>, Option<(String, String, String)>, Type)> {
        match &a.kind {
            ExprKind::Closure { is_move, captures, params, ret, body } if captures.is_empty() => {
                let id = self.g.closure_id();
                let ptypes: Vec<Type> = params.iter().map(|p| self.sub(&p.ty)).collect();
                let cl = format!("cl_{}", id);
                self.g.closures.insert(id, ClosureInfo { fields: Vec::new(), is_move: *is_move, params: ptypes, ret: Type::Void, cname: cl.clone(), writes_self: false });
                self.g.pending_pred_closure = true;
                let rt = self.lower_closure_body(id, params, ret.as_ref(), body);
                self.g.pending_pred_closure = false;
                let rt = rt?;
                self.g.closures.get_mut(&id).unwrap().ret = rt.clone();
                if want.as_ref().map_or(false, |w| *w != rt) {
                    return internal("a predicate that is not bool");
                }
                Ok((cl, Some("((cb_ref){ 0, 0 })".to_string()), None, rt))
            }
            ExprKind::Closure { .. } => {
                self.g.pending_pred_closure = true;
                let v = self.lower_expr(a, None);
                self.g.pending_pred_closure = false;
                let v = v?;
                let Type::Closure(id) = v.ty else { return internal("a closure literal of another type") };
                let info = self.g.closures[&id].clone();
                if want.as_ref().map_or(false, |w| *w != info.ret) {
                    return internal("a predicate that is not bool");
                }
                let t = self.into_temp(v, a.line)?;
                let loc = self.loc(a.line);
                let o = match &t.obj {
                    Some(o) => o.clone(),
                    None => {
                        let tid = self.tid(&t.ty)?;
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t.c, tid, loc));
                        o
                    }
                };
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_temp_path({});", tok, o));
                let st = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, CB_EXCLUSIVE, {});", st, tok, loc));
                let cc = self.g.ctype(&t.ty)?;
                Ok((info.cname.clone(), Some("((cb_ref){ (void *)c, ct })".to_string()), Some((cc, t.c.clone(), st)), info.ret.clone()))
            }
            ExprKind::Path(segs, _) => {
                // A named function with pure direct parameters: called as a
                // call to it would be, its arguments' addresses alone.
                let cname = self.g.request_fn(&segs.join("::"), Vec::new())?;
                let rt = self.g.items.fns.get(&segs.join("::")).map(|g| g.ret.clone()).unwrap_or(Type::Void);
                Ok((cname, None, None, rt))
            }
            _ => internal("a predicate pred_ok did not accept"),
        }
    }
}
