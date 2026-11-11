// Frames and bindings: pushing, binding, lookup and popping.

use super::*;

impl Interp {
    pub(super) fn frames(&self) -> &Vec<Frame> {
        self.frames_by_thread.get(&self.current_thread).expect("current_thread has no frame stack")
    }

    pub(super) fn frames_mut(&mut self) -> &mut Vec<Frame> {
        self.frames_by_thread.get_mut(&self.current_thread).expect("current_thread has no frame stack")
    }

    // ---- lookup_owns' index: one count per object the current thread's
    // frame bindings name; every bindings.insert/remove updates it. ----

    pub(super) fn bound_inc(&mut self, obj: u64) {
        *self.bound_count_by_thread.entry(self.current_thread).or_default().entry(obj).or_insert(0) += 1;
    }

    pub(super) fn bound_count(&self, obj: u64) -> usize {
        self.bound_count_by_thread.get(&self.current_thread).and_then(|m| m.get(&obj)).copied().unwrap_or(0) as usize
    }

    pub(super) fn bound_dec(&mut self, obj: u64) {
        if let Some(m) = self.bound_count_by_thread.get_mut(&self.current_thread) {
            if let Some(c) = m.get_mut(&obj) {
                *c -= 1;
                if *c == 0 {
                    m.remove(&obj);
                }
            }
        }
    }

    pub(super) fn unsafe_depth(&self) -> u32 {
        *self.unsafe_depth_by_thread.get(&self.current_thread).unwrap_or(&0)
    }

    pub(super) fn require_unsafe(&self) -> Result<(), Flow> {
        if self.unsafe_depth() > 0 {
            Ok(())
        } else {
            Err(Flow::Fault("diag.trusted-outside-unsafe".into()))
        }
    }

    pub(super) fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // D-0213: a new access-path token. An exclusive one formed by a
    // spawned thread carries that thread in its high bits (`token_owner`),
    // so the token itself says whose it is; the main thread's are plain.
    pub(super) fn mint_token(&mut self, mode: &Mode) -> u64 {
        let id = self.fresh_id();
        if *mode == Mode::Exclusive && self.current_thread != 0 {
            id | (self.current_thread << TOKEN_THREAD_SHIFT)
        } else {
            id
        }
    }

    pub(super) fn token_owner(&self, token: u64) -> u64 {
        self.tok_owner.get(&token).copied().unwrap_or(token >> TOKEN_THREAD_SHIFT)
    }

    // ---- frames / bindings ----

    pub(super) fn push_frame(&mut self) {
        self.frames_mut().push(Frame::new());
    }

    pub(super) fn push_call_frame(&mut self) {
        // D-0077: the stack is checked where each call begins
        // (`stack_exhausted`).
        let mut f = Frame::new();
        f.is_call = true;
        self.frames_mut().push(f);
    }

    pub(super) fn bind(&mut self, name: &str, obj: u64) {
        // A bound object is no longer a temporary of any statement.
        if self.temp_index.remove(&obj) {
            for t in self.stmt_temps.values_mut().flat_map(|s| s.iter_mut()) {
                t.retain(|o| *o != obj);
            }
        }
        let ty = self.objects.get(&obj).map(|o| o.ty.clone());
        let f = self.frames_mut().last_mut().unwrap();
        let old = f.bindings.insert(name.to_string(), obj);
        if let Some(ty) = ty {
            f.types.insert(name.to_string(), ty);
        }
        f.owned.push(obj);
        f.moved.remove(name);
        if let Some(old) = old {
            self.bound_dec(old);
        }
        self.bound_inc(obj);
    }

