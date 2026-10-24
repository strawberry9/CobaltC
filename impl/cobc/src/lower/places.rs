// Fx: places and their reads.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // ---- places ----

    pub(super) fn lower_place(&mut self, e: &Expr) -> R<P> {
        // `x.f…` over a `foreach` element alias, read in place.
        if let Some(r) = self.alias_field_chain(e) {
            return self.lower_place(&r);
        }
        // `*Vec::index_*(&x, i)` (`x[i]`, `vec_index_call`) for a confined
        // `x`: the element's address after the bounds check, as
        // `lower_expr` reaches it (`elem_access_place`'s confined case).
        if let ExprKind::Deref(inner) = &e.kind {
            if let ExprKind::Call(c, a) = &inner.kind {
                let index = matches!(&c.kind, ExprKind::Path(segs, _) if matches!(segs.join("::").as_str(), "std::Vec::index_shared" | "std::Vec::index_exclusive"));
                // (Not a `read_confined_vec`'s: a place may be borrowed, and
                // its element then needs the path its access establishes;
                // reads reach `elem_access_place` through `lower_expr`.)
                if index && a.first().map_or(false, |a0| self.confined_elem_vec(a0).is_some()) {
                    if let Some(p) = self.elem_access_place(inner, self.place_write, e.line)? {
                        return Ok(p);
                    }
                }
            }
        }
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 && self.lookup(&segs[0]).is_some() => {
                let b = self.lookup(&segs[0]).unwrap();
                match b.kind {
                    // An unchecked local has no access path: its accesses are
                    // all proven (`unchecked`), so none is checked.
                    BindKind::Local => {
                        let base = if b.root.is_empty() { None } else { Some(b.root) };
                        Ok(P { c: b.c, ty: b.ty, base, projs: Vec::new(), is_binding: true, raw: None })
                    }
                    BindKind::CaptureVal(i) => {
                        // `self.f_i`: a field of the closure through `self`.
                        let (sp, cid) = self.self_ref.clone().expect("capture outside a closure body");
                        let cty = self.g.ctype(&Type::Closure(cid))?;
                        Ok(P {
                            c: format!("(*({} *){}.p).f{}", cty, sp, i),
                            ty: b.ty,
                            base: Some(format!("{}.tok", sp)),
                            projs: vec![format!("{{ CB_FIELD, {}u }}", i)],
                            is_binding: false,
                            raw: None,
                        })
                    }
                    BindKind::CaptureRef(i) => {
                        // `*self.f_i`: through the captured reference.
                        let (sp, cid) = self.self_ref.clone().expect("capture outside a closure body");
                        let cty = self.g.ctype(&Type::Closure(cid))?;
                        let slot = format!("(*({} *){}.p).f{}", cty, sp, i);
                        let Type::Ref(pointee, _) = &b.ty else { return internal("borrow capture is not a reference") };
                        let tk = self.g.fresh("tk");
                        self.emit(&format!("uint64_t {} = cb_load_ref(&{});", tk, slot));
                        let pc = self.g.ctype(pointee)?;
                        Ok(P { c: format!("(*({} *){})", pc, slot), ty: (**pointee).clone(), base: Some(tk), projs: Vec::new(), is_binding: false, raw: None })
                    }
                }
            }
            ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "reclaim") => {
                // `[Reclaim]`: the object at a raw address, as a place
                // rooted in its own (reclaim) path.
                let ExprKind::Path(_, targs) = &callee.kind else { unreachable!() };
                let ty = targs.get(0).map(|t| self.sub(t)).expect("reclaim's type argument");
                let pv = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(pv, e.line)?;
                let tid = self.tid(&ty)?;
                let c = self.g.ctype(&ty)?;
                let root = self.g.fresh("root");
                self.emit(&format!("uint64_t {} = cb_reclaim({}, {}u);", root, pt.c, tid));
                Ok(P { c: format!("(*({} *){})", c, pt.c), ty, base: Some(root), projs: Vec::new(), is_binding: true, raw: None })
            }
            ExprKind::Paren(inner) => self.lower_place(inner),
            ExprKind::Field(base, f) => {
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                // D-0064: `g.f` through a guard is `(*g).f`.
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                let (sname, args) = match &pb.ty {
                    Type::Named(n, a) => (n.clone(), a.clone()),
                    _ => return internal("field access on a non-struct"),
                };
                let sd = self.g.struct_decl(&sname).expect("struct");
                let sub = subst_of(&sd.type_params, &args);
                let idx = sd.fields.iter().position(|x| &x.name == f).expect("field");
                let mut projs = pb.projs;
                projs.push(format!("{{ CB_FIELD, {}u }}", idx));
                Ok(P { c: format!("{}.{}", pb.c, san(f)), ty: apply(&sd.fields[idx].ty, &sub), base: pb.base, projs, is_binding: false, raw: None })
            }
            ExprKind::Index(base, idx) => {
                // `vec_index_call` probes the base's type; the D-0103 flag is
                // for the lowering proper.
                let temp_root = std::mem::take(&mut self.temp_root);
                let vx = self.vec_index_call(e, self.place_write)?;
                self.temp_root = temp_root;
                if let Some(x) = vx {
                    // The `Vec`'s borrow in the rewritten call is part of the
                    // same temporary (D-0103).
                    if temp_root {
                        let call = match &x.kind {
                            ExprKind::Deref(c) => &**c,
                            _ => &*x,
                        };
                        if let ExprKind::Call(_, xa) = &call.kind {
                            if let Some(a0 @ Expr { kind: ExprKind::Borrow(..), .. }) = xa.first() {
                                self.g.items.temp_borrows.lock().unwrap().insert(a0 as *const Expr as usize);
                            }
                        }
                        self.temp_root = false;
                    }
                    self.place_write = false;
                    return self.lower_place(&x);
                }
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                // D-0047: a `Vec`'s element is checked as an access to the
                // `Vec` (its base and path), in the mode of the use; a
                // slice's through the slice's own borrow.
                match pb.ty.clone() {
                    Type::Named(n, args) if n == "std::Vec" => {
                        let elem = args.first().cloned().unwrap_or(Type::Void);
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        // The element's own index on the `Vec`'s path: two
                        // elements are disjoint, the whole `Vec` overlaps each.
                        let mut projs = pb.projs;
                        projs.push(format!("{{ CB_INDEX, (uint64_t){} }}", it.c));
                        return Ok(P { c: format!("({}).ptr[{}]", pb.c, it.c), ty: elem, base: pb.base, projs, is_binding: false, raw: None });
                    }
                    Type::Slice(elem, _) if self.direct_refs.contains(&pb.c) && (pb.is_binding || pb.base.is_none()) => {
                        // A direct slice (`Gen::direct_refs_of`), or a pure
                        // view's `bytes` (`Gen::pure_view`): bounds only.
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        self.slice_elem_access(&pb.c, &it.c, &elem, loc.clone())?;
                        return Ok(P { c: format!("({}).data[{}]", pb.c, it.c), ty: (*elem).clone(), base: None, projs: Vec::new(), is_binding: false, raw: None });
                    }
                    Type::Slice(elem, _) => {
                        let tok = self.g.fresh("tok");
                        self.emit(&format!("uint64_t {} = cb_load_ref(&({}).src);", tok, pb.c));
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        self.slice_elem_access(&pb.c, &it.c, &elem, loc.clone())?;
                        return Ok(P { c: format!("({}).data[{}]", pb.c, it.c), ty: (*elem).clone(), base: Some(tok), projs: vec![format!("{{ CB_INDEX, (uint64_t){} }}", it.c)], is_binding: false, raw: None });
                    }
                    _ => {}
                }
                let (elem, n) = match &pb.ty {
                    Type::Array(inner, n) => ((**inner).clone(), *n),
                    // D-0254: a lane, as C subscripts a vector.
                    Type::Simd(inner, n) => {
                        let (elem, n) = ((**inner).clone(), *n);
                        self.dollar.push(format!("{}u", n));
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}u, {});", it.c, n, loc));
                        let mut projs = pb.projs;
                        projs.push(format!("{{ CB_INDEX, (uint64_t){} }}", it.c));
                        return Ok(P { c: format!("({})[{}]", pb.c, it.c), ty: elem, base: pb.base, projs, is_binding: false, raw: None });
                    }
                    _ => return internal("index on a non-array"),
                };
                self.dollar.push(format!("{}u", n));
                let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                self.dollar.pop();
                let i = i?;
                let it = self.into_temp(i, e.line)?;
                let loc = self.loc(e.line);
                self.emit(&format!("cb_index_check((uint64_t){}, {}u, {});", it.c, n, loc));
                let mut projs = pb.projs;
                projs.push(format!("{{ CB_INDEX, (uint64_t){} }}", it.c));
                Ok(P { c: format!("{}.a[{}]", pb.c, it.c), ty: elem, base: pb.base, projs, is_binding: false, raw: None })
            }
            ExprKind::Deref(inner) => {
                if let ExprKind::Path(segs, targs) = &inner.kind {
                    if segs.len() == 1 && targs.is_empty() {
                        if let Some(b) = self.lookup(&segs[0]).filter(|b| self.direct_refs.contains(&b.c)) {
                            let p = P { c: b.c, ty: b.ty, base: None, projs: Vec::new(), is_binding: true, raw: None };
                            return self.auto_deref(p);
                        }
                    }
                }
                if let Some(p) = self.unwrapped_map_get(inner, e.line)? {
                    return Ok(p);
                }
                if matches!(self.probe_type(inner, None)?, Type::Guard(_)) {
                    return self.guard_place(inner, e.line);
                }
                let v = self.lower_expr(inner, None)?;
                match &v.ty {
                    Type::Ref(pointee, _) => {
                        let r = self.into_temp(v.clone(), e.line)?;
                        let c = self.g.ctype(pointee)?;
                        Ok(P { c: format!("(*({} *){}.p)", c, r.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", r.c)), projs: Vec::new(), is_binding: false, raw: None })
                    }
                    Type::Rawptr(pointee) => {
                        let t = self.into_temp(v.clone(), e.line)?;
                        // A `rawptr<S>` names `struct S` by its forward
                        // declaration alone (`Gen::ctype`); the raw place
                        // `*p` is an `S`, so `S` is defined, as a field of
                        // it (`(*p).f`) needs even when no `S` value is
                        // ever made.
                        self.g.ctype(pointee)?;
                        Ok(P { c: format!("(*{})", t.c), ty: (**pointee).clone(), base: None, projs: Vec::new(), is_binding: false, raw: Some(t.c) })
                    }
                    _ => internal("deref of a non-pointer"),
                }
            }
            _ => {
                // A temporary used as a place (`make().f`): unchecked,
                // unless a borrow of part of it outlives the expression
                // (D-0103): then it is an object of this statement, and the
                // place is checked from its root path.
                let temp_root = std::mem::take(&mut self.temp_root);
                let v = self.lower_expr(e, None)?;
                let t = self.into_temp(v, e.line)?;
                if temp_root && !matches!(t.ty, Type::Ref(..)) {
                    let loc = self.loc(e.line);
                    let o = match &t.obj {
                        Some(o) => o.clone(),
                        None => {
                            let tid = self.tid(&t.ty)?;
                            let o = self.g.fresh("o");
                            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t.c, tid, loc));
                            o
                        }
                    };
                    let root = self.g.fresh("root");
                    self.emit(&format!("uint64_t {} = cb_temp_path({});", root, o));
                    return Ok(P { c: t.c, ty: t.ty, base: Some(root), projs: Vec::new(), is_binding: false, raw: None });
                }
                // A reference temporary (`index_shared(&w, 0).r`) is held in
                // register form (`cb_ref`), not in a memory slot: the place
                // it stands for is its referent, reached through that token.
                if let Type::Ref(pointee, _) = &t.ty {
                    let c = self.g.ctype(pointee)?;
                    let inner = P { c: format!("(*({} *){}.p)", c, t.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", t.c)), projs: Vec::new(), is_binding: false, raw: None };
                    return self.auto_deref(inner);
                }
                Ok(P { c: t.c, ty: t.ty, base: None, projs: Vec::new(), is_binding: false, raw: None })
            }
        }
    }

    // `K[i]` read, `K` an array constant (`const array<T, N> K = [..]`)
    // whose elements are integer or `bool` literals: the use of a constant
    // is a call of it, which builds the whole array, and indexing that
    // temporary copied every element at every read (`SHA256_K[i]` in a
    // round loop: 256 bytes per element read). Here the element is read
    // from the same values laid out once as C static data, with the same
    // bounds check. Only a read comes this way: the data is never written.
    pub(super) fn const_array_read(&mut self, e: &Expr) -> R<Option<V>> {
        if self.g.boot(2) {
            return Ok(None);
        }
        let ExprKind::Index(base, idx) = &e.kind else { return Ok(None) };
        let ExprKind::Call(c, args) = &strip_parens(base).kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &c.kind else { return Ok(None) };
        if !args.is_empty() || !targs.is_empty() {
            return Ok(None);
        }
        let name = segs.join("::");
        let Some(f) = self.g.items.fns.get(&name).filter(|f| f.is_const && f.params.is_empty() && f.type_params.is_empty()) else { return Ok(None) };
        let Type::Array(elem, n) = &f.ret else { return Ok(None) };
        let Some(ExprKind::ArrayLit(es)) = f.body.tail.as_ref().map(|t| &t.kind) else { return Ok(None) };
        if !f.body.stmts.is_empty() || es.len() as u128 != *n || *n == 0 {
            return Ok(None);
        }
        let (ec, vals): (String, Vec<String>) = match &**elem {
            Type::Int(it) if !matches!(it, IntTy::I128 | IntTy::U128) => {
                let bw = it.bitwidth(64);
                let mut vals = Vec::new();
                for x in es {
                    let ExprKind::IntLit(v, _) = &x.kind else { return Ok(None) };
                    // A literal the checker accepted fits the type; its bits
                    // as the unsigned type of that width, cast back.
                    if bw < 128 && *v >> bw != 0 {
                        return Ok(None);
                    }
                    vals.push(format!("({}){}ULL", c_int_type(*it), *v as u64));
                }
                (c_int_type(*it).to_string(), vals)
            }
            Type::Bool => {
                let mut vals = Vec::new();
                for x in es {
                    let ExprKind::BoolLit(b) = &x.kind else { return Ok(None) };
                    vals.push(if *b { "1" } else { "0" }.to_string());
                }
                ("uint8_t".to_string(), vals)
            }
            _ => return Ok(None),
        };
        self.dollar.push(format!("{}u", n));
        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
        self.dollar.pop();
        let it = self.into_temp(i?, e.line)?;
        let loc = self.loc(e.line);
        self.emit(&format!("cb_index_check((uint64_t){}, {}u, {});", it.c, n, loc));
        let t = self.g.fresh("t");
        self.emit(&format!("{} {} = ({{ static const {} k_[{}] = {{ {} }}; k_[{}]; }});", ec, t, ec, n, vals.join(", "), it.c));
        Ok(Some(V { c: t, ty: (**elem).clone(), obj: None }))
    }

    // `[Field-Access-Auto-Deref]`: a place of reference type used as a
    // struct/array place goes through the reference it holds.
    pub(super) fn auto_deref(&mut self, p: P) -> R<P> {
        match &p.ty {
            Type::Ref(pointee, _) if p.is_binding && self.direct_refs.contains(&p.c) => {
                let c = self.g.ctype(pointee)?;
                Ok(P { c: format!("(*({} *){})", c, p.c), ty: (**pointee).clone(), base: None, projs: Vec::new(), is_binding: false, raw: None })
            }
            Type::Ref(pointee, _) => {
                let tk = self.g.fresh("tk");
                let load = self.slot_tok(&p.c);
                self.emit(&format!("uint64_t {} = {};", tk, load));
                let c = self.g.ctype(pointee)?;
                let inner = P { c: format!("(*({} *){})", c, p.c), ty: (**pointee).clone(), base: Some(tk), projs: Vec::new(), is_binding: false, raw: None };
                self.auto_deref(inner)
            }
            _ => Ok(p),
        }
    }

    // A reference stored in memory, read back as a register value.
    pub(super) fn read_ref_slot(&self, slot: &str, ty: &Type) -> V {
        V { c: format!("((cb_ref){{ {}, {} }})", slot, self.slot_tok(slot)), ty: ty.clone(), obj: None }
    }

    // The token a reference slot holds: loaded, or, for an unstored
    // parameter, the one it came with.
    pub(super) fn slot_tok(&self, slot: &str) -> String {
        match self.unstored.get(slot) {
            Some(t) => t.clone(),
            None => format!("cb_load_ref(&{})", slot),
        }
    }

    // `[Read]` of a place as a value; a resource whole binding moves.
    pub(super) fn read_place(&mut self, p: P, line: usize) -> R<V> {
        let loc = self.loc(line);
        // A whole binding's move is `[Authority-Transfer]`, which
        // `cb_take` checks (temporally valid, then solitary: an alias of
        // either mode is `diag.move-while-aliased`); it is not a `[Read]`.
        // D-0196: an objectless `String` moved out (`return s`, or the
        // function's last expression): the value takes an object here, and
        // the binding keeps an empty `String`, so its drop frees nothing.
        // D-0200: any objectless local moved away (a call's argument, a
        // literal's part, `let y = x`, a `return`): stale if it already
        // was; then its flag is set, which keeps its drop from running and
        // makes any later use stale, as the binding's object would.
        if self.is_res(&p.ty) && p.raw.is_none() && p.is_binding && p.projs.is_empty() && p.base.is_none() && self.objectless_c.contains(&p.c) {
            let t = self.g.fresh("t");
            let c = self.g.ctype(&p.ty)?;
            let tid = self.tid(&p.ty)?;
            self.objectless_use(&p.c.clone(), line);
            self.emit(&format!("{} {} = {};", c, t, p.c));
            match self.moved_flag.get(&p.c).cloned() {
                Some(fl) => self.emit(&format!("{} = 1;", fl)),
                // A type probe (`probe_type`) lowers into a discarded buffer:
                // nothing moves there.
                None if self.g.probing > 0 => {}
                None => {
                    if matches!(&p.ty, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
                        self.emit(&format!("{}.bytes.ptr = (uint8_t *)1; {}.bytes.len = 0; {}.bytes.cap = 0;", p.c, p.c, p.c));
                    } else {
                        if std::env::var("COBC_PANIC").is_ok() { panic!("objectless move {}", p.c); } return internal("an objectless local moved away without a flag");
                    }
                }
            }
            if self.raw_ret && std::mem::take(&mut self.raw_return_path) {
                // A `__raw` form returns it as it is (the caller binds it
                // objectless too): no object at all. Only the result itself:
                // a move anywhere else (a call's argument) takes an object.
                self.inert_c.insert(t.clone());
                return Ok(V { c: t, ty: p.ty, obj: None });
            }
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
            return Ok(V { c: t, ty: p.ty, obj: Some(o) });
        }
        let whole_move = self.is_res(&p.ty) && p.raw.is_none() && p.is_binding && p.projs.is_empty();
        if let Some(base) = p.base.as_ref().filter(|_| !whole_move) {
            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
        }
        if self.is_res(&p.ty) {
            if let Some(ptr) = &p.raw {
                // `[Rawptr-Move-Out]`: the value leaves the raw cells; the
                // reclaimed identity there, if any, ends.
                let t = self.g.fresh("t");
                let c = self.g.ctype(&p.ty)?;
                self.emit(&format!("{} {} = {};", c, t, p.c));
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("cb_raw_move_out({}, &{}, {}u);", ptr, t, tid));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                return Ok(V { c: t, ty: p.ty, obj: Some(o) });
            }
            if !(p.is_binding && p.projs.is_empty()) {
                // A resource moved out of a part of a live object, or out
                // through a reference whose referent its owner still
                // reaches (`[Authority-Transfer-Aliased]`): the checks
                // above found the place valid, and the move itself is the
                // fault, as `coby` reports it.
                let d = if p.projs.is_empty() { "diag.move-while-aliased" } else { "diag.move-out-of-field" };
                self.emit(&format!("cb_fault(\"{}\", {});", d, loc));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = 0;", o));
                return Ok(V { c: p.c, ty: p.ty, obj: Some(o) });
            }
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_take({}, {});", o, p.base.as_deref().unwrap(), loc));
            return Ok(V { c: p.c, ty: p.ty, obj: Some(o) });
        }
        if matches!(p.ty, Type::Ref(..)) {
            return Ok(self.read_ref_slot(&p.c, &p.ty));
        }
        if matches!(p.ty, Type::Fn(..)) {
            // `[Read]` of a fn value (D-0089): its closure is copied, or
            // moves out and empties the slot when it owns a resource.
            let t = self.g.fresh("t");
            self.emit(&format!("void *{} = cb_fn_read((void **)&({}), {});", t, p.c, loc));
            return Ok(V { c: t, ty: p.ty, obj: None });
        }
        if self.holds_fn(&p.ty) {
            // A value holding fn values, read by value: the same, slot by
            // slot, after the copy of its bytes.
            let t = self.g.fresh("t");
            let c = self.g.ctype(&p.ty)?;
            let tid = self.tid(&p.ty)?;
            self.emit(&format!("{} {} = {};", c, t, p.c));
            self.emit(&format!("cb_fn_split(&{}, &({}), {}u, {});", t, p.c, tid, loc));
            return Ok(V { c: t, ty: p.ty, obj: None });
        }
        Ok(V { c: p.c, ty: p.ty, obj: None })
    }
}
