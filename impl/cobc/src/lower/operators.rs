// Fx: unary and binary operators, branches and raw strings.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    pub(super) fn lower_unary(&mut self, op: UnOp, inner: &Expr, expected: Option<&Type>, line: usize) -> R<V> {
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                if suf.is_none() && matches!(expected, Some(Type::F32 | Type::F64)) && self.g.items.number_literals.lock().unwrap().contains(&(inner as *const Expr as usize)) {
                    let ty = expected.cloned().unwrap();
                    return Ok(V { c: c_float(-(*v as f64), &ty), ty, obj: None });
                }
                let ty = int_lit_type(suf.as_deref(), expected);
                let neg = (*v as i128).wrapping_neg();
                return Ok(V { c: c_int(ty, neg), ty: Type::Int(ty), obj: None });
            }
        }
        let v = self.lower_expr(inner, expected)?;
        if matches!(v.ty, Type::Never) {
            return Ok(never());
        }
        let x = self.into_temp(v.clone(), line)?.c;
        let loc = self.loc(line);
        // D-0254: `-` and `~` lane by lane; a signed lane at its minimum
        // faults as the scalar `-` does.
        if let Type::Simd(lane, n) = &v.ty {
            let ct = self.g.ctype(&v.ty)?;
            let c = match (op, &**lane) {
                (UnOp::Neg, Type::F32 | Type::F64) => format!("(-{})", x),
                (UnOp::Neg, Type::Int(t)) => {
                    let (min, _) = bounds(*t);
                    let uc = c_uint_type(*t);
                    let at_min = super::simd::halve_reduce(&format!("(_a == {})", c_int(*t, min)), uc, *n, (t.bitwidth(64) / 8) as u128, super::simd::Fold::Or, "_any");
                    format!(
                        "({{ {ct} _a = {x}; {{ {uc} _any; {at_min} if (_any) cb_fault(\"diag.arith-overflow\", {loc}); }} ({ct})(-(cbvu_{m})_a); }})",
                        ct = ct, x = x, uc = uc, at_min = at_min, loc = loc, m = mangle(&v.ty)
                    )
                }
                (UnOp::BitNot, Type::Int(_)) => format!("(~{})", x),
                _ => return internal("unary operator on an unexpected simd"),
            };
            return Ok(V { c, ty: v.ty.clone(), obj: None });
        }
        Ok(match (op, &v.ty) {
            (UnOp::Neg, Type::Int(t)) => {
                let (min, _) = bounds(*t);
                let c = format!(
                    "({{ {ct} _a = {x}; if (_a == {min}) cb_fault(\"diag.arith-overflow\", {loc}); ({ct})(-_a); }})",
                    ct = c_int_type(*t),
                    x = x,
                    min = c_int(*t, min),
                    loc = loc
                );
                V { c, ty: v.ty.clone(), obj: None }
            }
            (UnOp::Neg, Type::F32 | Type::F64) => V { c: format!("(-{})", x), ty: v.ty.clone(), obj: None },
            (UnOp::Not, Type::Bool) => V { c: format!("((uint8_t)!{})", x), ty: Type::Bool, obj: None },
            (UnOp::BitNot, Type::Int(t)) => V { c: format!("(({})(~{}))", c_int_type(*t), x), ty: v.ty.clone(), obj: None },
            _ => return internal("unary operator on an unexpected type"),
        })
    }

    pub(super) fn lower_binary(&mut self, op: BinOp, l: &Expr, r: &Expr, expected: Option<&Type>, line: usize) -> R<V> {
        let proven = std::mem::take(&mut self.proven_next);
        if matches!(op, BinOp::And | BinOp::Or) {
            let lv = self.lower_expr(l, Some(&Type::Bool))?;
            if matches!(lv.ty, Type::Never) {
                return Ok(never());
            }
            // The right operand is skipped by `goto`, not wrapped in a C
            // block: a temporary it creates lives to the end of the whole
            // statement (D-0084), so its C storage must too — a block
            // ended it early, and `cb_stmt_pop`'s destruction read a dead
            // stack slot (stack-use-after-scope, found by the sanitizer
            // sweep on conf.hashmap-clear). Skipped declarations are
            // never read: every temporary is written before use, on the
            // one path that reaches it.
            let res = self.g.fresh("t");
            let skip = self.g.fresh("sc");
            self.emit(&format!("uint8_t {} = {};", res, lv.c));
            self.emit(&format!("if ({}{}) goto {};", if op == BinOp::And { "!" } else { "" }, res, skip));
            let rv = self.lower_expr(r, Some(&Type::Bool))?;
            if !matches!(rv.ty, Type::Never) {
                self.emit(&format!("{} = {};", res, rv.c));
            }
            self.emit(&format!("{}:;", skip));
            return Ok(V { c: res, ty: Type::Bool, obj: None });
        }
        let arith = matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor);
        let operand_expected = if arith { expected } else { None };
        // `[Read-Resource-Rejected]`: an operand is a value read; decided
        // before either operand runs, so nothing is moved first.
        // An operator's or a literal's result is never a resource (the
        // checker rejects one), and probing it would lower a long operator
        // chain once per level (a 40 000-term sum was quadratic).
        let plain = matches!(strip_parens(l).kind, ExprKind::Binary(..) | ExprKind::Unary(..) | ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_));
        let lt = if plain { Type::Void } else { self.probe_type(l, operand_expected)? };
        if self.is_res(&lt) {
            let loc = self.loc(line);
            self.emit(&format!("cb_fault(\"diag.read-of-resource\", {});", loc));
            return Ok(never());
        }
        // A literal on the left takes the right operand's type, so the right
        // goes first: a number, or for `==`/`!=` an enum literal whose
        // payload is a literal (`Some(5000000000) == x`). Neither has
        // effects, so the order cannot be observed.
        // A literal expression takes it too (D-0037: `((1 << 40) - 1) & k`).
        // Not for a shift in a context that gives a type: its right side is
        // the amount, and the literal on its left takes the context's type
        // (`x | (1 << n)` with `x : u64` shifts a `u64`, D-0037).
        let left_takes = (coby::ast::is_literal_expr(l) && !(matches!(op, BinOp::Shl | BinOp::Shr) && operand_expected.is_some())) || (matches!(op, BinOp::Eq | BinOp::Ne) && self.is_pure_variant_literal(l));
        let (lv, rv) = if left_takes && !coby::ast::is_literal_expr(r) && !self.is_pure_variant_literal(r) {
            let rv = self.lower_expr(r, operand_expected)?;
            if matches!(rv.ty, Type::Never) {
                return Ok(never());
            }
            let rv = self.into_temp(rv, r.line)?;
            let lv = self.lower_expr(l, Some(&rv.ty))?;
            let lv = self.into_temp(lv, l.line)?;
            (lv, rv)
        } else {
            let lv = self.lower_expr(l, operand_expected)?;
            if matches!(lv.ty, Type::Never) {
                return Ok(never());
            }
            let lv = self.into_temp(lv, l.line)?;
            let rv = self.lower_expr(r, Some(&lv.ty))?;
            if matches!(rv.ty, Type::Never) {
                return Ok(never());
            }
            let rv = self.into_temp(rv, r.line)?;
            (lv, rv)
        };
        let (a, b) = (lv.c.clone(), rv.c.clone());
        let loc = self.loc(line);
        let bool_of = |c: String| V { c: format!("((uint8_t)({}))", c), ty: Type::Bool, obj: None };
        if let Type::Simd(lane, n) = &lv.ty {
            let (lane, n) = ((**lane).clone(), *n);
            return self.simd_binop(op, &lv.ty, &lane, n, &a, &b, &loc);
        }
        Ok(match &lv.ty {
            Type::Int(t) => {
                let ct = c_int_type(*t);
                let (min, _) = bounds(*t);
                let bw = t.bitwidth(64);
                let ut = c_uint_type(*t);
                let same = |c: String| V { c, ty: lv.ty.clone(), obj: None };
                match op {
                    BinOp::Add if proven => same(format!("(({})({} + {}))", ct, a, b)),
                    BinOp::Add | BinOp::Sub | BinOp::Mul => {
                        let builtin = match op {
                            BinOp::Add => "__builtin_add_overflow",
                            BinOp::Sub => "__builtin_sub_overflow",
                            _ => "__builtin_mul_overflow",
                        };
                        same(format!("({{ {ct} _r; if ({builtin}({a}, {b}, &_r)) cb_fault(\"diag.arith-overflow\", {loc}); _r; }})"))
                    }
                    BinOp::Div | BinOp::Rem => {
                        let sym = if op == BinOp::Div { "/" } else { "%" };
                        let ovf = if t.signed() {
                            format!("if (_a == {} && _b == ({})-1) cb_fault(\"diag.div-overflow\", {});", c_int(*t, min), ct, loc)
                        } else {
                            String::new()
                        };
                        same(format!("({{ {ct} _a = {a}, _b = {b}; if (_b == 0) cb_fault(\"diag.div-by-zero\", {loc}); {ovf} ({ct})(_a {sym} _b); }})"))
                    }
                    BinOp::Shl => same(format!(
                        "({{ {ct} _a = {a}; uint64_t _n = (uint64_t)({b}); if (_n >= {bw}u) cb_fault(\"diag.shift-amount-out-of-range\", {loc}); ({ct})((({ut})_a) << _n); }})"
                    )),
                    BinOp::Shr => same(format!(
                        "({{ {ct} _a = {a}; uint64_t _n = (uint64_t)({b}); if (_n >= {bw}u) cb_fault(\"diag.shift-amount-out-of-range\", {loc}); ({ct})(_a >> _n); }})"
                    )),
                    BinOp::BitAnd => same(format!("(({})({} & {}))", ct, a, b)),
                    BinOp::BitOr => same(format!("(({})({} | {}))", ct, a, b)),
                    BinOp::BitXor => same(format!("(({})({} ^ {}))", ct, a, b)),
                    BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                    BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                    BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                    BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                    BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                    BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                    BinOp::And | BinOp::Or => unreachable!(),
                }
            }
            Type::F32 | Type::F64 => {
                let same = |c: String| V { c, ty: lv.ty.clone(), obj: None };
                match op {
                    BinOp::Add => same(format!("({} + {})", a, b)),
                    BinOp::Sub => same(format!("({} - {})", a, b)),
                    BinOp::Mul => same(format!("({} * {})", a, b)),
                    BinOp::Div => same(format!("({} / {})", a, b)),
                    BinOp::Rem => same(format!("fmod({}, {})", a, b)),
                    BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                    BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                    BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                    BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                    BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                    BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                    _ => return internal("bit operator on a float"),
                }
            }
            // D-0108: `false < true`.
            Type::Bool => match op {
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                _ => return internal("operator on bool"),
            },
            // D-0108: `str` in byte order.
            Type::Str => match op {
                BinOp::Eq => bool_of(format!("cb_str_eq({}, {})", a, b)),
                BinOp::Ne => bool_of(format!("!cb_str_eq({}, {})", a, b)),
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    let sym = match op { BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                    bool_of(format!("cb_bytes_cmp(({a}).p, ({a}).n, ({b}).p, ({b}).n) {sym} 0"))
                }
                _ => return internal("operator on str"),
            },
            Type::Rawptr(_) => match op {
                BinOp::Add => V { c: format!("({} + {})", a, b), ty: lv.ty.clone(), obj: None },
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                _ => return internal("operator on rawptr"),
            },
            // `[Eq-Ref]`: reference identity is the address it targets.
            Type::Ref(..) => match op {
                BinOp::Eq => bool_of(format!("{}.p == {}.p", a, b)),
                BinOp::Ne => bool_of(format!("{}.p != {}.p", a, b)),
                _ => return internal("operator on a reference"),
            },
            // `[Eq-Fn]`: two fn values are equal iff they name one item.
            Type::Fn(..) => match op {
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                _ => return internal("operator on a fn value"),
            },
            // `[Eq-Struct]`: structs, arrays and enums compare by value.
            // D-0271 `[Cmp-Key]`: a struct or enum key type is ordered by
            // its generated `cb_kc_…` function, as `Vec::sort` orders it.
            Type::Named(..) | Type::Array(..) => match op {
                BinOp::Eq | BinOp::Ne => {
                    let e = self.g.eq_expr(&lv.ty, &a, &b)?;
                    bool_of(if op == BinOp::Eq { e } else { format!("!{}", e) })
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if matches!(lv.ty, Type::Named(..)) && self.g.is_key_type(&lv.ty) => {
                    let f = self.g.key_cmp_fn(&lv.ty)?;
                    let sym = match op { BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                    bool_of(format!("{}(&({}), &({})) {} 0", f, a, b, sym))
                }
                _ => return internal("operator on an aggregate"),
            },
            // `[Eq-Unit]`: `() == ()` is true; both operands still run.
            Type::Void => match op {
                BinOp::Eq => bool_of(format!("((void)({}), (void)({}), 1)", a, b)),
                BinOp::Ne => bool_of(format!("((void)({}), (void)({}), 0)", a, b)),
                _ => return internal("operator on unit"),
            },
            _ => return unsupported("binary operator on an aggregate"),
        })
    }

    // D-0254: a binary operator on two `simd`s, lane by lane. Integer
    // arithmetic runs in the unsigned twin type (`cbvu_…`), so a wrap is
    // defined in C, and a lane that overflowed faults, tested on the whole
    // vector at once; `*` is checked lane by lane. `==` and `!=` compare the
    // whole value (`[Eq-Struct]`, IEEE equality for float lanes).
    #[allow(clippy::too_many_arguments)]
    fn simd_binop(&mut self, op: BinOp, ty: &Type, lane: &Type, n: u128, a: &str, b: &str, loc: &str) -> R<V> {
        let ct = self.g.ctype(ty)?;
        let m = mangle(ty);
        let same = |c: String| V { c, ty: ty.clone(), obj: None };
        // A fault when any lane of the mask `cond` (a vector expression over
        // `_a`, `_b`, `_r`) is set.
        // (Or-ed by halving, `halve_reduce`, on the lanes' unsigned type.)
        let mask_or = |cond: &str, it: IntTy| -> String {
            let uc = c_uint_type(it);
            format!("{uc} _any; {red}", uc = uc, red = super::simd::halve_reduce(cond, uc, n, (it.bitwidth(64) / 8) as u128, super::simd::Fold::Or, "_any"))
        };
        let any = |cond: &str, it: IntTy| format!("{{ {m} if (_any) cb_fault(\"diag.arith-overflow\", {loc}); }}", m = mask_or(cond, it), loc = loc);
        let Type::Int(it) = lane else {
            return Ok(match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                    let sym = match op { BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", _ => "/" };
                    same(format!("({} {} {})", a, sym, b))
                }
                BinOp::Eq | BinOp::Ne => self.simd_eq(op, &ct, n, a, b),
                _ => return internal("operator on a float simd"),
            });
        };
        let signed = it.signed();
        Ok(match op {
            BinOp::Add | BinOp::Sub => {
                let sym = if op == BinOp::Add { "+" } else { "-" };
                let ovf = match (op, signed) {
                    (BinOp::Add, true) => "((_a ^ _r) & (_b ^ _r)) < 0",
                    (BinOp::Sub, true) => "((_a ^ _b) & (_a ^ _r)) < 0",
                    (BinOp::Add, false) => "_r < _a",
                    _ => "_a < _b",
                };
                same(format!(
                    "({{ {ct} _a = {a}, _b = {b}; {ct} _r = ({ct})((cbvu_{m})_a {sym} (cbvu_{m})_b); {chk} _r; }})",
                    ct = ct, a = a, b = b, m = m, sym = sym, chk = any(ovf, *it)
                ))
            }
            BinOp::Mul => {
                let lc = c_int_type(*it);
                same(format!(
                    "({{ {ct} _a = {a}, _b = {b}, _r; for (int _i = 0; _i < {n}; _i++) {{ {lc} _l; if (__builtin_mul_overflow(_a[_i], _b[_i], &_l)) cb_fault(\"diag.arith-overflow\", {loc}); _r[_i] = _l; }} _r; }})",
                    ct = ct, a = a, b = b, n = n, lc = lc, loc = loc
                ))
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let sym = match op { BinOp::BitAnd => "&", BinOp::BitOr => "|", _ => "^" };
                same(format!("({} {} {})", a, sym, b))
            }
            BinOp::Shl | BinOp::Shr => {
                let bw = it.bitwidth(64);
                let shifted = if op == BinOp::Shl { format!("({ct})((cbvu_{m})_a << (cbvu_{m})_b)", ct = ct, m = m) } else { "(_a >> _b)".to_string() };
                same(format!(
                    "({{ {ct} _a = {a}, _b = {b}; {{ {test} if (_any) cb_fault(\"diag.shift-amount-out-of-range\", {loc}); }} {shifted}; }})",
                    ct = ct, a = a, b = b, loc = loc, shifted = shifted, test = mask_or(&format!("((cbvu_{m})_b >= {bw})", m = m, bw = bw), *it)
                ))
            }
            BinOp::Eq | BinOp::Ne => self.simd_eq(op, &ct, n, a, b),
            _ => return internal("operator on an integer simd"),
        })
    }

    fn simd_eq(&self, op: BinOp, ct: &str, n: u128, a: &str, b: &str) -> V {
        let e = format!("({{ {ct} _a = {a}, _b = {b}; int _e = 1; for (int _i = 0; _i < {n}; _i++) _e &= _a[_i] == _b[_i]; _e; }})", ct = ct, a = a, b = b, n = n);
        V { c: format!("((uint8_t)({}{}))", if op == BinOp::Ne { "!" } else { "" }, e), ty: Type::Bool, obj: None }
    }

    // An expression used as a branch: a Block goes through lower_block
    // with the sink; anything else is lowered and stored.
    // A block as a branch: its own C block, frame and scope. An `if`'s
    // then-branch is lowered here in place: the checker's side tables
    // (view operations, closure types, temporary borrows) are keyed by the
    // address of each expression, so a copy of the block would miss them.
    pub(super) fn lower_branch_block(&mut self, b: &Block, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        let Some((dst, dty, ov)) = sink else { return self.lower_block(b, expected) };
        let frame_line = self.frame_line.take();
        self.emit("{");
        self.ind += 1;
        self.push_frame_at(frame_line);
        let v = self.lower_block_body(b, expected, Some((dst, dty, ov)))?;
        if !matches!(v.ty, Type::Never) {
            self.pop_frame();
        } else {
            self.end_diverged();
        }
        self.ind -= 1;
        self.emit("}");
        Ok(v)
    }

    // D-0200: a `String` made by `e` with no object, where `e`'s maker has a
    // form that makes none: `String::clone`/`from_str` (their `_raw`
    // variants), `String::from_view(String::view(…))` (the substring
    // helper's), or a function with a `__raw` form (`raw_ret_ok`). Its
    // value is inert (`inert_c`). `None`: `e` is none of these.
    pub(super) fn lower_raw_string(&mut self, e: &Expr) -> R<Option<V>> {
        // D-0274: no `_raw` variants in the bootstrap form (no native bodies).
        if self.g.boot(5) {
            return Ok(None);
        }
        let string = Type::Named("std::String".to_string(), vec![]);
        let ExprKind::Call(callee, cargs) = &strip_parens(e).kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if !targs.is_empty() || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) {
            return Ok(None);
        }
        let k = segs.join("::");
        let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
        let Some(f) = self.g.items.fns.get(&k).cloned() else { return Ok(None) };
        match k.as_str() {
            "std::String::clone" | "std::String::from_str" if in_prelude(&f) && cargs.len() == 1 => {
                let a = self.lower_expr(&cargs[0], if k == "std::String::from_str" { Some(&Type::Str) } else { None })?;
                if matches!(a.ty, Type::Never) {
                    return Ok(Some(a));
                }
                let a = self.into_temp(a, e.line)?;
                let pname = self.g.request_fn(&k, Vec::new())?;
                let sc = self.g.ctype(&string)?;
                let t = self.g.fresh("t");
                self.at(e.line);
                self.emit(&format!("{} {} = {}_raw({});", sc, t, pname, a.c));
                self.inert_c.insert(t.clone());
                Ok(Some(V { c: t, ty: string, obj: None }))
            }
            "std::String::from_view" if in_prelude(&f) && cargs.len() == 1 => {
                self.raw_substring = Some(cargs.as_ptr() as usize);
                let v = self.lower_expr(e, Some(&string));
                self.raw_substring = None;
                v.map(Some)
            }
            _ if f.type_params.is_empty() && f.ret == string && self.g.raw_ret_ok(&k) => {
                self.raw_call = Some(self.g.request_fn(&k, Vec::new())?);
                let v = self.lower_expr(e, Some(&string));
                self.raw_call = None;
                v.map(Some)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn lower_branch(&mut self, e: &Expr, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        let own_scope = std::mem::take(&mut self.branch_scope);
        match (&e.kind, sink) {
            (ExprKind::Block(b), Some(sink)) | (ExprKind::Unsafe(b), Some(sink)) => {
                self.frame_line = Some(e.line);
                self.lower_branch_block(b, expected, Some(sink))
            }
            // An expression body gets its own statement scope, as a block
            // tail does: its temporaries live in the C block the caller
            // opened, whose storage the C compiler may reuse once it
            // closes, so they must end before it does. The result's
            // object, if any, is re-stamped to the enclosing statement.
            (_, Some((dst, dty, ov))) => {
                self.push_stmt();
                // A raw sink's `String` made with no object where it can be.
                let raw = if ov.map_or(false, |o| self.raw_sinks.contains(o)) && matches!(dty, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
                    self.lower_raw_string(e)?
                } else {
                    None
                };
                let v = match raw {
                    Some(v) => v,
                    // D-0286: in a decided program, the value of a raw sink
                    // (the function's result) is made with no object: a
                    // literal, a call's result, an objectless local moved.
                    None if self.g.decided && ov.map_or(false, |o| self.raw_sinks.contains(o)) => {
                        self.objectless = self.literal_expr(e);
                        self.arm_inert(e, true);
                        self.raw_return_path = matches!(strip_parens(e).kind, ExprKind::Path(..));
                        let v = self.lower_expr(e, expected);
                        self.objectless = false;
                        self.inert_result = None;
                        self.raw_return_path = false;
                        v?
                    }
                    None => self.lower_expr(e, expected)?,
                };
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                    return Ok(v);
                }
                if matches!(v.ty, Type::Void) {
                    self.emit(&format!("(void)({});", v.c));
                    self.pop_stmt();
                    return Ok(unit());
                }
                let obj = self.assign_sink(dst, dty, v, e.line, ov)?;
                if let Some(o) = &obj {
                    self.emit(&format!("cb_result({});", o));
                }
                self.pop_stmt();
                Ok(V { c: dst.to_string(), ty: dty.clone(), obj })
            }
            // An arm of a `match` that is a whole statement (`stmt_match`):
            // its own scope, which `resolve_scopes` drops if it is unused.
            (_, None) if own_scope => {
                self.push_stmt_at(Some(e.line));
                let v = self.lower_expr(e, expected)?;
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                } else {
                    self.pop_stmt();
                }
                Ok(v)
            }
            (_, None) => self.lower_expr(e, expected),
        }
    }
}
