// Fx: if, match and foreach aliases.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // The type a branch expression would produce, found by lowering it
    // into a discarded buffer (bodies are small; emission is cheap).
    // D-0118: `base.f` with `base` a bitstruct (directly or through a
    // reference or guard): the field's width, bit offset, backing and type.
    pub(super) fn bitfield_of(&mut self, base: &Expr, f: &str) -> R<Option<(u32, u32, IntTy, Type)>> {
        let bt = self.probe_type(base, None)?;
        let bt = match bt {
            Type::Ref(t, _) | Type::Guard(t) => *t,
            t => t,
        };
        let Type::Named(n, _) = &bt else { return Ok(None) };
        let Some(sd) = self.g.struct_decl(n.as_str()) else { return Ok(None) };
        if sd.bits.is_none() {
            return Ok(None);
        }
        Ok(bitfield_layout(&sd, f))
    }

    pub(super) fn probe_type(&mut self, e: &Expr, expected: Option<&Type>) -> R<Type> {
        let key = (e as *const Expr as usize, format!("{:?}", expected));
        if let Some(t) = self.probes.get(&key) {
            return Ok(t.clone());
        }
        let t = self.probe_with(|fx| fx.lower_expr(e, expected))?;
        self.probes.insert(key, t.clone());
        Ok(t)
    }

    // `probe_type` of a block (an `if`'s then-branch), cached apart from
    // expressions' probes, whose keys are expression addresses.
    pub(super) fn probe_block_type(&mut self, b: &Block, expected: Option<&Type>) -> R<Type> {
        let key = (b as *const Block as usize, format!("block {:?}", expected));
        if let Some(t) = self.probes.get(&key) {
            return Ok(t.clone());
        }
        let t = self.probe_with(|fx| fx.lower_block(b, expected))?;
        self.probes.insert(key, t.clone());
        Ok(t)
    }

    // The type of the place `e`, found as `probe_type` finds a value's:
    // lowered as a place into a discarded buffer.
    pub(super) fn probe_place_type(&mut self, e: &Expr) -> R<Type> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_uid = self.g.uid;
        let saved_names = self.snapshot_names();
        let saved_nstr = self.g.nstr;
        let saved_strings = self.g.strings.clone();
        let saved_str_names = self.g.str_names.clone();
        let saved_cleanup = self.cleanup.len();
        let saved_loops = self.loops.clone();
        let saved_scopes = self.scopes.len();
        let saved_write = std::mem::replace(&mut self.place_write, false);
        self.g.probing += 1;
        let p = self.lower_place(e);
        self.g.probing -= 1;
        self.place_write = saved_write;
        self.out = saved_out;
        self.g.uid = saved_uid;
        self.restore_names(saved_names);
        self.g.nstr = saved_nstr;
        self.g.strings = saved_strings;
        self.g.str_names = saved_str_names;
        self.cleanup.truncate(saved_cleanup);
        self.loops = saved_loops;
        self.scopes.truncate(saved_scopes);
        Ok(p?.ty)
    }

    // D-0200: the sets keyed by C names that a probe's discarded lowering
    // may add to. A probe restores the name counter (`g.uid`), so the real
    // pass reuses its names: what the probe recorded under them must not
    // carry over (a value marked inert there skipped its object's absorb).
    pub(super) fn snapshot_names(&self) -> NameSnap {
        NameSnap {
            inert_c: self.inert_c.clone(),
            obj_temps: self.obj_temps.clone(),
            objectless_c: self.objectless_c.clone(),
            moved_flag: self.moved_flag.clone(),
            direct_refs: self.direct_refs.clone(),
            confined: self.confined.clone(),
            unstored: self.unstored.clone(),
            held_c: self.held_c.clone(),
            raw_sinks: self.raw_sinks.clone(),
        }
    }

    pub(super) fn restore_names(&mut self, n: NameSnap) {
        self.inert_c = n.inert_c;
        self.obj_temps = n.obj_temps;
        self.objectless_c = n.objectless_c;
        self.moved_flag = n.moved_flag;
        self.direct_refs = n.direct_refs;
        self.confined = n.confined;
        self.unstored = n.unstored;
        self.held_c = n.held_c;
        self.raw_sinks = n.raw_sinks;
    }

    pub(super) fn probe_with(&mut self, lower: impl FnOnce(&mut Self) -> R<V>) -> R<Type> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_uid = self.g.uid;
        let saved_names = self.snapshot_names();
        let saved_nstr = self.g.nstr;
        let saved_strings = self.g.strings.clone();
        let saved_str_names = self.g.str_names.clone();
        let saved_cleanup = self.cleanup.len();
        let saved_loops = self.loops.clone();
        let saved_scopes = self.scopes.len();
        self.g.probing += 1;
        let v = lower(self);
        self.g.probing -= 1;
        self.out = saved_out;
        self.g.uid = saved_uid;
        self.restore_names(saved_names);
        self.g.nstr = saved_nstr;
        self.g.strings = saved_strings;
        self.g.str_names = saved_str_names;
        self.cleanup.truncate(saved_cleanup);
        self.loops = saved_loops;
        self.scopes.truncate(saved_scopes);
        Ok(v?.ty)
    }

    pub(super) fn lower_if(&mut self, c: &Expr, then_b: &Block, else_e: Option<&Expr>, expected: Option<&Type>, line: usize) -> R<V> {
        let cv = self.lower_expr(c, Some(&Type::Bool))?;
        if matches!(cv.ty, Type::Never) {
            return Ok(never());
        }
        let ct = self.into_temp(cv, line)?;
        // D-0049: a literal then-branch takes the else branch's type, and so
        // does a payloadless variant of a generic enum (`{ None }`), which
        // has no type arguments of its own.
        let else_hint = match else_e {
            Some(e) if expected.is_none() && (coby::ast::is_literal_block(then_b) || self.is_untyped_variant_block(then_b)) && !coby::ast::is_literal_branch(e) => {
                Some(self.probe_type(e, None)?).filter(|t| !matches!(t, Type::Never | Type::Void))
            }
            _ => None,
        };
        let expected = expected.or(else_hint.as_ref());
        let then_ty = self.probe_block_type(then_b, expected)?;
        // The else branch's type matters only when the then branch never
        // finishes. Probing it otherwise lowered every `else if` twice, once
        // to probe and once for real, at every level: a chain of n cost 2^n.
        let result_ty = match else_e {
            None => Type::Void,
            Some(e) if matches!(then_ty, Type::Never) => self.probe_type(e, expected)?,
            Some(_) => then_ty.clone(),
        };
        let (sink, ov) = if matches!(result_ty, Type::Void | Type::Never) {
            (None, None)
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            let ov = self.sink_obj_var(&result_ty);
            (Some(r), ov)
        };
        self.emit(&format!("if ({}) {{", ct.c));
        self.ind += 1;
        self.frame_line = Some(line);
        let tv = self.lower_branch_block(then_b, expected, sink.as_deref().map(|s| (s, &result_ty, ov.as_deref())))?;
        self.ind -= 1;
        let mut obj = tv.obj.clone();
        if let Some(e) = else_e {
            self.emit("} else {");
            self.ind += 1;
            let exp = expected.cloned().or_else(|| if matches!(then_ty, Type::Never) { None } else { Some(then_ty.clone()) });
            let ev = self.lower_branch(e, exp.as_ref(), sink.as_deref().map(|s| (s, &result_ty, ov.as_deref())))?;
            self.ind -= 1;
            if obj.is_none() {
                obj = ev.obj.clone();
            }
        }
        self.emit("}");
        Ok(match sink {
            Some(r) => V { c: r, ty: result_ty, obj },
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    pub(super) fn lower_match(&mut self, scrut: &Expr, arms: &[Arm], expected: Option<&Type>, line: usize) -> R<V> {
        let consume_temp = std::mem::replace(&mut self.propagate_scrutinee, false);
        let arm_scopes = std::mem::take(&mut self.stmt_match);
        // D-0058: a first arm that matches everything, on a scrutinee that
        // is not an enum (nor an integer or `bool`): no discriminant; the
        // arm binds the whole value, or (`_`) a place is left alone.
        if arms.first().map_or(false, |a| a.variant.is_none() && a.lit.is_none()) {
            let st = if matches!(strip_parens(scrut).kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_)) {
                self.probe_place_type(scrut)?
            } else {
                self.probe_type(scrut, None)?
            };
            let flat = match &st {
                Type::Named(n, _) => !self.g.items.enums.contains_key(n),
                Type::Ref(inner, _) => !matches!(&**inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)),
                Type::Int(_) | Type::Bool => false,
                _ => true,
            };
            if flat {
                let is_place = matches!(strip_parens(scrut).kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_));
                if arms[0].binder.is_none() && is_place {
                    let v = unit();
                    return self.lower_match_scalar(v, &arms[..1], expected, line, None, arm_scopes);
                }
                let sv = self.lower_expr(scrut, None)?;
                if matches!(sv.ty, Type::Never) {
                    return Ok(never());
                }
                return self.lower_match_scalar(sv, &arms[..1], expected, line, None, arm_scopes);
            }
        }
        // D-0049: a resource binding matched by value stays where it is
        // unless the arm taken moves its payload out; that arm moves it
        // (`cb_take`). The discriminant is read in place (`[Read]`'s checks).
        // Any place scrutinee of a resource enum type is matched in place:
        // an arm can move out of a whole binding only (the checker rejects
        // moving a payload out of a field, an element or `*r`), and an arm
        // that moves nothing leaves any place as it was.
        let mut place_root: Option<String> = None;
        let mut datum_tok: Option<String> = None;
        let mut acc_ptr: Option<String> = None;
        let res_place = match &strip_parens(scrut).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup(&segs[0]).map_or(false, |b| {
                // A borrow capture's place is the captured value.
                let t = match (&b.kind, &b.ty) {
                    (BindKind::CaptureRef(_), Type::Ref(t, _)) => (**t).clone(),
                    (_, t) => t.clone(),
                };
                (matches!(&t, Type::Named(n, _) if self.g.items.enums.contains_key(n)) || is_text_ty(&t)) && self.is_res(&t)
            }),
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                let t = self.probe_place_type(scrut)?;
                (matches!(&t, Type::Named(n, _) if self.g.items.enums.contains_key(n)) || is_text_ty(&t)) && self.is_res(&t)
            }
            _ => false,
        };
        // `match (&mut p)` / `match (&p)` on an enum place with a checked
        // base: the borrow's check (`cb_borrow_unminted`) and the tag's read
        // through the base, with no path minted for the scrutinee; a
        // binder's path is borrowed from the base, through the place's
        // projection and the payload. The scrutinee's own path would be
        // unheld all its life, and no check counts an unheld path (D-0018),
        // as an ancestor or otherwise.
        let mut fused: Option<(Mode, String, Vec<String>)> = None;
        if let (ExprKind::Borrow(bm, place), false) = (&strip_parens(scrut).kind, self.g.bootstrap) {
            let pt = self.probe_place_type(place)?;
            if matches!(&pt, Type::Named(n, _) if self.g.items.enums.contains_key(n)) {
                let p = self.lower_place(place)?;
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                if let (Some(base), None) = (p.base.clone(), &p.raw) {
                    let loc = self.loc(line);
                    let m = if *bm == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
                    self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                    fused = Some((bm.clone(), base, p.projs.clone()));
                    // Lowered once: the place stands for the scrutinee below.
                    self.fused_scrut = Some(V { c: p.c, ty: p.ty, obj: None });
                } else if p.raw.is_none() && self.direct_place(place) {
                    // A field of a direct parameter (`box_rec_uses`): no
                    // check can fail (the parameter is the only way in), so
                    // none is made; the binder's base is "" (direct).
                    fused = Some((bm.clone(), String::new(), p.projs.clone()));
                    self.fused_scrut = Some(V { c: p.c, ty: p.ty, obj: None });
                } else {
                    return internal("a fused match scrutinee was lowered without a base");
                }
            }
        }
        let sv = if let Some(v) = self.fused_scrut.take() {
            v
        } else if res_place {
            let p = self.lower_place(scrut)?;
            let loc = self.loc(line);
            if let Some(base) = p.base.clone() {
                self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                if p.is_binding && p.projs.is_empty() && p.raw.is_none() {
                    place_root = Some(base);
                }
            }
            V { c: p.c, ty: p.ty, obj: None }
        } else if let (ExprKind::Call(_, cargs), true) = (&strip_parens(scrut).kind, !self.g.bootstrap && arms.iter().all(|a| a.nested.is_empty() && a.lit.is_none() && !(a.variant.is_none() && a.binder.is_some()))) {
            // A call returning `Option<ref<T>>`, matched with one-level
            // patterns and no arm binding the whole value (that binder is
            // copied from the temporary's slot, which this leaves empty):
            // the reference's path is received unheld into the
            // statement's scope (`cb_recv_datum_tok`), as a returned
            // `ref<T>` is, rather than held by a temporary object that
            // ends with the statement; a binder holds it as a local does.
            self.datum_tok = Some(cargs.as_ptr() as usize);
            self.datum_tok_out = None;
            if get_mut_acc_arms(scrut, arms) {
                self.map_get_acc = Some(cargs.as_ptr() as usize);
                self.map_get_acc_out = None;
            }
            let v = self.lower_expr(scrut, None);
            self.datum_tok = None;
            self.map_get_acc = None;
            datum_tok = self.datum_tok_out.take();
            acc_ptr = self.map_get_acc_out.take();
            v?
        } else {
            self.lower_expr(scrut, None)?
        };
        if matches!(sv.ty, Type::Never) {
            return Ok(never());
        }
        // `[Match-By-Ref]` (D-0046): a reference scrutinee is matched on its
        // referent, and a binder is a reference of the same mode to the
        // payload, borrowed from the scrutinee's token.
        let mut by_ref: Option<(Mode, String)> = None;
        let mut by_ref_prefix: Vec<String> = Vec::new();
        if let Some((m, base, projs)) = fused.take() {
            by_ref = Some((m, base));
            by_ref_prefix = projs;
        }
        let sv = match sv.ty.clone() {
            Type::Ref(pointee, mode) if matches!(&*pointee, Type::Named(n, _) if self.g.items.enums.contains_key(n)) => {
                let r = self.into_temp(sv, line)?;
                let loc = self.loc(line);
                self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r.c, loc));
                let ec = self.g.ctype(&pointee)?;
                by_ref = Some((mode, format!("({}).tok", r.c)));
                V { c: format!("(*({} *)({}).p)", ec, r.c), ty: (*pointee).clone(), obj: None }
            }
            _ => sv,
        };
        if matches!(sv.ty, Type::Int(_) | Type::Bool) || is_text_ty(&sv.ty) {
            return self.lower_match_scalar(sv, arms, expected, line, place_root, arm_scopes);
        }
        let (ename, args) = match &sv.ty {
            Type::Named(n, a) if self.g.items.enums.contains_key(n) => (n.clone(), a.clone()),
            _ => {
                // `[Match]` on a non-enum: the evaluator's dynamic verdict.
                let loc = self.loc(line);
                self.emit(&format!("cb_fault(\"diag.type-mismatch\", {});", loc));
                return Ok(never());
            }
        };
        let ed = self.g.enum_decl(&ename).expect("enum");
        let sub = subst_of(&ed.type_params, &args);
        let s = if by_ref.is_some() || res_place || datum_tok.is_some() || acc_ptr.is_some() { sv } else { self.into_temp(sv, line)? };
        if let Some(tk) = &datum_tok {
            // The payload slot's token is the received one (`slot_tok`).
            for (i, v) in ed.variants.iter().enumerate() {
                if v.payload.is_some() {
                    self.unstored.insert(format!("{}.u.v{}", s.c, i), tk.clone());
                }
            }
        }
        let scrut_res = by_ref.is_none() && !res_place && self.is_res(&s.ty);
        // The first arm's type fixes the rest.
        let mut arm_expected = expected.cloned();
        let mut result_ty = Type::Never;
        // D-0049: an arm that is only a literal takes its type from the
        // first arm that is not, so that one is probed first.
        let mut order: Vec<&Arm> = arms.iter().collect();
        if expected.is_none() {
            if let Some(k) = arms.iter().position(|a| !coby::ast::is_literal_branch(&a.body)) {
                let a = order.remove(k);
                order.insert(0, a);
            }
        }
        for arm in order {
            // The first arm that finishes fixes the type; probing the rest
            // would lower each nested match twice more at every level.
            if !matches!(result_ty, Type::Never) {
                break;
            }
            let probe_exp = arm_expected.clone();
            let whole_ty = Type::Named(ename.clone(), args.clone());
            let t = self.probe_type_arm(arm, &ed, &sub, &whole_ty, probe_exp.as_ref(), by_ref.as_ref().map(|(m, _)| m.clone()))?;
            if !matches!(t, Type::Never | Type::Void) && arm_expected.is_none() {
                arm_expected = Some(t.clone());
            }
            if matches!(result_ty, Type::Never) && !matches!(t, Type::Never) {
                result_ty = t;
            }
        }
        let sink = if matches!(result_ty, Type::Void | Type::Never) {
            None
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            Some(r)
        };
        let objs_var = if sink.is_some() { self.match_sink_obj_var(&result_ty, arms.as_ptr() as usize) } else { None };
        for (i, arm) in arms.iter().enumerate() {
            // D-0056: the variant index at each level of the arm's pattern,
            // tested in turn down the payload slots; the binder takes the
            // innermost payload.
            let (vis, inner_ty, crosses) = self.arm_levels_crossing(&ed, &sub, arm)?;
            // D-0058: a binder alone binds the whole scrutinee.
            let inner_ty = if vis.is_empty() && arm.lit.is_none() && arm.binder.is_some() { Some(s.ty.clone()) } else { inner_ty };
            let mut slot_path = s.c.clone();
            let mut tests = Vec::new();
            // D-0109: the last reference the pattern looked through (its
            // slot), and the levels below it.
            let mut cross_slot: Option<String> = None;
            let mut below = vis.len();
            let loc = self.loc(line);
            let through = |me: &mut Self, k: usize, slot_path: &mut String, cross_slot: &mut Option<String>, below: &mut usize| -> R<()> {
                if let Some((_, _, pointee)) = crosses.iter().find(|c| c.0 == k) {
                    let ec = me.g.ctype(pointee)?;
                    let raw = slot_path.clone();
                    *slot_path = format!("(*({} *)({{ cb_read(cb_load_ref(&{}), NULL, 0, {}); {}; }}))", ec, raw, loc, raw);
                    *cross_slot = Some(raw);
                    *below = vis.len() - k;
                }
                Ok(())
            };
            for (k, v) in vis.iter().enumerate() {
                through(self, k, &mut slot_path, &mut cross_slot, &mut below)?;
                tests.push(format!("{}.tag == {}u", slot_path, v));
                slot_path = format!("{}.u.v{}", slot_path, v);
            }
            if arm.lit.is_some() {
                through(self, vis.len(), &mut slot_path, &mut cross_slot, &mut below)?;
            }
            // D-0057: a literal the innermost payload must equal; D-0127: a
            // text literal, its bytes.
            if let (Some(l), Some(lt)) = (&arm.lit, inner_ty.as_ref()) {
                if is_text_ty(lt) {
                    let lv = self.lower_expr(l, Some(&Type::Str))?;
                    let (p, n) = text_parts(&slot_path, lt);
                    tests.push(format!("cb_bytes_eq({}, {}, (const void *)({}).p, ({}).n)", p, n, lv.c, lv.c));
                } else {
                    let lv = self.lower_expr(l, Some(lt))?;
                    tests.push(format!("{} == {}", slot_path, lv.c));
                }
            }
            let head = if tests.is_empty() { "{".to_string() } else { format!("if ({}) {{", tests.join(" && ")) };
            self.emit(&format!("{}{}", if i > 0 { "else " } else { "" }, head));
            self.ind += 1;
            self.push_frame();
            self.scopes.push(HashMap::new());
            let moved_out = self.bind_match_arm(arm, &inner_ty, &slot_path, &cross_slot, below, vis.len(), &crosses, &by_ref, &by_ref_prefix, &place_root, &acc_ptr, line)?;
            if scrut_res {
                let o = s.obj.as_ref().expect("resource scrutinee has object");
                // D-0049: consumed only by an arm that moved the payload
                // out; otherwise the temporary ends with its statement.
                if moved_out {
                    self.emit(&format!("cb_end_moved_out({});", o));
                }
            }
            self.branch_scope = arm_scopes && sink.is_none();
            let v = self.lower_branch(&arm.body, arm_expected.as_ref(), sink.as_deref().map(|r| (r, &result_ty, objs_var.as_deref())))?;
            self.scopes.pop();
            if !matches!(v.ty, Type::Never) {
                self.pop_frame();
            } else {
                self.end_diverged();
            }
            self.ind -= 1;
            self.emit("}");
        }
        // D-0060: `?` consumes a temporary `Result`/`Option` that owns
        // nothing: it ends here, not with the statement, so a reference it
        // held no longer counts as a path.
        if consume_temp && by_ref.is_none() && !res_place && !scrut_res {
            if let Some(o) = &s.obj {
                self.emit(&format!("cb_end_moved_out({});", o));
            }
        }
        Ok(match sink {
            Some(r) => {
                // A raw sink's value has no object (`inert_c`).
                if objs_var.as_ref().map_or(false, |o| self.raw_sinks.contains(o)) {
                    self.inert_c.insert(r.clone());
                }
                V { c: r, ty: result_ty, obj: objs_var.filter(|o| !self.raw_sinks.contains(o)) }
            }
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    // A `match` arm's binder (`lower_match`): bound to the payload at
    // `slot_path`, by reference through a crossed or matched reference
    // (D-0046, D-0109), else by value, moving a resource payload out
    // (D-0049; whether it did is the result).
    #[allow(clippy::too_many_arguments)]
    fn bind_match_arm(&mut self, arm: &Arm, inner_ty: &Option<Type>, slot_path: &str, cross_slot: &Option<String>, below: usize, vis_len: usize, crosses: &[(usize, Mode, Type)], by_ref: &Option<(Mode, String)>, by_ref_prefix: &[String], place_root: &Option<String>, acc_ptr: &Option<String>, line: usize) -> R<bool> {
        let mut moved_out = false;
        // D-0109: through a reference the pattern crossed, the binder
        // borrows from that reference (in the weaker mode).
        let ref_bind: Option<(Mode, String, usize)> = match (&cross_slot, &by_ref, crossing_mode(&crosses)) {
            (Some(cs), b, Some(cm)) => Some((if matches!(b, Some((Mode::Shared, _))) { Mode::Shared } else { cm }, self.slot_tok(cs), below)),
            (_, Some((m, tok)), _) => Some((m.clone(), tok.clone(), vis_len)),
            _ => None,
        };
        if let (Some(_), Some(binder), Some((mode, tok, nproj))) = (inner_ty.as_ref(), &arm.binder, &ref_bind) {
            if let Some(pty) = inner_ty.clone() {
                let rty = Type::Ref(Box::new(pty), mode.clone());
                let rc = self.g.ctype(&rty)?;
                let cname = format!("l_{}_{}", san(binder), self.g.fresh(""));
                self.emit(&format!("{} {};", rc, cname));
                let loc = self.loc(line);
                let t = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                // Through a fused scrutinee, the place's projection first.
                let prefix: &[String] = if cross_slot.is_none() { &by_ref_prefix } else { &[] };
                let mut all: Vec<String> = prefix.to_vec();
                all.extend(std::iter::repeat("{ CB_PAYLOAD, 0u }".to_string()).take(*nproj));
                let projs = if all.is_empty() { "NULL".to_string() } else { format!("(cb_proj[]){{ {} }}", all.join(", ")) };
                // `Some(b) : … Box::get_mut(b) …`, `b` used only there,
                // first: `b`'s path would serve only that call's read of
                // the box's pointer. The borrow is checked
                // (`cb_borrow_unminted`) and none minted; the call is
                // made inline through the base (`unminted_binders`).
                let box_use = !self.g.bootstrap && binder_used_once_first(&arm.body, binder) && binder_box_call(&arm.body, binder).map_or(false, |g| g == "std::Box::get" || (g == "std::Box::get_mut" && *mode == Mode::Exclusive));
                if !box_use && tok.is_empty() {
                    return internal("a binder through a direct place used other than by `Box::get`");
                }
                if box_use {
                    if !tok.is_empty() {
                        self.emit(&format!("cb_borrow_unminted({}, {}, {}, {}, {});", tok, projs, all.len(), m, loc));
                    }
                    self.emit(&format!("{} = (void *)&({});", cname, slot_path));
                    self.unminted_binders.insert(cname.clone(), (tok.clone(), all.clone()));
                    self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: rty.clone(), root: String::new(), kind: BindKind::Local });
                    // (`t` unused.)
                    let _ = &t;
                } else {
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {}, {});", t, tok, projs, all.len(), m, loc));
                if !self.g.bootstrap && binder_used_once_first(&arm.body, binder) {
                    // Used once, by the first thing the arm evaluates:
                    // nothing runs between the binding and its use, so
                    // its path need not be held by a binding object
                    // (D-0018: no check counts an unheld path); the
                    // use goes through the borrow's token (`unstored`).
                    self.emit(&format!("{} = (void *)&({});", cname, slot_path));
                    self.unstored.insert(cname.clone(), t.clone());
                    self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: rty.clone(), root: String::new(), kind: BindKind::Local });
                } else {
                    let fv = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", slot_path, t), ty: rty.clone(), obj: None };
                    self.bind_value(binder, cname, &rty, &fv, line)?;
                }
                }
            }
        } else if let (Some(_), Some(binder)) = (inner_ty.as_ref(), &arm.binder) {
            if let Some(pty) = inner_ty.clone() {
                let pc = self.g.ctype(&pty)?;
                let cname = format!("l_{}_{}", san(binder), self.g.fresh(""));
                self.emit(&format!("{} {};", pc, cname));
                let slot = slot_path.to_string();
                let fv = if matches!(pty, Type::Ref(..)) { self.read_ref_slot(&slot, &pty) } else { V { c: slot, ty: pty.clone(), obj: None } };
                if self.is_res(&pty) {
                    // [Relocate-Out] of the payload into the binder; a
                    // binding scrutinee is moved from first (D-0049).
                    let taken = match &place_root {
                        Some(root) => {
                            let loc = self.loc(line);
                            let o = self.g.fresh("o");
                            self.emit(&format!("uint64_t {} = cb_take({}, {});", o, root, loc));
                            Some(o)
                        }
                        None => None,
                    };
                    self.store(&cname, &pty, &fv)?;
                    if let Some(o) = taken {
                        self.emit(&format!("cb_end_moved_out({});", o));
                    }
                    let tid = self.tid(&pty)?;
                    let arm_id = arm as *const Arm as usize;
                    if self.objectless_names.contains(&arm_id) && self.g.objectless_type(&pty) && self.g.probing == 0 {
                        // D-0200: an objectless binder (`objectless_strings`):
                        // the payload's bytes, no object.
                        let saved = self.cur_let.replace(arm_id);
                        self.objectless_bound(&cname, tid);
                        self.cur_let = saved;
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root: String::new(), kind: BindKind::Local });
                        moved_out = true;
                    } else {
                    let loc = self.loc(line);
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                    let root = self.g.fresh("root");
                    self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                    self.note_frame(binder, &root);
                    self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root, kind: BindKind::Local });
                    moved_out = true;
                    }
                } else if let (Some(pv), Type::Ref(..)) = (&acc_ptr, &pty) {
                    // `get_mut`'s `_acc` variant: `*c op= x` alone, the
                    // element's access made (`get_mut_acc_arms`).
                    self.emit(&format!("{} = (void *){};", cname, pv));
                    self.direct_refs.insert(cname.clone());
                    self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root: String::new(), kind: BindKind::Local });
                } else if !self.g.bootstrap && matches!(pty, Type::Ref(..)) && binder_used_once_first(&arm.body, binder) {
                    // The reference stays in the scrutinee's slot, held
                    // there until the statement ends; a binder used once,
                    // by the first thing the arm evaluates, needs no
                    // binding of its own (as `ref_bind`'s, D-0018).
                    let tk = self.g.fresh("tk");
                    self.emit(&format!("uint64_t {} = {};", tk, self.slot_tok(&slot_path)));
                    self.emit(&format!("{} = {};", cname, slot_path));
                    self.unstored.insert(cname.clone(), tk);
                    self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root: String::new(), kind: BindKind::Local });
                } else {
                    self.bind_value(binder, cname, &pty, &fv, line)?;
                }
            }
        }
        Ok(moved_out)
    }

    // A `foreach` over `&v`, `v` a `Vec` of plain, reference-free
    // elements, whose element name `x` the body uses only as `*x` read as
    // a value: `x`'s `let` is not lowered, and each `*x` is the checked
    // read `(*$c)[$i]`. Unobservable: the loop's `$c` already holds `v`
    // shared, so no write to an element can happen while `x` would have
    // lived; `x`'s own shared path, and the element object it would have
    // established, are seen by no check (the argument of
    // `cb_elem_access`, D-0023). One element object and path less per
    // step.
    // `x.f…` read as a value, `x` a `foreach` element aliased to
    // `(*$c)[$i]` (`foreach_alias`): the same fields of that element
    // (`(*x).f…`, the element being plain data). Kept alive with the body,
    // as `probe_type`'s cache needs.
    pub(super) fn alias_field_chain(&mut self, e: &Expr) -> Option<std::rc::Rc<Expr>> {
        fn rebuild(e: &Expr, alias: &HashMap<String, std::rc::Rc<Expr>>) -> Option<Expr> {
            match &e.kind {
                ExprKind::Field(a, f) => Some(Expr { kind: ExprKind::Field(Box::new(rebuild(a, alias)?), f.clone()), line: e.line, col: e.col }),
                ExprKind::Paren(a) => rebuild(a, alias),
                ExprKind::Path(segs, _) if segs.len() == 1 => alias.get(&segs[0]).map(|a| (**a).clone()),
                // `(*x).f…`: `*x` is the element itself.
                ExprKind::Deref(a) => match &strip_parens(a).kind {
                    ExprKind::Path(segs, _) if segs.len() == 1 => alias.get(&segs[0]).map(|a| (**a).clone()),
                    _ => None,
                },
                _ => None,
            }
        }
        if self.elem_alias.is_empty() || !matches!(e.kind, ExprKind::Field(..)) {
            return None;
        }
        let r = std::rc::Rc::new(rebuild(e, &self.elem_alias)?);
        self.alias_kept.push(r.clone());
        Some(r)
    }

    // `p` or `$each_drain(p, n)`, `p` a confined parameter or a holder
    // standing for one: its `confined_params` entry.
    pub(super) fn confined_holder(&self, e: &Expr) -> Option<(String, String, Type)> {
        let name = |e: &Expr| match &e.kind {
            ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(segs[0].clone()),
            _ => None,
        };
        let n = match &e.kind {
            ExprKind::Call(c, a) if name(c).as_deref() == Some("$each_drain") => name(a.first()?)?,
            _ => name(e)?,
        };
        // A confined parameter has no binding; a holder for one has the
        // direct binding `lower_stmt` gave it. Any other binding of the
        // name is not one.
        let entry = self.confined_params.get(&n)?;
        match self.lookup(&n) {
            Some(b) if !self.direct_refs.contains(&b.c) => None,
            _ => Some(entry.clone()),
        }
    }

    // `p = &m x[i]` (`elem_ref_let`), `x` a confined vector
    // (`confined_local`): after the bounds check, `p` is bound as a direct
    // reference to the element (`direct_refs`), its reads and writes
    // unchecked; nothing else reaches `x` while `p` lives. `None` when `x`
    // is not confined (the statement is lowered as usual), else whether
    // the index diverged.
    pub(super) fn bind_elem_ref(&mut self, name: &str, place: &Expr, mode: Mode, line: Option<usize>) -> R<Option<bool>> {
        let write = mode == Mode::Exclusive;
        let Some(synth) = self.vec_index_call(place, write)? else { return Ok(None) };
        let ExprKind::Deref(inner) = &synth.kind else { return Ok(None) };
        let ExprKind::Call(_, a) = &inner.kind else { return Ok(None) };
        if a.first().and_then(|a0| self.confined_local(a0)).is_none() {
            return Ok(None);
        }
        self.push_stmt_at(line);
        let Some(p) = self.elem_access_place(inner, write, line.unwrap_or(place.line))? else { return internal("a confined element with no place") };
        if matches!(p.ty, Type::Never) {
            return Ok(Some(true));
        }
        let l = format!("l_{}_{}", san(name), self.g.fresh(""));
        self.emit(&format!("void *{} = (void *)&({});", l, p.c));
        self.direct_refs.insert(l.clone());
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: l, ty: Type::Ref(Box::new(p.ty), mode), root: String::new(), kind: BindKind::Local });
        self.pop_stmt();
        Ok(Some(false))
    }

    // The `foreach` holder `name`, a reference to a `Vec` of plain,
    // non-zero-sized elements, recorded as confined for its loop
    // (`foreach_alias`).
    // D-0200: `ref<Vec<T>, shared> b = …;` (`T` plain), statement `at` of
    // `blk`: a held shared path to the `Vec` for `b`'s life. While it is
    // held, every exclusive borrow of the `Vec` or an element, every write,
    // move and destruction of it, clashes with it and faults there, so a
    // read through `b` cannot fail: `b` is confined as a `foreach`'s holder
    // is (`confine_holder`: its elements read with the bounds check only).
    // `b` is only read and lent to readers in the rest of its block
    // (`reader_uses`: nothing keeps it), never declared again, and its
    // initializer names no guard (whose drop invalidates the paths through
    // it without a fault there).
    pub(super) fn held_vec_reader(&self, name: &str, init: &Expr, blk: &Block, at: usize) -> bool {
        let Some(Binding { ty: Type::Ref(inner, Mode::Shared), kind: BindKind::Local, .. }) = self.lookup(name) else { return false };
        if !matches!(&*inner, Type::Named(n, a) if n == "std::Vec" && a.len() == 1 && self.g.plain_data(&a[0])) || name.starts_with('$') {
            return false;
        }
        let mut guarded = false;
        visit_expr(init, &mut |e| {
            if let ExprKind::Path(segs, _) = &e.kind {
                if segs.len() == 1 {
                    if let Some(b) = self.lookup(&segs[0]) {
                        if matches!(&b.ty, Type::Guard(_)) || matches!(&b.ty, Type::Ref(t, _) if matches!(**t, Type::Guard(_))) {
                            guarded = true;
                        }
                    }
                }
            }
        });
        if guarded || !held_names_ok(blk, at, name) {
            return false;
        }
        let rest = Block { stmts: blk.stmts[at + 1..].to_vec(), tail: blk.tail.clone() };
        let bound = bound_names(&rest);
        let g = &*self.g;
        reader_uses(&rest, name, false, &|segs: &[String], k: usize| {
            if segs.len() == 1 && bound.contains(&segs[0]) {
                return false;
            }
            g.reads_only(&segs.join("::"), k)
        })
    }

    pub(super) fn confine_holder(&mut self, name: &str) -> R<()> {
        let Some(Binding { c: hc, ty: Type::Ref(inner, _), .. }) = self.lookup(name) else { return Ok(()) };
        let Type::Named(n, a) = &*inner else { return Ok(()) };
        if n != "std::Vec" || a.len() != 1 || !self.g.plain_data(&a[0]) || self.g.size_align(&a[0])?.0 == 0 {
            return Ok(());
        }
        let vec_c = self.g.ctype(&inner)?;
        self.confined_params.insert(name.to_string(), (format!("(*({} *){})", vec_c, hc), "0".to_string(), a[0].clone()));
        Ok(())
    }

    pub(super) fn foreach_alias(&mut self, s: &Stmt, next: Option<&Stmt>) -> R<Option<(String, std::rc::Rc<Expr>)>> {
        if self.g.bootstrap {
            return Ok(None);
        }
        let Stmt::Let { ty: None, name, init: Some(init), .. } = s else { return Ok(None) };
        let ExprKind::Call(callee, args) = &init.kind else { return Ok(None) };
        // `$each_at` over `&c`; `$each_take` over a shared reference or slice
        // written without `&` (which `each.rs` loops over as that borrow).
        let mutable = matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "$each_at_mut");
        if !(mutable || matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && (p[0] == "$each_at" || p[0] == "$each_take"))) || args.len() != 3 {
            return Ok(None);
        }
        let ExprKind::Path(hsegs, _) = &args[0].kind else { return Ok(None) };
        if hsegs.len() != 1 {
            return Ok(None);
        }
        // `foreach (x in v)` over an exclusive reference written without
        // `&` (`v` a `ref<Vec<T>, exclusive>` parameter): `each.rs` loops
        // over it as `&mut *v`, its holder the reference itself, so the
        // loop is `$each_at_mut`'s.
        let mutable = mutable || (matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "$each_take")
            && matches!(self.lookup(&hsegs[0]).map(|b| b.ty), Some(Type::Ref(_, Mode::Exclusive))));
        let Some(Stmt::BlockLike(bx)) = next else { return Ok(None) };
        let ExprKind::Block(body) = &bx.kind else { return Ok(None) };
        // The element type, and whether the holder is a slice (indexed as
        // it is) rather than a reference to a `Vec` or an array (indexed
        // through `*`).
        // A holder standing for a confined parameter (`confined_holder`):
        // a shared `Vec`, its elements reached as the parameter's are.
        let (elem, slice) = match self.lookup(&hsegs[0]).map(|b| b.ty) {
            // `&mut v` (`$each_at_mut`): a `Vec` only, its elements read
            // and assigned whole through `*x` (`block_only_derefs`).
            Some(Type::Ref(inner, Mode::Exclusive)) if mutable => match *inner {
                Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => (a[0].clone(), false),
                _ => return Ok(None),
            },
            Some(Type::Ref(inner, Mode::Shared)) if !mutable => match *inner {
                Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => (a[0].clone(), false),
                Type::Array(t, _) => ((*t).clone(), false),
                _ => return Ok(None),
            },
            Some(Type::Slice(t, Mode::Shared)) => ((*t).clone(), true),
            _ => return Ok(None),
        };
        let (size, _) = self.g.size_align(&elem)?;
        // A shared loop over elements that own something but hold no
        // reference (`String`s), the body handing `x` only to reader
        // parameters (`Gen::reader_param`) or reading plain fields: `x` is
        // the element with token 0 (`lower_expr`'s alias case). The holder
        // `$c` holds the `Vec` shared from the first step to the last, so
        // no exclusive path to an element can exist meanwhile and every
        // shared read of one is proven; a reader keeps nothing of it.
        // D-0200: over an exclusive holder too (`$each_at_mut`), whose
        // element the body only reads: the holder keeps the `Vec`
        // exclusively for the loop, so no other path to an element exists.
        let reader = !slice && size > 0 && !self.g.plain_data(&elem) && !self.has_refs(&elem) && !self.holds_fn(&elem) && {
            let bound = bound_names(body);
            let g = &*self.g;
            reader_uses(body, name, false, &|segs: &[String], at: usize| {
                if segs.len() == 1 && bound.contains(&segs[0]) {
                    return false;
                }
                let k = segs.join("::");
                g.reads_only(&k, at)
            })
        };
        if size == 0 || !(reader || (self.g.plain_data(&elem) && block_only_derefs(body, name, mutable))) {
            return Ok(None);
        }
        let line = init.line;
        // The loop's holder `$c` of a `Vec` read this way is confined for
        // the loop (`confined_params`): it holds the `Vec` shared from
        // before the first step to after the last, so no path can write an
        // element, change the length or end the `Vec` meanwhile -- a write
        // through any other path clashes with `$c` and faults there, never
        // here. Each step's `(*$c)[$i]` and the condition's `$each_len($c)`
        // then need only the bounds check (`confined_local`): no access
        // check per element. `$c`'s own borrow is formed as before.
        if !slice {
            if let Some(Binding { c: hc, ty: Type::Ref(inner, _), .. }) = self.lookup(&hsegs[0]) {
                if matches!(&*inner, Type::Named(n, _) if n == "std::Vec") {
                    let vec_c = self.g.ctype(&inner)?;
                    self.confined_params.insert(hsegs[0].clone(), (format!("(*({} *){})", vec_c, hc), "0".to_string(), elem.clone()));
                }
            }
        }
        let base = if slice { args[0].clone() } else { Expr { kind: ExprKind::Deref(Box::new(args[0].clone())), line, col: init.col } };
        let alias = Expr { kind: ExprKind::Index(Box::new(base), Box::new(args[1].clone())), line, col: init.col };
        Ok(Some((name.clone(), std::rc::Rc::new(alias))))
    }

    // D-0057: `match` on an integer or `bool`: an `if`/`else if` chain of
    // equality tests against the arms' literals, in order; `_` is `else`.
    // `place_root`: the binding a `String` place scrutinee is (D-0127),
    // for a binder arm to move it out of.
    pub(super) fn lower_match_scalar(&mut self, sv: V, arms: &[Arm], expected: Option<&Type>, line: usize, place_root: Option<String>, arm_scopes: bool) -> R<V> {
        // D-0127: a `String` place is compared where it is (a bitwise
        // copy of a resource would need an object to move).
        let in_place = is_text_ty(&sv.ty) && self.is_res(&sv.ty) && sv.obj.is_none();
        let s = if matches!(sv.ty, Type::Void) || in_place { sv } else { self.into_temp(sv, line)? };
        let mut arm_expected = expected.cloned();
        let mut result_ty = Type::Never;
        let mut order: Vec<&Arm> = arms.iter().collect();
        if expected.is_none() {
            if let Some(k) = arms.iter().position(|a| !coby::ast::is_literal_branch(&a.body)) {
                let a = order.remove(k);
                order.insert(0, a);
            }
        }
        for arm in order {
            if !matches!(result_ty, Type::Never) {
                break;
            }
            let probe_exp = arm_expected.clone();
            self.scopes.push(HashMap::new());
            if let (Some(b), None) = (&arm.binder, &arm.lit) {
                self.scopes.last_mut().unwrap().insert(b.clone(), Binding { c: "probe".into(), ty: s.ty.clone(), root: "0".into(), kind: BindKind::Local });
            }
            let t = self.probe_type(&arm.body, probe_exp.as_ref());
            self.scopes.pop();
            let t = t?;
            if !matches!(t, Type::Never | Type::Void) && arm_expected.is_none() {
                arm_expected = Some(t.clone());
            }
            if matches!(result_ty, Type::Never) && !matches!(t, Type::Never) {
                result_ty = t;
            }
        }
        let sink = if matches!(result_ty, Type::Void | Type::Never) {
            None
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            Some(r)
        };
        let objs_var = if sink.is_some() { self.match_sink_obj_var(&result_ty, arms.as_ptr() as usize) } else { None };
        for (i, arm) in arms.iter().enumerate() {
            let head = match &arm.lit {
                // D-0127: a text literal, equal bytes.
                Some(l) if is_text_ty(&s.ty) => {
                    let lv = self.lower_expr(l, Some(&Type::Str))?;
                    let (p, n) = text_parts(&s.c, &s.ty);
                    format!("if (cb_bytes_eq({}, {}, (const void *)({}).p, ({}).n)) {{", p, n, lv.c, lv.c)
                }
                Some(l) => {
                    let lv = self.lower_expr(l, Some(&s.ty))?;
                    format!("if ({} == {}) {{", s.c, lv.c)
                }
                None => "{".to_string(),
            };
            self.emit(&format!("{}{}", if i > 0 { "else " } else { "" }, head));
            self.ind += 1;
            self.push_frame();
            self.scopes.push(HashMap::new());
            if let (Some(b), None) = (&arm.binder, &arm.lit) {
                // D-0058: the whole value (moved, for a resource).
                let bty = s.ty.clone();
                let cname = format!("l_{}_{}", san(b), self.g.fresh(""));
                let c = self.g.ctype(&bty)?;
                self.emit(&format!("{} {};", c, cname));
                if in_place {
                    // D-0127: a `String` binding matched against text
                    // literals, moved out whole into the binder, as the
                    // enum path moves a whole resource scrutinee.
                    let Some(root) = place_root.clone() else { return internal("a `String` scrutinee moved out of a place that is not a binding") };
                    let loc = self.loc(line);
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_take({}, {});", o, root, loc));
                    self.store(&cname, &bty, &s)?;
                    self.emit(&format!("cb_end_moved_out({});", o));
                    let tid = self.tid(&bty)?;
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                    let root = self.g.fresh("root");
                    self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                    self.note_frame(b, &root);
                    self.scopes.last_mut().unwrap().insert(b.clone(), Binding { c: cname, ty: bty, root, kind: BindKind::Local });
                } else {
                    self.bind_value(b, cname, &bty, &s.clone(), line)?;
                }
            }
            self.branch_scope = arm_scopes && sink.is_none();
            let v = self.lower_branch(&arm.body, arm_expected.as_ref(), sink.as_deref().map(|r| (r, &result_ty, objs_var.as_deref())))?;
            self.scopes.pop();
            if !matches!(v.ty, Type::Never) {
                self.pop_frame();
            } else {
                self.end_diverged();
            }
            self.ind -= 1;
            self.emit("}");
        }
        Ok(match sink {
            Some(r) => {
                // A raw sink's value has no object (`inert_c`).
                if objs_var.as_ref().map_or(false, |o| self.raw_sinks.contains(o)) {
                    self.inert_c.insert(r.clone());
                }
                V { c: r, ty: result_ty, obj: objs_var.filter(|o| !self.raw_sinks.contains(o)) }
            }
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    // D-0056: arm's variant index at each level of its pattern (empty for
    // `_`), and the type of the innermost payload it reaches; and where
    // the pattern looks through a reference (D-0109): each crossing's
    // level (`vis.len()` for a literal's), mode and referent type. The
    // payload is then the referent's.
    pub(super) fn arm_levels_crossing(&self, ed: &EnumDecl, sub: &HashMap<String, Type>, arm: &Arm) -> R<(Vec<usize>, Option<Type>, Vec<(usize, Mode, Type)>)> {
        let mut vis = Vec::new();
        let mut crosses = Vec::new();
        let mut decl = ed.clone();
        let mut dsub = sub.clone();
        let mut payload: Option<Type> = None;
        for (k, vn) in arm.chain().iter().enumerate() {
            if k > 0 {
                if let Some(Type::Ref(inner, m)) = payload.clone() {
                    if matches!(&*inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)) {
                        crosses.push((k, m, (*inner).clone()));
                        payload = Some(*inner);
                    }
                }
                match &payload {
                    Some(Type::Named(n, a)) if self.g.items.enums.contains_key(n) => {
                        let d = self.g.enum_decl(n).expect("enum");
                        dsub = subst_of(&d.type_params, a);
                        decl = (*d).clone();
                    }
                    _ => return internal("nested pattern into a payload that is not an enum"),
                }
            }
            let Some(vi) = decl.variants.iter().position(|v| &v.name == vn) else {
                return internal("unknown variant");
            };
            vis.push(vi);
            payload = decl.variants[vi].payload.as_ref().map(|p| apply(p, &dsub));
        }
        if arm.lit.is_some() && !vis.is_empty() {
            if let Some(Type::Ref(inner, m)) = payload.clone() {
                // D-0127: text through a reference too.
                if matches!(&*inner, Type::Int(_) | Type::Bool) || is_text_ty(&inner) {
                    crosses.push((vis.len(), m, (*inner).clone()));
                    payload = Some(*inner);
                }
            }
        }
        Ok((vis, payload, crosses))
    }

    pub(super) fn probe_type_arm(&mut self, arm: &Arm, ed: &EnumDecl, sub: &HashMap<String, Type>, whole_ty: &Type, expected: Option<&Type>, by_ref: Option<Mode>) -> R<Type> {
        self.scopes.push(HashMap::new());
        if let Some(binder) = &arm.binder {
            let whole = if arm.variant.is_none() && arm.lit.is_none() { Some(whole_ty.clone()) } else { None };
            if let Some(w) = whole {
                let pty = match &by_ref {
                    Some(m) => Type::Ref(Box::new(w), m.clone()),
                    None => w,
                };
                self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: "probe".into(), ty: pty, root: "0".into(), kind: BindKind::Local });
            } else if let (_, Some(p), crosses) = self.arm_levels_crossing(ed, sub, arm)? {
                let pty = match (by_ref.clone(), crossing_mode(&crosses)) {
                    (Some(m), None) | (None, Some(m)) => Type::Ref(Box::new(p), m),
                    (Some(a), Some(b)) => Type::Ref(Box::new(p), if a == Mode::Shared { Mode::Shared } else { b }),
                    (None, None) => p,
                };
                self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: "probe".into(), ty: pty, root: "0".into(), kind: BindKind::Local });
            }
        }
        let t = self.probe_type(&arm.body, expected);
        self.scopes.pop();
        t
    }
}
