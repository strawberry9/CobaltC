// Fields, indexing and slices.

use super::*;

impl Interp {
    pub(super) fn eval_field(&mut self, base: &Expr, name: &str, subst: &HashMap<String, Type>) -> EvalOutcome {
        // `(*p).f` with `p : rawptr<S>`: `*p` is a place (`spec/13` §1), so
        // its field is too. The cells are re-attached as the reclaimed
        // object over them, exactly as `reclaim<S>(p).f` would, and the
        // field is read or written through it.
        let mut bare = base;
        while let ExprKind::Paren(e) = &bare.kind {
            bare = e;
        }
        let r = if let ExprKind::Deref(inner) = &bare.kind {
            let pr = self.eval(inner, subst)?;
            let pv = match &pr {
                EvalResult::Val(v) => Some(v.clone()),
                EvalResult::Place(o, p, _) => Some(self.read_place(*o, p)),
                EvalResult::Temp(_) => None,
            };
            match pv {
                Some(Value::Rawptr(addr, pty)) if matches!(pty, Type::Named(..)) => {
                    self.require_unsafe()?;
                    let id = self.intrinsic_reclaim(addr, pty);
                    EvalResult::Place(id, vec![], vec![])
                }
                _ => self.eval_deref_result(pr)?,
            }
        } else {
            self.eval(base, subst)?
        };
        if let EvalResult::Temp(o) = r {
            self.stmt_temp(o);                      // `make().f`: the temporary ends with the statement
        }
        let (obj, path, chain) = match r {
            EvalResult::Place(obj, path, chain) => (obj, path, chain),
            EvalResult::Temp(obj) => (obj, vec![], vec![]),
            EvalResult::Val(Value::Ref { obj, path, token, mode, .. }) => {
                self.affine(token, mode == Mode::Exclusive)?;
                (obj, path, vec![token])
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let (obj, mut path, chain) = self.resolve_through_refs(obj, path, chain)?;
        let sty = self.type_at(obj, &path);
        let idx = self.field_index(&sty, name)?;
        path.push(Proj::Field(idx));
        Ok(EvalResult::Place(obj, path, chain))
    }

    pub(super) fn field_index(&self, sty: &Type, name: &str) -> Result<usize, Flow> {
        if let Type::Named(sname, _) = sty {
            if let Some(s) = self.items.structs.get(sname) {
                return s
                    .fields
                    .iter()
                    .position(|f| f.name == name)
                    .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()));
            }
            if sname == "__mutex__" {
                if name == "inner" {
                    return Ok(0);
                }
            }
        }
        if let Type::Mutex(_) = sty {
            if name == "inner" {
                return Ok(0);
            }
        }
        Err(Flow::Fault("diag.type-mismatch".into()))
    }

