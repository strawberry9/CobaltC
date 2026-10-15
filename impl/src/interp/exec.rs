// Running main and bodies; blocks, statements and bindings.

use super::*;

impl Interp {
    // ================= Program execution =================

    // One attempt at running (or resuming) `main` to completion. `Ok(Done)`
    // means the program terminated `ok`; `Ok(WouldBlock)` means a
    // `join`/`lock` in `main`'s own body is genuinely waiting on another
    // OS thread -- the caller (lib.rs, which holds the real lock this
    // `Interp` lives behind) must release it, wait briefly, and call this
    // again on the same `Interp` to resume exactly where it left off.
    //
    // `Ok(s)` is `[Terminate-Ok]`'s `ok(s)`: `main`'s value for
    // `fn main() : u8`, 0 otherwise (`rule.fn.program`).
    pub fn run_main(&mut self) -> Result<u8, String> {
        let main = match self.items.fns.get("main").cloned() {
            Some(m) => m,
            // D-0257: a library (functions C calls, no `main`) checks and
            // tests here, but has nothing to run.
            None if self.items.fns.values().any(|f| f.c_entry) => {
                return Err("unsupported: this is a library (it defines functions C calls and no `main`): `cobc --lib` builds it; its `unittest` blocks run with `--unittest`".to_string());
            }
            None => return Err("diag.no-main".to_string()),
        };
        self.current_thread = 0;
        self.faulting = false;
        crate::netio::wake_init();
        set_probe(&self.shared, 0);
        let r = self.run_body(&main.body, &HashMap::default(), true, &[], None);
        match r {
            Ok(res) => {
                // A worker may have faulted after main's last wait;
                // the program's outcome is that fault (spec/18).
                if let Some(d) = self.shared.lock().unwrap().program_fault.clone() {
                    return Err(d);
                }
                if main.ret != Type::Void {
                    let v = match res {
                        EvalResult::Val(v) => v,
                        other => self.result_to_value(other).map_err(|f| self.flow_to_string(f))?,
                    };
                    if let Value::Int(_, s) = v {
                        return Ok(s as u8);
                    }
                }
                Ok(0)
            }
            Err(Flow::Terminated) => Err(self.shared.lock().unwrap().program_fault.clone().unwrap_or_else(|| "terminated".into())),
            Err(Flow::Fault(d)) => {
                // Tell every still-live spawned thread to stop, too
                // (spec/18: the whole program terminates, not just
                // main's own thread).
                self.shared.lock().unwrap().program_fault = Some(d.clone());
                self.terminated = true;
                Err(d)
            }
            Err(_) => Err("unexpected control flow at top level".into()),
        }
    }

