// Fx: blocks and statements.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // ---- blocks and statements ----

    // The statements and tail of a block inside an already-opened
    // frame. With a sink, the tail's value is stored there inside the
    // tail's own statement scope (so its reference data is still live)
    // and its object, if any, is re-stamped to the enclosing statement.
    pub(super) fn lower_block_body(&mut self, b: &Block, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        self.scopes.push(HashMap::new());
        let mut result = unit();
        let mut aliases: Vec<String> = Vec::new();
        let mut bare_unlocks: Vec<String> = Vec::new();
        let mut skip_next = false;
        for (si, s) in b.stmts.iter().enumerate() {
            if std::mem::take(&mut skip_next) {
                continue;
            }
            if let Some(l) = stmt_line(s) {
                self.at(l);
            }
            // `p op= e` with a call in `p` (D-0035) is the block
            // `{ auto t = &mut p; *t = *t op e; }`. With `e` a literal or a
            // local, nothing runs between the borrow and the write but the
            // read through `t`, so `t` need not be a binding that holds the
            // path (D-0018): the two statements are lowered as one, `t`'s
            // uses through the borrow's token, which that one statement
            // keeps until its end.
            if let (Stmt::Let { ty: None, name, init: Some(e), line, .. }, Some(next)) = (s, b.stmts.get(si + 1)) {
                if !self.g.boot(4) && name.starts_with("__compound") && matches!(&e.kind, ExprKind::Borrow(Mode::Exclusive, _)) && compound_write_inert(next, name) {
                    self.push_stmt();
                    // `*HashMap::entry(…) op= e`: the native `entry`'s
                    // `_acc` variant gives the value's address, the
                    // element's access made; the read and write through it
                    // then need no check (`direct_refs`).
                    let acc_call = match &e.kind {
                        ExprKind::Borrow(_, inner) => match &strip_parens(inner).kind {
                            ExprKind::Deref(c) => match &strip_parens(c).kind {
                                ExprKind::Call(callee, cargs) if matches!(&callee.kind, ExprKind::Path(segs, _) if segs.join("::") == "std::HashMap::entry") => Some((&**c, cargs.as_ptr() as usize)),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some((c, target)) = acc_call {
                        self.map_acc = Some(target);
                        self.map_acc_used = false;
                        let v = self.lower_expr(c, None);
                        self.map_acc = None;
                        let v = v?;
                        if matches!(v.ty, Type::Never) {
                            self.end_diverged();
                            result = never();
                            break;
                        }
                        if std::mem::take(&mut self.map_acc_used) {
                            let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                            self.emit(&format!("void *{} = (void *){};", l, v.c));
                            self.names_add(NameSet::DirectRefs, l.clone());
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: v.ty.clone(), root: String::new(), kind: BindKind::Local });
                        } else {
                            // An ordinary call: its reference, as the borrow
                            // of `*call` would be (the same path).
                            let r = self.into_temp(v, *line)?;
                            let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                            self.emit(&format!("void *{} = ({}).p;", l, r.c));
                            self.names_put(NameMap::Unstored, l.clone(), format!("({}).tok", r.c));
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: r.ty.clone(), root: String::new(), kind: BindKind::Local });
                        }
                        let diverged = self.lower_stmt(next)?;
                        if diverged {
                            self.end_diverged();
                            result = never();
                            break;
                        }
                        self.pop_stmt();
                        skip_next = true;
                        continue;
                    }
                    self.compound_elem = true;
                    let v = self.lower_expr(e, None);
                    self.compound_elem = false;
                    let v = v?;
                    if matches!(v.ty, Type::Never) {
                        self.end_diverged();
                        result = never();
                        break;
                    }
                    let r = self.into_temp(v, *line)?;
                    let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                    self.emit(&format!("void *{} = ({}).p;", l, r.c));
                    self.names_put(NameMap::Unstored, l.clone(), format!("({}).tok", r.c));
                    self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: r.ty.clone(), root: String::new(), kind: BindKind::Local });
                    let diverged = self.lower_stmt(next)?;
                    if diverged {
                        self.end_diverged();
                        result = never();
                        break;
                    }
                    self.pop_stmt();
                    skip_next = true;
                    continue;
                }
            }
            // A named reference to an element of a confined vector
            // (`elem_ref_let`): the element's address, unchecked.
            if let Some((name, marg, line)) = bare_guard_let(s, &b.stmts[si + 1..], b.tail.as_deref()) {
                if self.lower_bare_guard(name, marg, line, &mut bare_unlocks)? {
                    continue;
                }
            }
            if let Some((name, place, mode, _)) = elem_ref_let(s, &b.stmts[si + 1..], b.tail.as_deref()) {
                match self.bind_elem_ref(name, place, mode, stmt_line(s))? {
                    Some(true) => {
                        self.end_diverged();
                        result = never();
                        break;
                    }
                    Some(false) => continue,
                    None => {}
                }
            }
            if let Some((name, alias)) = self.foreach_alias(s, b.stmts.get(si + 1))? {
                self.keep(alias.clone());
                self.elem_alias.insert(name.clone(), alias);
                aliases.push(name);
                continue;
            }
            self.push_stmt_at(stmt_line(s));
            // D-0315 (step 4): this `let`'s binder may be a noted value (the
            // enclosing statement's again after it: a block inside a `let`'s
            // value has statements of its own).
            let s1_outer = self.s1_let;
            self.s1_let = match s {
                Stmt::Let { init: Some(e), .. } if self.g.decided && !self.g.boot(4) => Some(&**e as *const Expr as usize),
                _ => None,
            };
            let diverged = self.lower_stmt(s);
            self.s1_let = s1_outer;
            let diverged = diverged?;
            if diverged {
                self.pending_sub = false;
                self.end_diverged();
                result = never();
                break;
            }
            self.pop_stmt();
            if std::mem::take(&mut self.pending_sub) {
                self.push_sub_frame();
            }
            self.end_ref_bindings(s);
            // `$c = &v` or `&mut v` of a `foreach` whose body only reads or
            // writes its elements in place (`foreach_reads_only`): `$c`
            // holds `v` for the whole loop, so the condition's
            // `Vec::len(&*$c)` needs no check either (`foreach_alias`, which
            // registers the same once the body begins).
            if let Stmt::Let { name, init: Some(init), .. } = s {
                if self.quiet_ids.contains(&(&**init as *const Expr as usize)) {
                    if let Some(bd) = self.lookup(name).filter(|bd| !bd.root.is_empty() && matches!(bd.kind, BindKind::Local)) {
                        self.quiet_roots.insert(bd.root.clone());
                    }
                }
                if self.g.boot(7) {
                    // D-0274: no holder is confined in the bootstrap form.
                } else if foreach_reads_only(name, init, b) {
                    self.confine_holder(name)?;
                } else if self.held_vec_reader(name, init, b, si) {
                    self.confine_holder(name)?;
                }
            }
        }
        if !matches!(result.ty, Type::Never) {
            if let Some(t) = &b.tail {
                self.at(t.line);
                self.push_stmt_at(Some(t.line));
                // A tail `match` is its statement's last evaluation as a
                // statement `match` is (`stmt_match`).
                self.stmt_match = matches!(t.kind, ExprKind::Match(..));
                // D-0200: a `__raw` form's last `match` (`raw_tail`), or a
                // local named alone (`raw_return_path`).
                // D-0286: in a decided program, a raw sink's block (a `__raw`
                // form's last block, `unsafe { … }` say) ends the same way.
                let raw_sink = self.g.decided && sink.map_or(false, |(_, _, ov)| ov.map_or(false, |o| self.raw_sinks.contains(o)));
                if (sink.is_none() && self.raw_ret) || raw_sink {
                    match &strip_parens(t).kind {
                        ExprKind::Match(_, arms) => self.raw_tail = Some(arms.as_ptr() as usize),
                        ExprKind::Block(ib) | ExprKind::Unsafe(ib) if self.g.decided => self.raw_tail = Some(ib as *const Block as usize),
                        // A decided program's last `if`, by its condition's
                        // address (`lower_if` probes its branches first).
                        ExprKind::If(c, ..) if self.g.decided => self.raw_tail = Some(&**c as *const Expr as usize),
                        ExprKind::Path(..) => self.raw_return_path = true,
                        _ => {}
                    }
                    // D-0286: a call's result returned with no object.
                    self.arm_inert(t, true);
                    // D-0294: in a decided program a literal, or a `Box` of a
                    // value, that is the result leaves with no object: the
                    // one made would be forgotten at once (`emit_return`).
                    if raw_sink || self.g.decided {
                        self.objectless = self.literal_expr(t) || (self.g.decided && self.box_new_expr(t));
                    }
                }
                let v = self.lower_expr(t, expected);
                self.stmt_match = false;
                self.raw_return_path = false;
                self.inert_result = None;
                self.objectless = false;
                let v = v?;
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                    result = never();
                } else if matches!(v.ty, Type::Void) {
                    self.emit(&format!("(void)({});", v.c));
                    self.pop_stmt();
                    result = unit();
                } else {
                    match sink {
                        Some((dst, dty, ov)) => {
                            let obj = self.assign_sink(dst, dty, v, t.line, ov)?;
                            if let Some(o) = &obj {
                                self.emit(&format!("cb_result({});", o));
                            }
                            self.pop_stmt();
                            result = V { c: dst.to_string(), ty: dty.clone(), obj };
                        }
                        None => {
                            // Value flows out to the function's return.
                            let t2 = self.into_temp(v, t.line)?;
                            if let Some(o) = &t2.obj {
                                self.emit(&format!("cb_result({});", o));
                            } else if matches!(t2.ty, Type::Ref(..)) {
                                self.emit(&format!("cb_result_ref({}.tok);", t2.c));
                            }
                            self.pop_stmt();
                            result = t2;
                        }
                    }
                }
            }
        }
        for a in aliases {
            self.elem_alias.remove(&a);
        }
        // Bare guards (`lower_bare_guard`) unlock as the block ends, after
        // its value is taken, the last made first.
        for l in bare_unlocks.iter().rev() {
            self.emit(&format!("cb_unlock_bare({});", l));
        }
        self.scopes.pop();
        Ok(result)
    }

    // D-0202 phase 4: `auto g = lock(m);` whose block uses `g` only as
    // `*g` or `g.f…`, read or written, plain data (`bare_guard_let`): the
    // mutex taken and given back by the book alone (`cb_lock_bare`,
    // `cb_unlock_bare`, as the channel natives use it), with no guard
    // object and no lock path. The interior is reached only through `g`
    // while the block runs, so its accesses need no check (`direct_refs`).
    // The reference to the mutex is checked as `lock` checks it. Returns
    // false, having emitted nothing, where the interior is not plain.
    pub(super) fn lower_bare_guard(&mut self, name: &str, marg: &Expr, line: usize, unlocks: &mut Vec<String>) -> R<bool> {
        if self.g.boot(2) {
            return Ok(false);
        }
        let inner = match self.probe_type(marg, None)? {
            Type::Ref(p, _) => match *p {
                Type::Mutex(i) => *i,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if self.is_res(&inner) || self.has_refs(&inner) {
            return Ok(false);
        }
        self.push_stmt_at(Some(line));
        let mv = self.lower_expr(marg, None)?;
        if matches!(mv.ty, Type::Never) {
            return internal("a bare guard's mutex expression diverges");
        }
        let mt = self.into_temp(mv, line)?;
        let loc = self.loc(line);
        self.emit(&format!("cb_read({}.tok, NULL, 0, {});", mt.c, loc));
        // D-0310: the mutex's type, for where its state cells are.
        let mtid = self.tid(&Type::Mutex(Box::new(inner.clone())))?;
        self.emit(&format!("cb_lock_bare({}.p, {}u, {});", mt.c, mtid, loc));
        let l = format!("l_{}_{}", san(name), self.g.fresh(""));
        self.emit(&format!("void *{} = {}.p;", l, mt.c));
        self.pop_stmt();
        self.names_add(NameSet::DirectRefs, l.clone());
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: l.clone(), ty: Type::Ref(Box::new(inner), Mode::Exclusive), root: String::new(), kind: BindKind::Local });
        unlocks.push(l);
        Ok(true)
    }

    // D-0111 `[Ref-Binding-Last-Use]`: after `s`, each local reference,
    // slice or `StringView` binding of this block the checker ended there
    // ends as at the block's exit (its object, and the path it holds).
    pub(super) fn end_ref_bindings(&mut self, s: &Stmt) {
        if !self.g.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let names = self.g.items.early_ends.lock().unwrap().get(&(s as *const Stmt as usize)).cloned();
        for n in names.unwrap_or_default() {
            let Some(b) = self.scopes.last().and_then(|sc| sc.get(&n)).cloned() else { continue };
            let is_ref = coby::typecheck::ends_at_last_use(&self.g.items, &b.ty);
            if self.held_c.contains(&b.c) {
                // A held local (`cb_frame_hold`): its frame lets go now.
                if let Some(tk) = self.unstored.get(&b.c).cloned() {
                    self.emit(&format!("cb_frame_unhold({});", tk));
                }
                continue;
            }
            let root = if b.root.is_empty() { self.holder_roots.get(&b.c).cloned().unwrap_or_default() } else { b.root.clone() };
            // D-0315 (step 4): a root of token 0 (a decided program's
            // reference binding) has nothing to end.
            if !is_ref || root.is_empty() || root == "0" || !matches!(b.kind, BindKind::Local) {
                continue;
            }
            self.emit(&format!("cb_end_binding({});", root));
        }
    }

    // Whether `e` is `r.f…` for a direct reference parameter `r`.
    pub(super) fn direct_place(&self, e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) => self.direct_place(a),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup(&segs[0]).map_or(false, |b| self.direct_refs.contains(&b.c) && matches!(b.ty, Type::Ref(..))),
            _ => false,
        }
    }

    // Stores a value into a result temporary declared outside the
    // current C block. The object now living there, if any, is recorded
    // in `obj_var` — a `uint64_t` also declared outside the block, since
    // the id computed here is a local of the block being closed.
    pub(super) fn assign_sink(&mut self, dst: &str, dty: &Type, v: V, line: usize, obj_var: Option<&str>) -> R<Option<String>> {
        if matches!(v.ty, Type::Never | Type::Void) {
            return Ok(None);
        }
        if matches!(dty, Type::Ref(..)) {
            // A reference result stays in register form (`cb_ref`) and
            // outlives the statement that formed it.
            self.emit(&format!("{} = {};", dst, v.c));
            self.emit(&format!("cb_result_ref({}.tok);", dst));
            return Ok(None);
        }
        self.store(dst, dty, &v)?;
        // A raw sink (`match_sink_obj_var`): the value's record ends here.
        if obj_var.map_or(false, |ov| self.raw_sinks.contains(ov)) {
            if let Some(o) = &v.obj {
                self.emit(&format!("cb_forget_obj({});", o));
            }
            return Ok(None);
        }
        let o = if self.is_res(dty) {
            // D-0315: an inert value (`inert_c`) is given its object here.
            let Some(o) = self.materialize(&v, line)? else { return internal("a resource result without an object") };
            self.emit(&format!("cb_move_to({}, &{});", o, dst));
            o
        } else if self.has_refs(dty) {
            match v.obj {
                Some(o) => {
                    self.emit(&format!("cb_move_to({}, &{});", o, dst));
                    o
                }
                None if self.plain_slice(dty) => return Ok(None),
                None => {
                    let tid = self.tid(dty)?;
                    let loc = self.loc(line);
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, dst, tid, loc));
                    o
                }
            }
        } else {
            return Ok(None);
        };
        match obj_var {
            Some(ov) => {
                self.emit(&format!("{} = {};", ov, o));
                Ok(Some(ov.to_string()))
            }
            None => Ok(Some(o)),
        }
    }

    // The holder for a sink's object id, declared beside the sink.
    pub(super) fn sink_obj_var(&mut self, ty: &Type) -> Option<String> {
        if matches!(ty, Type::Ref(..)) || self.plain_slice(ty) || !(self.is_res(ty) || self.has_refs(ty)) {
            return None;
        }
        let ov = self.g.fresh("ob");
        self.emit(&format!("uint64_t {} = 0;", ov));
        Some(ov)
    }

    // A `match`'s sink object holder, raw when the `match` (`id`: its arms'
    // address) is a `__raw` form's last expression (`raw_tail`) and its
    // value is a resource holding no reference.
    pub(super) fn match_sink_obj_var(&mut self, ty: &Type, id: usize) -> Option<String> {
        let ov = self.sink_obj_var(ty)?;
        if self.raw_tail == Some(id) && self.is_res(ty) && !self.has_refs(ty) {
            self.raw_tail = None;
            self.names_add(NameSet::RawSinks, ov.clone());
        } else if self.raw_arm == Some(id) && self.g.probing == 0 && self.is_res(ty) && !self.has_refs(ty) && (self.g.objectless_type(ty) || std::mem::take(&mut self.raw_arm_noted)) {
            // D-0315: its consumer takes it with no object (`arm_inert`).
            self.raw_arm = None;
            self.names_add(NameSet::RawSinks, ov.clone());
        }
        Some(ov)
    }

    // A block as a value: opens a frame; its tail lands in a temporary
    // declared outside the braces, whose type is known only afterwards.
    pub(super) fn lower_block(&mut self, b: &Block, expected: Option<&Type>) -> R<V> {
        let frame_line = self.frame_line.take();
        // First pass into a buffer to learn the tail's type, then again
        // with the sink? Two passes would emit twice. Instead: lower the
        // body without a sink into a buffer, and if it produced a value,
        // re-lower with the sink. Bodies are small; the cost is compile time.
        // The probe's answer is cached like `probe_type`'s (a nested block
        // was otherwise lowered twice per level of nesting).
        let key = (b as *const Block as usize, format!("block {:?}", expected));
        let value_ty = match self.probes.get(&key) {
            Some(t) => t.clone(),
            None => {
                let saved_out = std::mem::take(&mut self.out);
                let saved_uid = self.g.uid;
                let saved_names = self.snapshot_names();
                let saved_nstr = self.g.nstr;
                let saved_strs = self.g.str_mark();
                let saved_cleanup = self.cleanup.len();
                let saved_scopes = self.scopes.len();
                let probe_ind = self.ind;
                self.ind += 1;
                self.push_frame_at(frame_line);
                self.g.probing += 1;
                let probe = self.lower_block_body(b, expected, None);
                self.g.probing -= 1;
                let probe = probe?;
                self.cleanup.truncate(saved_cleanup);
                self.scopes.truncate(saved_scopes);
                self.ind = probe_ind;
                self.out = saved_out;
                self.g.uid = saved_uid;
                self.restore_names(saved_names);
                self.g.nstr = saved_nstr;
                self.g.str_restore(saved_strs);
                self.probes.insert(key, probe.ty.clone());
                probe.ty
            }
        };
        let (sink, ov) = if matches!(value_ty, Type::Never | Type::Void) {
            (None, None)
        } else {
            let r = self.g.fresh("b");
            let c = self.g.reg_ctype(&value_ty)?;
            self.emit(&format!("{} {};", c, r));
            // D-0286: raw as a `match`'s is, when the block is a `__raw`
            // form's last expression (`raw_tail`).
            let ov = self.match_sink_obj_var(&value_ty, b as *const Block as usize);
            (Some(r), ov)
        };
        self.emit("{");
        self.ind += 1;
        self.push_frame_at(frame_line);
        let v = self.lower_block_body(b, expected, sink.as_deref().map(|s| (s, &value_ty, ov.as_deref())))?;
        if !matches!(v.ty, Type::Never) {
            self.pop_frame();
        } else {
            self.end_diverged();
        }
        self.ind -= 1;
        self.emit("}");
        // A raw sink's value has no object (`inert_c`).
        if let (Some(s), Some(o)) = (&sink, &ov) {
            if self.raw_sinks.contains(o) {
                self.names_add(NameSet::InertC, s.clone());
            }
        }
        Ok(match sink {
            Some(s) => V { c: s, ty: value_ty, obj: v.obj },
            None => v,
        })
    }

    // Returns true when the statement diverges.
    pub(super) fn lower_stmt(&mut self, s: &Stmt) -> R<bool> {
        match s {
            // `foreach (x in p)` over a confined parameter `p`
            // (`foreach_param_reads_only`): its hidden holders `$e = p` and
            // `$d = $each_drain($e, n)` (the reference itself, `each.rs`)
            // are `p` under other names, reaching the vector as `p` does.
            Stmt::Let { ty: None, name, init: Some(e), .. } if name.starts_with('$') && self.confined_holder(e).is_some() => {
                let (lv, path, elem) = self.confined_holder(e).unwrap();
                // Bound as a direct reference is (`bind_param`): the
                // address alone, its accesses unchecked.
                let vty = Type::Named("std::Vec".to_string(), vec![elem.clone()]);
                let vec_c = self.g.ctype(&vty)?;
                let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                self.emit(&format!("void *{} = (void *)&({});", l, lv));
                self.names_add(NameSet::DirectRefs, l.clone());
                self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l.clone(), ty: Type::Ref(Box::new(vty), Mode::Shared), root: String::new(), kind: BindKind::Local });
                self.confined_params.insert(name.clone(), (format!("(*({} *){})", vec_c, l), path, elem));
                Ok(false)
            }
            // D-0315: `_ = e;` (`{ auto $discard = e; }`, D-0099) in a decided
            // program: `e`'s value is ended where its statement ends, as a
            // discarded expression statement's is.
            Stmt::Let { ty: None, name, init: Some(e), .. } if self.g.decided && name == "$discard" => self.lower_discarded(e),
            Stmt::Let { ty, name, init, .. } => {
                // D-0320: a value known before the run that nothing assigns
                // (a `foreach` range's end among them), for the length guard
                // of a loop that follows; a binding of the name that is not
                // such a value hides an earlier one.
                if !self.g.boot(2) {
                    let v = init.as_ref().filter(|_| !self.assigned.contains(name)).and_then(|e| guards::known_value(name, e, &self.g.items.fns));
                    match v {
                        Some(v) => self.const_locals.insert(name.clone(), v),
                        None => self.const_locals.remove(name),
                    };
                }
                let declared = ty.as_ref().map(|t| self.sub(t));
                let cname = format!("l_{}_{}", san(name), self.g.fresh(""));
                if let Some(e) = init.as_ref().filter(|e| self.elem_ref_ids.contains(&(&***e as *const Expr as usize)) && self.g.probing == 0) {
                    if let Some(diverged) = self.lower_elem_ref_let(name, &cname, declared.as_ref(), e)? {
                        return Ok(diverged);
                    }
                }
                match init {
                    Some(e) => {
                        // D-0200: a fused text loop's bare piece trimmed into a
                        // local (`bare_trims`): a bare view of the trimmed
                        // range, as `native_view_trim` finds it.
                        if self.bare_trims.contains(name) {
                            let svt = Type::Named("std::StringView".to_string(), vec![]);
                            if let Some((k, x)) = trim_of(e).filter(|_| declared.as_ref().map_or(true, |t| *t == svt)) {
                                if let Some(b) = self.lookup(&x).filter(|b| self.direct_refs.contains(&format!("{}.bytes", b.c)) && b.ty == svt) {
                                    let svc = self.g.ctype(&svt)?;
                                    let (i, n) = (self.g.fresh("ti"), self.g.fresh("tn"));
                                    self.emit(&format!("uint64_t {} = 0, {} = ({}).bytes.len;", i, n, b.c));
                                    let d = format!("({}).bytes.data", b.c);
                                    if k != "std::StringView::trim_end" {
                                        self.emit(&format!("while ({i} < {n} && ({d}[{i}] == ' ' || ({d}[{i}] >= 9 && {d}[{i}] <= 13))) {i}++;", i = i, n = n, d = d));
                                    }
                                    if k != "std::StringView::trim_start" {
                                        self.emit(&format!("while ({n} > {i} && ({d}[{n} - 1] == ' ' || ({d}[{n} - 1] >= 9 && {d}[{n} - 1] <= 13))) {n}--;", i = i, n = n, d = d));
                                    }
                                    self.emit(&format!("{} {};", svc, cname));
                                    self.emit(&format!("{}.bytes.src = ({}).bytes.src;", cname, b.c));
                                    self.emit(&format!("{}.bytes.data = {} + {};", cname, d, i));
                                    self.emit(&format!("{}.bytes.len = {} - {};", cname, n, i));
                                    self.names_add(NameSet::DirectRefs, format!("{}.bytes", cname));
                                    self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty: svt, root: String::new(), kind: BindKind::Local });
                                    return Ok(false);
                                }
                            }
                        }
                        // An objectless `String` made by `String::copy_str`
                        // (`sprintf` is one) or `String::clone`: the copy's
                        // `_raw` variant, with no object to receive and end.
                        if let Some(done) = self.bind_fresh_objectless(name, &cname, declared.as_ref(), e)? {
                            return Ok(done);
                        }
                        // D-0196: `String s = f(…);` into an objectless `s`, `f`
                        // a function whose `__raw` form exists (`raw_ret_ok`).
                        // CHG-0345: only when the binding is objectless too
                        // (`objectless_binding`): an indexed `Vec<String>` is
                        // not, and needs the object the raw call would not give.
                        if self.objectless_names.contains(&(&**e as *const Expr as usize)) {
                            if let ExprKind::Call(callee, _) = &strip_parens(e).kind {
                                if let ExprKind::Path(segs, targs) = &callee.kind {
                                    let k = segs.join("::");
                                    let id = &**e as *const Expr as usize;
                                    // D-0315: `std`'s functions by their full name too; a
                                    // variant (no function) by the declared type.
                                    let ret = self.g.items.fns.get(&k).or_else(|| self.g.items.fns.get(&format!("std::{}", k))).map(|f| f.ret.clone());
                                    // D-0315 (step 4): an intrinsic's, by the declared
                                    // type or the call's probed one.
                                    let ret = match ret {
                                        None if self.g.decided && self.g.probing == 0 && self.lookup(&k).is_none() => match &declared {
                                            Some(d) => Some(d.clone()),
                                            None => Some(self.probe_type(e, None)?),
                                        },
                                        r => r,
                                    };
                                    let fits = ret.map_or(false, |t| self.objectless_binding(id, declared.as_ref().unwrap_or(&t)))
                                        || (self.g.decided && self.literal_expr(e) && declared.as_ref().map_or(false, |t| self.objectless_binding(id, t)));
                                    if targs.is_empty() && self.lookup(&k).is_none() && self.g.raw_ret_ok(&k) && fits {
                                        self.raw_call = Some(self.g.request_fn(&k, Vec::new())?);
                                    } else if fits {
                                        // D-0286: any call's, in a decided program.
                                        self.arm_inert(e, true);
                                    }
                                }
                            } else if self.g.decided && matches!(strip_parens(e).kind, ExprKind::Match(..) | ExprKind::If(..) | ExprKind::Block(_) | ExprKind::Unsafe(_) | ExprKind::Propagate(_) | ExprKind::StructLit(..) | ExprKind::ArrayLit(_)) {
                                // D-0315: a `match`, `if`, block or `?` given
                                // into the objectless binding with no object.
                                let id = &**e as *const Expr as usize;
                                // (With no declared type, by the value's.)
                                let dt = match &declared {
                                    Some(t) => Some(t.clone()),
                                    None if self.g.probing == 0 => Some(self.probe_type(e, None)?),
                                    None => None,
                                };
                                if dt.as_ref().map_or(false, |t| self.objectless_binding(id, t)) {
                                    self.arm_inert(e, true);
                                }
                            }
                        }
                        let v = self.lower_expr(e, declared.as_ref());
                        self.raw_call = None;
                        self.inert_result = None;
                        let v = v?;
                        // A closure-typed binding the body borrows (`&g`,
                        // passed where `ref<fn(…) : R, m>` is expected) is
                        // kept as a callable box: a reference to it is then
                        // a reference to a `fn` value, as the callee reads
                        // it. Its value, copies and calls are the box's.
                        // So is one the body assigns whole: the closure
                        // assigned is of another type (`f = move [b]() …`
                        // after `auto f = move [a]() …`), boxed as well.
                        let v = match (&declared, &v.ty) {
                            (None, Type::Closure(id)) if self.pinned.contains(name) || self.assigned.contains(name) => {
                                let info = self.g.closures[id].clone();
                                let dst = Type::Fn(info.params.clone(), Box::new(info.ret.clone()));
                                self.box_closure(v, dst, e.line)?
                            }
                            _ => v,
                        };
                        let ty = declared.unwrap_or_else(|| v.ty.clone());
                        if matches!(v.ty, Type::Never) {
                            return Ok(true);
                        }
                        let v = if matches!(ty, Type::Ref(..)) { self.into_temp(v, e.line)? } else { v };
                        let c = self.g.ctype(&ty)?;
                        self.emit(&format!("{} {};", c, cname));
                        self.cur_let = Some(&**e as *const Expr as usize);
                        let bound = self.bind_value(name, cname.clone(), &ty, &v, e.line);
                        self.cur_let = None;
                        bound?;
                        self.bind_confined(name, &cname, &ty)?;
                        self.bind_reader_slice(name, &cname, &ty)?;
                    }
                    None => {
                        let ty = declared.expect("`auto x;` has no type");
                        let c = self.g.ctype(&ty)?;
                        self.emit(&format!("{} {};", c, cname));
                        if self.unchecked(name, &ty) {
                            // Definite assignment (`spec/11` §3) proves every read.
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root: String::new(), kind: BindKind::Local });
                            return Ok(false);
                        }
                        // D-0315: pinned plain data of a decided program: root token 0.
                        if self.pinned_plain(&ty) {
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root: "0".to_string(), kind: BindKind::Local });
                            return Ok(false);
                        }
                        // D-0315 (step 4): a decided program's candidate whose end
                        // frees memory alone: no object, its moved flag set until
                        // the first assignment.
                        let key = s as *const Stmt as usize;
                        if self.g.decided && self.g.probing == 0 && self.objectless_names.contains(&key) && self.is_res(&ty) && !self.has_refs(&ty) && self.g.objectless_type(&ty) && self.objectless_binding(key, &ty) {
                            let tid = self.tid(&ty)?;
                            let saved = self.cur_let.replace(key);
                            self.objectless_bound(&cname, tid);
                            self.cur_let = saved;
                            let Some(fl) = self.moved_flag.get(&cname).cloned() else { return internal("an objectless local with no value and no moved flag") };
                            self.emit(&format!("{} = 1;", fl));
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root: String::new(), kind: BindKind::Local });
                            return Ok(false);
                        }
                        let tid = self.tid(&ty)?;
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new_uninit(&{}, {}u, {});", o, cname, tid, NOLOC));
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                        self.note_frame(name, &root);
                        self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root, kind: BindKind::Local });
                    }
                }
                Ok(false)
            }
            Stmt::Destructure { struct_name, fields, names, rest, init } => {
                // D-0315: a decided program's call result destructured is
                // taken with no object (`arm_inert`).
                self.arm_inert(init, true);
                // And an objectless local destructured, with none.
                self.raw_return_path = self.g.decided && matches!(strip_parens(init).kind, ExprKind::Path(..));
                let v = self.lower_expr(init, None);
                self.inert_result = None;
                self.raw_return_path = false;
                let v = v?;
                if matches!(v.ty, Type::Never) {
                    return Ok(true);
                }
                let (sname, args) = match &v.ty {
                    Type::Named(n, a) => (n.clone(), a.clone()),
                    _ => return internal("destructuring a non-struct"),
                };
                if &sname != struct_name && !sname.ends_with(&format!("::{}", struct_name)) {
                    return internal("destructuring struct name mismatch");
                }
                let sd = self.g.struct_decl(&sname).expect("struct");
                let sub = subst_of(&sd.type_params, &args);
                // D-0315: a value with no object (an objectless binding moved,
                // an inert call result) stays so: every field moves out to a
                // binder (a named one or `$rest_f`).
                let whole = if self.g.decided && v.obj.is_none() && self.is_res(&v.ty) && self.g.objectless_type(&v.ty) {
                    let t = self.g.fresh("t");
                    let c = self.g.ctype(&v.ty)?;
                    self.emit(&format!("{} {} = {};", c, t, v.c));
                    V { c: t, ty: v.ty, obj: None }
                } else {
                    self.into_temp(v, init.line)?
                };
                let decl_names: Vec<String> = sd.fields.iter().map(|f| f.name.clone()).collect();
                for (fname, binder) in coby::ast::destructure_pairs(names, fields, *rest, &decl_names) {
                    let field = &fname;
                    let fd = sd.fields.iter().find(|f| &f.name == field).expect("field");
                    let fty = apply(&fd.ty, &sub);
                    let cname = format!("l_{}_{}", san(&binder), self.g.fresh(""));
                    let c = self.g.ctype(&fty)?;
                    self.emit(&format!("{} {};", c, cname));
                    let fv = V { c: format!("{}.{}", whole.c, san(field)), ty: fty.clone(), obj: None };
                    let fv = if matches!(fty, Type::Ref(..)) { self.read_ref_slot(&fv.c, &fty) } else { fv };
                    // D-0315: a binder `objectless_strings` found a candidate,
                    // of a type whose end frees memory alone: no object.
                    // A field `..` passed over (`$rest_f`, which nothing names)
                    // likewise, with no moved flag.
                    let key = fields.iter().find(|n| **n == binder).map(destructure_key);
                    let hidden = binder.starts_with("$rest_") && self.g.decided && self.g.probing == 0 && self.is_res(&fty) && !self.has_refs(&fty) && self.g.objectless_type(&fty);
                    if hidden || key.map_or(false, |k| self.g.decided && self.g.probing == 0 && self.is_res(&fty) && self.objectless_binding(k, &fty)) {
                        self.store(&cname, &fty, &fv)?;
                        let tid = self.tid(&fty)?;
                        let saved = std::mem::replace(&mut self.cur_let, key.filter(|_| !hidden));
                        self.objectless_bound(&cname, tid);
                        self.cur_let = saved;
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: fty, root: String::new(), kind: BindKind::Local });
                        continue;
                    }
                    // [Relocate-Out]: each field becomes its own object; the
                    // container ends without destroying what moved out.
                    if self.is_res(&fty) {
                        self.store(&cname, &fty, &fv)?;
                        let tid = self.tid(&fty)?;
                        let loc = self.loc(init.line);
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                        self.note_frame(&binder, &root);
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: fty, root, kind: BindKind::Local });
                    } else {
                        self.bind_value(&binder, cname, &fty, &fv, init.line)?;
                    }
                }
                if let Some(o) = &whole.obj {
                    self.emit(&format!("cb_end_moved_out({});", o));
                }
                Ok(false)
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => self.lower_discarded(e),
        }
    }

    // An expression statement: its value, if any, discarded.
    pub(super) fn lower_discarded(&mut self, e: &Expr) -> R<bool> {
        self.stmt_match = matches!(e.kind, ExprKind::Match(..));
        // D-0315: a decided program's discarded call result (or `?`'s
        // value) is taken with no object and dropped where the statement
        // ends.
        if self.g.decided && matches!(strip_parens(e).kind, ExprKind::Call(..) | ExprKind::Propagate(_)) {
            self.arm_inert(e, true);
        }
        let v = self.lower_expr(e, None);
        self.inert_result = None;
        self.stmt_match = false;
        let v = v?;
        if matches!(v.ty, Type::Never) {
            return Ok(true);
        }
        if v.obj.is_none() && self.inert_c.contains(&v.c) && self.is_res(&v.ty) {
            let t = self.into_temp(v, e.line)?;
            let tid = self.tid(&t.ty)?;
            // A value with a destructor of its own is noted, as a statement
            // temporary is (D-0315 step 4).
            let noted = self.g.decided && !self.g.objectless_type(&t.ty) && self.g.noted_type(&t.ty);
            let dropped = !self.has_refs(&t.ty) && (self.g.objectless_type(&t.ty) || noted) && self.stmt_temp_drop(&t.c, tid, noted);
            if !dropped {
                // Its end is the statement's, by its object.
                self.materialize(&t, e.line)?;
            }
            self.emit(&format!("(void)({});", t.c));
            return Ok(false);
        }
        if !matches!(v.ty, Type::Void) {
            if v.obj.is_none() && self.holds_fn(&v.ty) {
                // D-0089: a discarded value's closures end with it.
                let t = self.into_temp(v, e.line)?;
                let tid = self.tid(&t.ty)?;
                self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
            } else {
                self.emit(&format!("(void)({});", v.c));
            }
        }
        Ok(false)
    }
}
