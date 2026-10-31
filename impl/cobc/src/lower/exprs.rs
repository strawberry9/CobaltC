// Fx: expressions.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // ---- expressions ----

    // A value where a `fn` type is expected: a closure moves into a
    // callable box ([Callable-Closure]); anything else is itself.
    pub(super) fn lower_expr(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        let saved = std::mem::replace(&mut self.place_write, false);
        let v = self.lower_expr_inner(e, expected);
        self.place_write = saved;
        let v = v?;
        if let (Some(dst @ Type::Fn(..)), Type::Closure(_)) = (expected, &v.ty) {
            return self.box_closure(v, dst.clone(), e.line);
        }
        if self.shared_fn_read(e) && !matches!(v.ty, Type::Never) {
            // D-0218 `[Read-Fn-Through-Shared]`: no closure in a value read
            // through a shared reference holds an exclusive capture.
            let loc = self.loc(e.line);
            let v = self.into_temp(v, e.line)?;
            if matches!(v.ty, Type::Fn(..)) {
                self.emit(&format!("cb_fn_shared({}, {});", v.c, loc));
            } else {
                let tid = self.tid(&v.ty)?;
                self.emit(&format!("cb_fn_shared_value(&{}, {}u, {});", v.c, tid, loc));
            }
            return Ok(v);
        }
        Ok(v)
    }

    pub(super) fn shared_fn_read(&self, e: &Expr) -> bool {
        self.g.items.any_shared_fn_reads.load(std::sync::atomic::Ordering::Relaxed) && self.g.items.shared_fn_reads.lock().unwrap().contains(&(e as *const Expr as usize))
    }

    // `[Callable-Closure]`: a closure value moves into a callable box.
    pub(super) fn box_closure(&mut self, v: V, dst: Type, line: usize) -> R<V> {
        let Type::Closure(id) = v.ty else { return internal("boxing a non-closure") };
        let info = self.g.closures[&id].clone();
        // A closure read out of a binding is copied (non-resource) or
        // moved (resource), as any struct; either way the box takes
        // an object of its own.
        let v = if v.obj.is_none() { self.into_temp(v, line)? } else { v };
        let o = match &v.obj {
            Some(o) => o.clone(),
            None => {
                let tid = self.tid(&v.ty)?;
                let loc = self.loc(line);
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, v.c, tid, loc));
                o
            }
        };
        let tid = self.tid(&v.ty)?;
        let h = self.g.fresh("h");
        self.emit(&format!("void *{} = cb_fn_box({}, (void *){}, {}u);", h, o, info.cname, tid));
        if info.fields.is_empty() {
            // Its code never reaches `self` (`lower_closure_body`): a call
            // through the box forms no borrow of the object (`CbFnBox`).
            self.emit(&format!("((cb_fnbox *){})->closure = 2u;", h));
        } else if info.writes_self {
            // D-0239: its calls write its own captures.
            self.emit(&format!("((cb_fnbox *){})->closure = 3u;", h));
        }
        if self.g.pure_closures.contains(&id) && plain_ret(&self.g, &info.ret) {
            self.emit(&format!("cb_fn_mark_pure((void *){});", info.cname));
        }
        Ok(V { c: h, ty: dst, obj: None })
    }

    pub(super) fn lower_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        match &e.kind {
            ExprKind::IntLit(v, None) if matches!(expected, Some(Type::F32 | Type::F64)) && self.g.items.number_literals.lock().unwrap().contains(&(e as *const Expr as usize)) => {
                // D-0090: `T x = 0` with `T` a floating-point type.
                let ty = expected.cloned().unwrap();
                Ok(V { c: c_float(*v as f64, &ty), ty, obj: None })
            }
            ExprKind::IntLit(v, suf) => {
                let ty = int_lit_type(suf.as_deref(), expected);
                Ok(V { c: c_int(ty, *v as i128), ty: Type::Int(ty), obj: None })
            }
            ExprKind::FloatLit(f, suf) => {
                let ty = match (suf.as_deref(), expected) {
                    (Some("f32"), _) => Type::F32,
                    (Some(_), _) => Type::F64,
                    (None, Some(Type::F32)) => Type::F32,
                    _ => Type::F64,
                };
                Ok(V { c: c_float(*f, &ty), ty, obj: None })
            }
            ExprKind::StrLit(s) => {
                // Named by content, so the same literal has the same name in
                // every program and is defined once per program.
                let bytes = s.as_bytes();
                let mut name = format!("cb_s_{:08x}", fnv1a32(bytes) ^ (bytes.len() as u32).rotate_left(16));
                while self.g.str_names.get(&name).map_or(false, |b| b.as_slice() != bytes) {
                    name.push('x');
                }
                if !self.g.str_names.contains_key(&name) {
                    self.g.str_names.insert(name.clone(), bytes.to_vec());
                    self.g.str_log.push(name.clone());
                    let mut lit = String::new();
                    for b in bytes {
                        write!(lit, "{},", b).unwrap();
                    }
                    writeln!(self.g.strings, "static const uint8_t {}[] = {{{}0}};", name, lit).unwrap();
                }
                self.g.nstr += 1;
                Ok(V { c: format!("((cb_str){{ {}, {}u }})", name, bytes.len()), ty: Type::Str, obj: None })
            }
            ExprKind::BoolLit(b) => Ok(V { c: if *b { "1".into() } else { "0".into() }, ty: Type::Bool, obj: None }),
            ExprKind::Unit => Ok(unit()),
            ExprKind::Paren(inner) => self.lower_expr(inner, expected),
            ExprKind::Path(segs, targs) => self.lower_path(e, segs, targs, expected),
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => self.lower_place_value(e, expected),
            ExprKind::Call(callee, _) if matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "reclaim") => {
                let p = self.lower_place(e)?;
                self.read_place(p, e.line)
            }
            ExprKind::StructLit(segs, targs, fields) => self.lower_struct_lit(segs, targs, fields, expected, e.line),
            // D-0254: a `simd` literal, a C compound literal of its vector type.
            ExprKind::ArrayLit(es) if matches!(expected, Some(Type::Simd(..))) => {
                let ty = expected.cloned().unwrap();
                let Type::Simd(lane, _) = &ty else { unreachable!() };
                let lane = (**lane).clone();
                let mut vals = Vec::new();
                for x in es {
                    let v = self.lower_expr(x, Some(&lane))?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    vals.push(self.into_temp(v, x.line)?.c);
                }
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({}){{ {} }})", c, vals.join(", ")), ty, obj: None })
            }
            ExprKind::ArrayRepeat(x, n) if matches!(expected, Some(Type::Simd(..))) => {
                let ty = expected.cloned().unwrap();
                let Type::Simd(lane, _) = &ty else { unreachable!() };
                let lane = (**lane).clone();
                let v = self.lower_expr(x, Some(&lane))?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let one = self.into_temp(v, x.line)?.c;
                let c = self.g.ctype(&ty)?;
                let lanes = vec![one; *n as usize].join(", ");
                Ok(V { c: format!("(({}){{ {} }})", c, lanes), ty, obj: None })
            }
            ExprKind::ArrayLit(es) => {
                let mut elem_ty = match expected {
                    Some(Type::Array(inner, _)) => Some((**inner).clone()),
                    _ => None,
                };
                let mut vals = Vec::new();
                for x in es {
                    let v = self.lower_expr(x, elem_ty.as_ref())?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    if elem_ty.is_none() {
                        elem_ty = Some(v.ty.clone());
                    }
                    vals.push(self.into_temp(v, x.line)?);
                }
                let Some(elem) = elem_ty else { return internal("empty array literal without an expected type") };
                let ty = Type::Array(Box::new(elem.clone()), es.len() as u128);
                let c = self.g.ctype(&ty)?;
                let t = self.g.fresh("arr");
                self.emit(&format!("{} {};", c, t));
                for (i, v) in vals.iter().enumerate() {
                    self.store(&format!("{}.a[{}]", t, i), &elem, v)?;
                    if self.is_res(&elem) {
                        self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource element has object")));
                    }
                }
                self.register_temp(t, ty, e.line)
            }
            // `[v; N]` (D-0072): v once, copied into every element (its type
            // is plain: no resource, reference or function value).
            ExprKind::ArrayRepeat(x, n) => {
                let elem_ty = match expected {
                    Some(Type::Array(inner, _)) => Some((**inner).clone()),
                    _ => None,
                };
                let v = self.lower_expr(x, elem_ty.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let elem = elem_ty.unwrap_or_else(|| v.ty.clone());
                let ec = self.g.ctype(&elem)?;
                let one = self.g.fresh("rep");
                self.emit(&format!("{} {} = {};", ec, one, v.c));
                let ty = Type::Array(Box::new(elem), *n);
                let c = self.g.ctype(&ty)?;
                let t = self.g.fresh("arr");
                let k = self.g.fresh("k");
                self.emit(&format!("{} {};", c, t));
                self.emit(&format!("for (uint64_t {k} = 0; {k} < {n}ULL; {k}++) {t}.a[{k}] = {one};"));
                self.register_temp(t, ty, e.line)
            }
            ExprKind::Unary(op, inner) => self.lower_unary(*op, inner, expected, e.line),
            ExprKind::Binary(op, l, r) => {
                if let Some(vo) = self.view_op(e) {
                    return self.lower_view_op(e, vo);
                }
                self.proven_next = self.no_overflow.contains(&(e as *const Expr as usize));
                self.lower_binary(*op, l, r, expected, e.line)
            }
            ExprKind::Dollar => match self.dollar.last() {
                Some(c) => Ok(V { c: c.clone(), ty: Type::Int(IntTy::Usize), obj: None }),
                None => internal("`$` outside an index"),
            },
            ExprKind::SliceOf(mode, base, lo, hi) => match self.view_op(e) {
                Some(vo) => self.lower_view_op(e, vo),
                None => {
                    // D-0103: a temporary sliced as a call's argument.
                    self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                    let r = self.lower_slice_of(mode, base, lo, hi, e.line);
                    self.temp_root = false;
                    r
                }
            },
            // D-0073: a temporary borrowed as a call's argument: the value
            // is an object of this statement, borrowed from its root path.
            ExprKind::Borrow(mode, inner)
                if (coby::typecheck::is_temp_value(inner)
                    || (matches!(inner.kind, ExprKind::Path(..) | ExprKind::Paren(_)) && self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize))))
                    && !self.is_global(inner) =>
            {
                // D-0084: a borrowed literal takes the referent type expected.
                let pointee = match expected {
                    Some(Type::Ref(t, _)) => Some((**t).clone()),
                    _ => None,
                };
                let v = self.lower_expr(inner, pointee.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, e.line)?;
                let loc = self.loc(e.line);
                let o = match &v.obj {
                    Some(o) => o.clone(),
                    None => {
                        let tid = self.tid(&v.ty)?;
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, v.c, tid, loc));
                        o
                    }
                };
                let root = self.g.fresh("root");
                self.emit(&format!("uint64_t {} = cb_temp_path({});", root, o));
                let tok = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, {}, {});", tok, root, m, loc));
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", v.c, tok), ty: Type::Ref(Box::new(v.ty), mode.clone()), obj: None })
            }
            // `&s` of an objectless `String` (`Gen::objectless_strings`):
            // every such borrow is handed to a reader, whichever way the call
            // is lowered, so its address with token 0 (every check proven).
            // D-0196: `&mut s` of an objectless `String`, which the analysis
            // allowed only as a `std` builder's first argument: its address,
            // token 0.
            ExprKind::Borrow(Mode::Exclusive, inner)
                if path_name(inner).and_then(|n| self.lookup(n)).map_or(false, |b| self.objectless_c.contains(&b.c)) =>
            {
                let b = self.lookup(path_name(inner).unwrap()).unwrap();
                self.objectless_use(&b.c, e.line);
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", b.c), ty: Type::Ref(Box::new(b.ty), Mode::Exclusive), obj: None })
            }
            ExprKind::Borrow(Mode::Shared, inner)
                if path_name(inner).and_then(|n| self.lookup(n)).map_or(false, |b| self.objectless_c.contains(&b.c)) =>
            {
                let b = self.lookup(path_name(inner).unwrap()).unwrap();
                self.objectless_use(&b.c, e.line);
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", b.c), ty: Type::Ref(Box::new(b.ty), Mode::Shared), obj: None })
            }
            ExprKind::Borrow(mode, inner) => {
                // D-0258: `&NAME` of a static, `&K` of an addressed constant:
                // a C global's address with token 0 (`statics.rs`).
                if let Some(v) = self.global_borrow(inner)? {
                    return Ok(v);
                }
                self.place_write = *mode == Mode::Exclusive;
                // D-0103: part of a temporary, as a call's argument: its
                // root becomes an object of this statement.
                self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                let p = self.lower_place(inner);
                self.temp_root = false;
                self.place_write = false;
                let p = p?;
                // The compound pair's `&mut v[i]` on a confined vector
                // (`compound_elem`): the element's address, its bounds
                // checked; only a read and a write through it follow.
                if self.compound_elem && p.base.is_none() && p.raw.is_none() && !p.is_binding && !matches!(p.ty, Type::Never) {
                    let ad = self.g.fresh("ad");
                    self.emit(&format!("void *{} = (void *)&({});", ad, p.c));
                    return Ok(V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                }
                // D-0286: a field of an objectless local (a decided program's,
                // `Gen::objectless_strings_with`): its address, token 0.
                if p.base.is_none() && p.raw.is_none() && !matches!(p.ty, Type::Never) && self.objectless_root(inner) {
                    return Ok(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", p.c), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                }
                let Some(base) = &p.base else { return internal(&format!("borrow of a temporary (at {})", match coby::prelude::std_location(e.line) { Some((f, l)) => format!("std/{}:{}", f, l), None => format!("line {}", e.line.saturating_sub(prelude::line_count())) })) };
                // `&v[i]` / `&mut v[i]` on a `Vec`: the index call, in the
                // borrow's own mode (`vec_index_call` chose it from
                // `place_write`), has just formed the element's path; a
                // borrow of it in that mode, with nothing in between,
                // cannot fail and would be a second path no check could
                // tell from the first. That path is the result.
                // The same for `&*f(…)` / `&mut *f(…)`, `f` returning a
                // reference of the borrow's mode: the reference the call
                // just returned is that path (`*HashMap::entry(…) += 1`).
                if let ExprKind::Deref(x) = &strip_parens(inner).kind {
                    if matches!(&strip_parens(x).kind, ExprKind::Call(..)) && p.projs.is_empty() && p.raw.is_none() && base.ends_with(".tok") && !p.is_binding {
                        if let Ok(Type::Ref(_, rm)) = self.probe_type(x, None) {
                            if rm == *mode {
                                return Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, base), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                            }
                        }
                    }
                }
                if matches!(&strip_parens(inner).kind, ExprKind::Index(..)) && p.projs.is_empty() && p.raw.is_none() && base.ends_with(".tok") && !p.is_binding {
                    if let Some(Type::Named(n, _)) = self.probe_place_type(match &strip_parens(inner).kind { ExprKind::Index(b, _) => b, _ => unreachable!() }).ok().map(|t| match t { Type::Ref(x, _) => *x, t => t }) {
                        if n == "std::Vec" {
                            return Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, base), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                        }
                    }
                }
                let loc = self.loc(e.line);
                let tok = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {});", tok, base, Self::projs_args(&p.projs), m, loc));
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None })
            }
            ExprKind::Call(callee, args) => self.lower_call(callee, args, expected, e.line),
            ExprKind::Assign(place, val) => self.lower_assign(e, place, val, expected),
            ExprKind::Propagate(inner) => {
                // `rule.fail.propagate`'s own desugaring:
                // `match (e) { Ok(v) : v, Err(err) : return Err(err) }`.
                let l = e.line;
                let path = |n: &str| Expr { kind: ExprKind::Path(vec![n.to_string()], vec![]), line: l, col: e.col };
                let is_option = matches!(self.probe_type(inner, None)?, Type::Named(ref n, _) if n == "std::Option");
                let arms = if is_option {
                    // On an `Option`: `match (e) { Some(v) : v, None : return None }`.
                    vec![
                        Arm { variant: Some("Some".into()), nested: Vec::new(), lit: None, binder: Some("__pv".into()), body: Box::new(path("__pv")) },
                        Arm { variant: Some("None".into()), nested: Vec::new(), lit: None, binder: None, body: Box::new(Expr { kind: ExprKind::Return(Some(Box::new(path("None")))), line: l, col: e.col }) },
                    ]
                } else {
                    let err_call = Expr { kind: ExprKind::Call(Box::new(path("Err")), vec![path("__pe")]), line: l, col: e.col };
                    vec![
                        Arm { variant: Some("Ok".into()), nested: Vec::new(), lit: None, binder: Some("__pv".into()), body: Box::new(path("__pv")) },
                        Arm { variant: Some("Err".into()), nested: Vec::new(), lit: None, binder: Some("__pe".into()), body: Box::new(Expr { kind: ExprKind::Return(Some(Box::new(err_call))), line: l, col: e.col }) },
                    ]
                };
                let arms = std::rc::Rc::new(arms);
                self.kept.push(arms.clone());
                self.propagate_scrutinee = true;
                self.lower_match(inner, &arms, expected, l)
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => {
                if let Some(v) = self.lower_fused_text_loop(b, e.line)? {
                    return Ok(v);
                }
                self.frame_line = Some(e.line);
                self.lower_block(b, expected)
            }
            ExprKind::If(c, then_b, else_e) => self.lower_if(c, then_b, else_e.as_deref(), expected, e.line),
            ExprKind::While(c, b, step) => self.lower_while(c, b, step.as_deref()),
            ExprKind::Match(scrut, arms) => self.lower_match(scrut, arms, expected, e.line),
            ExprKind::Return(opt) => {
                let ret = self.ret.clone();
                let v = match opt {
                    Some(x) => {
                        self.raw_return_path = (self.raw_ret || self.g.decided) && matches!(strip_parens(x).kind, ExprKind::Path(..));
                        // D-0286: a call's result returned with no object.
                        self.arm_inert(x, true);
                        // D-0294: a literal returned in a decided program
                        // gets none either (as a block's tail, `lower_block_body`).
                        self.objectless = self.g.decided && (self.literal_expr(x) || self.box_new_expr(x));
                        let v = self.lower_expr(x, Some(&ret));
                        self.objectless = false;
                        self.raw_return_path = false;
                        self.inert_result = None;
                        v?
                    }
                    None => unit(),
                };
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                self.emit_return(v)?;
                Ok(never())
            }
            ExprKind::Break | ExprKind::Continue => {
                let (target, cont) = self.loops.last().cloned().expect("static pass: break inside a loop");
                if matches!(e.kind, ExprKind::Break) {
                    if let Some(b) = self.loop_broken.last_mut() {
                        *b = true;
                    }
                }
                self.emit_unwind_to(target);
                match (&e.kind, cont) {
                    (ExprKind::Break, _) => self.emit("break;"),
                    (_, Some(label)) => self.emit(&format!("goto {};", label)),
                    (_, None) => self.emit("continue;"),
                }
                Ok(never())
            }
            ExprKind::Closure { is_move, captures, params, ret, body } => self.lower_closure(*is_move, captures, params, ret.as_ref(), body, e.line),
        }
    }

    // A place read as a value: a field, an element or a dereference.
    fn lower_place_value(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        // CHG-0237: plain data in a field of a frozen parameter
        // (`frozen_root`): read with no check.
        if matches!(e.kind, ExprKind::Field(..)) && self.frozen_root(e) {
            let p = self.lower_place(e)?;
            if self.g.plain_data(&p.ty) {
                let v = V { c: p.c, ty: p.ty, obj: None };
                return self.into_temp(v, e.line);
            }
        }
        // D-0103: a field moved out of a temporary.
        if matches!(e.kind, ExprKind::Field(..)) && self.g.items.any_temp_moves.load(std::sync::atomic::Ordering::Relaxed) {
            let r = self.g.items.temp_moves.lock().unwrap().get(&(e as *const Expr as usize)).cloned();
            if let Some(r) = r {
                return self.lower_expr(&r, expected);
            }
        }
        if let Some(v) = self.elem_field_read(e)? {
            return Ok(v);
        }
        if let ExprKind::Deref(inner) = &e.kind {
            if let ExprKind::Path(segs, _) = &inner.kind {
                if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
                    return self.lower_expr(&alias, expected);
                }
            }
        }
        if let Some(r) = self.alias_field_chain(e) {
            return self.lower_expr(&r, expected);
        }
        if let Some(v) = self.const_array_read(e)? {
            return Ok(v);
        }
        if let Some(x) = self.vec_index_call(e, false)? {
            return self.lower_expr(&x, expected);
        }
        if let ExprKind::Deref(inner) = &e.kind {
            if let Some(p) = self.elem_access_place(inner, false, e.line)? {
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                // Loaded now: the element's address need not
                // outlive this expression.
                let v = self.read_place(p, e.line)?;
                return self.into_temp(v, e.line);
            }
        }
        // D-0118: a bit field is read out of the whole backing integer.
        if let ExprKind::Field(base, f) = &e.kind {
            if let Some((w, off, backing, fty)) = self.bitfield_of(base, f)? {
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                let v = self.read_place(pb, e.line)?;
                let v = self.into_temp(v, e.line)?;
                let fc = self.g.ctype(&fty)?;
                let c = format!("(({})((({}).bits >> {}u) & {}))", fc, v.c, off, c_bits_mask(backing, w));
                return Ok(V { c, ty: fty, obj: None });
            }
        }
        let p = self.lower_place(e)?;
        self.read_place(p, e.line)
    }

    // A `while` loop (and the `for` loops desugared to one), with its step.
    fn lower_while(&mut self, c: &Expr, b: &Block, step: Option<&Expr>) -> R<V> {
        if self.g.boot(2) {
            // D-0274: no proof that the step cannot overflow.
        } else if let Some(inc) = loop_increment(c, b, step.as_deref(), |x| self.lookup(x).map_or(false, |b| b.root.is_empty() && matches!(b.kind, BindKind::Local) && matches!(b.ty, Type::Int(_)))) {
            self.no_overflow.insert(inc);
        }
        // A `for` loop's `continue` goes to its step (a C `continue`
        // would skip it), through a label placed before it.
        // A `foreach` over a vector whose holder the body confines
        // (`foreach_alias`): confined before the condition too, its
        // `$each_len` a plain read -- the holder holds the vector
        // from before the first step to after the last.
        if let Some(first) = b.stmts.first() {
            let saved = self.out.len();
            let _ = self.foreach_alias(first, b.stmts.get(1))?;
            self.out.truncate(saved);
        }
        let cont = step.as_ref().map(|_| self.g.fresh("cont"));
        self.loops.push((self.cleanup.len(), cont.clone()));
        self.loop_broken.push(false);
        self.emit("for (;;) {");
        self.ind += 1;
        // The condition is evaluated in a scope of its own each
        // iteration, so what it forms ends with it.
        self.push_stmt();
        let cv = self.lower_expr(c, Some(&Type::Bool))?;
        let ct = self.g.fresh("c");
        self.emit(&format!("uint8_t {} = {};", ct, cv.c));
        self.pop_stmt();
        self.emit(&format!("if (!{}) break;", ct));
        self.lower_block(b, None)?;
        if let (Some(st), Some(label)) = (step, &cont) {
            self.emit(&format!("{}: ;", label));
            self.at(st.line);
            self.push_stmt();
            self.lower_expr(st, None)?;
            self.pop_stmt();
        }
        self.ind -= 1;
        self.emit("}");
        self.loops.pop();
        let broken = self.loop_broken.pop().unwrap_or(true);
        // D-0175: nothing follows a `while (true)` no `break` leaves.
        if matches!(c.kind, ExprKind::BoolLit(true)) && !broken {
            return Ok(never());
        }
        Ok(unit())
    }

    // A path as a value: a binding, a constant, a variant or a function item.
    // D-0258: whether a borrow's operand names a static or an addressed
    // constant (`K()`, the use of a constant whose address is taken).
    // D-0194's quiet write with no path (`cb_write_quiet(0, …)`), which
    // destroys the old value only where it owns something. An enum's
    // variants with no payload, or a plain one, own nothing (`owns`
    // reads the tag), so for those the call is skipped inline: a node's
    // `n.left = Some(…)` over a `None` makes no runtime call. An owner
    // enum owns in every variant and is always asked.
    pub(super) fn quiet_write_pathless(&self, c: &str, ty: &Type, tid: u32, loc: &str) -> String {
        let call = format!("cb_write_quiet(0, NULL, 0, (void *)&({}), {}u, {});", c, tid, loc);
        let Type::Named(n, args) = ty else { return call };
        let Some(ed) = self.g.enum_decl(n) else { return call };
        if coby::typecheck::type_is_owner(&self.g.items, n) {
            return call;
        }
        let sub = subst_of(&ed.type_params, args);
        let idle: Vec<usize> = ed.variants.iter().enumerate().filter(|(_, v)| v.payload.as_ref().map_or(true, |p| !self.g.is_resource(&apply(p, &sub)))).map(|(i, _)| i).collect();
        if idle.is_empty() || idle.len() == ed.variants.len() {
            return call;
        }
        let test: Vec<String> = idle.iter().map(|i| format!("!({})", self.g.tag_is(ty, c, *i))).collect();
        format!("if ({}) {}", test.join(" && "), call)
    }

    pub(super) fn is_global(&self, inner: &Expr) -> bool {
        match &strip_parens(inner).kind {
            ExprKind::Path(segs, _) => self.g.items.statics.contains_key(&segs.join("::")),
            ExprKind::Call(callee, args) => matches!(&callee.kind, ExprKind::Path(segs, _) if args.is_empty() && self.g.items.const_addrs.lock().unwrap().contains(&segs.join("::"))),
            _ => false,
        }
    }

    // D-0258: the operand of a borrow that names a static or an addressed
    // constant: the global's address with token 0, a shared reference.
    pub(super) fn global_borrow(&mut self, inner: &Expr) -> R<Option<V>> {
        if let ExprKind::Path(segs, _) = &strip_parens(inner).kind {
            let key = segs.join("::");
            if self.g.items.statics.contains_key(&key) {
                let (c, ty) = self.g.static_ref(&key)?;
                return Ok(Some(V { c, ty: Type::Ref(Box::new(ty), Mode::Shared), obj: None }));
            }
        }
        if let ExprKind::Call(callee, args) = &strip_parens(inner).kind {
            if let (ExprKind::Path(segs, _), true) = (&callee.kind, args.is_empty()) {
                let key = segs.join("::");
                if self.g.items.const_addrs.lock().unwrap().contains(&key) {
                    let (c, ty) = self.g.const_ref(&key)?;
                    return Ok(Some(V { c, ty: Type::Ref(Box::new(ty), Mode::Shared), obj: None }));
                }
            }
        }
        Ok(None)
    }

    fn lower_path(&mut self, e: &Expr, segs: &[String], targs: &[Type], expected: Option<&Type>) -> R<V> {
        // A direct reference parameter handed on whole: the
        // reference the call was given, its path unchanged.
        if let Some(b) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.lookup(n)).filter(|b| self.direct_refs.contains(&b.c) && matches!(b.ty, Type::Ref(..))) {
            let pc = b.c.strip_prefix("l_").unwrap_or(&b.c).to_string();
            return Ok(V { c: format!("((cb_ref){{ {}, ({}).tok }})", b.c, pc), ty: b.ty, obj: None });
        }
        if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
            let p = self.lower_place(e)?;
            return self.read_place(p, e.line);
        }
        // A `foreach` element aliased for a reader loop
        // (`foreach_alias`), used whole: the element's address with
        // token 0, every check on it proven for the loop's step.
        if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
            let p = self.lower_place(&alias)?;
            if matches!(p.ty, Type::Never) {
                return Ok(never());
            }
            return Ok(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", p.c), ty: Type::Ref(Box::new(p.ty), Mode::Shared), obj: None });
        }
        if let Some((ename, vi)) = self.variant_of(segs) {
            return self.construct_variant(&ename, vi, targs, None, expected, e.line);
        }
        let name = segs.join("::");
        if let Some(f) = self.g.items.fns.get(&name).cloned() {
            // `[T-Item]`: a fn item as a value — an interned callable box.
            // A generic item is instantiated from its explicit type
            // arguments, else from the expected `fn` type.
            let mut m: HashMap<String, Type> = HashMap::new();
            for (tp, t) in f.type_params.iter().zip(targs.iter()) {
                m.insert(tp.clone(), self.sub(t));
            }
            if let Some(Type::Fn(ps, r)) = expected {
                for (p, a) in f.params.iter().zip(ps.iter()) {
                    unify(&p.ty, a, &f.type_params, &mut m);
                }
                unify(&f.ret, r, &f.type_params, &mut m);
            }
            let Some(inst) = f.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<Vec<Type>>>() else {
                // D-0093 rejects the no-evidence binding up front;
                // what still reaches here is an argument position
                // whose call this lowering cannot solve — a limit,
                // not an internal error.
                return unsupported(&format!("cannot infer type arguments of `{}` as a value here", name));
            };
            let cname = self.g.request_fn(&name, inst.clone())?;
            let fsub = subst_of(&f.type_params, &inst);
            let fty = Type::Fn(f.params.iter().map(|p| self.sub(&apply(&p.ty, &fsub))).collect(), Box::new(self.sub(&apply(&f.ret, &fsub))));
            // D-0191: a function whose reference parameters are all
            // pure direct (and with no slice, which travels in
            // flight) keeps nothing of its arguments: its handle says
            // so (`cb_fn_pure`), for the `Vec` calls that test it.
            // D-0200: or a reader (`Gen::reader_param`), which a bare
            // address serves as well: it reads and lends to readers.
            let pure = f.params.iter().enumerate().all(|(j, p)| match apply(&p.ty, &fsub) {
                Type::Ref(_, Mode::Shared) if self.g.reader_param(&name, j) => true,
                Type::Ref(..) => self.g.pure_direct(&name, j),
                Type::Slice(..) => false,
                t => self.g.plain_data(&t) && !self.has_refs(&t) && !self.holds_fn(&t),
            });
            let mk = if pure { "cb_fn_item_pure" } else { "cb_fn_item" };
            return Ok(V { c: format!("{}((void *){})", mk, cname), ty: fty, obj: None });
        }
        internal(&format!("unresolved path `{}`", name))
    }

    // An assignment `place = val` (compound ones arrive desugared).
    fn lower_assign(&mut self, e: &Expr, place: &Expr, val: &Expr, expected: Option<&Type>) -> R<V> {
        // `*x = e` for a `foreach (x in &mut v)` element aliased to
        // `(*$c)[$i]` (`foreach_alias`): the write is to that element.
        if let ExprKind::Deref(inner) = &strip_parens(place).kind {
            if let ExprKind::Path(segs, _) = &inner.kind {
                if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
                    let w = std::rc::Rc::new(Expr { kind: ExprKind::Assign(Box::new((*alias).clone()), Box::new(val.clone())), line: e.line, col: e.col });
                    self.alias_kept.push(w.clone());
                    return self.lower_expr(&w, expected);
                }
            }
        }
        // D-0118: a write into a bit field: the whole backing integer
        // is the place written; the value must fit the width.
        if let ExprKind::Field(base, f) = &strip_parens(place).kind {
            if let Some((w, off, backing, fty)) = self.bitfield_of(base, f)? {
                self.place_write = true;
                let pb = self.lower_place(base);
                self.place_write = false;
                let pb = self.auto_deref(pb?)?;
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                let v = self.lower_expr(val, Some(&fty))?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, val.line)?;
                let loc = self.loc(e.line);
                if let Some(b) = &pb.base {
                    self.emit(&format!("cb_write({}, {}, 0u, {});", b, Self::projs_args(&pb.projs), loc));
                }
                let bc = c_int_type(backing);
                let mask = c_bits_mask(backing, w);
                self.emit(&format!("if (({})({}) > {}) cb_fault(\"diag.narrowing-overflow\", {});", bc, v.c, mask, loc));
                self.emit(&format!("({}).bits = (({}).bits & ~({} << {}u)) | ((({})({})) << {}u);", pb.c, pb.c, mask, off, bc, v.c, off));
                return Ok(unit());
            }
        }
        // `v[i] = x` on a `Vec` is `*Vec::index_exclusive(&mut v, i) = x`.
        let synth = self.vec_index_call(place, true)?;
        let place: &Expr = synth.as_deref().unwrap_or(place);
        // A stateless value leaves nothing between the element's
        // borrow and the write for `cb_elem_access` to skip.
        // A confined vector's element (`bind_confined`) has nothing
        // to check at all; the analysis saw that `val` pushes
        // nothing onto it, so the address stays good.
        let confined = match &place.kind {
            ExprKind::Deref(inner) => matches!(&inner.kind, ExprKind::Call(_, a) if a.first().and_then(|a0| self.confined_elem_vec(a0)).is_some()),
            _ => false,
        };
        let fused = match &place.kind {
            ExprKind::Deref(inner) if confined || (!self.g.boot(8) && self.stateless(val)) => self.elem_access_place(inner, true, e.line)?,
            _ => None,
        };
        let p = match fused {
            Some(p) if matches!(p.ty, Type::Never) => return Ok(never()),
            Some(p) => p,
            None => {
                self.place_write = true;
                let p = self.lower_place(place);
                self.place_write = false;
                p?
            }
        };
        // A value built in place that owns nothing observable
        // (`inert_value`): only the target's checks come between it
        // and the store, and if one faults, destroying the value
        // would run nothing; it needs no object. Not into raw
        // memory, where the move also ends what lay there.
        self.objectless = p.raw.is_none() && self.is_res(&p.ty) && self.inert_value(val)?;
        let v = self.lower_expr(val, Some(&p.ty));
        self.objectless = false;
        let v = v?;
        if matches!(v.ty, Type::Never) {
            return Ok(never());
        }
        let v = self.into_temp(v, val.line)?;
        let loc = self.loc(e.line);
        if let Some(base) = &p.base {
            // D-0033 (1): a whole binding whose value moved away or
            // ended (before or during `val`) gets a new object in
            // its own frame, as its first value did.
            if let Some(frame) = self.rebind_frames.get(base).cloned().filter(|_| p.is_binding && p.projs.is_empty()) {
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("{} = cb_rebind({}, &{}, {}u, {}, {});", base, base, p.c, tid, frame, loc));
            }
            // D-0194: a quiet target's live value is destroyed by the
            // write (`cb_write_quiet`), before the store below.
            if self.is_res(&p.ty) && p.raw.is_none() && coby::typecheck::quiet_ty(&self.g.items, &p.ty) {
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("cb_write_quiet({}, {}, (void *)&({}), {}u, {});", base, Self::projs_args(&p.projs), p.c, tid, loc));
                self.store(&p.c, &p.ty, &v)?;
                if !self.inert_c.contains(&v.c) {
                    let o = v.obj.as_ref().expect("resource value has object");
                    self.emit(&format!("cb_absorb({});", o));
                }
                return Ok(unit());
            }
            // D-0049: a resource target is live only if its value owns
            // something now, which the runtime decides unless every
            // value of the type does.
            let target = if !self.is_res(&p.ty) {
                "0u".to_string()
            } else if coby::typecheck::always_owns(&self.g.items, &p.ty) {
                "1u".to_string()
            } else {
                let tid = self.tid(&p.ty)?;
                format!("cb_owns(&({}), {}u)", p.c, tid)
            };
            self.emit(&format!("cb_write({}, {}, {}, {});", base, Self::projs_args(&p.projs), target, loc));
        } else if self.is_res(&p.ty) && p.raw.is_none() && self.g.decided && self.objectless_root(place) {
            // D-0294: a write into an objectless local, whole or a field
            // (`objectless_strings`, decided): the old value, live unless
            // the whole binding was moved away (its flag), is destroyed by
            // the write, a quiet type's in place (`cb_write_quiet`), any
            // other's a fault as a direct parameter's below; the binding
            // holds the new value after.
            let tid = self.tid(&p.ty)?;
            let root_c = {
                let mut e = strip_parens(place);
                while let ExprKind::Field(a, _) = &e.kind {
                    e = strip_parens(a);
                }
                path_name(e).and_then(|n| self.lookup(n)).map(|b| b.c).unwrap_or_default()
            };
            let whole = p.projs.is_empty() && matches!(strip_parens(place).kind, ExprKind::Path(..));
            let flag = self.moved_flag.get(&root_c).cloned();
            if !whole {
                // A field of a moved-away binding is stale (`objectless_use`).
                self.objectless_use(&root_c, e.line);
            }
            let destroy = if coby::typecheck::quiet_ty(&self.g.items, &p.ty) {
                self.quiet_write_pathless(&p.c, &p.ty, tid, &loc)
            } else {
                let target = if coby::typecheck::always_owns(&self.g.items, &p.ty) { "1".to_string() } else { format!("cb_owns(&({}), {}u)", p.c, tid) };
                format!("if ({}) cb_fault(\"diag.overwrite-of-live-resource\", {});", target, loc)
            };
            match (&flag, whole) {
                (Some(fl), true) => {
                    self.emit(&format!("if (!{}) {{ {} }}", fl, destroy));
                    self.emit(&format!("{} = 0;", fl));
                }
                _ => self.emit(&destroy),
            }
        } else if self.is_res(&p.ty) && p.raw.is_none() && self.direct_place(place) && coby::typecheck::quiet_ty(&self.g.items, &p.ty) {
            // D-0194: the same through a direct parameter: no path,
            // and the place always holds its value.
            let tid = self.tid(&p.ty)?;
            let w = self.quiet_write_pathless(&p.c, &p.ty, tid, &loc);
            self.emit(&w);
        } else if self.is_res(&p.ty) && p.raw.is_none() && self.direct_place(place) {
            // A resource field through a direct parameter
            // (`box_rec_uses`): no path to check, but the value's
            // check stays -- overwriting a live resource faults
            // (`[Write-Resource-Overwrite-Rejected]`), as `cb_write`
            // would for the target's value.
            let target = if coby::typecheck::always_owns(&self.g.items, &p.ty) {
                "1".to_string()
            } else {
                let tid = self.tid(&p.ty)?;
                format!("cb_owns(&({}), {}u)", p.c, tid)
            };
            self.emit(&format!("if ({}) cb_fault(\"diag.overwrite-of-live-resource\", {});", target, loc));
        }
        // D-0089: a plain value holding `fn` values that is overwritten
        // ends, as a discarded one does: its closures end with it, and so
        // do the references they hold (`f = move [b]() …` after
        // `f = move [a]() …` lets `a` go). The slot owns its closures:
        // a read copies them, or empties the slot (`cb_fn_read`).
        let old_fns = if !self.is_res(&p.ty) && p.raw.is_none() && self.holds_fn(&p.ty) {
            let c = self.g.ctype(&p.ty)?;
            let t = self.g.fresh("old");
            self.emit(&format!("{} {} = {};", c, t, p.c));
            Some((t, self.tid(&p.ty)?))
        } else {
            None
        };
        self.store(&p.c, &p.ty, &v)?;
        if let Some((t, tid)) = old_fns {
            self.emit(&format!("cb_drop_value(&{}, {}u);", t, tid));
        }
        if self.is_res(&p.ty) && !self.inert_c.contains(&v.c) {
            let o = v.obj.as_ref().expect("resource value has object");
            match &p.raw {
                // `[Rawptr-Move-In]`: the object now lives at the raw address.
                Some(ptr) => self.emit(&format!("cb_raw_move_in({}, {});", o, ptr)),
                None => self.emit(&format!("cb_absorb({});", o)),
            }
        }
        Ok(unit())
    }

    // D-0200: `ref<T, m> r = &x[i]…;` for a `let` `Gen::elem_ref_lets`
    // proved: the checks the index calls make in the borrow's mode -- `x`'s
    // borrow, then at each level the index, the bounds fault, and the
    // element's access (`cb_elem_access`: where an object lies over it) --
    // and `r` bound to the element's address with token 0, every check
    // through it proven (`slot_tok`). `Some(diverged)` when done.
    pub(super) fn lower_elem_ref_let(&mut self, name: &str, cname: &str, declared: Option<&Type>, e: &Expr) -> R<Option<bool>> {
        let ExprKind::Borrow(mode, place) = &strip_parens(e).kind else { return Ok(None) };
        // The steps from the root, outermost last.
        let mut steps: Vec<&Expr> = Vec::new();
        let mut cur: &Expr = place;
        let root = loop {
            match &strip_parens(cur).kind {
                ExprKind::Index(a, _) | ExprKind::Field(a, _) => {
                    steps.push(strip_parens(cur));
                    cur = a;
                }
                ExprKind::Path(..) => break strip_parens(cur),
                _ => return Ok(None),
            }
        };
        steps.reverse();
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        let line = e.line;
        let loc = self.loc(line);
        // The types along the way, before anything is emitted: the root an
        // owned vector, each step an element of one or a field of a struct;
        // anything else is lowered as written.
        let ExprKind::Path(rs, _) = &root.kind else { return Ok(None) };
        let Some(rb) = self.lookup(&rs[0]) else { return Ok(None) };
        if !matches!(rb.kind, BindKind::Local) {
            return Ok(None);
        }
        {
            let mut t = rb.ty.clone();
            for st in &steps {
                t = match (&st.kind, &t) {
                    (ExprKind::Index(..), Type::Named(n, a)) if n == "std::Vec" && a.len() == 1 => a[0].clone(),
                    (ExprKind::Field(_, f), Type::Named(n, a)) => {
                        let Some(sd) = self.g.struct_decl(n.as_str()) else { return Ok(None) };
                        let Some(fd) = sd.fields.iter().find(|fd| &fd.name == f) else { return Ok(None) };
                        apply(&fd.ty, &subst_of(&sd.type_params, a.as_slice()))
                    }
                    _ => return Ok(None),
                };
            }
            if declared.map_or(false, |d| *d != Type::Ref(Box::new(t), mode.clone())) {
                return Ok(None);
            }
        }
        let pr = self.lower_place(root)?;
        if matches!(pr.ty, Type::Never) {
            return Ok(Some(true));
        }
        if pr.raw.is_some() || !pr.projs.is_empty() {
            return internal("an element reference's root that is not a whole local");
        }
        if let Some(b) = &pr.base {
            self.emit(&format!("cb_borrow_check({}, NULL, 0, {}, {});", b, m, loc));
        } else if self.objectless_c.contains(&pr.c) {
            self.objectless_use(&pr.c.clone(), line);
        }
        let mut cur_c = pr.c.clone();
        let mut cur_ty = pr.ty.clone();
        for st in steps {
            match &st.kind {
                ExprKind::Index(_, i) => {
                    let Type::Named(n, a) = &cur_ty else { return internal("an element reference into a non-vector") };
                    if n != "std::Vec" || a.len() != 1 {
                        return internal("an element reference into a non-vector");
                    }
                    let elem = a[0].clone();
                    let vec_c = self.g.ctype(&cur_ty)?;
                    let elem_c = self.g.ctype(&elem)?;
                    let tid = self.tid(&elem)?;
                    let iv = self.lower_expr(i, Some(&Type::Int(IntTy::Usize)))?;
                    if matches!(iv.ty, Type::Never) {
                        return Ok(Some(true));
                    }
                    let iv = self.into_temp(iv, line)?;
                    let (v, el) = (self.g.fresh("v"), self.g.fresh("e"));
                    self.at(line);
                    self.emit(&format!("{} *{} = &({});", vec_c, v, cur_c));
                    self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", iv.c, v));
                    self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, el, v, iv.c));
                    self.emit(&format!("cb_elem_access({}, sizeof(*{}), {}u, {}, {}, {});", el, el, tid, m, (*mode == Mode::Exclusive) as u32, loc));
                    cur_c = format!("(*{})", el);
                    cur_ty = elem;
                }
                ExprKind::Field(_, f) => {
                    let Type::Named(n, a) = &cur_ty else { return internal("a field of a non-struct element") };
                    let Some(sd) = self.g.struct_decl(n.as_str()) else { return internal("a field of a non-struct element") };
                    let Some(fd) = sd.fields.iter().find(|fd| &fd.name == f) else { return internal("an unknown field") };
                    cur_ty = apply(&fd.ty, &subst_of(&sd.type_params, a.as_slice()));
                    cur_c = format!("({}).{}", cur_c, san(f));
                }
                _ => unreachable!(),
            }
        }
        let rty = Type::Ref(Box::new(cur_ty), mode.clone());
        if declared.map_or(false, |d| *d != rty) {
            return internal("an element reference of another type");
        }
        self.emit(&format!("void *{} = (void *)&({});", cname, cur_c));
        self.unstored.insert(cname.to_string(), "0".to_string());
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname.to_string(), ty: rty, root: String::new(), kind: BindKind::Local });
        Ok(Some(false))
    }

    // D-0200: `foreach (c in T::chars(x))` or `T::split_whitespace(x)`
    // (`T` `StringView` or `String`, the prelude's), as the parser writes
    // it (`$e = …; $d = $each_drain($e, 1); $i = 0; while ($i < …) { c =
    // $each_take($d, $i, 1); { body } }`), where the body uses `c` only as
    // a pure view (`view_uses` with `Gen::pure_view`) and names nothing the
    // collection's expression names: the pieces are found in place, one per
    // turn, with no vector, and `c` is a bare view of each (no path, no
    // object), as a pure view parameter is (`bind_param`). The vector's
    // views would hold shared paths on the text for the whole loop; nothing
    // in the body can reach the text to write it (it names nothing that
    // leads there, and any exclusive path already held clashed with the
    // text's borrow, which is still made), so none of those paths could
    // ever be met, and dropping them is unobservable (D-0018).
    pub(super) fn lower_fused_text_loop(&mut self, b: &Block, line: usize) -> R<Option<V>> {
        let scopes = &self.scopes;
        let Some(plan) = self.g.fusion_plan(b, &|n| scopes.iter().rev().any(|m| m.contains_key(n))) else { return Ok(None) };
        let FusionPlan { source, cname, be, on_string, chars, derived } = plan;
        let cname = &cname;
        let ExprKind::Block(body) = &be.kind else { return Ok(None) };
        let args = std::slice::from_ref(source);
        let _ = body;
        let svt = Type::Named("std::StringView".to_string(), vec![]);
        let svc = self.g.ctype(&svt)?;
        self.at(line);
        // The text, evaluated as the call's argument, held for the loop.
        self.push_stmt();
        let (data, len, src) = if on_string {
            let st = Type::Named("std::String".to_string(), vec![]);
            let sc = self.g.ctype(&st)?;
            let a = self.lower_expr(&args[0], Some(&Type::Ref(Box::new(st), Mode::Shared)))?;
            if matches!(a.ty, Type::Never) {
                self.pop_stmt();
                return Ok(Some(a));
            }
            let a = self.into_temp(a, line)?;
            (format!("(({} *)({}).p)->bytes.ptr", sc, a.c), format!("(({} *)({}).p)->bytes.len", sc, a.c), format!("(void *)&(({} *)({}).p)->bytes", sc, a.c))
        } else {
            let a = self.lower_expr(&args[0], Some(&svt))?;
            if matches!(a.ty, Type::Never) {
                self.pop_stmt();
                return Ok(Some(a));
            }
            let a = self.into_temp(a, line)?;
            (format!("({}).bytes.data", a.c), format!("({}).bytes.len", a.c), format!("({}).bytes.src", a.c))
        };
        let (fd, fl, fp, fs) = (self.g.fresh("fd"), self.g.fresh("fl"), self.g.fresh("fp"), self.g.fresh("fs"));
        self.emit(&format!("const uint8_t *{} = {};", fd, data));
        self.emit(&format!("uint64_t {} = {};", fl, len));
        self.emit(&format!("void *{} = {};", fs, src));
        self.emit(&format!("uint64_t {} = 0;", fp));
        self.loops.push((self.cleanup.len(), None));
        self.loop_broken.push(false);
        self.emit("for (;;) {");
        self.ind += 1;
        let start = self.g.fresh("fa");
        if chars {
            self.emit(&format!("if ({} >= {}) break;", fp, fl));
            self.emit(&format!("uint64_t {} = {};", start, fp));
            self.emit(&format!("{}++;", fp));
            self.emit(&format!("while ({p} < {n} && ({d}[{p}] & 0xc0) == 0x80) {p}++;", p = fp, n = fl, d = fd));
        } else {
            self.emit(&format!("while ({p} < {n} && ({d}[{p}] == ' ' || ({d}[{p}] >= 9 && {d}[{p}] <= 13))) {p}++;", p = fp, n = fl, d = fd));
            self.emit(&format!("if ({} >= {}) break;", fp, fl));
            self.emit(&format!("uint64_t {} = {};", start, fp));
            self.emit(&format!("while ({p} < {n} && !({d}[{p}] == ' ' || ({d}[{p}] >= 9 && {d}[{p}] <= 13))) {p}++;", p = fp, n = fl, d = fd));
        }
        // `c`: a bare view of the piece.
        let cc = format!("l_{}_{}", san(cname), self.g.fresh(""));
        self.emit(&format!("{} {};", svc, cc));
        self.emit(&format!("{}.bytes.src = {};", cc, fs));
        self.emit(&format!("{}.bytes.data = (uint8_t *){} + {};", cc, fd, start));
        self.emit(&format!("{}.bytes.len = {} - {};", cc, fp, start));
        self.direct_refs.insert(format!("{}.bytes", cc));
        self.scopes.push(HashMap::new());
        self.scopes.last_mut().unwrap().insert(cname.clone(), Binding { c: cc, ty: svt, root: String::new(), kind: BindKind::Local });
        let saved_derived = std::mem::replace(&mut self.bare_trims, derived.into_iter().collect());
        // The body as the loop's own block lowers it: a statement whose
        // value, if any, is discarded (`lower_stmt`'s `BlockLike`; the
        // expression itself, not a copy: analyses know it by address).
        let v = self.lower_expr(be, None);
        self.bare_trims = saved_derived;
        let v = v?;
        if !matches!(v.ty, Type::Void | Type::Never) {
            if v.obj.is_none() && self.holds_fn(&v.ty) {
                let t = self.into_temp(v, be.line)?;
                let tid = self.tid(&t.ty)?;
                self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
            } else {
                self.emit(&format!("(void)({});", v.c));
            }
        }
        self.scopes.pop();
        self.ind -= 1;
        self.emit("}");
        self.loops.pop();
        self.loop_broken.pop();
        self.pop_stmt();
        Ok(Some(unit()))
    }
}
