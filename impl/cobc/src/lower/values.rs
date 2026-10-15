// Fx: arguments, variants, slices and struct literals.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // Lowers already-typed arguments and puts resources and reference
    // data in flight; `None` when an argument diverged.
    pub(super) fn lower_args(&mut self, ptypes: &[Type], args: &[Expr]) -> R<Option<Vec<String>>> {
        let mut argv = Vec::new();
        for (pty, a) in ptypes.iter().zip(args.iter()) {
            let v = self.lower_expr(a, Some(pty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(None);
            }
            let t = self.into_temp(v, a.line)?;
            argv.push((pty.clone(), t));
        }
        let mut names = Vec::new();
        for (pty, t) in &argv {
            if self.is_res(pty) {
                self.emit(&format!("cb_send({});", t.obj.as_ref().expect("resource argument has object")));
            } else if self.has_refs(pty) && !matches!(pty, Type::Ref(..)) {
                let tid = self.tid(pty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
            names.push(t.c.clone());
        }
        Ok(Some(names))
    }

    // `[Closure-Call]` on a closure place: an exclusive borrow of it is `self`.
    pub(super) fn call_closure_place(&mut self, id: u64, p: &P, args: &[Expr], line: usize) -> R<V> {
        let info = self.g.closures[&id].clone();
        let Some(base) = &p.base else { return internal("closure call on a temporary") };
        let loc = self.loc(line);
        let tok = self.g.fresh("tok");
        self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_EXCLUSIVE, {});", tok, base, Self::projs_args(&p.projs), loc));
        let self_ref = format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok);
        let Some(names) = self.lower_args(&info.params, args)? else { return Ok(never()) };
        let call = format!("{}({}{})", info.cname, self_ref, names.iter().map(|n| format!(", {}", n)).collect::<String>());
        self.finish_call(call, info.ret, line)
    }

    // A call through a fn value's handle.
    pub(super) fn call_fn_value(&mut self, handle: String, fty: &Type, args: &[Expr], line: usize) -> R<V> {
        let Type::Fn(ps, r) = fty else { return internal("call through a non-fn value") };
        let thunk = self.g.thunk(fty)?;
        let ps = ps.clone();
        let ret = (**r).clone();
        let Some(names) = self.lower_args(&ps, args)? else { return Ok(never()) };
        let call = format!("{}({}{})", thunk, handle, names.iter().map(|n| format!(", {}", n)).collect::<String>());
        self.finish_call(call, ret, line)
    }

    // A freshly built aggregate in local `t`: registered as a temporary
    // object when the runtime must know about it.
    pub(super) fn register_temp(&mut self, t: String, ty: Type, line: usize) -> R<V> {
        let obj = if self.is_res(&ty) || self.has_refs(&ty) {
            let tid = self.tid(&ty)?;
            let loc = self.loc(line);
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
            self.obj_temps.insert(t.clone());
            Some(o)
        } else {
            None
        };
        Ok(V { c: t, ty, obj })
    }

    // A field or payload `dst` of an aggregate being built takes the value
    // `v`. Plain data holding references that arrives with an object of
    // its own (a temporary: a slice, a call's result) moves in, and that
    // object's record ends with its references left where they now are,
    // in the aggregate (`cb_absorb_refs`), rather than being copied there
    // while the temporary's ends with the statement. Anything else is
    // stored as before.
    pub(super) fn store_part(&mut self, dst: &str, ty: &Type, v: &V) -> R<()> {
        if !self.is_res(ty) && self.has_refs(ty) && !matches!(ty, Type::Ref(..)) {
            if let Some(o) = &v.obj {
                self.emit(&format!("{} = {};", dst, v.c));
                self.emit(&format!("cb_move_to({}, &{});", o, dst));
                self.emit(&format!("cb_absorb_refs({});", o));
                return Ok(());
            }
        }
        self.store(dst, ty, v)
    }

    // `V(lit)` (or nested: `Some(Some(1))`) for a variant V with a payload,
    // not shadowed by a local or a function.
    pub(super) fn is_pure_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, _) => {
                    (segs.len() > 1 || self.lookup(&segs[0]).is_none())
                        && !self.g.items.fns.contains_key(&segs.join("::"))
                        && self.variant_of(segs).is_some()
                        && (is_bare_literal(&args[0]) || self.is_pure_variant_literal(&args[0]))
                }
                _ => false,
            },
            ExprKind::Paren(inner) => self.is_pure_variant_literal(inner),
            _ => false,
        }
    }

    pub(super) fn variant_of(&self, segs: &[String]) -> Option<(String, usize)> {
        let vname = segs[segs.len() - 1].clone();
        let mut ename = None;
        if segs.len() >= 2 {
            let prefix = segs[..segs.len() - 1].join("::");
            if self.g.items.enums.contains_key(&prefix) {
                ename = Some(prefix);
            } else {
                // `m::V`: the enum of module `m` with that variant.
                ename = self.g.items.module_variant_enum(&prefix, &vname);
            }
        }
        let ename = match ename {
            Some(e) => e,
            None => self.g.items.enum_of_variant.get(&vname)?.clone(),
        };
        let ed = self.g.enum_decl(&ename)?;
        let vi = ed.variants.iter().position(|v| v.name == vname)?;
        Some((ename, vi))
    }

    pub(super) fn construct_variant(&mut self, ename: &str, vi: usize, targs: &[Type], payload: Option<&Expr>, expected: Option<&Type>, line: usize) -> R<V> {
        let objectless = std::mem::take(&mut self.objectless);
        let ed = self.g.enum_decl(ename).expect("enum");
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in ed.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(Type::Named(n, args)) = expected {
            if n == ename {
                for (p, t) in ed.type_params.iter().zip(args.iter()) {
                    m.entry(p.clone()).or_insert_with(|| t.clone());
                }
            }
        }
        let pv = match (&ed.variants[vi].payload, payload) {
            (Some(pty), Some(pe)) => {
                let hint = apply(pty, &m);
                let exp = if resolved(&hint, &ed.type_params) { Some(hint) } else { None };
                // The payload is the last thing evaluated before the
                // variant is: an inert literal, or a `Box` of one, needs no
                // object of its own (`objectless`).
                self.objectless = self.inert_expr(pe)? || self.inert_box_new(pe)?;
                let v = self.lower_expr(pe, exp.as_ref());
                self.objectless = false;
                let v = v?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                unify(pty, &v.ty, &ed.type_params, &mut m);
                Some(self.into_temp(v, pe.line)?)
            }
            (None, None) => None,
            _ => return internal("variant payload arity"),
        };
        let args: Vec<Type> = ed.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if args.len() != ed.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", ename));
        }
        let ty = Type::Named(ename.to_string(), args);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("en");
        self.emit(&format!("{} {};", c, t));
        self.emit(&format!("{}.tag = {}u;", t, vi));
        if let Some(v) = pv {
            let pty = v.ty.clone();
            self.store_part(&format!("{}.u.v{}", t, vi), &pty, &v)?;
            if self.is_res(&pty) && !self.inert_c.contains(&v.c) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource payload has object")));
            }
        }
        if objectless && self.is_res(&ty) && !self.has_refs(&ty) {
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty, obj: None });
        }
        self.register_temp(t, ty, line)
    }

    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047): the source is
    // borrowed as `&base` would be, and the slice holds that borrow in its
    // reference slot, with a pointer to its first element and its length.
    pub(super) fn lower_slice_of(&mut self, mode: &Mode, base: &Expr, lo: &Expr, hi: &Expr, line: usize) -> R<V> {
        self.lower_slice_of_as(mode, base, lo, hi, line, true)
    }

    // `minted` false: for a pure direct slice parameter (`Gen::pure_direct`),
    // the borrow's checks only, and a bare `{ data, len }`: no path, no
    // tracked slot, no temporary object.
    pub(super) fn lower_slice_of_as(&mut self, mode: &Mode, base: &Expr, lo: &Expr, hi: &Expr, line: usize, minted: bool) -> R<V> {
        let pb = self.lower_place(base)?;
        let pb = self.auto_deref(pb)?;
        let loc = self.loc(line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        let (elem, len, data, parent) = match pb.ty.clone() {
            Type::Array(el, n) => ((*el).clone(), format!("{}u", n), format!("&({}).a[0]", pb.c), None),
            Type::Named(n, args) if n == "std::Vec" => (args.first().cloned().unwrap_or(Type::Void), format!("({}).len", pb.c), format!("({}).ptr", pb.c), None),
            Type::Slice(el, _) => {
                let t0 = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_load_ref(&({}).src);", t0, pb.c));
                ((*el).clone(), format!("({}).len", pb.c), format!("({}).data", pb.c), Some(t0))
            }
            _ => return internal("slice of something that is not an array, a Vec or a slice"),
        };
        let lenv = self.g.fresh("len");
        self.emit(&format!("uint64_t {} = {};", lenv, len));
        self.dollar.push(lenv.clone());
        let a = self.lower_expr(lo, Some(&Type::Int(IntTy::Usize)));
        let b = match &a {
            Ok(_) => Some(self.lower_expr(hi, Some(&Type::Int(IntTy::Usize)))),
            Err(_) => None,
        };
        self.dollar.pop();
        let at = self.into_temp(a?, line)?;
        let bt = self.into_temp(b.unwrap()?, line)?;
        self.emit(&format!("if ((uint64_t){} > (uint64_t){} || (uint64_t){} > {}) cb_fault(\"diag.index-out-of-bounds\", {});", at.c, bt.c, bt.c, lenv, loc));
        if !minted {
            match (&parent, &pb.base) {
                (Some(t0), _) => self.emit(&format!("cb_borrow_range_unminted({}, NULL, 0, (uint64_t){}, (uint64_t){}, {}, {});", t0, at.c, bt.c, m, loc)),
                (None, Some(base)) => self.emit(&format!("cb_borrow_range_unminted({}, {}, (uint64_t){}, (uint64_t){}, {}, {});", base, Self::projs_args(&pb.projs), at.c, bt.c, m, loc)),
                (None, None) => return internal("slice of an unchecked place"),
            }
            let sty = Type::Slice(Box::new(elem), mode.clone());
            let c = self.g.ctype(&sty)?;
            let t = self.g.fresh("sl");
            self.emit(&format!("{} {};", c, t));
            self.emit(&format!("{}.src = (void *)&({});", t, pb.c));
            self.emit(&format!("{}.data = {} + {};", t, data, at.c));
            self.emit(&format!("{}.len = (uint64_t){} - (uint64_t){};", t, bt.c, at.c));
            return Ok(V { c: t, ty: sty, obj: None });
        }
        // D-0070: the slice borrows its range (made absolute by the runtime
        // when the source is itself a slice), not the whole source.
        let tok = self.g.fresh("tok");
        match (&parent, &pb.base) {
            (Some(t0), _) => self.emit(&format!("uint64_t {} = cb_borrow_range({}, NULL, 0, (uint64_t){}, (uint64_t){}, {}, {});", tok, t0, at.c, bt.c, m, loc)),
            (None, Some(base)) => self.emit(&format!("uint64_t {} = cb_borrow_range({}, {}, (uint64_t){}, (uint64_t){}, {}, {});", tok, base, Self::projs_args(&pb.projs), at.c, bt.c, m, loc)),
            (None, None) => return internal("slice of an unchecked place"),
        }
        let sty = Type::Slice(Box::new(elem), mode.clone());
        let c = self.g.ctype(&sty)?;
        let t = self.g.fresh("sl");
        self.emit(&format!("{} {};", c, t));
        let rv = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", pb.c, tok), ty: Type::Ref(Box::new(pb.ty.clone()), mode.clone()), obj: None };
        self.store(&format!("{}.src", t), &rv.ty.clone(), &rv)?;
        self.emit(&format!("{}.data = {} + {};", t, data, at.c));
        self.emit(&format!("{}.len = (uint64_t){} - (uint64_t){};", t, bt.c, at.c));
        self.register_temp(t, sty, line)
    }

    pub(super) fn lower_struct_lit(&mut self, segs: &[String], targs: &[Type], fields: &[(String, Expr)], expected: Option<&Type>, line: usize) -> R<V> {
        let objectless = std::mem::take(&mut self.objectless);
        let name = segs.join("::");
        let Some(sd) = self.g.struct_decl(&name) else { return internal(&format!("unknown struct `{}`", name)) };
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in sd.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(Type::Named(n, args)) = expected {
            if *n == name {
                for (p, t) in sd.type_params.iter().zip(args.iter()) {
                    m.entry(p.clone()).or_insert_with(|| t.clone());
                }
            }
        }
        let mut vals = Vec::new();
        for (fname, fe) in fields {
            let fd = sd.fields.iter().find(|f| &f.name == fname).expect("field");
            let hint = apply(&fd.ty, &m);
            let exp = if resolved(&hint, &sd.type_params) { Some(hint) } else { None };
            // D-0200: a `String` field made with no object where its maker
            // can (`lower_raw_string`): the aggregate's object, made after,
            // covers it, so it is neither received nor absorbed. A fault in
            // a later field leaves its bytes unfreed, which nothing can see
            // (no code of the program's runs to destroy a `String`).
            let string = Type::Named("std::String".to_string(), vec![]);
            if exp.as_ref() == Some(&string) {
                if let Some(v) = self.lower_raw_string(fe)? {
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let t = self.into_temp(v, fe.line)?;
                    vals.push((fname.clone(), t));
                    continue;
                }
            }
            // An inert field owns nothing: a fault in a later field leaves
            // nothing of it to destroy (`inert_expr`).
            self.objectless = self.inert_expr(fe)?;
            let v = self.lower_expr(fe, exp.as_ref());
            self.objectless = false;
            let v = v?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            unify(&fd.ty, &v.ty, &sd.type_params, &mut m);
            let t = self.into_temp(v, fe.line)?;
            vals.push((fname.clone(), t));
        }
        let args: Vec<Type> = sd.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if args.len() != sd.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", name));
        }
        let ty = Type::Named(name.clone(), args);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("st");
        self.emit(&format!("{} {};", c, t));
        if let Some(backing) = sd.bits {
            // D-0118: the fields packed, each checked against its width.
            let loc = self.loc(line);
            let bc = c_int_type(backing);
            self.emit(&format!("{}.bits = 0;", t));
            for (f, v) in vals {
                let (w, off, _, _) = bitfield_layout(&sd, &f).expect("bit field");
                let mask = c_bits_mask(backing, w);
                self.emit(&format!("if (({})({}) > {}) cb_fault(\"diag.narrowing-overflow\", {});", bc, v.c, mask, loc));
                self.emit(&format!("{}.bits |= (({})({})) << {}u;", t, bc, v.c, off));
            }
            return self.register_temp(t, ty, line);
        }
        for (f, v) in vals {
            let fty = v.ty.clone();
            self.store_part(&format!("{}.{}", t, san(&f)), &fty, &v)?;
            if self.is_res(&fty) && !self.inert_c.contains(&v.c) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource field has object")));
            }
        }
        if objectless && self.is_res(&ty) && !self.has_refs(&ty) {
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty, obj: None });
        }
        self.register_temp(t, ty, line)
    }
}