    pub(super) fn eval_index(&mut self, base: &Expr, idx: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let write = std::mem::replace(&mut self.write_ctx, false);
        self.write_ctx = write;
        let r = self.eval(base, subst);
        self.write_ctx = false;
        let r = r?;
        if let EvalResult::Temp(o) = r {
            self.stmt_temp(o);                      // `make()[i]`: ends with the statement
        }
        let (obj, path, chain) = match r {
            EvalResult::Place(o, p, chain) => (o, p, chain),
            EvalResult::Temp(o) => (o, vec![], vec![]),
            EvalResult::Val(Value::Ref { obj, path, token, mode, .. }) => {
                self.affine(token, mode == Mode::Exclusive)?;
                (obj, path, vec![token])
            }
            // D-0254: a lane of a `simd` value (an operator's or an
            // intrinsic's result), held for the statement as `make()[i]` is.
            EvalResult::Val(v @ Value::Array(_)) => {
                let ty = Self::simd_type_of(&v).ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                let o = self.new_object(ty, v);
                self.stmt_temp(o);
                (o, vec![], vec![])
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let (obj, path, chain) = self.resolve_through_refs(obj, path, chain)?;
        let aty = self.type_at(obj, &path);
        // `$` (D-0047) is this collection's length.
        let len = self.indexed_len(obj, &path, &aty)?;
        self.dl().push(len);
        let iv = self.eval_to_value(idx, subst);
        self.dl().pop();
        let i = match iv? {
            Value::Int(_, n) if n >= 0 => n as usize,
            Value::Int(..) => return Err(Flow::Fault("diag.index-out-of-bounds".into())),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if i >= len {
            return Err(Flow::Fault("diag.index-out-of-bounds".into()));
        }
        self.index_at(obj, path, chain, &aty, i, write, subst)
    }

    // Element `i` (already checked below the length) of the collection at
    // `obj.path` of type `aty`, reached as `[Index-Checked]` reaches it:
    // `eval_index`'s second half, and `simd_load`'s and `simd_store`'s
    // lanes (D-0254).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn index_at(&mut self, obj: u64, mut path: Vec<Proj>, mut chain: Vec<u64>, aty: &Type, i: usize, write: bool, subst: &HashMap<String, Type>) -> EvalOutcome {
        match aty {
            Type::Array(..) | Type::Simd(..) => {
                path.push(Proj::Index(i));
                Ok(EvalResult::Place(obj, path, chain))
            }
            // A `Vec`'s element (D-0047): the `Vec` is read or written as a
            // whole, in the mode of the access, then the element is reached
            // in its buffer as `Vec::index_shared`/`index_exclusive` do.
            Type::Named(n, args) if n == "std::Vec" => {
                let mode = if write { Mode::Exclusive } else { Mode::Shared };
                // The element's own index on the `Vec`'s path: a slice of
                // other elements (D-0070) is not in the way, the `Vec`
                // borrowed whole is.
                let mut epath = path.clone();
                epath.push(Proj::Index(i));
                if self.clash(obj, &epath, mode.clone(), &chain) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let elem = args.first().map(|t| self.apply_subst(t, subst)).unwrap_or(Type::Void);
                let id = self.vec_element(obj, &path, &elem, i)?;
                // `[Index-Vec]`: the borrow is checked as an access to
                // element k -- the whole element, so a live reference into
                // part of it (`&mut v[k].y`) conflicts with reaching
                // `v[k].x`, as one into another element does not.
                if self.clash(id, &[], mode, &chain) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                Ok(EvalResult::Place(id, vec![], chain))
            }
            // A slice's element: through the slice's own reference to its
            // source, so the slice's borrow is this access's ancestor.
            Type::Slice(elem, _) => {
                let (src, start, _) = self.slice_parts(obj, &path)?;
                let Value::Ref { obj: so, path: sp, token, mode: smode, .. } = src else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                if !self.object_live(so) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                self.affine(token, smode == Mode::Exclusive)?;
                chain.push(token);
                let sty = self.type_at(so, &sp);
                match &sty {
                    Type::Array(..) => {
                        let mut p = sp.clone();
                        p.push(Proj::Index(start + i));
                        Ok(EvalResult::Place(so, p, chain))
                    }
                    _ => {
                        // The element type is the source `Vec`'s own, which is
                        // concrete; the slice's may name a type parameter of
                        // the function this runs in (`slice<T, shared>`).
                        let et = match &sty {
                            Type::Named(n, args) if n == "std::Vec" => args.first().cloned().unwrap_or_else(|| self.apply_subst(elem, subst)),
                            _ => self.apply_subst(elem, subst),
                        };
                        // D-0248: the access is checked on the vector too, at
                        // the element's index, as `[Index-Vec]` checks it: a
                        // reborrow of this slice lent to a thread, or a borrow
                        // of the whole vector, is in the way (with the slice's
                        // own borrow among the ancestors).
                        let mode = if write { Mode::Exclusive } else { Mode::Shared };
                        let mut epath = sp.clone();
                        epath.push(Proj::Index(start + i));
                        if self.clash(so, &epath, mode, &chain) {
                            return Err(Flow::Fault("diag.aliasing-conflict".into()));
                        }
                        let id = self.vec_element(so, &sp, &et, start + i)?;
                        Ok(EvalResult::Place(id, vec![], chain))
                    }
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // How many elements the collection at `obj.path` of type `ty` has.
    pub(super) fn indexed_len(&self, obj: u64, path: &[Proj], ty: &Type) -> Result<usize, Flow> {
        match ty {
            Type::Array(_, n) | Type::Simd(_, n) => Ok(*n as usize),
            Type::Named(n, _) if n == "std::Vec" => match self.read_place(obj, path) {
                Value::Struct(fs) => match fs.get(1) {
                    Some(Value::Int(_, l)) => Ok(*l as usize),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                },
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            Type::Slice(..) => Ok(self.slice_parts(obj, path)?.2),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // A slice value's parts: the reference to its source, where it starts
    // in the source, and its length.
    pub(super) fn slice_parts(&self, obj: u64, path: &[Proj]) -> Result<(Value, usize, usize), Flow> {
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        match self.read_place(obj, path) {
            Value::Struct(fs) if fs.len() == 3 => match (&fs[1], &fs[2]) {
                (Value::Int(_, a), Value::Int(_, n)) => Ok((fs[0].clone(), *a as usize, *n as usize)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Element `i` of the `Vec` at `obj.path`, as the object in its buffer.
    pub(super) fn vec_element(&mut self, obj: u64, path: &[Proj], elem: &Type, i: usize) -> Result<u64, Flow> {
        let Value::Struct(fs) = self.read_place(obj, path) else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let Some(Value::Rawptr(addr, _)) = fs.first() else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let at = addr + (i as u64) * self.sizeof_ty(elem);
        Ok(self.intrinsic_reclaim(at, elem.clone()))
    }

    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047): a slice value
    // `{ ref to the source, start, length }`. The reference is formed as
    // `&source` would be, so the source stays borrowed while the slice
    // lives; a slice of a slice refers to the same source.
    // `key_less` of two references to keys: integers by value, `false`
    // before `true`, text by its bytes.
    pub(super) fn key_less_values(&mut self, a: Value, b: Value) -> Result<bool, Flow> {
        // D-0110: a struct or enum key.
        if let (Value::Ref { obj: oa, path: pa, pointee: pt @ Type::Named(n, _), .. }, Value::Ref { obj: ob, path: pb, .. }) = (&a, &b) {
            if n != "std::String" {
                let (x, y) = (self.read_place(*oa, pa), self.read_place(*ob, pb));
                return Ok(self.key_cmp(&x, &y, pt)?.is_lt());
            }
        }
        let read = |me: &Self, v: &Value| -> Option<Value> {
            match v {
                Value::Ref { obj, path, pointee, .. } if !matches!(pointee, Type::Named(n, _) if n == "std::String") => Some(me.read_place(*obj, path)),
                _ => None,
            }
        };
        match (read(self, &a), read(self, &b)) {
            (Some(Value::Int(t, x)), Some(Value::Int(_, y))) => Ok(if t.signed() { x < y } else { (x as u128) < (y as u128) }),
            (Some(Value::Bool(x)), Some(Value::Bool(y))) => Ok(!x & y),
            // D-0181: a float's order, a NaN after every other value.
            (Some(Value::F64(x)), Some(Value::F64(y))) => Ok(x < y || (y.is_nan() && !x.is_nan())),
            (Some(Value::F32(x)), Some(Value::F32(y))) => Ok(x < y || (y.is_nan() && !x.is_nan())),
            _ => {
                let ka = self.key_bytes(a)?;
                let kb = self.key_bytes(b)?;
                Ok(ka < kb)
            }
        }
    }

    pub(super) fn eval_slice_of(&mut self, mode: Mode, base: &Expr, lo: &Expr, hi: &Expr, subst: &HashMap<String, Type>, temp_ok: bool) -> EvalOutcome {
        self.write_ctx = mode == Mode::Exclusive;
        let r = self.eval(base, subst);
        self.write_ctx = false;
        let (obj, path, chain) = match r? {
            EvalResult::Place(o, p, chain) => (o, p, chain),
            EvalResult::Val(Value::Ref { obj, path, token, mode, .. }) => {
                self.affine(token, mode == Mode::Exclusive)?;
                (obj, path, vec![token])
            }
            // D-0103: the temporary lives to the end of the statement.
            EvalResult::Temp(o) if temp_ok => {
                self.stmt_temp(o);
                (o, vec![], vec![])
            }
            EvalResult::Val(v) if temp_ok => {
                let ty = self.type_of_value(&v);
                let o = self.new_object(ty, v);
                self.stmt_temp(o);
                (o, vec![], vec![])
            }
            EvalResult::Temp(_) => return Err(Flow::Fault("diag.borrow-of-temporary".into())),
            _ => return Err(Flow::Fault("diag.borrow-of-non-place".into())),
        };
        let (obj, path, mut chain) = self.resolve_through_refs(obj, path, chain)?;
        let ty = self.type_at(obj, &path);
        let len = self.indexed_len(obj, &path, &ty)?;
        self.dl().push(len);
        let a = self.eval_to_value(lo, subst);
        let b = a.as_ref().ok().map(|_| self.eval_to_value(hi, subst));
        self.dl().pop();
        let (a, b) = match (a?, b.unwrap()?) {
            (Value::Int(_, a), Value::Int(_, b)) => (a, b),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if a < 0 || b < a || b as usize > len {
            return Err(Flow::Fault("diag.index-out-of-bounds".into()));
        }
        let (elem, src_obj, src_path, offset) = match &ty {
            Type::Array(el, _) => ((**el).clone(), obj, path.clone(), 0usize),
            Type::Named(n, args) if n == "std::Vec" => (args.first().cloned().unwrap_or(Type::Void), obj, path.clone(), 0),
            Type::Slice(el, _) => {
                let (src, start, _) = self.slice_parts(obj, &path)?;
                let Value::Ref { obj: so, path: sp, token, mode: smode, .. } = src else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                self.affine(token, smode == Mode::Exclusive)?;
                chain.push(token);
                ((**el).clone(), so, sp, start)
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if !self.object_live(src_obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        // D-0070: the slice borrows its elements, not the whole source.
        let (lo, hi) = (offset + a as usize, offset + b as usize);
        let mut ranged = src_path.clone();
        ranged.push(Proj::Range(lo, hi));
        if self.clash(src_obj, &ranged, mode.clone(), &chain) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let token = self.mint_token(&mode);
        let anc = self.record_token_ancestors(token, &chain);
        let pointee = self.type_at(src_obj, &src_path);
        let rv = Value::Ref { obj: src_obj, path: src_path, mode: mode.clone(), pointee, token, range: Some((lo, hi)), anc };
        let v = Value::Struct(vec![rv, Value::Int(IntTy::Usize, (offset as i128) + a), Value::Int(IntTy::Usize, b - a)]);
        let id = self.new_object(Type::Slice(Box::new(elem), mode), v);
        Ok(EvalResult::Temp(id))
    }
}
