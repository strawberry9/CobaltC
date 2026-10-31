// Threads: waiting, blocking, cancellation and lending across threads.

use super::*;

impl Interp {
    // Blocks this CobaltC thread until `ready` holds of the shared
    // state, releasing the GIL meanwhile. Fails with `Flow::Terminated`
    // if the program terminates first, and with a hard error if there is
    // no GIL (nothing could ever make `ready` true).
    #[inline(never)]
    // A wait (`[Join]`, `[Lock]`, a channel, the clock) until `ready`.
    // D-0204: it ends instead in a fault, contained like any other, when
    // this thread is asked to stop (`cancel`), when waiting would close a
    // cycle of joins and locks, or when every live thread waits and none
    // can end its wait (`diag.deadlock`).
    pub(super) fn wait_for(&mut self, kind: Wait, mut ready: impl FnMut(&Shared) -> bool) -> Result<(), Flow> {
        let me = self.current_thread;
        // A thread unwinding a contained fault waits for its own threads
        // (`[Handle-Destructor]`); being asked to stop does not end that.
        let unwinding = self.faulting || self.fault_pending;
        let stop = move |s: &Shared| -> Option<&'static str> {
            if s.deadlocked.contains(&me) {
                Some("diag.deadlock")
            } else if s.cancel.contains(&me) && !s.lent_excl.contains(&me) && !unwinding {
                Some("diag.thread-cancelled")
            } else {
                None
            }
        };
        {
            let mut s = self.shared.lock().unwrap();
            if ready(&s) {
                return Ok(());
            }
            if let Some(d) = stop(&s) {
                s.deadlocked.remove(&me);
                return Err(Flow::Fault(d.into()));
            }
            if closes_cycle(&s, me, &kind) {
                return Err(Flow::Fault("diag.deadlock".into()));
            }
            s.waits.insert(me, kind);
            find_deadlock(&mut s);
        }
        let r = self.block_until(|s| ready(s) || stop(s).is_some());
        let mut s = self.shared.lock().unwrap();
        s.waits.remove(&me);
        r?;
        if ready(&s) {
            return Ok(());
        }
        let d = stop(&s).unwrap_or("diag.deadlock");
        s.deadlocked.remove(&me);
        Err(Flow::Fault(d.into()))
    }

    pub(super) fn block_until(&mut self, mut ready: impl FnMut(&Shared) -> bool) -> Result<(), Flow> {
        if ready(&self.shared.lock().unwrap()) {
            return Ok(());
        }
        let gil = match self.gil.as_ref().and_then(|w| w.upgrade()) {
            Some(g) => g,
            None => return Err(Flow::Fault("deadlock: nothing can satisfy this wait (no other thread exists)".into())),
        };
        let shared = self.shared.clone();
        let me = self.current_thread;
        // The line being run is this thread's again once it resumes: the
        // others ran their own meanwhile.
        let line = self.current_line;
        let uline = self.last_user_line;
        let wc = self.write_ctx;
        let unwind = self.unwind_flags();
        let r = gil_wait(&gil, &shared, &mut ready);
        self.write_ctx = wc;
        std::hint::black_box(&mut *self);
        self.current_thread = me;
        self.current_line = line;
        self.last_user_line = uline;
        self.set_unwind_flags(unwind);
        match r {
            Ok(()) => Ok(()),
            Err(_) => {
                self.terminated = true;
                Err(Flow::Terminated)
            }
        }
    }

    // Runs `f`, which touches nothing of the interpreter, with the GIL
    // released while other threads are live, so a call that blocks (a
    // socket, a child process) lets them run; this thread's position is
    // its own again afterwards, as after `block_until`.
    #[inline(never)]
    pub(super) fn unblocked<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let gil = if self.active_threads > 1 { self.gil.as_ref().and_then(|w| w.upgrade()) } else { None };
        let Some(g) = gil else { return f() };
        let me = self.current_thread;
        let line = self.current_line;
        let uline = self.last_user_line;
        let wc = self.write_ctx;
        let unwind = self.unwind_flags();
        // D-0204: a wait outside the runtime may end by itself: while it
        // lasts, no deadlock is declared.
        self.shared.lock().unwrap().in_io.insert(me);
        g.io(me, true);
        g.release();
        let r = f();
        g.io(me, false);
        g.acquire();
        std::hint::black_box(&mut *self);
        self.current_thread = me;
        self.current_line = line;
        self.last_user_line = uline;
        self.write_ctx = wc;
        self.set_unwind_flags(unwind);
        {
            // A thread back from outside the runtime may leave every live
            // thread waiting (D-0219: found by the model).
            let mut s = self.shared.lock().unwrap();
            s.in_io.remove(&me);
            find_deadlock(&mut s);
        }
        r
    }

    // D-0204: this thread's unwinding state, which the interpreter keeps
    // in fields of its own: another thread's contained fault, unwinding
    // while this one waits, must not leave them changed for it.
    pub(super) fn unwind_flags(&self) -> (bool, bool, bool) {
        (self.faulting, self.fault_pending, self.exit_pending)
    }

    pub(super) fn set_unwind_flags(&mut self, f: (bool, bool, bool)) {
        self.faulting = f.0;
        self.fault_pending = f.1;
        self.exit_pending = f.2;
    }

    // A spawned thread's fault `d`, its frames already unwound. D-0204:
    // contained -- the thread's result is the failure -- unless the thread
    // was lent an exclusive reference, `d` is `exit`'s, or the program is
    // already ending: then, as `[Fault-Unwind]` has it, the program ends
    // and every other thread stops the moment it next waits or yields.
    // (No process exit: this interpreter may be one of many in one host
    // process, the conformance test binary.)
    pub(super) fn thread_failed(&mut self, ell: u64, d: String, rty: Type) {
        let mut s = self.shared.lock().unwrap();
        let contained = !s.lent_excl.contains(&ell) && s.program_fault.is_none() && !d.starts_with(EXIT_MARK);
        if !contained {
            s.program_fault = Some(d);
            drop(s);
            self.terminated = true;
            return;
        }
        let shown = match &self.render {
            Some(r) => r(&d),
            None => d.clone(),
        };
        let id = s.failures.len() as u64;
        s.failures.push((d, shown));
        s.thread_status.insert(ell, ThreadStatus::Failed(id, rty));
        // One thread fewer running: the rest may now all wait (D-0219).
        find_deadlock(&mut s);
    }

    // Whether this thread's fault, now, would be contained (D-0204).
    pub(super) fn contained_here(&self) -> bool {
        let me = self.current_thread;
        me != 0 && !self.terminated && !self.exit_pending && !self.shared.lock().unwrap().lent_excl.contains(&me)
    }

    // `[Thread-Step]`: between statements, let another live thread run.
    #[inline(never)]
    pub(super) fn yield_step(&mut self) {
        if self.active_threads <= 1 {
            return;
        }
        // Without a schedule (`--schedule-seed`), a thread runs a slice of
        // steps before offering the GIL: every interleaving is still one the
        // language permits, and a hand-off at every step, which wakes every
        // waiting thread, cost CPU-bound programs most of their time.
        if SCHEDULE_SEED.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            if self.yield_budget > 0 {
                self.yield_budget -= 1;
                return;
            }
            self.yield_budget = YIELD_SLICE;
        }
        if let Some(gil) = self.gil.as_ref().and_then(|w| w.upgrade()) {
            let me = self.current_thread;
            let line = self.current_line;
            let uline = self.last_user_line;
            let wc = self.write_ctx;
            let unwind = self.unwind_flags();
            gil_yield(&gil);
            self.write_ctx = wc;
            std::hint::black_box(&mut *self);
            self.current_thread = me;
            self.current_line = line;
            self.last_user_line = uline;
            self.set_unwind_flags(unwind);
        }
    }

    pub(super) fn thread_running(&self, t: u64) -> bool {
        t == 0 || matches!(self.shared.lock().unwrap().thread_status.get(&t), Some(ThreadStatus::Running))
    }

    // D-0213 `[Thread-Affine]` (spec/08 §4): an access by this thread
    // through an exclusive token. Its own token passes; one lent and not
    // yet accessed (`TRANSIT`), or one whose thread has ended, is taken
    // over; one of another running thread is `diag.aliasing-conflict`.
    pub(super) fn affine(&mut self, token: u64, exclusive: bool) -> Result<(), Flow> {
        if !exclusive || self.active_threads <= 1 {
            return Ok(());
        }
        let me = self.current_thread;
        let owner = self.token_owner(token);
        if owner == me {
            return Ok(());
        }
        if owner != TRANSIT && self.thread_running(owner) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        // D-0237 (spec/18 `contained`): a spawned thread that takes over
        // an exclusive path -- one in transit, or one whose thread has
        // ended -- changes data that is not its own, as a thread lent one
        // at `spawn` does: it is not cancelled, and its fault is not
        // contained.
        if me != 0 {
            self.shared.lock().unwrap().lent_excl.insert(me);
        }
        if me == token >> TOKEN_THREAD_SHIFT {
            self.tok_owner.remove(&token);
        } else {
            self.tok_owner.insert(token, me);
        }
        Ok(())
    }

    // D-0213 `[Thread-Lend]` (spec/19 §1, §3): every exclusive reference in
    // `v` is replaced by a path borrowed from it (`[Borrow]`'s checks), in
    // transit to whichever thread first accesses through it: a spawned
    // thread's arguments and captures, a value a channel is to carry. The
    // sender's own copies stay valid and now clash with it while it lives.
    pub(super) fn lend_value(&mut self, v: &mut Value) -> Result<(), Flow> {
        match v {
            Value::Ref { obj, path, mode: Mode::Exclusive, token, range, anc, .. } => {
                self.affine(*token, true)?;
                if !self.object_live(*obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                let mut p = path.clone();
                if let Some((lo, hi)) = range {
                    p.push(Proj::Range(*lo, *hi));
                }
                if self.clash(*obj, &p, Mode::Exclusive, &[*token]) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let c = self.fresh_id();
                *anc = self.record_token_ancestors(c, &[*token]);
                self.tok_owner.insert(c, TRANSIT);
                *token = c;
                Ok(())
            }
            Value::Struct(fs) | Value::Array(fs) => {
                for f in fs.iter_mut() {
                    self.lend_value(f)?;
                }
                Ok(())
            }
            Value::Enum(_, p) => self.lend_value(p),
            _ => Ok(()),
        }
    }

    // D-0214 `[Release]`, `[Rawptr-Move-Out]`: whether another thread holds
    // an exclusive path to `obj` -- one of another running thread, or lent
    // and not yet taken. Only while another thread runs.
    pub(super) fn foreign_held(&self, obj: u64) -> bool {
        if self.active_threads <= 1 {
            return false;
        }
        let me = self.current_thread;
        self.distinct_refs_to(obj).into_iter().any(|(_, m, tok, is_guard)| {
            (m == Mode::Exclusive || is_guard) && {
                let o = self.token_owner(tok);
                o != me && (o == TRANSIT || self.thread_running(o))
            }
        })
    }

    pub(super) fn check_not_foreign(&self, ids: &[u64]) -> Result<(), Flow> {
        if ids.iter().any(|&i| self.foreign_held(i)) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        Ok(())
    }

    // `lend_value` over an object's stored value.
    pub(super) fn lend_object(&mut self, o: u64) -> Result<(), Flow> {
        let Some(mut v) = self.objects.get(&o).map(|ob| ob.value.clone()) else { return Ok(()) };
        if !Self::value_has_ref(&v) {
            return Ok(());
        }
        self.lend_value(&mut v)?;
        self.index_refs(o, &v);
        if let Some(ob) = self.objects.get_mut(&o) {
            ob.value = v;
        }
        Ok(())
    }
}
