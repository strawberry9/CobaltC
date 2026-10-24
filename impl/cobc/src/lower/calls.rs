// Fx: calls, view operations and substrings.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    pub(super) fn lower_call(&mut self, callee: &Expr, args: &[Expr], expected: Option<&Type>, line: usize) -> R<V> {
        let (segs, targs) = match &callee.kind {
            ExprKind::Path(segs, targs) => (segs, targs),
            _ => {
                // A place holding a `fn` value (`(t.run)()`, `(*f)(x)`,
                // `(fs[i])(x)`): called where it is, as a binding holding
                // one is. Read as a value, a box whose closure owns a
                // resource would be a read of a resource.
                let bare = strip_parens(callee);
                if matches!(bare.kind, ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_)) {
                    if let Ok(Type::Fn(..)) = self.probe_place_type(bare) {
                        let p = self.lower_place(bare)?;
                        let loc = self.loc(line);
                        if let Some(base) = &p.base {
                            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                        }
                        let h = self.g.fresh("h");
                        self.emit(&format!("void *{} = {};", h, p.c));
                        if let Some(base) = &p.base {
                            // D-0239: a closure that writes its own captures
                            // is borrowed exclusively through the place.
                            self.emit(&format!("if ({h} && ((cb_fnbox *){h})->closure == 3u) cb_write({}, {}, 0u, {});", base, Self::projs_args(&p.projs), loc, h = h));
                        }
                        if self.shared_fn_read(bare) {
                            // D-0218 `[Read-Fn-Through-Shared]`.
                            self.emit(&format!("cb_fn_shared({}, {});", h, loc));
                        }
                        let fty = p.ty.clone();
                        return self.call_fn_value(h, &fty, args, line);
                    }
                }
                // A callee that is itself an expression: a `fn` value is
                // called through its thunk; a closure temporary gets a path
                // of its own for `[Closure-Call]`'s exclusive `self` borrow,
                // and stays a temporary of this statement.
                let v = self.lower_expr(callee, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let t = self.into_temp(v, line)?;
                return match t.ty.clone() {
                    Type::Fn(..) => {
                        let fty = t.ty.clone();
                        self.call_fn_value(t.c, &fty, args, line)
                    }
                    Type::Closure(id) => {
                        let loc = self.loc(line);
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
                        let p = P { c: t.c, ty: t.ty, base: Some(tok), projs: Vec::new(), is_binding: false, raw: None };
                        self.call_closure_place(id, &p, args, line)
                    }
                    _ => internal("call of a non-callable value"),
                };
            }
        };
        if segs.len() == 1 {
            if self.lookup(&segs[0]).is_some() {
                // The place's type, not the binding's: a name a closure
                // captured by borrow is `*self.f_i`, whose place is the
                // `fn` value or closure the reference reaches.
                let p = self.lower_place(callee)?;
                return match &p.ty.clone() {
                    Type::Closure(id) => self.call_closure_place(*id, &p, args, line),
                    Type::Fn(..) => {
                        let loc = self.loc(line);
                        if let Some(base) = &p.base {
                            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                        }
                        let h = self.g.fresh("h");
                        self.emit(&format!("void *{} = {};", h, p.c));
                        if let Some(base) = &p.base {
                            // D-0239, as above.
                            self.emit(&format!("if ({h} && ((cb_fnbox *){h})->closure == 3u) cb_write({}, {}, 0u, {});", base, Self::projs_args(&p.projs), loc, h = h));
                        }
                        let fty = p.ty.clone();
                        self.call_fn_value(h, &fty, args, line)
                    }
                    _ => internal("call of a non-callable binding"),
                };
            }
        }
        let name = segs.join("::");
        if let Some((ename, vi)) = self.variant_of(segs) {
            if !self.g.items.fns.contains_key(&name) && args.len() == 1 {
                return self.construct_variant(&ename, vi, targs, Some(&args[0]), expected, line);
            }
        }
        if let Some(f) = self.g.items.fns.get(&name).cloned() {
            if f.name == "drop" && f.assoc_type.is_some() {
                return internal("direct destructor call");
            }
            return self.lower_user_call(&name, &f, targs, args, expected, line, None);
        }
        if self.g.items.externs.contains_key(&name) {
            if name == "std::stdout_write" || name == "std::stderr_write" {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                let f = if name == "std::stdout_write" { "cb_write_out" } else { "cb_write_err" };
                return Ok(V { c: format!("{}({}, {})", f, pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::file_read" || name == "std::file_write" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k % 2 == 1 { Some(Type::Int(IntTy::Usize)) } else { None };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                let f = if name == "std::file_read" { "cb_file_read" } else { "cb_file_write" };
                return Ok(V { c: format!("{}((void *)({}), {}, (void *)({}), {})", f, cs[0], cs[1], cs[2], cs[3]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::file_op" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 2 { None } else { Some(Type::Int(IntTy::Usize)) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_file_op({}, {}, (void *)({}), {})", cs[0], cs[1], cs[2], cs[3]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::fs_op" || name == "std::fs_query" {
                // D-0061: (op, a, an, b-or-buf, bn-or-cap).
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 1 || k == 3 { None } else { Some(Type::Int(IntTy::Usize)) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                let f = if name == "std::fs_op" { "cb_fs_op" } else { "cb_fs_query" };
                return Ok(V { c: format!("{}({}, (void *)({}), {}, (void *)({}), {})", f, cs[0], cs[1], cs[2], cs[3], cs[4]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::task_op" {
                // D-0204: (op, id, out, cap), a failure's text's length back.
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 2 { None } else { Some(Type::Int(IntTy::Usize)) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_task_op({}, {}, (void *)({}), {})", cs[0], cs[1], cs[2], cs[3]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::event_op" {
                // D-0063: (op, id, seen), a count or an id back.
                let mut cs = Vec::new();
                for a in args {
                    let v = self.lower_expr(a, Some(&Type::Int(IntTy::Usize)))?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_event_op({}, {}, {})", cs[0], cs[1], cs[2]), ty: Type::Int(IntTy::Usize), obj: None });
            }
            if name == "std::clock_read" {
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let t = self.into_temp(v, line)?;
                return Ok(V { c: format!("cb_clock_read({})", t.c), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::tz_offset" {
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::I64)))?;
                let t = self.into_temp(v, line)?;
                return Ok(V { c: format!("cb_tz_offset({})", t.c), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::sleep_ns" {
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::U64)))?;
                let t = self.into_temp(v, line)?;
                return Ok(V { c: format!("cb_sleep_ns({})", t.c), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::file_at" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 2 { Type::Int(IntTy::U64) } else { Type::Int(IntTy::Usize) };
                    let v = self.lower_expr(a, Some(&hint))?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_file_at({}, {}, {})", cs[0], cs[1], cs[2]), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::arg_bytes" {
                let i = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let it = self.into_temp(i, line)?;
                let p = self.lower_expr(&args[1], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                return Ok(V { c: format!("cb_arg_bytes({}, {}, {})", it.c, pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::stdin_read" {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                return Ok(V { c: format!("cb_read_in({}, {})", pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            // D-0141..D-0144: primitives private to `std` realized by the
            // runtime under `cb_` names declared in `cbrt.h`; the arguments
            // as the declaration types them.
            let runtime = match name.as_str() {
                "std::path_op" => Some("cb_path_op"),
                "std::proc_op" => Some("cb_proc_op"),
                "std::net_op" => Some("cb_net_op"),
                "std::os_random" => Some("cb_os_random"),
                "std::stdout_flush" => Some("cb_flush_out"),
                "std::stdin_read_line" => Some("cb_read_line_in"),
                _ => None,
            };
            if let Some(cname) = runtime {
                let ext = self.g.items.externs[&name].clone();
                let mut cs = Vec::new();
                for (p, a) in ext.params.iter().zip(args.iter()) {
                    let hint = if matches!(p.ty, Type::Rawptr(_)) { None } else { Some(p.ty.clone()) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    let c = self.into_temp(v, line)?.c;
                    cs.push(if matches!(p.ty, Type::Rawptr(_)) { format!("(void *)({})", c) } else { c });
                }
                let ret = ext.ret.clone();
                return Ok(V { c: format!("{}({})", cname, cs.join(", ")), ty: ret, obj: None });
            }
            return self.lower_extern_call(&name, args, line);
        }
        self.lower_intrinsic(&name, targs, args, expected, line)
    }

    // `access`: as for `lower_vec_index_pre`.
    // D-0053: the `StringView` operation the static pass found at `e`.
    pub(super) fn view_op(&self, e: &Expr) -> Option<coby::interp::ViewOp> {
        self.g.items.view_ops.lock().unwrap().get(&(e as *const Expr as usize)).copied()
    }

    // D-0053: a `StringView` operation, carried out by `std`, as `coby`
    // does: the expression's own operands in order, then the call.
    pub(super) fn lower_view_op(&mut self, e: &Expr, op: coby::interp::ViewOp) -> R<V> {
        use coby::interp::ViewOp;
        let line = e.line;
        let loc = self.loc(line);
        match (op, &e.kind) {
            (ViewOp::SliceString, ExprKind::SliceOf(_, base, lo, hi)) => {
                // `&s` as a borrow forms it (through a reference, its referent).
                // D-0135: a temporary `String` (or part of one), as an
                // argument, is an object of this statement (D-0103's root).
                self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                let p = self.lower_place(base);
                self.temp_root = false;
                let p = p?;
                let p = self.auto_deref(p)?;
                let Some(b) = p.base.clone() else { return internal("a view of a String with no access path") };
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_SHARED, {});", tok, b, Self::projs_args(&p.projs), loc));
                // The whole text, `&s[0..$]` (`String::as_view` and the
                // `String` helpers that hand their text to a `StringView`
                // one): `String::view(&s, 0, len)` made in place. Both ends
                // are character boundaries and in bounds, so of its checks
                // only the borrow of the bytes remains, which this one
                // range borrow makes on the same path.
                let whole = matches!(&lo.kind, ExprKind::IntLit(0, _)) && matches!(&hi.kind, ExprKind::Dollar);
                if whole {
                    let vty = Type::Named("std::StringView".to_string(), vec![]);
                    let vc = self.g.ctype(&vty)?;
                    let tid = self.tid(&vty)?;
                    let (rt, t, o) = (self.g.fresh("tok"), self.g.fresh("t"), self.g.fresh("o"));
                    self.emit(&format!("uint64_t {} = cb_borrow_range({}, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, ({}).bytes.len, CB_SHARED, {});", rt, tok, p.c, loc));
                    self.emit(&format!("{} {};", vc, t));
                    self.emit(&format!("{}.bytes.src = (void *)&(({}).bytes);", t, p.c));
                    self.emit(&format!("cb_store_ref(&{}.bytes.src, {});", t, rt));
                    self.emit(&format!("{}.bytes.data = ({}).bytes.ptr;", t, p.c));
                    self.emit(&format!("{}.bytes.len = ({}).bytes.len;", t, p.c));
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                    return Ok(V { c: t, ty: vty, obj: Some(o) });
                }
                let r = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty.clone()), Mode::Shared), obj: None };
                let r = self.into_temp(r, line)?;
                let len = format!("(({}).bytes.len)", p.c);
                let (l, h) = self.lower_view_bounds(len, lo, hi)?;
                self.call_std_values("std::String::view", &[r, l, h], line)
            }
            (ViewOp::SliceView, ExprKind::SliceOf(_, base, lo, hi)) => {
                let v = self.lower_expr(base, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, line)?;
                let len = format!("(({}).bytes.len)", v.c);
                let (l, h) = self.lower_view_bounds(len, lo, hi)?;
                self.call_std_values("std::StringView::sub", &[v, l, h], line)
            }
            (ViewOp::Eq { view_left, other_is_str, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.lower_expr(a, None)?;
                let va = self.into_temp(va, line)?;
                let vb = self.lower_expr(b, None)?;
                let vb = self.into_temp(vb, line)?;
                let (v, o) = if view_left { (va, vb) } else { (vb, va) };
                let name = if other_is_str { "std::StringView::eq_str" } else { "std::StringView::eq" };
                let r = self.call_std_values(name, &[v, o], line)?;
                Ok(if negate { V { c: format!("((uint8_t)!({}))", r.c), ty: Type::Bool, obj: None } } else { r })
            }
            (ViewOp::Less { swap, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.lower_expr(a, None)?;
                let va = self.into_temp(va, line)?;
                let vb = self.lower_expr(b, None)?;
                let vb = self.into_temp(vb, line)?;
                let (x, y) = if swap { (vb, va) } else { (va, vb) };
                let r = self.call_std_values("std::StringView::less", &[x, y], line)?;
                Ok(if negate { V { c: format!("((uint8_t)!({}))", r.c), ty: Type::Bool, obj: None } } else { r })
            }
            (ViewOp::TextCmp, ExprKind::Binary(op, a, b)) => {
                // Another instantiation of a generic body: not text.
                let lt = if is_place_syntax(a) { self.probe_place_type(a)? } else { self.probe_type(a, None)? };
                if !matches!(&lt, Type::Named(n, _) if n == "std::String") {
                    return self.lower_binary(*op, a, b, None, line);
                }
                let x = self.lower_text_operand(a, line)?;
                let y = self.lower_text_operand(b, line)?;
                let sym = match op { BinOp::Eq => "==", BinOp::Ne => "!=", BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                Ok(V { c: format!("((uint8_t)(cb_bytes_cmp({}, {}) {} 0))", x, y, sym), ty: Type::Bool, obj: None })
            }
            (ViewOp::KeyCmp, ExprKind::Binary(op, a, b)) => {
                // D-0271 `[Cmp-Key]`: a struct or enum key type, compared
                // where it stands by its generated `cb_kc_…`/`cb_eq_…`
                // function. Another instantiation of a generic body: text
                // (`[Cmp-Text]`) or a scalar.
                let lt = if is_place_syntax(a) { self.probe_place_type(a)? } else { self.probe_type(a, None)? };
                if matches!(&lt, Type::Named(n, _) if n == "std::String") {
                    let x = self.lower_text_operand(a, line)?;
                    let y = self.lower_text_operand(b, line)?;
                    let sym = match op { BinOp::Eq => "==", BinOp::Ne => "!=", BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                    return Ok(V { c: format!("((uint8_t)(cb_bytes_cmp({}, {}) {} 0))", x, y, sym), ty: Type::Bool, obj: None });
                }
                if !matches!(&lt, Type::Named(..)) || !self.g.is_key_type(&lt) {
                    return self.lower_binary(*op, a, b, None, line);
                }
                // A key type that is plain data has no object to borrow: it
                // is read as a value, as `[Eq-Struct]` reads it.
                let res = self.is_res(&lt);
                let x = if res { self.lower_borrowed_operand(a, line)?.0 } else { let v = self.lower_expr(a, Some(&lt))?; self.into_temp(v, line)?.c };
                let y = if res { self.lower_borrowed_operand(b, line)?.0 } else { let v = self.lower_expr(b, Some(&lt))?; self.into_temp(v, line)?.c };
                let c = match op {
                    BinOp::Eq | BinOp::Ne => {
                        let e = self.g.eq_expr(&lt, &x, &y)?;
                        if *op == BinOp::Eq { e } else { format!("!{}", e) }
                    }
                    _ => {
                        let f = self.g.key_cmp_fn(&lt)?;
                        let sym = match op { BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                        format!("{}(&({}), &({})) {} 0", f, x, y, sym)
                    }
                };
                Ok(V { c: format!("((uint8_t)({}))", c), ty: Type::Bool, obj: None })
            }
            _ => internal("a StringView operation of an unexpected shape"),
        }
    }

    // D-0108 `[Cmp-Text]`: a `String` comparison operand as the C text
    // `ptr, len` of its bytes. A place is borrowed shared, as `&e` would
    // be; a temporary lives to the statement's end.
    pub(super) fn lower_text_operand(&mut self, e: &Expr, line: usize) -> R<String> {
        let (c, _) = self.lower_borrowed_operand(e, line)?;
        Ok(format!("(const void *)({}).bytes.ptr, ({}).bytes.len", c, c))
    }

    // A comparison operand read where it stands, as a C lvalue with its
    // type (D-0108 for a `String`, D-0271 for a key type holding one): a
    // place is borrowed shared, as `&e` would be; a temporary lives to the
    // statement's end.
    pub(super) fn lower_borrowed_operand(&mut self, e: &Expr, line: usize) -> R<(String, Type)> {
        // CHG-0363: a field of a call's reference result (`Box::get(a).name`)
        // is a place through that reference; CHG-0367: so is an element
        // (`Box::get(p).segs[0]`).
        let mut root = e;
        while let ExprKind::Field(x, _) | ExprKind::Paren(x) | ExprKind::Index(x, _) = &root.kind {
            root = x;
        }
        let through_ref = !std::ptr::eq(root, e) && !is_place_syntax(root) && matches!(self.probe_type(root, None), Ok(Type::Ref(..)));
        let c = if is_place_syntax(e) || through_ref {
            let loc = self.loc(line);
            let p = self.lower_place(e)?;
            let p = self.auto_deref(p)?;
            // A place with no path (through a direct reference, whose
            // accesses need no check: D-0192's predicate parameters) is
            // read as it stands.
            let tok = self.g.fresh("tok");
            match p.base.clone() {
                Some(b) => self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_SHARED, {});", tok, b, Self::projs_args(&p.projs), loc)),
                None if p.raw.is_none() && self.pred_body => self.emit(&format!("uint64_t {} = 0;", tok)),
                // D-0200: an objectless local (`objectless_strings`): its
                // address, token 0, every check on it proven.
                None if p.raw.is_none() && p.is_binding && p.projs.is_empty() && self.objectless_c.contains(&p.c) => {
                    self.objectless_use(&p.c.clone(), line);
                    self.emit(&format!("uint64_t {} = 0;", tok));
                }
                None => return internal("a compared String with no access path"),
            }
            let r = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty.clone()), Mode::Shared), obj: None };
            let r = self.into_temp(r, line)?;
            (format!("(*(({} *)({}).p))", self.g.ctype(&p.ty)?, r.c), p.ty.clone())
        } else {
            let v = self.lower_expr(e, None)?;
            let v = self.into_temp(v, line)?;
            (v.c.clone(), v.ty.clone())
        };
        Ok(c)
    }

    // A view's bounds, with `$` its length.
    pub(super) fn lower_view_bounds(&mut self, len: String, lo: &Expr, hi: &Expr) -> R<(V, V)> {
        let lenv = self.g.fresh("len");
        self.emit(&format!("uint64_t {} = {};", lenv, len));
        self.dollar.push(lenv);
        let l = self.lower_expr(lo, Some(&Type::Int(IntTy::Usize))).and_then(|v| self.into_temp(v, lo.line));
        let h = match &l {
            Ok(_) => Some(self.lower_expr(hi, Some(&Type::Int(IntTy::Usize))).and_then(|v| self.into_temp(v, hi.line))),
            Err(_) => None,
        };
        self.dollar.pop();
        Ok((l?, h.expect("lowered")?))
    }

    // `String::as_view(r)`: its body, `&(*r)[0..$]` (`lower_view_op`'s
    // whole-text case), made here on the argument's path, with no call.
    pub(super) fn lower_as_view(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::String::as_view" || !in_prelude(f) || args.len() != 1 {
            return Ok(None);
        }
        let r = self.lower_expr(&args[0], Some(&f.params[0].ty))?;
        if matches!(r.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let r = self.into_temp(r, line)?;
        let loc = self.loc(line);
        let vty = Type::Named("std::StringView".to_string(), vec![]);
        let vc = self.g.ctype(&vty)?;
        let tid = self.tid(&vty)?;
        let string_c = self.g.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let sc = format!("(*({} *){}.p)", string_c, r.c);
        let (tok, rt, t, o) = (self.g.fresh("tok"), self.g.fresh("tok"), self.g.fresh("t"), self.g.fresh("o"));
        self.emit(&format!("uint64_t {} = cb_borrow({}.tok, NULL, 0, CB_SHARED, {});", tok, r.c, loc));
        self.emit(&format!("uint64_t {} = cb_borrow_range({}, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, ({}).bytes.len, CB_SHARED, {});", rt, tok, sc, loc));
        self.emit(&format!("{} {};", vc, t));
        self.emit(&format!("{}.bytes.src = (void *)&(({}).bytes);", t, sc));
        self.emit(&format!("cb_store_ref(&{}.bytes.src, {});", t, rt));
        self.emit(&format!("{}.bytes.data = ({}).bytes.ptr;", t, sc));
        self.emit(&format!("{}.bytes.len = ({}).bytes.len;", t, sc));
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
        Ok(Some(V { c: t, ty: vty, obj: Some(o) }))
    }

    // `String::as_view(r)` for a pure view parameter (`lower_user_call`):
    // the checks `lower_as_view`'s borrows make, with none minted.
    pub(super) fn lower_as_view_unminted(&mut self, x: &Expr, line: usize) -> R<V> {
        // CHG-0237: `&x.f…` of a frozen parameter (`frozen_root`): the
        // view of the field's text, no check -- the parameter's held path
        // excludes every clashing one.
        if let ExprKind::Borrow(Mode::Shared, place) = &strip_parens(x).kind {
            if self.frozen_root(place) {
                let pl = self.lower_place(place)?;
                if matches!(&pl.ty, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
                    let vty = Type::Named("std::StringView".to_string(), vec![]);
                    let vc = self.g.ctype(&vty)?;
                    let t = self.g.fresh("t");
                    self.emit(&format!("{} {};", vc, t));
                    self.emit(&format!("{}.bytes.src = (void *)&(({}).bytes);", t, pl.c));
                    self.emit(&format!("{}.bytes.data = ({}).bytes.ptr;", t, pl.c));
                    self.emit(&format!("{}.bytes.len = ({}).bytes.len;", t, pl.c));
                    return Ok(V { c: t, ty: vty, obj: None });
                }
            }
        }
        let sref = Type::Ref(Box::new(Type::Named("std::String".to_string(), vec![])), Mode::Shared);
        // `&s` of a local: its root, as the native map operations' and
        // `append`'s passthroughs hand it (`native_map_passthrough`); the
        // checks below are the borrow's, and no path is minted for it.
        let local = match &strip_parens(x).kind {
            ExprKind::Borrow(Mode::Shared, place) => path_name(place).and_then(|n| self.lookup(n)).filter(|b| {
                matches!(b.kind, BindKind::Local) && !b.root.is_empty() && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && !matches!(b.ty, Type::Ref(..))
            }),
            _ => None,
        };
        let r = match local {
            Some(b) => V { c: format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root), ty: sref.clone(), obj: None },
            None => self.lower_expr(x, Some(&sref))?,
        };
        if matches!(r.ty, Type::Never) {
            return Ok(never());
        }
        let r = self.into_temp(r, line)?;
        let loc = self.loc(line);
        let vty = Type::Named("std::StringView".to_string(), vec![]);
        let vc = self.g.ctype(&vty)?;
        let string_c = self.g.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let sc = format!("(*({} *){}.p)", string_c, r.c);
        let t = self.g.fresh("t");
        self.emit(&format!("cb_borrow_unminted({}.tok, NULL, 0, CB_SHARED, {});", r.c, loc));
        self.emit(&format!("cb_borrow_range_unminted({}.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, ({}).bytes.len, CB_SHARED, {});", r.c, sc, loc));
        self.emit(&format!("{} {};", vc, t));
        self.emit(&format!("{}.bytes.src = (void *)&(({}).bytes);", t, sc));
        self.emit(&format!("{}.bytes.data = ({}).bytes.ptr;", t, sc));
        self.emit(&format!("{}.bytes.len = ({}).bytes.len;", t, sc));
        Ok(V { c: t, ty: vty, obj: None })
    }

    // Every parameter of `f` that is or holds a reference is a shared one:
    // a `StringView`, or a shared reference or slice to plain data
    // (`Gen::direct_refs_of`'s all-shared case).
    // `shared_refish_params`, or an exclusive reference to data that holds
    // no reference and no `fn` (`lower_user_call`'s `as_view` argument).
    pub(super) fn view_safe_params(&self, f: &FnDecl) -> bool {
        f.params.iter().all(|p| match &p.ty {
            Type::Ref(inner, Mode::Exclusive) => self.g.scalars_only(inner),
            _ => self.shared_refish_params(&FnDecl { params: vec![p.clone()], ..f.clone() }),
        })
    }

    pub(super) fn shared_refish_params(&self, f: &FnDecl) -> bool {
        f.params.iter().all(|p| {
            let t = &p.ty;
            if !(self.has_refs(t) || self.g.holds_fn(t)) {
                return true;
            }
            match t {
                Type::Named(n, a) if n == "std::StringView" && a.is_empty() => true,
                Type::Ref(inner, Mode::Shared) | Type::Slice(inner, Mode::Shared) => self.g.plain_data(inner),
                _ => false,
            }
        })
    }

    // A call of a non-generic `std` function with arguments already
    // lowered (as `lower_user_call` ends: in flight, then the call).
    pub(super) fn call_std_values(&mut self, name: &str, args: &[V], line: usize) -> R<V> {
        let f = self.g.items.fns.get(name).cloned().expect("a std function");
        let cname = self.g.request_variant(name, vec![], 0)?;
        let mut ended: Vec<String> = Vec::new();
        for (j, (p, v)) in f.params.iter().zip(args.iter()).enumerate() {
            if self.g.pure_view(name, j) {
                // Kept by the caller for the call (`Gen::pure_view`), and a
                // temporary one ended after it.
                if let Some(o) = &v.obj {
                    ended.push(o.clone());
                }
            } else if self.is_res(&p.ty) {
                self.emit(&format!("cb_send({});", v.obj.as_ref().expect("resource argument has object")));
            } else if self.has_refs(&p.ty) && !matches!(p.ty, Type::Ref(..)) {
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", v.c, tid));
            }
        }
        let call = format!("{}({})", cname, args.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", "));
        let r = self.finish_call(call, f.ret.clone(), line)?;
        for o in ended {
            self.emit(&format!("cb_absorb({});", o));
        }
        Ok(r)
    }

    // D-0049: an argument `ref<τ, exclusive>` (`slice<τ, exclusive>`) for
    // a parameter `ref<τ, shared>` (`slice<τ, shared>`) is passed as
    // `&*e`: a shared borrow through it.
    pub(super) fn weaken_arg(&mut self, t: V, pty: &Type, line: usize) -> R<V> {
        let loc = self.loc(line);
        match (&t.ty, pty) {
            (Type::Ref(inner, Mode::Exclusive), Type::Ref(_, Mode::Shared)) => {
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow(({}).tok, NULL, 0, CB_SHARED, {});", tok, t.c, loc));
                let r = self.g.fresh("r");
                self.emit(&format!("cb_ref {} = ((cb_ref){{ ({}).p, {} }});", r, t.c, tok));
                Ok(V { c: r, ty: Type::Ref(inner.clone(), Mode::Shared), obj: None })
            }
            (Type::Slice(el, Mode::Exclusive), Type::Slice(_, Mode::Shared)) => {
                let sty = Type::Slice(el.clone(), Mode::Shared);
                let c = self.g.ctype(&sty)?;
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow(cb_load_ref(&({}).src), NULL, 0, CB_SHARED, {});", tok, t.c, loc));
                let n = self.g.fresh("sl");
                self.emit(&format!("{} {};", c, n));
                self.emit(&format!("{}.src = ({}).src; cb_store_ref(&{}.src, {});", n, t.c, n, tok));
                self.emit(&format!("{}.data = ({}).data; {}.len = ({}).len;", n, t.c, n, t.c));
                self.register_temp(n, sty, line)
            }
            _ => Ok(t),
        }
    }

    // `String::from_view(String::view(s, lo, hi))`, both the prelude's: the
    // view call's arguments evaluated in order as its call would evaluate
    // them, then `Gen::substring_helper` in place of the two bodies.
    pub(super) fn lower_substring(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if !in_prelude(f) {
            return Ok(None);
        }
        let (dst, a0) = match (name, args) {
            ("std::String::from_view", [a0]) => (None, a0),
            ("std::String::append", [d, a0]) => (Some(d), a0),
            _ => return Ok(None),
        };
        // An objectless destination (D-0196, D-0200) is lent with token 0
        // (its borrow's lowering), which the helper's checks take as proven.
        let ExprKind::Call(callee, inner) = &strip_parens(a0).kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if !targs.is_empty() || inner.len() != 3 {
            return Ok(None);
        }
        // Resolved as `pure_view_in` resolves a callee: a program's own
        // `String::view` (a type of its own named `String`) is not this.
        let k = segs.join("::");
        let iname = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        if iname != "std::String::view" || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) {
            return Ok(None);
        }
        let Some(vf) = self.g.items.fns.get(&iname).cloned() else { return Ok(None) };
        if !in_prelude(&vf) {
            return Ok(None);
        }
        let st = Type::Named("std::String".to_string(), vec![]);
        // `append`'s first argument, evaluated first, as the call would.
        let d = match dst {
            Some(d) => {
                let dref = Type::Ref(Box::new(st.clone()), Mode::Exclusive);
                let dv = self.lower_expr(d, Some(&dref))?;
                if matches!(dv.ty, Type::Never) {
                    return Ok(Some(dv));
                }
                if !matches!(dv.ty, Type::Ref(_, Mode::Exclusive)) {
                    return internal("`String::append`'s first argument is not an exclusive reference");
                }
                Some(self.into_temp(dv, line)?)
            }
            None => None,
        };
        let sref = Type::Ref(Box::new(st.clone()), Mode::Shared);
        let s = self.lower_expr(&inner[0], Some(&sref))?;
        if matches!(s.ty, Type::Never) {
            return Ok(Some(s));
        }
        if !matches!(s.ty, Type::Ref(..)) {
            return internal("`String::view`'s first argument is not a reference");
        }
        let s = self.into_temp(s, line)?;
        let lo = self.lower_expr(&inner[1], Some(&Type::Int(IntTy::Usize)))?;
        if matches!(lo.ty, Type::Never) {
            return Ok(Some(lo));
        }
        let lo = self.into_temp(lo, line)?;
        let hi = self.lower_expr(&inner[2], Some(&Type::Int(IntTy::Usize)))?;
        if matches!(hi.ty, Type::Never) {
            return Ok(Some(hi));
        }
        let hi = self.into_temp(hi, line)?;
        if let Some(d) = d {
            let helper = self.g.append_substring_helper()?;
            self.at(line);
            self.emit(&format!("{}({}, {}, {}, {});", helper, d.c, s.c, lo.c, hi.c));
            return Ok(Some(unit()));
        }
        let helper = self.g.substring_helper()?;
        let sc = self.g.ctype(&st)?;
        self.at(line);
        let t = self.g.fresh("t");
        if self.raw_substring == Some(args.as_ptr() as usize) {
            self.raw_substring = None;
            self.emit(&format!("{} {} = {}_raw({}, {}, {});", sc, t, helper, s.c, lo.c, hi.c));
            self.inert_c.insert(t.clone());
            return Ok(Some(V { c: t, ty: st, obj: None }));
        }
        self.emit(&format!("{} {} = {}({}, {}, {});", sc, t, helper, s.c, lo.c, hi.c));
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_recv(&{});", o, t));
        Ok(Some(V { c: t, ty: st, obj: Some(o) }))
    }
}