    // D-0033 (1): `x = rhs` where `x`'s value was moved away or destroyed
    // (before `rhs`, or by it, as in `x = f(x)`). The new value becomes a
    // new object bound to `x` in the frame that declared it, as `x`'s
    // first value was (`[Let]`). `r` is `rhs`, already evaluated.
    pub(super) fn reinit_binding(&mut self, name: &str, r: EvalResult) -> EvalOutcome {
        let Some(idx) = self.frames().iter().rposition(|f| f.bindings.contains_key(name)) else {
            return Err(Flow::Fault("diag.unbound-name".into()));
        };
        let ty = self.frames()[idx].types.get(name).cloned();
        self.store_binding(name, r, ty)?;
        // `store_binding` bound it in the innermost frame; move it to
        // `x`'s own, so it ends with `x`'s scope.
        let last = self.frames().len() - 1;
        if idx != last {
            let f = self.frames_mut().last_mut().unwrap();
            let obj = f.bindings.remove(name).expect("just bound");
            let t = f.types.remove(name);
            f.owned.retain(|o| *o != obj);
            let g = &mut self.frames_mut()[idx];
            // The same object moves from the last frame to `x`'s: its own
            // count is unchanged, but the overwritten stale entry's drops.
            let old = g.bindings.insert(name.to_string(), obj);
            if let Some(t) = t {
                g.types.insert(name.to_string(), t);
            }
            g.owned.push(obj);
            g.moved.remove(name);
            if let Some(old) = old {
                self.bound_dec(old);
            }
        }
        Ok(EvalResult::Val(Value::Unit))
    }

    // Whether binding `name` currently has no value (moved away or
    // destroyed), and the type it was declared with.
    pub(super) fn binding_gone(&self, name: &str) -> Option<Option<Type>> {
        let (obj, moved) = self.lookup(name)?;
        if !(moved || !self.object_live(obj)) {
            return None;
        }
        let idx = self.frames().iter().rposition(|f| f.bindings.contains_key(name))?;
        Some(self.frames()[idx].types.get(name).cloned())
    }

    // Binds a name to an object this frame does *not* own — used only for
    // a borrow-captured closure variable, which names the referent's own
    // identity directly (see `bind_closure_captures`) without becoming
    // responsible for ending it.
    pub(super) fn bind_alias(&mut self, name: &str, obj: u64) {
        let f = self.frames_mut().last_mut().unwrap();
        let old = f.bindings.insert(name.to_string(), obj);
        f.moved.remove(name);
        if let Some(old) = old {
            self.bound_dec(old);
        }
        self.bound_inc(obj);
    }

    pub(super) fn lookup(&self, name: &str) -> Option<(u64, bool)> {
        for f in self.frames().iter().rev() {
            if let Some(&obj) = f.bindings.get(name) {
                return Some((obj, !f.moved.is_empty() && f.moved.contains(name)));
            }
            if f.is_call {
                break;
            }
        }
        None
    }

    // Pop the current frame, destroying every object it still owns
    // (reverse declaration order, spec/14 [Block-Exit]/D-0008), except the
    // identity named by `keep` (the block's own result), which flows out.
    pub(super) fn pop_frame(&mut self, keep: Option<u64>) -> Result<(), Flow> {
        // A fault while a function body's objects end (a destructor, a
        // handle's wait) is reported at the body's last statement, the line
        // being run when the exit began, when nothing nearer located it
        // (D-0219: a deadlock there had no location; `cobc` reports that
        // line). A nested block's exit is located by its own statement.
        if !self.frames().last().map_or(false, |f| f.is_call) {
            return self.pop_frame_inner(keep);
        }
        let at = if user_line(self.current_line) { self.current_line } else { self.last_user_line };
        match self.pop_frame_inner(keep) {
            Err(Flow::Fault(d)) if !crate::diagnostics::has_location(&d) && user_line(at) => Err(Flow::Fault(format!("{d}@{at}"))),
            other => other,
        }
    }

    pub(super) fn pop_frame_inner(&mut self, keep: Option<u64>) -> Result<(), Flow> {
        let f = self.frames_mut().pop().unwrap();
        for &obj in f.bindings.values() {
            self.bound_dec(obj);
        }
        if self.terminated {
            return Ok(());
        }
        for &obj in f.owned.iter().rev() {
            if Some(obj) == keep {
                continue;
            }
            if self.objects.contains_key(&obj) {
                self.destroy_at_scope_end(obj)?;
            }
        }
        Ok(())
    }
}
