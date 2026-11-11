// Fx: closures and Box::new.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // `[Closure-Form-Borrow]`/`[Closure-Form-Move]`: the capture struct,
    // then the body as a C function taking `self`.
    pub(super) fn lower_closure(&mut self, is_move: bool, captures: &[String], params: &[Param], declared: Option<&Type>, body: &Block, line: usize) -> R<V> {
        let id = self.g.closure_id();
        let mut fields: Vec<(String, Type)> = Vec::new();
        let mut inits: Vec<V> = Vec::new();
        for c in captures {
            let Some(b) = self.lookup(c) else { return internal(&format!("unbound capture `{}`", c)) };
            let vty = match &b.kind {
                BindKind::CaptureRef(_) => match &b.ty {
                    Type::Ref(t, _) => (**t).clone(),
                    _ => return internal("borrow capture is not a reference"),
                },
                _ => b.ty.clone(),
            };
            let path = Expr { kind: ExprKind::Path(vec![c.clone()], vec![]), line, col: 0 };
            if is_move {
                let path = std::rc::Rc::new(path);
                self.keep(path.clone());
                // D-0315 (step 4): an objectless local moves in with none.
                self.raw_return_path = self.g.decided;
                let v = self.lower_expr(&path, None);
                self.raw_return_path = false;
                let v = v?;
                let v = self.into_temp(v, line)?;
                fields.push((c.clone(), vty));
                inits.push(v);
            } else {
                let mode = if body_writes(body, c) { Mode::Exclusive } else { Mode::Shared };
                let bexpr = std::rc::Rc::new(Expr { kind: ExprKind::Borrow(mode.clone(), Box::new(path)), line, col: 0 });
                self.keep(bexpr.clone());
                let v = self.lower_expr(&bexpr, None)?;
                let v = self.into_temp(v, line)?;
                fields.push((c.clone(), Type::Ref(Box::new(vty), mode)));
                inits.push(v);
            }
        }
        let ptypes: Vec<Type> = params.iter().map(|p| self.sub(&p.ty)).collect();
        let cname = format!("cl_{}", id);
        let writes_self = is_move && captures.iter().any(|c| body_writes(body, c));
        self.g.closures.insert(id, ClosureInfo { fields: fields.clone(), is_move, params: ptypes, ret: Type::Void, cname: cname.clone(), writes_self });
        let ty = Type::Closure(id);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("cl");
        self.emit(&format!("{} {};", c, t));
        for (i, v) in inits.iter().enumerate() {
            let fty = fields[i].1.clone();
            self.store(&format!("{}.f{}", t, i), &fty, v)?;
            // (A value moved in with no object is the closure's own.)
            if self.is_res(&fty) && !(v.obj.is_none() && self.inert_c.contains(&v.c)) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource capture has object")));
            }
        }
        // Always an object: a call borrows it exclusively.
        let tid = self.tid(&ty)?;
        let loc = self.loc(line);
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
        // D-0200: a closure capturing nothing whose reference parameters are
        // all shared and only read through or lent to readers
        // (`reader_uses`), the rest plain data: lowered as a predicate
        // (`pending_pred_closure`), its parameters bare addresses. Whoever
        // calls it formed each borrow against every live path, and nothing
        // the closure does can form an exclusive one meanwhile (it reaches
        // nothing else), so no check through them can fail; with every
        // parameter used directly it is pure (`cb_fn_mark_pure`), and the
        // natives that take a `less` call it with bare element addresses.
        // A closure capturing by reference only what it never writes
        // (`body_writes`) qualifies too: through shared captures it can
        // write nothing, and having no storage of its own it can keep
        // nothing it is given.
        let read_captures = captures.is_empty() || (!is_move && captures.iter().all(|c| !body_writes(body, c)));
        if read_captures && !self.g.pending_pred_closure && !self.g.boot(10) {
            let bound = bound_names(body);
            let g = &*self.g;
            let sub = self.subst.clone();
            let read_only = !params.is_empty()
                && params.iter().all(|p| match apply(&p.ty, &sub) {
                    Type::Ref(_, Mode::Shared) => reader_uses(body, &p.name, false, &|segs: &[String], at: usize| {
                        if segs.len() == 1 && bound.contains(&segs[0]) {
                            return false;
                        }
                        let k = segs.join("::");
                        let k = if g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                        g.reads_only(&k, at)
                    }),
                    t => g.plain_data(&t) && !g.has_refs(&t) && !g.holds_fn(&t),
                });
            if read_only {
                self.g.pending_pred_closure = true;
                self.g.pending_reader_closure = true;
            }
        }
        let ret = self.lower_closure_body(id, params, declared, body)?;
        self.g.closures.get_mut(&id).unwrap().ret = ret;
        Ok(V { c: t, ty, obj: Some(o) })
    }

    // The body as `ret cl_N(cb_ref p_self, params…)`, with each captured
    // name resolving to `*self.f_i` or `self.f_i` ([Closure-Call]).
    pub(super) fn lower_closure_body(&mut self, id: u64, params: &[Param], declared: Option<&Type>, body: &Block) -> R<Type> {
        let info = self.g.closures[&id].clone();
        let pred = std::mem::replace(&mut self.g.pending_pred_closure, false);
        let reader_closure = std::mem::replace(&mut self.g.pending_reader_closure, false);
        let self_ty = Type::Ref(Box::new(Type::Closure(id)), Mode::Exclusive);
        // First pass to learn the result type, then the real one; a
        // written result type (D-0081) needs no first pass.
        let mut ret = declared.map(|t| self.sub(t)).unwrap_or(Type::Void);
        for pass in (if declared.is_some() { 1 } else { 0 })..2 {
            let probing = pass == 0;
            if probing {
                self.g.probing += 1;
            }
            let saved_uid = self.g.uid;
            // Only a probing pass is undone.
            let saved_names = if probing { Some(self.snapshot_names()) } else { None };
            let saved_nstr = self.g.nstr;
            let saved_strs = self.g.str_mark();
            // A closure's own parameters and its captures are its bindings.
            let bound: Vec<String> = params.iter().map(|p| p.name.clone()).chain(info.fields.iter().map(|(n, _)| n.clone())).collect();
            let confined_names = if self.g.boot(4) { HashSet::new() } else { self.g.confined_vecs(body, &bound, &[]) };
            let cl_bound: HashSet<String> = bound_names(body).into_iter().chain(bound.iter().cloned()).collect();
            let pinned = pinned_names(body, &|segs: &[String], j: usize| self.g.pure_direct_ref_arg(segs, j, &cl_bound), &HashSet::new());
            let mut fx = Fx {
                g: &mut *self.g,
                subst: self.subst.clone(),
                scopes: vec![HashMap::new()],
                out: String::new(),
                ret: ret.clone(),
                cleanup: Vec::new(),
                loops: Vec::new(),
                loop_broken: Vec::new(),
                ind: 1,
                self_ref: Some(("p_self".to_string(), id)),
                pinned_ws: pinned.clone(),
                pinned,
                assigned: assigned_names(body),
                rebind_frames: HashMap::new(),
                holder_roots: HashMap::new(),
                no_overflow: HashSet::new(),
                proven_next: false,
                confined_names,
                confined_fields: HashSet::new(),
                read_confined: HashSet::new(),
                objectless_names: HashSet::new(),
                objectless_fields: HashMap::new(),
                cur_let: None,
                raw_ret: false, raw_tail: None, raw_arm: None, raw_arm_last: false, raw_arm_noted: false, noted_pending: HashMap::new(), stack_box: false, stack_box_at: None, raw_sinks: HashSet::new(), raw_substring: None, raw_return_path: false,
                raw_call: None,
                objectless_c: HashSet::new(),
                moved_flag: HashMap::new(),
                bare_trims: HashSet::new(),
                elem_ref_ids: HashSet::new(),
                quiet_ids: HashSet::new(),
                quiet_roots: HashSet::new(),
                drop_lines: Vec::new(),
                confined: HashSet::new(),
                hoisted: HashMap::new(),
                versions: Vec::new(), guard_depth: 0, pending_note: None, const_locals: HashMap::new(),
                mask_params: HashSet::new(),
                self_tail_now: false,
                tail_targets: Vec::new(),
                tail_used: false,
                confined_params: HashMap::new(),
                probes: HashMap::new(),
               
                dollar: Vec::new(),
                place_write: false,
            elem_mode: None,
            fused_scrut: None,
            map_acc: None,
            map_acc_used: false,
            pc_site: None,
            tracked: self.tracked,
            compound_elem: false,
            datum_tok: None,
            datum_tok_out: None,
            map_get_acc: None,
            map_get_acc_out: None,
            temp_root: false,
                frame_line: None,
                vec_index_exprs: HashMap::new(),
                index_sugar: HashSet::new(),
                propagate_scrutinee: false,
                elem_alias: HashMap::new(),
                direct_params: HashSet::new(),
                entry_tested: false,
                direct_refs: HashSet::new(), direct_live: HashMap::new(), fast_half: false, elem_reader: false, frozen_on: false, pred_body: false, field_confined: HashMap::new(),
                pure_slices: HashSet::new(),
                pure_views: HashSet::new(),
                unstored: HashMap::new(),
                unstored_params: HashSet::new(),
                held_locals: HashSet::new(),
                callee_only: HashSet::new(),
                notes_always: false,
                notes_bottom: false,
                s1_let: None,
                s1_now: false,
                pending_sub: false,
                sub_frames: HashSet::new(),
                held_c: HashSet::new(),
                unminted_binders: HashMap::new(),
               
                stmt_match: false,
                branch_scope: false,
                objectless: false, lit_inert: false,
                inert_c: HashSet::new(), names_undo: Vec::new(), name_marks: 0,
                obj_temps: HashSet::new(), inert_result: None, inert_now: None, notes: 0, stmt_ind: HashMap::new(), tl: 0, root_frame: 0,
            };
            let tracked = fx.tracked;
            fx.push_frame_fn(tracked);
            fx.root_frame = fx.g.nscope;
            // A closure that captures nothing never reaches `self`: it is
            // bound as a bare address (`bind_param`'s direct form), and its
            // own parameters may be direct as a function's are.
            if fx.g.boot(10) {
                // D-0274: nothing direct in the bootstrap form.
            } else if !info.fields.is_empty() && !pred {
                // D-0192: a capturing closure may reach storage outside it
                // through its captures, so a parameter of it is direct only
                // if never handed on at all (a function's may be handed to a
                // callee returning plain data, `direct_refs_of`). Each
                // parameter is a borrow its caller formed against every live
                // path, the captures' included, so nothing the body reaches
                // through them overlaps it in a conflicting mode.
                let d = fx.g.direct_refs_of(params, body, &fx.subst)?;
                fx.direct_params = d.into_iter().filter(|n| direct_uses(body, n, &|_, _| false)).collect();
                // A closure that captures by reference reaches each capture
                // through the reference stored in it, checked through that
                // reference's own path: `self` serves only to find them, and
                // the call's exclusive borrow is the one path to the object
                // while it runs. Bound as its address (a `move` closure's
                // captured values keep `self`'s path).
                if !info.is_move {
                    fx.direct_params.insert("self".to_string());
                }
            }
            if !fx.g.boot(10) && (info.fields.is_empty() || pred) {
                fx.direct_params = fx.g.direct_refs_of(params, body, &fx.subst)?;
                fx.pred_body = pred;
                if pred {
                    // A predicate `lower_pred_literal` accepted: every
                    // parameter used only directly, or only read (`pred_ok`).
                    let refs: Vec<String> = params.iter().filter(|p| matches!(apply(&p.ty, &fx.subst), Type::Ref(..))).map(|p| p.name.clone()).collect();
                    fx.direct_params.extend(refs);
                }
                fx.direct_params.insert("self".to_string());
            }
            if !probing && !fx.g.boot(10) {
                // Pure: every reference parameter direct and never handed on,
                // none a slice (which travels in flight), the result plain.
                let all_direct = params.iter().all(|p| match apply(&p.ty, &fx.subst) {
                    Type::Ref(..) => fx.direct_params.contains(&p.name) && (reader_closure || direct_uses(body, &p.name, &|_, _| false)),
                    Type::Slice(..) => false,
                    t => fx.g.plain_data(&t) && !fx.g.has_refs(&t) && !fx.g.holds_fn(&t),
                });
                if all_direct {
                    fx.g.pure_closures.insert(id);
                }
            }
            // D-0315 (step 4): `self` is never written, borrowed or captured
            // by the body: unstored (no object) in a decided program.
            if fx.g.decided {
                fx.unstored_params.insert("self".to_string());
            }
            fx.bind_param(&Param { ty: self_ty.clone(), name: "self".to_string() })?;
            for p in params {
                fx.bind_param(p)?;
            }
            // D-0315 (step 4): a resource result leaves as a function's does.
            if !probing && fx.g.decided && fx.is_res(&ret) {
                fx.raw_ret = true;
            }
            for (i, (name, fty)) in info.fields.iter().enumerate() {
                let kind = if info.is_move { BindKind::CaptureVal(i) } else { BindKind::CaptureRef(i) };
                fx.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: String::new(), ty: fty.clone(), root: String::new(), kind });
            }
            let v = fx.lower_block_body(body, if probing { None } else { Some(&ret) }, None)?;
            if probing {
                ret = if matches!(v.ty, Type::Never) { Type::Void } else { v.ty.clone() };
                self.g.probing -= 1;
                self.g.uid = saved_uid;
                if let Some(n) = saved_names {
                    self.restore_names(n);
                }
                self.g.nstr = saved_nstr;
                self.g.str_restore(saved_strs);
                continue;
            }
            if matches!(v.ty, Type::Never) {
                fx.emit("__builtin_unreachable();");
            } else {
                fx.emit_return(v)?;
            }
            while !fx.cleanup.is_empty() {
                fx.end_diverged();
            }
            let prologue = fx.notes_finish();
            // D-0274: in the bootstrap form no text pass elides anything.
            let out = if fx.g.boot(1) {
                resolve_scopes(&fx.out)
            } else {
                super::functions::dumped_text_passes(&fx.out, self.g.line_directives, self.g.decided, None)
            };
            if self.g.probing == 0 {
                let ret_c = self.g.reg_ctype(&ret)?;
                let mut params_c = vec!["cb_ref p_self".to_string()];
                for p in params {
                    let pty = self.sub(&p.ty);
                    let pc = self.g.param_ctype(&pty)?;
                    params_c.push(format!("{} p_{}", pc, san(&p.name)));
                }
                let sig = format!("{} {}({})", ret_c, info.cname, params_c.join(", "));
                writeln!(self.g.protos, "{};", sig).unwrap();
                // D-0255: over-aligned parameters copied at entry (`overaligned_params`).
                let tys: Vec<(String, Type)> = params.iter().map(|p| (p.name.clone(), self.sub(&p.ty))).collect();
                let (entry, exit) = self.g.overaligned_params(&tys)?;
                // D-0077, as for a function (`emit_fn_body`): a body that
                // calls nothing, its prologue included, cannot go deeper;
                // its caller's check, and the floor's margin, cover its frame.
                let check = if !self.g.boot(1) && super::passes::calls_nothing(&prologue) && super::passes::calls_nothing(&out) { "" } else { "    cb_stack_check();\n" };
                write!(self.g.fns, "{}\n{{\n{}{}{}{}{}}}\n\n", sig, entry, check, prologue, out, exit).unwrap();
            }
        }
        Ok(ret)
    }

    // `Vec::push(&mut v, s)` for a `Vec<String>`, `s` being
    // `String::copy_str(…)` or `String::clone(…)` (so `sprintf`): as
    // `lower_map_fresh_key` passes a fresh key, the new `String` goes to
    // the native push (`native_vec_push`) with no object -- received,
    // then moved into the buffer, where a reference-free object ends at
    // once, with nothing able to see it. `v` a local is passed with its
    // root path after the borrow's check (the native push only checks
    // through it).
    // `Box::new(v)` for a `T` with no reference and no `fn` value in it:
    // `std`'s body inline (`spec/21` §0: its bodies are normative for what
    // they do, not how). The cells `Box::cells<T>()` allocates, the alloc
    // fault, the value stored, and `Box { .ptr, .full = true }`. The
    // body's `*bp = v` moves `v` into raw memory, where a reference-free
    // object ends at once (`cb_raw_move_in`); here `v`'s object, if it has
    // one, ends the same way, and a value built in place never gets one.
    // The `Box` gets its object as any fresh resource value does.
    pub(super) fn lower_box_new(&mut self, name: &str, f: &FnDecl, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize) -> R<Option<V>> {
        if name != "std::Box::new" || !in_prelude(f) || args.len() != 1 {
            return Ok(None);
        }
        let objectless = std::mem::take(&mut self.objectless);
        // D-0315: armed by its consumer (`arm_inert`), by its arguments.
        let armed = self.g.decided && self.g.probing == 0 && self.inert_result.map_or(false, |(a, _)| a == args.as_ptr() as usize);
        if armed {
            self.inert_result = None;
        }
        // `T` as the call would infer it: written, from the `Box<T>`
        // expected (a literal argument takes it, D-0079), or the argument's.
        let t = match (targs.first(), expected) {
            (Some(t), _) => self.sub(t),
            (None, Some(Type::Named(n, a))) if n == "std::Box" && a.len() == 1 => a[0].clone(),
            _ => self.probe_type(&args[0], None)?,
        };
        if self.has_refs(&t) || self.holds_fn(&t) || matches!(t, Type::Closure(_) | Type::Never) {
            return Ok(None);
        }
        self.objectless = self.inert_expr(&args[0])?;
        // D-0294: in a decided program a call's result goes into the box
        // with no object, when its end frees memory alone (the allocation
        // between it and the store can fault, `objectless_type`).
        if self.g.objectless_type(&t) {
            self.arm_inert(&args[0], true);
        }
        // D-0315: and an objectless local moved in, with none.
        self.raw_return_path = self.g.decided && self.g.objectless_type(&t) && matches!(strip_parens(&args[0]).kind, ExprKind::Path(..));
        let v = self.lower_expr(&args[0], Some(&t));
        self.raw_return_path = false;
        self.objectless = false;
        self.inert_result = None;
        let v = v?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let (size, align) = self.g.size_align(&t)?;
        let tc = self.g.ctype(&t)?;
        let bt = Type::Named("std::Box".to_string(), vec![t.clone()]);
        let bc = self.g.ctype(&bt)?;
        self.at(line);
        let p = self.g.fresh("bx");
        self.emit(&format!("uint8_t *{} = cb_allocate({}u, {}u);", p, size.max(1), align));
        self.emit(&format!("if (!{}) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);", p));
        if size > 0 {
            self.emit(&format!("*({} *){} = {};", tc, p, v.c));
        }
        if let Some(o) = &v.obj {
            self.emit(&format!("cb_raw_move_in({}, {});", o, p));
        }
        let b = self.g.fresh("b");
        self.emit(&format!("{} {};", bc, b));
        self.emit(&format!("{}.ptr = ({} *){};", b, tc, p));
        // As a variant's payload (`inert_box_new`) it is absorbed next, with
        // nothing evaluated between: no object.
        // D-0315: and one its consumer armed, whose end frees memory alone.
        if objectless || (armed && self.g.objectless_type(&bt)) {
            self.names_add(NameSet::InertC, b.clone());
            return Ok(Some(V { c: b, ty: bt, obj: None }));
        }
        // Its object, established over it as a struct literal's is.
        let tid = self.tid(&bt)?;
        let loc = self.loc(line);
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, b, tid, loc));
        Ok(Some(V { c: b, ty: bt, obj: Some(o) }))
    }
}