    // Runs `body` as an *outermost* per-(real-)thread execution --
    // `main`'s own body, or a spawned thread's callee body -- with its
    // parameters (and, for a spawned move closure, its captures) bound
    // in a fresh frame first.
    pub(super) fn run_body(
        &mut self,
        body: &Block,
        subst: &HashMap<String, Type>,
        first_call: bool,
        params: &[(String, EvalResult, Option<Type>)],
        closure_bind: Option<(u64, &[String])>,
    ) -> Result<EvalResult, Flow> {
        let mut cells = Vec::new();
        // The call's own line: a fault binding a parameter (`[Call]`'s
        // `store`, D-0049's reborrow) is the call's (D-0219: it took a line
        // of the callee's body).
        let call_line = self.current_line;
        if first_call {
            self.push_call_frame();
            self.binding_thread_params = self.current_thread != 0;
            let mut bound = Ok(());
            for (name, val, ty) in params {
                // D-0049: an exclusive reference (or slice) passed for a
                // shared parameter is reborrowed shared, as `&*e` would be.
                let val = match self.weaken_arg(val, ty.as_ref()) {
                    Ok(v) => v,
                    Err(e) => {
                        bound = Err(match e {
                            Flow::Fault(d) if !crate::diagnostics::has_location(&d) && user_line(call_line) => Flow::Fault(format!("{d}@{call_line}")),
                            other => other,
                        });
                        break;
                    }
                };
                if let Err(e) = self.store_binding(name, val, ty.clone()) {
                    bound = Err(e);
                    break;
                }
            }
            self.binding_thread_params = false;
            bound?;
            if let Some((obj, captures)) = closure_bind {
                // Only a `move` closure can be spawned (`rule.conc.spawn`):
                // the cells are the per-call views of its captures.
                cells = self.bind_closure_captures(obj, captures);
            }
        }
        let r = match self.exec_block_stmts(body, subst, None) {
            Ok(res) => {
                let res = self.into_result_form(res)?;
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(Flow::Return(v)) => {
                let keep = if let EvalResult::Temp(o) = &v { Some(*o) } else { None };
                self.pop_frame(keep)?;
                Ok(v)
            }
            Err(other) => {
                self.pop_frame(None)?;
                Err(other)
            }
        };
        self.release_capture_cells(&cells);
        // The spawned closure itself ends with its thread, and with it
        // anything it owns; `[Spawn]` gave its authority to this thread.
        if let (Ok(_), Some((obj, _))) = (&r, closure_bind) {
            if let Some(o) = self.objects.get_mut(&obj) {
                o.owner_thread = self.current_thread;
            }
            if self.objects.contains_key(&obj) {
                self.destroy_object(obj)?;
            }
        }
        r
    }

    pub(super) fn flow_to_string(&self, f: Flow) -> String {
        match f {
            Flow::Fault(d) => d,
            Flow::Terminated => self.shared.lock().unwrap().program_fault.clone().unwrap_or_else(|| "terminated".into()),
            _ => "unexpected control flow".into(),
        }
    }

    // Runs a function body's block in a *nested* frame (the block's own
    // frame, inside whatever parameter frame the caller pushed) and
    // returns its result in result form.
    // Thin wrapper (see `current_line`'s doc comment): catches a fault
    // raised by `pop_frame`'s own automatic block-exit destruction
    // sweep (D-0008) below, which is not reached by `exec_stmt`'s own
    // wrapping since it runs *after* every statement in the block, not
    // as part of any one of them.
    pub(super) fn exec_block_body(&mut self, b: &Block, subst: &HashMap<String, Type>) -> EvalOutcome {
        self.exec_block_body_expected(b, subst, None)
    }

    pub(super) fn exec_block_body_expected(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        match self.exec_block_body_inner(b, subst, expected) {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        }
    }

    pub(super) fn exec_block_body_inner(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        self.push_frame();
        let r = self.exec_block_stmts(b, subst, expected);
        match r {
            Ok(res) => {
                // Put the result in result form (spec/14 [Block-Exit]
                // reads `r ∈ {v, temp o}` — a raw `place` is never what
                // flows out of a closing frame) *before* deciding what
                // survives the sweep, so a trailing plain identifier
                // (`p` as a match arm's/block's tail) is read out rather
                // than left as a dangling place into an object this very
                // frame is about to destroy.
                let res = self.into_result_form(res)?;
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(flow) => {
                // unwind: pop the frame's owned objects too (D-0008 sweep
                // during return/break/continue/fault unwinding) -- except a
                // value being returned, which is the caller's now.
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                Err(flow)
            }
        }
    }

    pub(super) fn exec_block_stmts(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        let early = self.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed);
        for s in &b.stmts {
            self.exec_stmt(s, subst)?;
            if early {
                let names = self.items.early_ends.lock().unwrap().get(&(s as *const Stmt as usize)).cloned();
                if let Some(names) = names {
                    for n in &names {
                        self.end_ref_binding(n)?;
                    }
                }
            }
            self.yield_step();
        }
        // `rule.type.expected`: a block's trailing expression takes the
        // expected type of the block itself -- a function body's result
        // is expected to be its return type, an `if` branch's or `match`
        // arm's result the type expected of the whole `if`/`match`.
        // A trailing expression is a statement for its temporaries
        // (`rule.control.stmt`, "and of a block's trailing expression"):
        // after its result is in result form, every other temporary it
        // created ends, and the result's own temporary passes to the
        // enclosing statement.
        let Some(e) = &b.tail else {
            return Ok(EvalResult::Val(Value::Unit));
        };
        self.eval_stmt_scoped(e, subst, expected)
    }

    // An expression evaluated as a statement of its own (`stmt-scoped`,
    // spec/14 `rule.control.stmt`): a block's trailing expression, and a
    // match arm's body (`[Match]`: `block'(f_arm, stmt-scoped(e_i, keep))`).
    // After its result is in result form, every other temporary it
    // created ends; the result's own temporary, and any it refers into,
    // pass to the enclosing statement. An arm's temporaries thus end
    // before the arm's frame — and its binding — does, so
    // `Some(r) : f(&g(&r))` destroys `r` solitary (the divergence of
    // 2026-10-07, `stress/std2/div1.cb`, where this was missing).
    pub(super) fn eval_stmt_scoped(&mut self, e: &Expr, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        self.st().push(Vec::new());
        let r = match expected {
            Some(ty) => self.eval_expected(e, subst, ty),
            None => self.eval(e, subst),
        };
        let temps = self.st().pop().unwrap_or_default();
        let res = match r {
            Ok(res) => res,
            Err(flow) => return Err(self.end_temps_on_exit(temps, flow)),
        };
        // The result, and any temporary it refers into, stays with the
        // enclosing statement.
        let used: Vec<u64> = match &res {
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => vec![*o],
            EvalResult::Val(v) => Self::ref_targets(v),
        };
        let (keep, end): (Vec<u64>, Vec<u64>) = temps.into_iter().partition(|t| used.contains(t));
        for o in keep {
            self.stmt_temp(o);
        }
        self.end_stmt_temps(end)?;
        Ok(res)
    }

    // The objects a value's references point into.
    pub(super) fn ref_targets(v: &Value) -> Vec<u64> {
        let mut out = Vec::new();
        fn walk(v: &Value, out: &mut Vec<u64>) {
            match v {
                Value::Ref { obj, .. } => out.push(*obj),
                Value::Struct(fs) | Value::Array(fs) => fs.iter().for_each(|f| walk(f, out)),
                Value::Enum(_, p) => walk(p, out),
                _ => {}
            }
        }
        walk(v, &mut out);
        out
    }

    // Thin wrapper (see `current_line`'s doc comment): most statement
    // faults already arrive pre-tagged, having passed through `eval()`
    // on their way out of `exec_stmt_inner`; this only ever fires for
    // the few direct `Flow::Fault` constructions in this statement's
    // own post-`eval()` bookkeeping (e.g. `store_binding`,
    // `destroy_object`), tagging them with the last expression line
    // touched in this statement as a reasonable, honestly-approximate
    // fallback -- never a fabricated exact location.
    pub(super) fn exec_stmt(&mut self, s: &Stmt, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        self.st().push(Vec::new());
        let r = self.exec_stmt_inner(s, subst);
        let temps = self.st().pop().unwrap_or_default();
        let r = match r {
            Ok(()) => self.end_stmt_temps(temps).and_then(|_| self.end_dropped_closures()),
            Err(flow) => Err(self.end_temps_on_exit(temps, flow)),
        };
        // D-0261: with `COBALTC_VERIFY`, the state the statement left is
        // checked against the invariants as a whole.
        if r.is_ok() && self.verify_due() {
            self.verify_invariants();
        }
        // A fault from the statement's own bookkeeping (ending its
        // temporaries, storing its result) is the statement's: its line,
        // when that is the program's (the last line evaluated may be
        // inside `std`).
        let line = match s {
            Stmt::Let { line, .. } => *line,
            Stmt::Expr(e) | Stmt::BlockLike(e) => e.line,
            Stmt::Destructure { init, .. } => init.line,
        };
        match r {
            Err(Flow::Fault(d)) if !crate::diagnostics::has_location(&d) && user_line(line) => Err(Flow::Fault(format!("{d}@{line}"))),
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        }
    }

    // `exec_stmt` of `Stmt::Expr(e)`, without building one (a `for`
    // loop's step, run each iteration).
    pub(super) fn exec_expr_stmt(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        self.st().push(Vec::new());
        let r = self.eval_temp_discard(e, subst);
        let temps = self.st().pop().unwrap_or_default();
        let r = match r {
            Ok(()) => self.end_stmt_temps(temps).and_then(|_| self.end_dropped_closures()),
            Err(flow) => Err(self.end_temps_on_exit(temps, flow)),
        };
        let line = e.line;
        match r {
            Err(Flow::Fault(d)) if !crate::diagnostics::has_location(&d) && user_line(line) => Err(Flow::Fault(format!("{d}@{line}"))),
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        }
    }

    // A temporary to be ended with the current statement (or, outside
    // any statement, left as before).
    pub(super) fn stmt_temp(&mut self, o: u64) {
        if let Some(t) = self.st().last_mut() {
            if !t.contains(&o) {
                t.push(o);
                self.temp_index.insert(o);
            }
        }
    }

    // A statement left by `return`, `break` or `continue` ends its
    // temporaries as one that finishes does (`rule.control.stmt`): a
    // temporary `Option<ref<…>>` scrutinee must not keep its borrow alive
    // after the `return` in one of its arms. What a `return` carries out
    // (and any temporary it refers into) is the caller's and stays. A
    // fault keeps its own unwinding. Returns the flow to propagate: the
    // original one, or a fault an ending destructor raised.
    pub(super) fn end_temps_on_exit(&mut self, temps: Vec<u64>, flow: Flow) -> Flow {
        if matches!(flow, Flow::Fault(_)) {
            self.faulting = true;
        }
        let keep: Vec<u64> = match &flow {
            Flow::Return(EvalResult::Temp(o)) | Flow::Return(EvalResult::Place(o, _, _)) => vec![*o],
            Flow::Return(EvalResult::Val(v)) => Self::ref_targets(v),
            Flow::Break | Flow::Continue => Vec::new(),
            _ => return flow,
        };
        let end: Vec<u64> = temps.into_iter().filter(|t| !keep.contains(t)).collect();
        match self.end_stmt_temps(end) {
            Ok(()) => flow,
            Err(f) => f,
        }
    }

    pub(super) fn end_stmt_temps(&mut self, temps: Vec<u64>) -> Result<(), Flow> {
        for o in temps.into_iter().rev() {
            self.temp_index.remove(&o);
            if self.objects.contains_key(&o) {
                self.destroy_at_scope_end(o)?;
            }
        }
        Ok(())
    }

    pub(super) fn exec_stmt_inner(&mut self, s: &Stmt, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        match s {
            Stmt::Let { ty, name, init, .. } => {
                let declared_ty = ty.as_ref().map(|t| self.apply_subst(t, subst));
                if init.is_none() {
                    // [Let-Uninit] (spec/11): `τ x;` establishes a fresh
                    // object with no value yet -- `rule.init.definite-
                    // assignment` requires a read to fault
                    // `diag.use-of-uninitialized` before the first write,
                    // not silently observe a placeholder. `ty` is always
                    // `Some` here (the rule's own text: "there being
                    // nothing to synthesize from, uninitialized always
                    // requires the written type").
                    let dty = declared_ty.clone().unwrap_or(Type::Void);
                    let id = self.new_object(dty, Value::Unit);
                    self.uninit_bindings.insert(id);
                    self.bind(name, id);
                    return Ok(());
                }
                let r = match (init, &declared_ty) {
                    (Some(e), Some(dty)) => self.eval_expected(e, subst, dty)?,
                    (Some(e), None) => self.eval(e, subst)?,
                    (None, _) => unreachable!("handled above"),
                };
                self.store_binding(name, r, declared_ty)?;
                Ok(())
            }
            Stmt::Destructure { fields, names, rest, init, .. } => {
                let r = self.eval(init, subst)?;
                let (obj, val) = match r {
                    EvalResult::Temp(o) => (o, None),
                    EvalResult::Place(o, path, _) => (o, Some(path)),
                    EvalResult::Val(_) => unreachable!("destructure source is a place/temp"),
                };
                let base_path = val.unwrap_or_default();
                let sty = self.type_at(obj, &base_path);
                let decl_names: Vec<String> = match &sty {
                    Type::Named(n, _) => self.items.structs.get(n).map(|s| s.fields.iter().map(|f| f.name.clone()).collect()).unwrap_or_default(),
                    _ => Vec::new(),
                };
                for (field, binder) in crate::ast::destructure_pairs(names, fields, *rest, &decl_names) {
                    let mut fpath = base_path.clone();
                    fpath.push(Proj::Field(self.field_index(&sty, &field)?));
                    let fty = self.type_at(obj, &fpath);
                    let fval = self.read_place(obj, &fpath);
                    let fresh = self.new_object(fty, fval);
                    self.bind(&binder, fresh);
                }
                // [Relocate-Out]: the fields now belong to their bindings;
                // the container ends without destroying them (as `cobc`'s
                // `cb_end_moved_out`), or they would be destroyed twice.
                self.end_moved_out(obj);
                Ok(())
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => {
                self.eval_temp_discard(e, subst)?;
                Ok(())
            }
        }
    }

    // Evaluate an expression whose result is discarded as a statement:
    // if it produced a fresh temporary, destroy it now (spec/14 [Stmt-Exit]).
    pub(super) fn eval_temp_discard(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        let r = self.eval(e, subst)?;
        if let EvalResult::Temp(o) = r {
            self.destroy_at_scope_end(o)?;
        }
        Ok(())
    }

    // D-0111 `[Ref-Binding-Last-Use]`: a local reference, slice or
    // `StringView` binding of this block ends after its last use, as at
    // its block's exit: its object (and the path it holds) ends.
    pub(super) fn end_ref_binding(&mut self, name: &str) -> Result<(), Flow> {
        let Some(&obj) = self.frames().last().and_then(|f| f.bindings.get(name)) else { return Ok(()) };
        let ty = self.type_at(obj, &[]);
        let is_ref = crate::typecheck::ends_at_last_use(&self.items, &ty);
        if !is_ref || self.is_resource(&ty) {
            return Ok(());
        }
        let f = self.frames_mut().last_mut().unwrap();
        f.bindings.remove(name);
        f.owned.retain(|o| *o != obj);
        self.bound_dec(obj);
        if self.objects.contains_key(&obj) {
            self.destroy_at_scope_end(obj)?;
        }
        Ok(())
    }

    pub(super) fn store_binding(&mut self, name: &str, r: EvalResult, declared: Option<Type>) -> Result<(), Flow> {
        match r {
            EvalResult::Val(v) => {
                let ty = declared.unwrap_or_else(|| self.type_of_value(&v));
                let id = self.new_object(ty, v);
                self.bind(name, id);
            }
            EvalResult::Temp(o) => {
                // Adoption re-keys a temporary produced elsewhere (a
                // thread's result claimed by `join`) to the adopter.
                // Whether the object is a resource follows its type: a
                // `None` made as `Option<?>` is one when written
                // `Option<Noisy>`, or what is later stored in it is never
                // destroyed.
                let is_res = declared.as_ref().filter(|d| !matches!(d, Type::Void)).map(|d| self.is_resource(d));
                if let Some(ob) = self.objects.get_mut(&o) {
                    ob.owner_thread = self.current_thread;
                    // A written type is the binding's type: `Option<u64> x
                    // = None;` makes an `Option<u64>`, whose later
                    // assignments then type their payloads as u64.
                    // A closure keeps its own type: a `fn(…)` declared for
                    // it is the type it is used at, and a call dispatches on
                    // the closure's identity.
                    if let (Some(d), Some(r)) = (&declared, is_res) {
                        if !matches!(ob.ty, Type::Closure(_)) {
                            ob.ty = d.clone();
                            ob.is_resource = r;
                        }
                    }
                }
                self.bind(name, o);
            }
            EvalResult::Place(obj, path, excl) => {
                let ty = self.type_at(obj, &path);
                if self.is_resource(&ty) {
                    if !path.is_empty() {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    // transfer: rebind the same object identity.
                    // [Authority-Transfer-Aliased]: no other live path may
                    // reach it (excluding the source binding itself, which
                    // this very transfer is about to invalidate). Reached
                    // through a reference, its owner's path is such a path.
                    if !excl.is_empty() || !self.solitary(obj, &excl) {
                        return Err(Flow::Fault("diag.move-while-aliased".into()));
                    }
                    // [Authority-Transfer]: the performer must hold the
                    // authority -- except when binding a spawned thread's
                    // parameters, the one case where the destination is
                    // formed in another thread and the authority is
                    // re-keyed to it (rule.conc.spawn).
                    let owner = self.objects.get(&obj).map(|o| o.owner_thread).unwrap_or(self.current_thread);
                    if owner != self.current_thread && !self.binding_thread_params {
                        return Err(Flow::Fault("diag.transfer-without-authority".into()));
                    }
                    if let Some(o) = self.objects.get_mut(&obj) {
                        o.owner_thread = self.current_thread;
                    }
                    let keep_frame = self.frames().len() - 1;
                    self.bind(name, obj);
                    self.invalidate_other_bindings(obj, keep_frame, name);
                } else {
                    // `[Store-Binding-Place-Copy]` copies through `[Read]`
                    // (spec/05), so it carries every one of `[Read]`'s own
                    // checks -- `¬clash(a, shared)` against live
                    // non-ancestor exclusive paths, and definite
                    // assignment -- not a raw peek at the cells.
                    let v = self.result_to_value(EvalResult::Place(obj, path, excl))?;
                    let id = self.new_object(ty, v);
                    self.bind(name, id);
                }
            }
        }
        Ok(())
    }

    // After moving object `obj` into a new binding, remove/mark stale every
    // OTHER binding across all frames that pointed at it as its owner.
    // Marks every current binding of `obj`, in every frame, as moved —
    // used when `obj` is being fully consumed (turned into a `temp`, or
    // destroyed), so nothing pointing at it survives at all.
    pub(super) fn invalidate_binding_pointing_at(&mut self, obj: u64) {
        // The binding-count index says how many bindings of this thread
        // name `obj`; they are almost always in the newest frames, so
        // search from the top and stop once all are found.
        let mut left = self.bound_count(obj);
        if left == 0 {
            return;
        }
        for f in self.frames_mut().iter_mut().rev() {
            let names: Vec<String> = f.bindings.iter().filter(|(_, &o)| o == obj).map(|(n, _)| n.clone()).collect();
            left = left.saturating_sub(names.len());
            for n in names {
                f.moved.insert(n);
            }
            if left == 0 {
                break;
            }
        }
    }

    // Marks every OTHER current binding of `obj` as moved, sparing
    // exactly `(keep_frame, keep_name)` — the destination binding a
    // transfer (`[Store-Binding-Place-Transfer]`) just created. Finding
    // "the destination" by scanning for "whichever binding is newest"
    // is not well-defined once source and destination coexist in the
    // *same* frame's binding map (e.g. `auto w = v;`), so the caller
    // names it explicitly instead.
    pub(super) fn invalidate_other_bindings(&mut self, obj: u64, keep_frame: usize, keep_name: &str) {
        // As above: stop once every binding of `obj` (the kept one
        // included) has been seen; at most one means only the kept one.
        let mut left = self.bound_count(obj);
        if left <= 1 {
            return;
        }
        for (fi, f) in self.frames_mut().iter_mut().enumerate().rev() {
            let names: Vec<String> = f.bindings.iter().filter(|(_, &o)| o == obj).map(|(n, _)| n.clone()).collect();
            left = left.saturating_sub(names.len());
            for n in names {
                if fi == keep_frame && n == keep_name {
                    continue;
                }
                f.moved.insert(n);
            }
            if left == 0 {
                break;
            }
        }
    }
}
