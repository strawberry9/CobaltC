// Fx: frames, temporaries and bindings.

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // After `cb_bind` of binding `name` with root `root`: when the body
    // assigns to `name` as a whole, remember the frame it was bound in.
    pub(super) fn note_frame(&mut self, name: &str, root: &str) {
        if !root.is_empty() && self.assigned.contains(name) {
            let f = self.g.fresh("frame");
            self.emit(&format!("uint64_t {} = cb_frame_here();", f));
            self.rebind_frames.insert(root.to_string(), f);
        }
    }

    pub(super) fn emit(&mut self, s: &str) {
        for _ in 0..self.ind {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub(super) fn sub(&self, t: &Type) -> Type {
        apply(t, &self.subst)
    }

    pub(super) fn is_res(&self, t: &Type) -> bool {
        self.g.is_resource(t)
    }

    pub(super) fn has_refs(&self, t: &Type) -> bool {
        self.g.has_refs(t)
    }

    // D-0315: a value of a decided program that is plain data there (no
    // resource, no `fn` value, not a reference): a local of it that `unchecked`
    // refuses (pinned by a borrow, a slice or a capture) needs no object,
    // since every use of its root is a check that token 0 makes nothing.
    pub(super) fn pinned_plain(&self, ty: &Type) -> bool {
        self.g.decided && !self.is_res(ty) && !self.holds_fn(ty) && !matches!(ty, Type::Ref(..) | Type::Closure(_)) && (self.g.plain_data(ty) || self.g.decided_plain(ty))
    }

    // D-0315: a reference in a decided program: its token is 0 and no path
    // is kept (`cbrt.h` under `CB_DECIDED`), so a binding of one needs no
    // object.
    pub(super) fn decided_ref(&self, ty: &Type) -> bool {
        self.g.decided && matches!(ty, Type::Ref(..))
    }

    // D-0315: a slice in a decided program: plain data there (every token
    // is 0, and its end destroys nothing), so it never needs an object.
    pub(super) fn plain_slice(&self, t: &Type) -> bool {
        self.g.decided && matches!(t, Type::Slice(..))
    }

    pub(super) fn may_carry_ref_to(&self, t: &Type, inner: &Type, params: &[String]) -> bool {
        self.g.may_carry_ref_to(t, inner, params)
    }

    pub(super) fn holds_fn(&self, t: &Type) -> bool {
        self.g.holds_fn(t)
    }

    pub(super) fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    // D-0100: a format argument holding the name of the variant `tag`
    // selects, as text.
    pub(super) fn variant_name_text(&mut self, tag: &str, names: &[String]) -> String {
        let nm = self.g.fresh("vn");
        let nl = self.g.fresh("vl");
        let mut sw = format!("const char *{} = \"\"; uint64_t {} = 0; switch ({}) {{", nm, nl, tag);
        for (i, n) in names.iter().enumerate() {
            sw.push_str(&format!(" case {}u: {} = \"{}\"; {} = {}u; break;", i, nm, n, nl, n.len()));
        }
        sw.push_str(" default: break; }");
        self.emit(&sw);
        format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *){}, {} }}", nm, nl)
    }

    pub(super) fn loc(&mut self, line: usize) -> String {
        self.g.loc(line)
    }

    // The location a runtime fault without one of its own reports
    // (`cb_at`): set only at the program's own lines. Code inside `std`
    // leaves it at the program's line that called into `std`, as `coby`
    // reports such a fault there.
    pub(super) fn at(&mut self, line: usize) {
        let loc = self.loc(line);
        if loc != NOLOC {
            self.emit(&format!("cb_at({});", loc));
        }
    }

    // CHG-0362: after the arguments of a `std` call, one of which called a
    // function (moving the current location into it): the call's line
    // again, where a fault the `std` body raises is reported.
    pub(super) fn at_after_arg_calls(&mut self, f: &FnDecl, args: &[Expr], line: usize) {
        if (in_prelude(f) || !f.requires.is_empty()) && args.iter().any(|a| {
            let mut call = false;
            visit_expr(a, &mut |x| call |= matches!(x.kind, ExprKind::Call(..)));
            call
        }) {
            self.at(line);
        }
    }

    pub(super) fn tid(&mut self, t: &Type) -> R<u32> {
        self.g.type_id(t)
    }

    pub(super) fn projs_args(projs: &[String]) -> String {
        if projs.is_empty() {
            "NULL, 0".to_string()
        } else {
            format!("(cb_proj[]){{ {} }}, {}", projs.join(", "), projs.len())
        }
    }

    // ---- cleanup stack ----

    // Scopes are emitted as markers and settled by `resolve_scopes`
    // once the whole function is lowered.
    pub(super) fn scope_marker(&mut self, kind: char, id: u32) {
        self.emit(&format!("\u{1}{}{}", kind, id));
    }

    pub(super) fn push_frame(&mut self) {
        self.push_frame_at(None);
    }

    // D-0292: a function's own frame. `R`: the body's references are
    // recorded (`Fx::tracked`), emitted as `cb_frame_push_rec` whether or
    // not anything binds into it; `G`: a decided body, light, emitted as
    // `cb_frame_push_fn` where the frame is used.
    pub(super) fn push_frame_fn(&mut self, tracked: bool) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        self.scope_marker(if tracked { 'R' } else { 'G' }, id);
        self.cleanup.push(Cleanup::Frame(id));
    }

    // A frame whose end reports `line` for a destruction fault (the
    // block expression's, or its `if`'s): the marker carries the
    // location to `resolve_scopes`, which emits `cb_frame_push_at`.
    pub(super) fn push_frame_at(&mut self, line: Option<usize>) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        match line {
            Some(l) => {
                let loc = self.loc(l);
                self.emit(&format!("\u{1}F{}\u{2}{}", id, loc));
            }
            None => self.scope_marker('F', id),
        }
        self.cleanup.push(Cleanup::Frame(id));
    }

    pub(super) fn pop_frame(&mut self) {
        // The frame's objectless locals end first (`push_drop`), then
        // `cb_frame_pop` ends the objects the runtime records, last bound
        // first. Between the two groups this is not `[Block-Exit]`'s order
        // when an objectless local was bound after a recorded object, but
        // nothing can tell: no reference reaches an objectless local, its
        // end runs no destructor and calls no `extern`, so the program's
        // observable behaviour (`term.conformance`) is the same.
        // D-0315 (step 4): a noted value's sub-frame (`push_sub_frame`) holds
        // what was bound after it, and closes with the frame below it: the
        // objects after the value, the value, the objects before it.
        loop {
            while let Some(Cleanup::Drop(k)) = self.cleanup.last().copied() {
                self.cleanup.pop();
                let line = self.drop_lines[k as usize].clone();
                self.emit(&line);
            }
            let Some(Cleanup::Frame(id)) = self.cleanup.pop() else { panic!("cobc: frame pop closes a statement scope") };
            self.scope_marker('f', id);
            if !self.sub_frames.contains(&id) {
                break;
            }
        }
    }

    // D-0315 (step 4): the frame opened after a noted value's statement.
    pub(super) fn push_sub_frame(&mut self) {
        self.push_frame();
        let id = self.g.nscope;
        self.sub_frames.insert(id);
    }

    // An objectless local bound in the innermost frame: its drop runs where
    // that frame ends or a jump leaves it. Placed just above the frame and
    // the frame's earlier drops, below any statement scope open now, so
    // those pop first and the frame's locals end last bound first (D-0008's
    // order; nothing observes an objectless local's end, but a heap freed in
    // the order it was filled is cheaper: a large block freed last).
    pub(super) fn push_drop(&mut self, line: String) {
        let k = self.drop_lines.len() as u32;
        self.drop_lines.push(line);
        let mut at = self.cleanup.iter().rposition(|c| matches!(c, Cleanup::Frame(_))).map_or(0, |i| i + 1);
        while matches!(self.cleanup.get(at), Some(Cleanup::Drop(_))) {
            at += 1;
        }
        self.cleanup.insert(at, Cleanup::Drop(k));
    }

    // D-0315 (step 4): a parameter's drop, below the root frame (bound before
    // anything in it, it ends after the frame's objects, `[Block-Exit]`), above
    // the parameters' drops placed earlier. Made only while the parameters are
    // bound, before any loop has taken the cleanup stack's depth.
    pub(super) fn push_drop_under_root(&mut self, line: String) {
        let k = self.drop_lines.len() as u32;
        self.drop_lines.push(line);
        let root = self.root_frame;
        let at = self.cleanup.iter().position(|c| matches!(c, Cleanup::Frame(id) if *id == root)).unwrap_or(0);
        self.cleanup.insert(at, Cleanup::Drop(k));
    }

    pub(super) fn push_stmt(&mut self) {
        self.push_stmt_at(None);
    }

    // A statement scope whose end reports `line` for a destruction fault
    // (a temporary that ends with the statement while a kept reference
    // holds it, D-0103/D-0135): `resolve_scopes` emits `cb_stmt_push_at`.
    pub(super) fn push_stmt_at(&mut self, line: Option<usize>) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        match line {
            Some(l) => {
                let loc = self.loc(l);
                self.emit(&format!("\u{1}S{}\u{2}{}", id, loc));
            }
            None => self.scope_marker('S', id),
        }
        self.stmt_ind.insert(id, self.ind);
        self.cleanup.push(Cleanup::Stmt(id));
    }

    pub(super) fn pop_stmt(&mut self) {
        // D-0315: the statement's own temporaries dropped in the C
        // (`stmt_temp_drop`) end first, the last made first: `[Stmt-Exit]`'s
        // descending origin.
        while let Some(Cleanup::Drop(k)) = self.cleanup.last().copied() {
            self.cleanup.pop();
            let line = self.drop_lines[k as usize].clone();
            self.emit(&line);
        }
        let Some(Cleanup::Stmt(id)) = self.cleanup.pop() else { panic!("cobc: statement pop closes a frame") };
        self.scope_marker('s', id);
    }

    // D-0315: a decided body's temporary `cname` of a type whose end frees
    // memory alone, made by the innermost statement in that statement's own
    // C block, ends with the statement as `[Stmt-Exit]` ends it: by a C drop
    // where the statement ends or a jump leaves it, with no object. Noted in
    // the body's table (D-0269) so that a contained fault frees it as well.
    // False (nothing done) where the innermost scope is not such a statement.
    // D-0315 (step 4): `noted`, a type with a destructor of its own: its entry
    // is registered where it is made and removed (`cb_notes_drop`) where it
    // ends, before its destructor runs.
    pub(super) fn stmt_temp_drop(&mut self, cname: &str, tid: u32, noted: bool) -> bool {
        // Under the statement's own temporaries' drops, made before this one.
        let Some(Cleanup::Stmt(id)) = self.cleanup.iter().rev().find(|c| !matches!(c, Cleanup::Drop(_))).copied() else { return false };
        if self.stmt_ind.get(&id) != Some(&self.ind) {
            return false;
        }
        let drop = self.objectless_drop(cname, tid);
        let k = self.notes;
        self.notes += 1;
        // Its flag (`cb_tl`, zero at entry): only where it was made is it
        // dropped, since a short-circuit operand (`a && f(&g())`) or a branch
        // of the statement may have passed it by.
        let j = self.tl;
        self.tl += 1;
        self.emit(&format!("cb_nt[{}] = (struct cb_note){{ (void *)&{}, 0, {}u, 1 }};", k, cname, tid));
        self.emit(&format!("cb_tl[{}] = 1;", j));
        let d = self.drop_lines.len() as u32;
        if noted {
            self.notes_always = true;
            self.noted_pending.insert(cname.to_string(), (k, j));
            self.emit(&format!("cb_notes_push(&cb_nt[{}], 1u);", k));
            self.drop_lines.push(format!("if (cb_tl[{}]) {{ cb_nt[{}].live = 0; cb_notes_drop(&cb_nt[{}]); {} }} cb_tl[{}] = 0;", j, k, k, drop, j));
        } else {
            self.drop_lines.push(format!("if (cb_tl[{}]) {{ {} }} cb_tl[{}] = 0; cb_nt[{}].live = 0;", j, drop, j, k));
        }
        self.cleanup.push(Cleanup::Drop(d));
        true
    }

    // D-0315 (step 4): a pending noted temporary taken by its consumer: its
    // entry and its registration end, and the statement's end drops nothing.
    pub(super) fn noted_take(&mut self, c: &str) {
        if let Some((k, j)) = self.noted_pending.remove(c) {
            self.emit(&format!("cb_nt[{}].live = 0; cb_notes_drop(&cb_nt[{}]); cb_tl[{}] = 0;", k, k, j));
        }
    }

    // D-0315 (step 4): an inert value with a destructor of its own that more of
    // its statement runs after before its consumer takes it: a noted
    // temporary of the statement until then (`noted_take`); an object where
    // the innermost scope is not its statement's.
    pub(super) fn noted_pend(&mut self, t: V, line: usize) -> R<V> {
        let tid = self.tid(&t.ty)?;
        self.names_add(NameSet::InertC, t.c.clone());
        if self.stmt_temp_drop(&t.c, tid, true) {
            return Ok(t);
        }
        let o = self.materialize(&t, line)?;
        self.names_drop(NameSet::InertC, &t.c);
        Ok(V { obj: o, ..t })
    }

    // D-0315 (step 4): `e` evaluates nothing that could fault or run a
    // destructor: a literal, a unit variant, an empty container, a plain
    // local, a struct literal of such.
    pub(super) fn harmless(&self, e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) => true,
            ExprKind::Path(segs, _) if segs.len() == 1 => match self.lookup(&segs[0]) {
                Some(b) => !self.is_res(&b.ty) && !self.has_refs(&b.ty) && !self.moved_flag.contains_key(&b.c),
                None => self.literal_expr(e),
            },
            ExprKind::Path(..) => self.literal_expr(e),
            ExprKind::Call(c, a) => a.is_empty() && self.empty_ctor(c),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, x)| self.harmless(x)),
            _ => false,
        }
    }

    // The innermost scope or frame, left by a jump already emitted (its
    // pop is part of the jump's unwind): its extent in the text ends here.
    pub(super) fn end_diverged(&mut self) {
        // A jump's unwind already ran the drops above this frame.
        loop {
            while matches!(self.cleanup.last(), Some(Cleanup::Drop(_))) {
                self.cleanup.pop();
            }
            // D-0315 (step 4): the drops below the root frame were the last.
            if self.cleanup.is_empty() {
                return;
            }
            let (Some(Cleanup::Frame(id)) | Some(Cleanup::Stmt(id))) = self.cleanup.pop() else { panic!("cobc: no scope to end") };
            self.scope_marker('e', id);
            // A noted value's sub-frame ends with the frame below it.
            if !self.sub_frames.contains(&id) {
                break;
            }
        }
    }

    // Emits the pops that leave every scope and frame above `depth`,
    // without changing the lowering's own stack (the jump leaves them).
    pub(super) fn emit_unwind_to(&mut self, depth: usize) {
        for k in self.cleanup[depth..].to_vec().into_iter().rev() {
            match k {
                Cleanup::Frame(id) => self.scope_marker('g', id),
                Cleanup::Stmt(id) => self.scope_marker('t', id),
                Cleanup::Drop(k) => {
                    let line = self.drop_lines[k as usize].clone();
                    self.emit(&line);
                }
            }
        }
    }

    // ---- values ----

    // A value in a named C local; a reference as a `cb_ref`; a value
    // that needs an object gets one (or keeps the one it has).
    pub(super) fn into_temp(&mut self, v: V, line: usize) -> R<V> {
        if matches!(v.ty, Type::Never) {
            return Ok(v);
        }
        if matches!(v.ty, Type::Void) {
            return Ok(v);
        }
        if matches!(v.ty, Type::Ref(..)) {
            let t = self.g.fresh("r");
            self.emit(&format!("cb_ref {} = {};", t, v.c));
            return Ok(V { c: t, ty: v.ty, obj: None });
        }
        if v.obj.is_some() && self.obj_temps.contains(&v.c) {
            return Ok(v);
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&v.ty)?;
        self.emit(&format!("{} {} = {};", c, t, v.c));
        if v.obj.is_none() && self.inert_c.contains(&v.c) {
            self.names_add(NameSet::InertC, t.clone());
            return Ok(V { c: t, ty: v.ty, obj: None });
        }
        let obj = if self.is_res(&v.ty) {
            let Some(o) = v.obj else { return internal("a resource value without an object") };
            self.emit(&format!("cb_move_to({}, &{});", o, t));
            Some(o)
        } else if self.has_refs(&v.ty) {
            let tid = self.tid(&v.ty)?;
            match v.obj {
                Some(o) => {
                    self.emit(&format!("cb_move_to({}, &{});", o, t));
                    Some(o)
                }
                None if self.plain_slice(&v.ty) => None,
                None => {
                    let loc = self.loc(line);
                    self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", t, v.c, tid));
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                    Some(o)
                }
            }
        } else {
            None
        };
        if obj.is_some() {
            self.names_add(NameSet::ObjTemps, t.clone());
        }
        Ok(V { c: t, ty: v.ty, obj })
    }

    // Stores a materialized value into an lvalue of the same type: the
    // C store plus the reference data. Object handling is the caller's.
    pub(super) fn store(&mut self, dst: &str, ty: &Type, v: &V) -> R<()> {
        if matches!(ty, Type::Void | Type::Never) {
            return Ok(());
        }
        if let Type::Ref(inner, m) = ty {
            // D-0292: the reference's place goes with it: a light one
            // becomes a holder here.
            let tid = self.tid(inner)?;
            let md = if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            self.emit(&format!("{} = ({}).p;", dst, v.c));
            self.emit(&format!("cb_hold_ref(&{}, ({}).tok, ({}).p, {}u, {}, CB_NOLOC, 0);", dst, v.c, v.c, tid, md));
            return Ok(());
        }
        self.emit(&format!("{} = {};", dst, v.c));
        if self.has_refs(ty) && v.obj.is_none() {
            let tid = self.tid(ty)?;
            self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", dst, v.c, tid));
        } else if self.has_refs(ty) {
            let tid = self.tid(ty)?;
            self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", dst, v.c, tid));
        }
        Ok(())
    }

    // A value becomes a binding's initial contents: the binding's
    // object is the moved resource, or a fresh object at its address.
    // Stage 3 (`COBC-PLAN.md` §10, T1): `rule.control.flow-analysis`
    // proves every check on a local of a plain type (no resource, no
    // reference inside, not a callable) that the function never borrows,
    // captures or drops: `valid(x)` holds while it is in scope (a plain
    // value is never consumed), `init(x)` is definite assignment's own
    // static guarantee, and with `escaped(x)` and every `deriv(·, x, ·)`
    // false, `¬clash` is proven. Proven checks are not performed
    // (`spec/14` §6 "Discharge"), and such a local needs no runtime
    // object at all.
    pub(super) fn unchecked(&self, name: &str, ty: &Type) -> bool {
        // D-0315: a slice of a decided program is plain data (`plain_slice`).
        !self.is_res(ty) && (!self.has_refs(ty) || self.plain_slice(ty)) && !self.holds_fn(ty) && !matches!(ty, Type::Closure(_)) && !self.pinned_ws.contains(name)
    }

    // The same proof for a local that holds references (a reference, or
    // plain data with one inside): a reference is never consumed, and
    // nothing else reaches the variable's own storage. Its runtime
    // object stays — the slots it holds are what make its references
    // held — but accesses to the variable itself are not checked.
    pub(super) fn unchecked_holder(&self, name: &str, ty: &Type) -> bool {
        !self.is_res(ty) && self.has_refs(ty) && !self.holds_fn(ty) && !matches!(ty, Type::Closure(_)) && !self.pinned.contains(name)
    }

    // An objectless local `cname` is bound (`bind_value`, the `String`
    // copy's `_raw` form): no object; its drop where the binding ends; a
    // flag for one the body may move away (`objectless_strings`' marker,
    // `!id`), which that drop and every later use test.
    pub(super) fn objectless_bound(&mut self, cname: &str, tid: u32) {
        self.names_add(NameSet::ObjectlessC, cname.to_string());
        let moved = self.cur_let.map_or(false, |id| self.objectless_names.contains(&!id));
        let drop = self.objectless_drop(cname, tid);
        // D-0269: the local's entry in the body's table (`notes_finish`):
        // live from here, until its drop has run.
        let k = self.notes;
        self.notes += 1;
        // D-0315 (step 4): a noted value has its own registration, at the
        // depth where it is bound, popped by its drop; a sub-frame follows
        // its statement (`push_sub_frame`).
        let s1 = std::mem::take(&mut self.s1_now);
        // (A noted value's entry and registration end first: its destructor
        // may fault, and the unwind must not run it again.)
        if moved {
            let fl = self.g.fresh("mv");
            self.emit(&format!("uint8_t {} = 0;", fl));
            self.emit(&format!("cb_nt[{}] = (struct cb_note){{ (void *)&{}, &{}, {}u, 1 }};", k, cname, fl, tid));
            if s1 {
                self.push_drop(format!("cb_nt[{}].live = 0; cb_notes_pop(); if (!{}) {{ {} }}", k, fl, drop));
            } else {
                self.push_drop(format!("if (!{}) {{ {} }} cb_nt[{}].live = 0;", fl, drop, k));
            }
            self.names_put(NameMap::MovedFlag, cname.to_string(), fl);
        } else {
            self.emit(&format!("cb_nt[{}] = (struct cb_note){{ (void *)&{}, 0, {}u, 1 }};", k, cname, tid));
            if s1 {
                self.push_drop(format!("cb_nt[{}].live = 0; cb_notes_pop(); {}", k, drop));
            } else {
                self.push_drop(format!("{} cb_nt[{}].live = 0;", drop, k));
            }
        }
        if s1 {
            self.notes_always = true;
            self.emit(&format!("cb_notes_push(&cb_nt[{}], 1u);", k));
            self.pending_sub = true;
        }
    }

    // D-0269: the body is lowered. Its objectless locals' table (`cb_nt`,
    // one entry per `objectless_bound`), declared at the body's start and
    // registered with the runtime while threads run (`cb_threaded`), so
    // that a contained fault's unwind (`[Fault-Contain]`) destroys the
    // locals the body's own statements would have, which the unwind's
    // jump leaves unrun. Every exit of the root frame pops it, under the
    // same flag read at entry (the first spawn may fall between). Nothing
    // where the body bound no objectless local.
    pub(super) fn notes_finish(&mut self) -> String {
        if self.notes == 0 {
            return String::new();
        }
        let root = self.root_frame;
        let mut out = String::with_capacity(self.out.len() + 256);
        for line in self.out.lines() {
            if let Some(m) = line.trim_start().strip_prefix('\u{1}') {
                let kind = m.chars().next();
                let id: Option<u32> = m.get(1..).and_then(|s| s.split('\u{2}').next()).and_then(|s| s.parse().ok());
                // D-0315 (step 4): a table registered on every call is popped
                // by its own entry below the root frame (`push_drop_under_root`).
                if matches!(kind, Some('f' | 'g')) && id == Some(root) && !self.notes_bottom {
                    out.push_str(&line[..line.len() - line.trim_start().len()]);
                    out.push_str("if (cb_nt_on) cb_notes_pop();\n");
                }
            }
            out.push_str(line);
            out.push('\n');
        }
        self.out = out;
        // D-0315 (step 4): on every call where a noted value's end is one a
        // fault must run and could observe (`notes_always`).
        let on = if self.notes_always { "1" } else { "cb_threaded" };
        // CHG-0440: the table is cleared only when it is registered. An
        // entry is written before it is read (`cb_notes_push` of one entry
        // follows its write), and an unregistered table is read by no one, so
        // a call in a thread-free run zeroes nothing (it was the self-hosted
        // compiler's largest source of cache misses).
        let mut p = format!("    struct cb_note cb_nt[{n}];\n    uint8_t cb_nt_on = {on};\n    if (cb_nt_on) cb_notes_push(memset(cb_nt, 0, sizeof cb_nt), {n}u);\n", n = self.notes, on = on);
        // D-0315: the statement temporaries' flags (`stmt_temp_drop`).
        if self.tl > 0 {
            p.push_str(&format!("    uint8_t cb_tl[{}] = {{0}};\n", self.tl));
        }
        p
    }

    // D-0200: an objectless local's drop: for a `String` or a `Vec` of plain
    // elements, its buffer freed at once where no object can lie over it
    // (`cb_drop_plain_buf`), else `cb_drop_local`'s general path.
    pub(super) fn objectless_drop(&mut self, cname: &str, tid: u32) -> String {
        let ty = self.g.type_ids.iter().find(|(_, &i)| i == tid).map(|(t, _)| t.clone());
        match ty {
            Some(Type::Named(n, a)) if n == "std::String" && a.is_empty() => {
                format!("cb_drop_plain_buf(&{c}, {t}u, (void *){c}.bytes.ptr, {c}.bytes.cap);", c = cname, t = tid)
            }
            Some(Type::Named(n, a)) if n == "std::Vec" && a.len() == 1 && self.g.plain_data(&a[0]) => match self.g.size_align(&a[0]) {
                Ok((sz, al)) if sz > 0 && al <= 16 => format!("cb_drop_plain_buf(&{c}, {t}u, (void *){c}.ptr, {c}.cap * {s}u);", c = cname, t = tid, s = sz),
                _ => format!("cb_drop_local(&{}, {}u);", cname, tid),
            },
            // D-0315: its drop glue (`Gen::drop_glue`), which asks of each
            // buffer whether an object lies over it (CHG-0427).
            Some(t) => match self.g.drop_glue(&t) {
                Ok(Some(g)) => format!("{g}(&{c}, 0u);", c = cname, g = g),
                _ => format!("cb_drop_local(&{}, {}u);", cname, tid),
            },
            None => format!("cb_drop_local(&{}, {}u);", cname, tid),
        }
    }

    // A use of the objectless local `cname` (a borrow, a read, a move):
    // stale once moved away (`[Binding-Lookup-Stale]`), as its object
    // would have been.
    pub(super) fn objectless_use(&mut self, cname: &str, line: usize) {
        if let Some(fl) = self.moved_flag.get(cname).cloned() {
            let loc = self.loc(line);
            self.emit(&format!("if ({}) cb_fault(\"diag.stale-binding\", {});", fl, loc));
        }
    }

    pub(super) fn bind_value(&mut self, name: &str, cname: String, ty: &Type, v: &V, line: usize) -> R<()> {
        // A new binding of the name: whatever `confined_params` recorded for
        // an earlier one (a `foreach` holder registered by `foreach_alias`,
        // perhaps in a probe pass, under that pass's C name) is not this one.
        self.confined_params.remove(name);
        if matches!(ty, Type::Ref(..)) && self.held_locals.contains(name) && !self.unchecked(name, ty) {
            // A reference local never assigned, borrowed or captured: the
            // frame holds its path (`cb_frame_hold`) as its binding object
            // would, and its uses go through that path's token.
            let r = self.into_temp(v.clone(), line)?;
            let tk = self.g.fresh("tk");
            self.emit(&format!("{} = ({}).p;", cname, r.c));
            self.emit(&format!("uint64_t {} = ({}).tok;", tk, r.c));
            self.emit(&format!("cb_frame_hold({});", tk));
            self.names_put(NameMap::Unstored, cname.clone(), tk);
            self.names_add(NameSet::HeldC, cname.clone());
            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        // A plain value holding references that arrives with an object of
        // its own (a temporary `into_temp` made, a call's received result):
        // the binding takes that object, as it takes a resource's, rather
        // than copying the value's references into a second object while
        // the first ends with the statement. The temporary is fresh and
        // nothing holds a path to it, so only which record names the
        // binding's storage differs, and that is unobservable (D-0023).
        // Only for a `let` (`cur_let`): a pattern binder's value may be read
        // again (a whole-value binder in each arm of a `match`).
        let adopt = self.cur_let.is_some() && !self.is_res(ty) && self.has_refs(ty) && !matches!(ty, Type::Ref(..)) && v.obj.is_some() && v.ty == *ty && !self.unchecked(name, ty);
        if adopt {
            self.emit(&format!("{} = {};", cname, v.c));
        } else {
            self.store(&cname, ty, v)?;
        }
        if self.unchecked(name, ty) {
            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        // D-0315: pinned plain data of a decided program, or a reference
        // there (its token is 0 and no path is kept): root token 0.
        if v.obj.is_none() && (self.pinned_plain(ty) || self.decided_ref(ty)) {
            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: "0".to_string(), kind: BindKind::Local });
            return Ok(());
        }
        let loc = self.loc(line);
        let tid = self.tid(ty)?;
        let o = self.g.fresh("o");
        // An objectless `String` (`Gen::objectless_strings`): the received
        // object's record ends, the local owns the value, its drop is made
        // where the binding ends; every use is a reader's, handed the
        // address with token 0 (`reader_passthrough`).
        let objectless_ok = self.cur_let.map_or(false, |id| self.objectless_binding(id, ty));
        if objectless_ok && std::env::var("COBC_DEBUG_OBJECTLESS").is_ok() {
            eprintln!("OBJECTLESS {} : {:?} (line {})", name, ty, line);
        }
        // D-0315 (step 4): a noted value (`noted_type`), not memory alone.
        self.s1_now = objectless_ok && self.is_res(ty) && !self.g.objectless_type(ty) && !matches!(ty, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if objectless_ok {
            if let Some(src) = &v.obj {
                self.emit(&format!("cb_forget_obj({});", src));
                self.objectless_bound(&cname, tid);
                self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
                return Ok(());
            }
            if self.inert_c.contains(&v.c) {
                // A `__raw` call's result (`finish_call`): nothing to forget.
                self.objectless_bound(&cname, tid);
                self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
                return Ok(());
            }
        }
        // D-0315: an inert value (`inert_c`) bound with an object is given it here.
        let v = match self.materialize(&v, line)? {
            Some(o) if v.obj.is_none() => V { obj: Some(o), ..v.clone() },
            _ => v.clone(),
        };
        if self.is_res(ty) || adopt {
            let Some(src) = &v.obj else { return internal(&format!("a resource binding without an object (`{}`, line {})", name, line)) };
            self.emit(&format!("cb_move_to({}, &{});", src, cname));
            self.emit(&format!("uint64_t {} = {};", o, src));
        } else {
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
        }
        let root = self.g.fresh("root");
        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
        let root = if self.unchecked_holder(name, ty) {
            self.holder_roots.insert(cname.clone(), root);
            String::new()
        } else {
            root
        };
        self.note_frame(name, &root);
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root, kind: BindKind::Local });
        Ok(())
    }

    // Whether the `let` whose initializer is `id` binds a `ty` objectless
    // (`Gen::objectless_strings` found it a candidate, and its uses suit
    // the type). The binding and the raw call that feeds it ask alike.
    pub(super) fn objectless_binding(&self, id: usize, ty: &Type) -> bool {
        let is_string = matches!(ty, Type::Named(n, a) if n == "std::String" && a.is_empty());
        // A candidate compared (`id | 1`) only as a `String`: other types'
        // comparisons read the value where it is tracked.
        // Indexed (`id | 2`) only as a `Vec` of plain data, whose element
        // accesses are only bounds-checked.
        // D-0315: in a decided program, also one whose elements' end frees
        // memory alone (`x[i]` is `*Vec::index_*(&x, i)`, `&x` its address).
        let plain_vec = matches!(ty, Type::Named(n, a) if n == "std::Vec" && a.len() == 1 && ((self.g.plain_data(&a[0]) && !self.has_refs(&a[0]) && !self.holds_fn(&a[0])) || (self.g.decided && self.g.objectless_type(&a[0]))));
        // D-0286: a field read by value is plain data (no field is moved out).
        // D-0294: a field written (`=f`) is plain data or of a quiet type,
        // whose old value the write destroys (`lower_assign`).
        // D-0315: by each entry's whole path (`Fx::field_entry_ok`).
        let fields_plain = self.objectless_fields.get(&id).map_or(true, |fs| fs.iter().all(|f| self.field_entry_ok(ty, f)));
        // D-0294: assigned whole (`id | 4`) only as a quiet type.
        let assigned_ok = !self.objectless_names.contains(&(id | 4)) || coby::typecheck::quiet_ty(&self.g.items, ty);
        let ok = self.objectless_names.contains(&id)
            && fields_plain
            && assigned_ok
            && (!self.objectless_names.contains(&(id | 2)) || plain_vec)
            && (is_string || ((self.g.objectless_type(ty) || (self.s1_let == Some(id) && self.g.noted_type(ty))) && !self.objectless_names.contains(&(id | 1))));
        if !ok && std::env::var_os("COBC_DEBUG_W").is_some() && self.g.is_resource(ty) {
            let why = if !self.objectless_names.contains(&id) { "bad" } else if !fields_plain { "fields" } else if !assigned_ok { "assigned" } else if self.objectless_names.contains(&(id | 2)) && !plain_vec { "indexed" } else if !is_string && !self.g.objectless_type(ty) { "type" } else { "compared" };
            eprintln!("OB reject id {} {} {}{}", id, why, super::cnames::mangle(ty), if why == "fields" { format!(" {:?}", self.objectless_fields.get(&id)) } else { String::new() });
        }
        ok
    }

    // D-0315: an entry of `objectless_fields` on a value of `ty` (`=f` a field
    // written, `~p` a path compared, `p` a path read: `f`, `f.g`, `f[].g`, a
    // `Vec` field's element by `[]`): allowed when what it reaches is plain
    // data (or plain in a decided program), a quiet type written (its old
    // value destroyed by the write), or a `String` compared (read in place).
    pub(super) fn field_entry_ok(&self, ty: &Type, f: &str) -> bool {
        // `*whole`: matched by an arm binding the whole value; an enum's.
        if f == "*whole" {
            return matches!(ty, Type::Named(n, _) if self.g.enum_decl(n).is_some());
        }
        let (written, compared, path) = match (f.strip_prefix('='), f.strip_prefix('~')) {
            (Some(p), _) => (true, false, p),
            (_, Some(p)) => (false, true, p),
            _ => (false, false, f),
        };
        let mut t = ty.clone();
        for seg in path.split('.') {
            let cut = seg.find('[').unwrap_or(seg.len());
            let (name, rest) = seg.split_at(cut);
            let Type::Named(n, a) = &t else { return false };
            let Some(sd) = self.g.struct_decl(n) else { return false };
            let sub = subst_of(&sd.type_params, a);
            let Some(fd) = sd.fields.iter().find(|fd| fd.name == name) else { return false };
            t = apply(&fd.ty, &sub);
            for _ in 0..rest.matches("[]").count() {
                match &t {
                    Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => t = a[0].clone(),
                    _ => return false,
                }
            }
        }
        let plain = (self.g.plain_data(&t) && !self.has_refs(&t) && !self.holds_fn(&t)) || (self.g.decided && self.g.decided_plain(&t));
        plain || (written && coby::typecheck::quiet_ty(&self.g.items, &t)) || (compared && matches!(&t, Type::Named(n, a) if n == "std::String" && a.is_empty()))
    }

    // `let s = String::copy_str(x);` (or `String::clone(r)`) for an
    // objectless `s` (`Gen::objectless_strings`): the native copy's `_raw`
    // variant gives the value with no object (`lower_map_fresh_key`), and
    // the binding is objectless from the start (`bind_value`'s case).
    // `Some(diverged)` when done here.
    pub(super) fn bind_fresh_objectless(&mut self, name: &str, cname: &str, declared: Option<&Type>, e: &Expr) -> R<Option<bool>> {
        // D-0274: no `_raw` variants in the bootstrap form (no native bodies).
        if self.g.boot(5) {
            return Ok(None);
        }
        // D-0200: `String::new()` / `Vec::new()` into an objectless local:
        // the empty value in place, with no object to make and forget.
        if self.objectless_names.contains(&(e as *const Expr as usize)) {
            if let ExprKind::Call(callee, cargs) = &strip_parens(e).kind {
                if let ExprKind::Path(segs, targs) = &callee.kind {
                    let k = segs.join("::");
                    let k = if self.g.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
                    if cargs.is_empty() && targs.is_empty() && matches!(k.as_str(), "std::String::new" | "std::Vec::new") && self.g.items.fns.get(&k).map_or(false, |pf| in_prelude(pf)) {
                        let ty = match (k.as_str(), declared) {
                            ("std::String::new", _) => Some(Type::Named("std::String".to_string(), vec![])),
                            (_, Some(t @ Type::Named(n, a))) if n == "std::Vec" && a.len() == 1 => Some(t.clone()),
                            _ => None,
                        };
                        if let Some(ty) = ty.filter(|t| declared.map_or(true, |d| d == t) && self.g.objectless_type(t)) {
                            let c = self.g.ctype(&ty)?;
                            let tid = self.tid(&ty)?;
                            let init = self.g.empty_value_init(&ty)?;
                            self.emit(&format!("{} {};", c, cname));
                            self.emit(&init.replace("@", cname));
                            let saved = self.cur_let.replace(e as *const Expr as usize);
                            self.objectless_bound(cname, tid);
                            self.cur_let = saved;
                            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname.to_string(), ty: ty.clone(), root: String::new(), kind: BindKind::Local });
                            // A confined `Vec` (`bind_confined`) stays so.
                            self.bind_confined(name, cname, &ty)?;
                            return Ok(Some(false));
                        }
                    }
                }
            }
        }
        let string = Type::Named("std::String".to_string(), vec![]);
        if !self.objectless_names.contains(&(e as *const Expr as usize)) || declared.map_or(false, |t| *t != string) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(e).kind else { return Ok(None) };
        let ExprKind::Path(segs, _) = &callee.kind else { return Ok(None) };
        let prod = segs.join("::");
        if !matches!(prod.as_str(), "std::String::copy_str" | "std::String::clone") || cargs.len() != 1 {
            return Ok(None);
        }
        if !self.g.items.fns.get(&prod).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        let a = self.lower_expr(&cargs[0], if prod == "std::String::copy_str" { Some(&Type::Str) } else { None })?;
        if matches!(a.ty, Type::Never) {
            return Ok(Some(true));
        }
        let a = self.into_temp(a, e.line)?;
        let pname = self.g.request_fn(&prod, Vec::new())?;
        let sc = self.g.ctype(&string)?;
        let tid = self.tid(&string)?;
        self.emit(&format!("{} {} = {}_raw({});", sc, cname, pname, a.c));
        let saved = self.cur_let.replace(e as *const Expr as usize);
        self.objectless_bound(cname, tid);
        self.cur_let = saved;
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname.to_string(), ty: string, root: String::new(), kind: BindKind::Local });
        Ok(Some(false))
    }

    // A binding of a confined name (`Gen::confined_vecs`) whose type is
    // `Vec<T>` for a plain, non-zero-sized `T` is confined. Its borrows are
    // unheld and end with their calls (`Vec::push` keeps only `T`,
    // `Vec::len` returns a number, an element reference is dereferenced
    // at once), it is never moved but as the function's result, and it
    // is initialized where it is declared; so no path but its root ever
    // holds it, and neither `[Borrow]`'s check nor the checked reads and
    // writes in `push`, `len` and `index_*` can fail on it. No element of
    // it ever has an object: elements are reached only through these
    // calls, which establish none (`cb_elem_access`'s first case), and
    // `grow` only copies bytes and releases the old buffer. So none of
    // those checks is made (`confined_local`).
    pub(super) fn bind_confined(&mut self, name: &str, cname: &str, ty: &Type) -> R<()> {
        if !self.confined_names.contains(name) {
            return Ok(());
        }
        let Type::Named(n, a) = ty else { return Ok(()) };
        let [elem] = a.as_slice() else { return Ok(()) };
        if n != "std::Vec" || self.g.size_align(elem)?.0 == 0 || !self.g.plain_data(elem) {
            return Ok(());
        }
        // D-0200: or an objectless one (`objectless_c`), whose checks are
        // proven anyway; confined, its elements are reached with the bounds
        // check alone rather than through `Vec::index_*`.
        if self.lookup(name).map_or(false, |b| !b.root.is_empty() || self.objectless_c.contains(&b.c)) {
            self.names_add(NameSet::Confined, cname.to_string());
        }
        Ok(())
    }

    // `&x`, `&x.f…` or `&(*p).f…` (`p` a reference), a `Vec` of plain,
    // non-zero-sized elements, passed to a shared confining parameter (`j`)
    // and named by no other argument: the borrow formed for the call
    // checks that no exclusive path to the vector is held (one to an
    // element is on the element's own object: the call tests for those at
    // run time and takes the checked variant, CHG-0237), and none can be
    // formed while the call runs (its caller is
    // suspended, the callee only reads through the parameter, and any
    // other path that could write was exclusive and would have clashed),
    // so the variant for a confined argument (`confining_table`) reads it
    // unchecked. Its path is the borrow's.
    pub(super) fn fresh_shared_vec_arg(&mut self, a: &Expr, j: usize, args: &[Expr]) -> R<bool> {
        let ExprKind::Borrow(Mode::Shared, inner) = &a.kind else { return Ok(false) };
        let mut root: &Expr = inner;
        while let ExprKind::Field(b, _) | ExprKind::Paren(b) = &root.kind {
            root = b;
        }
        if let ExprKind::Deref(p) = &root.kind {
            root = strip_parens(p);
        }
        let ExprKind::Path(segs, targs) = &root.kind else { return Ok(false) };
        if segs.len() != 1 || !targs.is_empty() || self.lookup(&segs[0]).is_none() {
            return Ok(false);
        }
        if args.iter().enumerate().any(|(k, b)| k != j && mentions(b, &segs[0])) {
            return Ok(false);
        }
        let ty = self.probe_type(inner, None)?;
        let Type::Named(n, el) = &ty else { return Ok(false) };
        if n != "std::Vec" || el.len() != 1 || !self.g.plain_data(&el[0]) || self.g.size_align(&el[0])?.0 == 0 {
            return Ok(false);
        }
        Ok(true)
    }

    // A confined vector passed as a `Vec` function's reference argument:
    // `&x` / `&mut x` for a confined local `x`, or `p` / `&*p` / `&mut *p`
    // for a confined parameter `p` of this variant. The vector as a C
    // lvalue, the path to hand on (the local's root; the parameter's
    // own), and the element type.
    pub(super) fn confined_local(&self, e: &Expr) -> Option<(String, String, Type)> {
        let name = |e: &Expr| match &e.kind {
            ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(segs[0].clone()),
            _ => None,
        };
        match &e.kind {
            ExprKind::Borrow(_, inner) if !self.field_confined.is_empty() && matches!(strip_parens(inner).kind, ExprKind::Field(..)) => {
                let ExprKind::Field(base, f) = &strip_parens(inner).kind else { return None };
                let b = match &strip_parens(base).kind {
                    ExprKind::Deref(p) => name(strip_parens(p))?,
                    _ => name(strip_parens(base))?,
                };
                let (c, elem) = self.field_confined.get(&(b, f.clone()))?;
                Some((c.clone(), "0".to_string(), elem.clone()))
            }
            ExprKind::Borrow(_, inner) => {
                if let ExprKind::Deref(p) = &inner.kind {
                    return self.confined_params.get(&name(p)?).cloned();
                }
                let b = self.lookup(&name(inner)?)?;
                if !self.confined.contains(&b.c) {
                    return None;
                }
                let Type::Named(_, a) = &b.ty else { return None };
                // D-0200: an objectless confined `Vec` has no root: token 0.
                let root = if b.root.is_empty() { "0".to_string() } else { b.root.clone() };
                Some((b.c.clone(), root, a.first()?.clone()))
            }
            ExprKind::Path(..) => self.confined_params.get(&name(e)?).cloned(),
            _ => None,
        }
    }

    // `&h.f` for a local struct `h` whose `Vec` field `f` of plain,
    // non-zero-sized elements `confined_fields` found reached only as
    // `h.f[i]` (read, or written with nothing pushed meanwhile) and
    // `Vec::len(&h.f)`: no path but `h`'s root ever reaches `h.f` or an
    // element of it, as for a confined local (`bind_confined`), so an
    // element access needs its bounds check alone and `len` no check.
    // The vector as a C lvalue, no path (none is ever handed on: the
    // analysis admits no call that would take one), and the element type.
    pub(super) fn confined_field(&self, e: &Expr) -> Option<(String, String, Type)> {
        let ExprKind::Borrow(_, inner) = &strip_parens(e).kind else { return None };
        let ExprKind::Field(base, f) = &strip_parens(inner).kind else { return None };
        let h = path_name(base)?;
        if !self.confined_fields.contains(&(h.to_string(), f.clone())) {
            return None;
        }
        let b = self.lookup(h)?;
        if !matches!(b.kind, BindKind::Local) || b.root.is_empty() {
            return None;
        }
        let Type::Named(sn, sa) = &b.ty else { return None };
        let sd = self.g.struct_decl(sn)?;
        let sub = subst_of(&sd.type_params, sa);
        let fd = sd.fields.iter().find(|x| &x.name == f)?;
        let Type::Named(vn, va) = apply(&fd.ty, &sub) else { return None };
        let [elem] = va.as_slice() else { return None };
        if vn != "std::Vec" || !self.g.plain_data(elem) || self.g.size_align(elem).map_or(true, |(n, _)| n == 0) {
            return None;
        }
        Some((format!("{}.{}", b.c, san(f)), String::new(), elem.clone()))
    }

    // `&x` / `&mut x` for a local `x` that `read_confined_vecs` found no
    // exclusive path can ever be held to (a `Vec` of plain, non-zero-sized
    // elements): a read of an element, or of the length, cannot clash with
    // any path -- the only ones held are shared, and an exclusive one
    // (`push`'s) lives within its own statement -- so it needs its bounds
    // check alone. Writes and calls that change the vector are not served
    // (`elem_access_place`, `lower_confined_call` ask for reads only).
    pub(super) fn read_confined_vec(&self, e: &Expr) -> Option<(String, Type)> {
        let ExprKind::Borrow(_, inner) = &strip_parens(e).kind else { return None };
        let x = path_name(inner)?;
        if !self.read_confined.contains(x) {
            return None;
        }
        let b = self.lookup(x)?;
        if !matches!(b.kind, BindKind::Local) || b.root.is_empty() {
            return None;
        }
        let Type::Named(n, a) = &b.ty else { return None };
        let [elem] = a.as_slice() else { return None };
        if n != "std::Vec" || !self.g.plain_data(elem) || self.g.size_align(elem).map_or(true, |(n, _)| n == 0) {
            return None;
        }
        Some((b.c.clone(), elem.clone()))
    }

    // CHG-0237: `x` (or `&*x`) for a parameter `x : ref<Vec<T>, shared>`
    // that is never written, borrowed or captured (`unstored_params`): the
    // vector it refers to (a C lvalue) and `T`, sized. While the call runs
    // the parameter's path is held, so no exclusive path to the vector or
    // any element of it can exist, and its buffer cannot move: a read of
    // an element needs its bounds check alone.
    pub(super) fn frozen_vec(&mut self, e: &Expr) -> Option<(String, Type)> {
        if !self.frozen_on {
            return None;
        }
        let e = strip_parens(e);
        let name = match &e.kind {
            ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => &s[0],
            ExprKind::Borrow(Mode::Shared, x) => match &strip_parens(x).kind {
                ExprKind::Deref(y) => match &strip_parens(y).kind {
                    ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() => &s[0],
                    _ => return None,
                },
                _ => return None,
            },
            _ => return None,
        };
        let b = self.lookup(name)?;
        if !self.unstored.contains_key(&b.c) {
            return None;
        }
        let Type::Ref(inner, Mode::Shared) = &b.ty else { return None };
        let Type::Named(n, a) = &**inner else { return None };
        let [elem] = a.as_slice() else { return None };
        if n != "std::Vec" || self.g.size_align(elem).map_or(true, |(n, _)| n == 0) {
            return None;
        }
        let vc = self.g.ctype(inner).ok()?;
        Some((format!("(*({} *){})", vc, b.c), elem.clone()))
    }

    // CHG-0237: `x.f…` (or `(*x).f…`), one field or more, for `x` a shared
    // reference parameter that is never written, borrowed or captured
    // (`unstored_params`).
    pub(super) fn frozen_root(&self, e: &Expr) -> bool {
        if !self.frozen_on {
            return false;
        }
        let mut at = strip_parens(e);
        let mut fields = 0;
        loop {
            match &at.kind {
                ExprKind::Field(x, _) => {
                    fields += 1;
                    at = strip_parens(x);
                }
                // Only the parameter itself is dereferenced (`(*x).f`): no
                // box or stored reference inside the chain.
                ExprKind::Deref(x) if fields > 0 && matches!(&strip_parens(x).kind, ExprKind::Path(..)) => at = strip_parens(x),
                ExprKind::Path(s, t) if s.len() == 1 && t.is_empty() && fields > 0 => {
                    return self.lookup(&s[0]).map_or(false, |b| self.unstored.contains_key(&b.c) && matches!(b.ty, Type::Ref(_, Mode::Shared)));
                }
                _ => return false,
            }
        }
    }

    // An element access's vector: a confined local's or a confined field's.
    pub(super) fn confined_elem_vec(&self, e: &Expr) -> Option<(String, String, Type)> {
        self.confined_local(e).or_else(|| self.confined_field(e))
    }

    // A direct slice's elements are read and written without the slice's
    // own check, but an element may have an object whose paths an access
    // still meets (a `Vec`'s element borrowed and held, `spec/08` §4): one
    // `cb_live_in` over them at entry decides whether its accesses ask.
    // Nothing the body does can give one of them an object: a direct
    // slice is handed to nothing.
    // D-0193: a local shared slice the body never assigns, borrows,
    // captures or drops (`pinned`, `assigned`): the path it holds stands
    // until its scope ends, since anything that would end it or write what
    // it covers clashes with it at that act (`spec/08` §4), so its element
    // reads go unchecked, as a pure slice parameter's do, meeting element
    // objects only where its entry test found one (`bind_direct_live`).
    pub(super) fn bind_reader_slice(&mut self, name: &str, c: &str, ty: &Type) -> R<()> {
        // D-0274: not in the bootstrap form.
        if self.g.boot(7) {
            return Ok(());
        }
        // Not a hidden binding (`$…`, a `foreach`'s): its uses are expanded
        // only as it is lowered (`each.rs`), so `pinned` never saw them.
        if name.starts_with('$') || !matches!(ty, Type::Slice(_, Mode::Shared)) || self.pinned.contains(name) || self.assigned.contains(name) || self.self_ref.is_some() {
            return Ok(());
        }
        if self.lookup(name).map_or(true, |b| b.c != c) {
            return Ok(());
        }
        let was = self.fast_half;
        self.fast_half = false;
        let r = self.bind_direct_live(c, ty);
        self.fast_half = was;
        r?;
        self.names_add(NameSet::DirectRefs, c.to_string());
        Ok(())
    }

    pub(super) fn bind_direct_live(&mut self, pc: &str, ty: &Type) -> R<()> {
        let Type::Slice(elem, _) = ty else { return Ok(()) };
        let size = self.g.size_align(elem)?.0;
        let flag = format!("lv_{}", pc);
        if self.fast_half {
            self.emit(&format!("const uint8_t {} = 0;", flag));
        } else {
            self.emit(&format!("uint8_t {} = cb_live_in((void *)({}).data, ({}).len * {}u);", flag, pc, pc, size));
        }
        self.direct_live.insert(pc.to_string(), flag);
        Ok(())
    }

    // An element of a slice reached for reading or writing (`place_write`):
    // the slice's own path is checked as before, and the element's object,
    // if it has one, is met as `Vec` indexing meets it (`cb_elem_access`):
    // a path held on one element is not on the slice's object, and only
    // the access finds it (`[Read-Conflict]`, as the interpreter). For a
    // direct slice, only where its entry flag found such an object.
    pub(super) fn slice_elem_access(&mut self, sc: &str, idx: &str, elem: &Type, loc: String) -> R<()> {
        let tid = self.tid(elem)?;
        let (mode, w) = if self.place_write { ("CB_EXCLUSIVE", 1) } else { ("CB_SHARED", 0) };
        let call = format!("(void *)&({}).data[{}], {}u, {}, {}u, {}", sc, idx, tid, mode, w, loc);
        match self.direct_live.get(sc) {
            Some(flag) => self.emit(&format!("cb_elem_access_if({}, {});", flag, call)),
            None => {
                let size = self.g.size_align(elem)?.0;
                self.emit(&format!("cb_elem_access((void *)&({}).data[{}], {}u, {}u, {}, {}u, {});", sc, idx, size, tid, mode, w, loc))
            }
        }
        Ok(())
    }

    pub(super) fn bind_param(&mut self, p: &Param) -> R<()> {
        let ty = self.sub(&p.ty);
        let pc = format!("p_{}", san(&p.name));
        if self.pure_views.contains(&p.name) {
            // Not received: the caller's copy holds the view's path for the
            // call (`Gen::pure_view`); only its bytes and length are read.
            self.names_add(NameSet::DirectRefs, format!("{}.bytes", pc));
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.pure_slices.contains(&p.name) {
            // Not received: the caller holds the slice's path for the call
            // (`Gen::pure_direct`); only its elements and length are used.
            self.names_add(NameSet::DirectRefs, pc.clone());
            self.bind_direct_live(&pc, &ty)?;
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.direct_params.contains(&p.name) && matches!(ty, Type::Slice(..)) {
            // A direct slice (`Gen::direct_refs_of`) is received and bound
            // as any slice -- every caller, a `fn` value's thunk too, puts
            // it in flight -- and its path held for the call; only its
            // element accesses go unchecked.
            self.names_add(NameSet::DirectRefs, pc.clone());
            self.bind_direct_live(&pc, &ty)?;
        } else if self.direct_params.contains(&p.name) {
            // A direct reference (`Gen::direct_ref_params`): the address
            // alone; no object, no stored path.
            let l = format!("l_{}", pc);
            self.emit(&format!("void *{} = {}.p;", l, pc));
            self.names_add(NameSet::DirectRefs, l.clone());
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: l, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.unstored_params.contains(&p.name) && matches!(ty, Type::Ref(..)) {
            // Its accesses go through the token it came with; the binding
            // itself is never written, borrowed or captured.
            let l = format!("l_{}", pc);
            self.emit(&format!("void *{} = {}.p;", l, pc));
            // D-0292: in a recording frame the parameter's reference is
            // recorded on arrival, held by the frame for the call.
            if self.tracked {
                if let Type::Ref(inner, m) = &ty {
                    let tid = self.tid(inner)?;
                    let md = if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    self.emit(&format!("{}.tok = cb_rec_ref({}.tok, {}.p, {}u, {}, 1u, CB_NOLOC, 0);", pc, pc, pc, tid, md));
                }
            }
            self.names_put(NameMap::Unstored, l.clone(), format!("{}.tok", pc));
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: l, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.unchecked(&p.name, &ty) {
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        // D-0315: pinned plain data of a decided program: root token 0.
        if self.pinned_plain(&ty) {
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: "0".to_string(), kind: BindKind::Local });
            return Ok(());
        }
        // D-0315 (step 4): a `fn` parameter the body only calls: no object;
        // its entry in the body's table, registered on every call, and its
        // end at every exit (a `fn` item owns nothing).
        if self.g.decided && self.callee_only.contains(&p.name) {
            let tid = self.tid(&ty)?;
            let k = self.notes;
            self.notes += 1;
            if !self.notes_bottom {
                // The table's pop: the last thing every return does.
                self.notes_always = true;
                self.notes_bottom = true;
                self.push_drop_under_root("if (cb_nt_on) cb_notes_pop();".to_string());
            }
            self.emit(&format!("cb_nt[{}] = (struct cb_note){{ (void *)&{}, 0, {}u, 1 }};", k, pc, tid));
            self.push_drop_under_root(format!("if ({c} && ((cb_fnbox *){c})->closure) cb_drop_value(&{c}, {t}u); cb_nt[{k}].live = 0;", c = pc, t = tid, k = k));
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        let tid = self.tid(&ty)?;
        let o = self.g.fresh("o");
        let cname = if let Type::Ref(inner, m) = &ty {
            let l = format!("l_{}", pc);
            let itid = self.tid(inner)?;
            let md = if *m == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            // D-0292: in a recording frame the reference is recorded on
            // arrival, held by the frame: the binding's slot and every use
            // of the parameter's own token (a `move` closure's captures are
            // read through `self`'s) see the record.
            if self.tracked {
                self.emit(&format!("{}.tok = cb_rec_ref({}.tok, {}.p, {}u, {}, 1u, CB_NOLOC, 0);", pc, pc, pc, itid, md));
            }
            self.emit(&format!("void *{} = {}.p;", l, pc));
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, l, tid, NOLOC));
            self.emit(&format!("cb_hold_ref(&{}, {}.tok, {}.p, {}u, {}, CB_NOLOC, 0);", l, pc, pc, itid, md));
            l
        } else if self.is_res(&ty) && self.g.decided {
            // D-0286: it arrives with no object. Objectless where the
            // analysis allows (a candidate by its declaration's address);
            // otherwise it takes one here.
            let id = p as *const Param as usize;
            // D-0315 (step 4): a parameter may be a noted value as a `let`'s
            // binder is (its sub-frame opens after the parameters).
            let s1_saved = self.s1_let.replace(id);
            let ok = self.objectless_binding(id, &ty);
            self.s1_let = s1_saved;
            if ok {
                self.s1_now = !self.g.objectless_type(&ty);
                let saved = self.cur_let.replace(id);
                self.objectless_bound(&pc, tid);
                self.cur_let = saved;
                self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
                return Ok(());
            }
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, pc, tid, NOLOC));
            pc
        } else if self.is_res(&ty) {
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, pc));
            pc
        } else {
            if self.has_refs(&ty) {
                self.emit(&format!("cb_recv_datum(&{}, {}u);", pc, tid));
            }
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, pc, tid, NOLOC));
            pc
        };
        let root = self.g.fresh("root");
        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
        let root = if self.unchecked_holder(&p.name, &ty) { String::new() } else { root };
        self.note_frame(&p.name, &root);
        self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: cname.clone(), ty: ty.clone(), root, kind: BindKind::Local });
        // A `Vec` taken by value: the caller moved it, so no path but this
        // binding's holds it, as for a local (`Gen::confined_vecs`).
        self.bind_confined(&p.name, &cname, &ty)
    }

    // Leaves the function with `v` as its result: the resource or
    // reference data goes in flight first, then every open scope and
    // frame ends, then the C return.
    pub(super) fn emit_return(&mut self, v: V) -> R<()> {
        let ret = self.ret.clone();
        if matches!(v.ty, Type::Never) {
            return Ok(());
        }
        let hold = if matches!(ret, Type::Void) {
            None
        } else {
            let t = self.into_temp(v, 0)?;
            if self.is_res(&ret) {
                // D-0286: so does every result in a decided program.
                let raw = self.raw_ret || self.g.decided;
                match t.obj.clone() {
                    // D-0200: a `__raw` form's result leaves with no object:
                    // the caller binds it objectless.
                    Some(o) if raw => self.emit(&format!("cb_forget_obj({});", o)),
                    Some(o) => self.emit(&format!("cb_send({});", o)),
                    None if raw => {}
                    None => return internal("a resource result without an object"),
                }
            } else if matches!(ret, Type::Ref(..)) {
                self.emit(&format!("cb_send_ref({}.tok);", t.c));
            } else if self.has_refs(&ret) {
                let tid = self.tid(&ret)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
            Some(t.c)
        };
        self.emit_unwind_to(0);
        match hold {
            Some(t) => self.emit(&format!("return {};", t)),
            None => self.emit("return;"),
        }
        Ok(())
    }
}
