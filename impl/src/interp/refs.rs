// References: weakening, reborrowing, dereference and resolution.

use super::*;

impl Interp {
    // D-0049: an argument of type `ref<τ, exclusive>` (`slice<τ,
    // exclusive>`) for a parameter of type `ref<τ, shared>` (`slice<τ,
    // shared>`) is passed as `&*e`: a shared borrow through it, with it as
    // the ancestor (`[Borrow]`, spec/08).
    pub(super) fn weaken_arg(&mut self, val: &EvalResult, ty: Option<&Type>) -> Result<EvalResult, Flow> {
        let wants_shared = matches!(ty, Some(Type::Ref(_, Mode::Shared)) | Some(Type::Slice(_, Mode::Shared)));
        if !wants_shared {
            return Ok(val.clone());
        }
        let v = match val {
            EvalResult::Val(v) => v.clone(),
            EvalResult::Place(o, p, _) => self.read_place(*o, p),
            EvalResult::Temp(_) => return Ok(val.clone()),
        };
        match v {
            Value::Ref { mode: Mode::Exclusive, .. } => Ok(EvalResult::Val(self.reborrow_shared(v)?)),
            Value::Struct(mut parts) if matches!(ty, Some(Type::Slice(..))) && matches!(parts.first(), Some(Value::Ref { mode: Mode::Exclusive, .. })) => {
                parts[0] = self.reborrow_shared(parts[0].clone())?;
                Ok(EvalResult::Val(Value::Struct(parts)))
            }
            _ => Ok(val.clone()),
        }
    }

    pub(super) fn reborrow_shared(&mut self, v: Value) -> Result<Value, Flow> {
        let Value::Ref { obj, path, pointee, token, mode, .. } = v else { return Ok(v) };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        self.affine(token, mode == Mode::Exclusive)?;
        let ancestors = vec![token];
        if self.clash(obj, &path, Mode::Shared, &ancestors) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let t = self.fresh_id();
        let anc = self.record_token_ancestors(t, &ancestors);
        Ok(Value::Ref { obj, path, mode: Mode::Shared, pointee, token: t, range: None, anc })
    }

    // `[Read-Fn-Through-Shared]` (spec/08, D-0218): a value holding `fn`
    // values, reached through a shared reference and read or called, must
    // hold no closure with an exclusive capture: the closure's body would
    // write through it.
    pub(super) fn fn_through_shared(&mut self, e: &Expr, r: EvalResult) -> EvalOutcome {
        if !self.items.any_shared_fn_reads.load(std::sync::atomic::Ordering::Relaxed) || !self.items.shared_fn_reads.lock().unwrap().contains(&(e as *const Expr as usize)) {
            return Ok(r);
        }
        // A closure object itself (a closure binding borrowed as a `fn`
        // reference, `(*f)(x)`) is a struct of its captures.
        let (v, is_closure) = match &r {
            EvalResult::Place(o, p, _) => (self.read_place(*o, p), matches!(self.type_at(*o, p), Type::Closure(_))),
            EvalResult::Temp(o) => match self.objects.get(o) {
                Some(ob) => (ob.value.clone(), matches!(ob.ty, Type::Closure(_))),
                None => return Ok(r),
            },
            EvalResult::Val(v) => (v.clone(), false),
        };
        // D-0239: nor a `move` closure that writes a value it captured: the
        // call would change what the shared reference reaches.
        let self_writing = match &r {
            EvalResult::Place(o, p, _) if is_closure && p.is_empty() => self.closure_writes_self(*o),
            EvalResult::Temp(o) if is_closure => self.closure_writes_self(*o),
            _ => false,
        };
        if self_writing || self.holds_self_writing(&v, 0) {
            return Err(Flow::Fault(crate::modres::named("diag.borrow-exceeds-source", "this closure, reached through a shared reference, changes a value it captured by `move` (D-0239); pass its owner by `ref<_, exclusive>`".to_string())));
        }
        if self.holds_exclusive_capture(&v, is_closure, 0) {
            return Err(Flow::Fault(crate::modres::named("diag.borrow-exceeds-source", "this closure, reached through a shared reference, writes through an exclusive reference it captured (D-0218); pass its owner by `ref<_, exclusive>`".to_string())));
        }
        Ok(r)
    }

    // D-0239: whether `v` holds a `fn` value whose closure writes its own
    // captures (a `move` closure whose body writes one), at any depth.
    pub(super) fn holds_self_writing(&self, v: &Value, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        match v {
            Value::Closure(h) => {
                self.closure_writes_self(h.id()) || self.objects.get(&h.id()).map_or(false, |ob| self.holds_self_writing(&ob.value, depth + 1))
            }
            Value::Struct(fs) | Value::Array(fs) => fs.iter().any(|f| self.holds_self_writing(f, depth + 1)),
            Value::Enum(_, p) => self.holds_self_writing(p, depth + 1),
            _ => false,
        }
    }

    // Whether `v` holds a closure one of whose captures is (or holds) an
    // exclusive reference.
    pub(super) fn holds_exclusive_capture(&self, v: &Value, in_closure: bool, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        match v {
            Value::Ref { mode: Mode::Exclusive, .. } => in_closure,
            Value::Closure(h) => match self.objects.get(&h.id()) {
                Some(ob) => self.holds_exclusive_capture(&ob.value, true, depth + 1),
                None => false,
            },
            Value::Struct(fs) | Value::Array(fs) => fs.iter().any(|f| self.holds_exclusive_capture(f, in_closure, depth + 1)),
            Value::Enum(_, p) => self.holds_exclusive_capture(p, in_closure, depth + 1),
            _ => false,
        }
    }

    pub(super) fn eval_deref(&mut self, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let r = self.eval(inner, subst)?;
        self.eval_deref_result(r)
    }

    pub(super) fn eval_deref_result(&mut self, r: EvalResult) -> EvalOutcome {
        // `*e` on a `ref`/`guard` is `[Ref-Deref-Place]`/`[Guard-Deref]`,
        // not `[Read]` — a guard is `is-resource` but dereferencing it
        // unwraps the lock-path token itself, not a value `[Read]` would
        // reject; peek the stored token directly rather than through the
        // general (resource-rejecting) value-read path.
        let v = match r {
            EvalResult::Val(v) => v,
            EvalResult::Place(o, p, anc) => {
                // Reading the reference `r` is a `[Read]` of its own place:
                // an exclusive borrow of `r` itself (`&mut r`, or a closure
                // capturing `r` whose body writes through it) clashes.
                if self.clash(o, &p, Mode::Shared, &anc) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                self.read_place(o, &p)
            }
            EvalResult::Temp(o) => self.objects.get(&o).unwrap().value.clone(),
        };
        let excl = matches!(&v, Value::Ref { mode: Mode::Exclusive, .. } | Value::Guard { .. });
        match v {
            Value::Ref { obj, path, token, anc, .. } | Value::Guard { obj, path, token, anc, .. } => {
                self.affine(token, excl)?;
                // The referent object may have ended since this token was
                // formed (`[Object-End]` — e.g. `release`/`deallocate`
                // freed the reclaimed object it pointed into). Per
                // `[Read-Stale]`, dereferencing a reference into an ended
                // object is a dynamic `diag.stale-binding` fault, checked
                // here at use-time, not a Rust-level panic on a missing
                // object id.
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                // `*r`'s place literally *is* the path `r` holds
                // (`[Ref-Deref-Place]`), so `r`'s own token is this
                // access's ancestor and must be excluded from its clash
                // check (spec/08 `ancestors`); `r`'s own recorded
                // ancestors follow transitively through `token_ancestors`
                // at `clash` time. The chain travels with this one
                // access only: it must not be remembered against the
                // place, or every later access to it by anyone would
                // inherit `r` as an ancestor and `r` would stop being a
                // conflict witness for as long as it lives.
                // `r`'s ancestors go with it: when `r` is a temporary (a
                // call's result, `*f()`), the table entry is weak and ends
                // with the value, before the access is checked.
                let mut chain = vec![token];
                if let Some(a) = &anc {
                    chain.extend(a.iter().copied());
                }
                Ok(EvalResult::Place(obj, path, chain))
            }
            Value::Rawptr(addr, pty) => {
                self.require_unsafe()?;
                if self.is_resource(&pty) {
                    // [Rawptr-Move-Out]: recover the value and end whatever
                    // reclaimed identity previously sat at this address.
                    if let Some(&old) = self.reclaimed_by_addr.get(&addr) {
                        self.check_not_foreign(&[old])?;
                    }
                    let v = self.arena_read(addr, &pty);
                    if let Some(old_id) = self.reclaimed_by_addr.remove(&addr) {
                        self.reclaimed_addr.remove(&old_id);
                        self.remove_object(&old_id);
                    }
                    // The cells are uninitialised now; what they held
                    // moved with the value.
                    let n = self.sizeof_ty(&pty);
                    self.clear_side(addr, n);
                    let id = self.new_object(pty, v);
                    Ok(EvalResult::Temp(id))
                } else {
                    let v = self.arena_read(addr, &pty);
                    Ok(EvalResult::Val(v))
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Is object `obj` still a live target `read_place`/`write_place` can
    // reach — either ordinary fresh storage (`self.objects`) or a
    // currently-attached reclaimed identity (`self.reclaimed_addr`)? False
    // once `[Object-End]` has ended it (e.g. `release`/`deallocate`).
    pub(super) fn object_live(&self, obj: u64) -> bool {
        self.objects.contains_key(&obj) || self.reclaimed_addr.contains_key(&obj)
    }

    // `e.f`: `[Field-Access]` if `e` is already a place on the aggregate
    // itself, `[Field-Access-Auto-Deref]` if it's (or resolves through) a
    // reference/guard — chased transitively since a binding can hold a
    // `ref` whose own place is what we get back from evaluating `base`.
    // Fallible: `[Read-Stale]` if any reference in the chain now targets
    // an ended object (spec/08 `ancestors`/`[Object-End]`), rather than
    // panicking on a missing object id.
    pub(super) fn resolve_through_refs(&mut self, mut obj: u64, mut path: Vec<Proj>, mut chain: Vec<u64>) -> Result<(u64, Vec<Proj>, Vec<u64>), Flow> {
        #[derive(PartialEq)]
        enum Kind {
            Ref,
            Guard,
            Other,
        }
        loop {
            let kind = self.with_type_at(obj, &path, |t| match t {
                Type::Ref(..) => Kind::Ref,
                Type::Guard(..) => Kind::Guard,
                _ => Kind::Other,
            });
            match kind {
                Kind::Ref => {
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    match self.read_place(obj, &path) {
                        Value::Ref { obj: o2, path: p2, token, mode, .. } => {
                            self.affine(token, mode == Mode::Exclusive)?;
                            chain.push(token);
                            obj = o2;
                            path = p2;
                        }
                        _ => break,
                    }
                }
                Kind::Guard => {
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    match self.read_place(obj, &path) {
                        Value::Guard { obj: o2, path: p2, token, .. } => {
                            self.affine(token, true)?;
                            chain.push(token);
                            obj = o2;
                            path = p2;
                        }
                        _ => break,
                    }
                }
                Kind::Other => break,
            }
        }
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        Ok((obj, path, chain))
    }

    // Records `token`'s persistent ancestor set (spec/08 `ancestors`) as
    // the closure of `direct` (the excludes chain active when `token` was
    // formed) union each of those tokens' own already-recorded ancestors
    // — so a chain of sub-borrows (a sub-borrow of a sub-borrow) flattens
    // to one direct lookup at `clash` time instead of needing recursion
    // there. Call this exactly once, at the point a genuinely new
    // `Ref`/`Guard` token is minted from an existing place (not when a
    // value carrying an existing token is merely copied).
    // The set is returned for the new value to carry: the table holds it
    // weakly, so the entry ends with the last value carrying the token,
    // stored or in flight, and dead entries are dropped as the table
    // doubles.
    pub(super) fn record_token_ancestors(&mut self, token: u64, direct: &[u64]) -> Option<Arc<Vec<u64>>> {
        let mut set: IdSet = direct.iter().cloned().collect();
        for d in direct {
            if let Some(anc) = self.token_ancestors.get(d).and_then(|w| w.upgrade()) {
                set.extend(anc.iter().cloned());
            }
        }
        if set.is_empty() {
            return None;
        }
        let mut v: Vec<u64> = set.into_iter().collect();
        v.sort_unstable();
        let a = Arc::new(v);
        self.token_ancestors.insert(token, Arc::downgrade(&a));
        if self.token_ancestors.len() > self.token_prune_at {
            self.token_ancestors.retain(|_, w| w.strong_count() > 0);
            self.token_prune_at = self.token_ancestors.len() * 2 + 4096;
        }
        Some(a)
    }

    // Mode-monotonicity (spec/08 [Borrow-Exceeds-Source]/[Write-Not-
    // Exclusive]): does evaluating `e` as a place, right now, cross a
    // `ref<_, shared>` — an explicit `*r`, or an auto-deref field/index
    // access through a shared `r` ([Field-Access-Auto-Deref])? Computed
    // fresh from `e`'s own structure and the *current* value of whatever
    // it dereferences every time this is called — deliberately never
    // cached/persisted anywhere on `Interp`. A persistent table keyed
    // only by (obj, path) cannot distinguish "this exact access, right
    // now, goes through a shared reference" from "some unrelated,
    // possibly-already-ended access to the same storage happened to go
    // through one earlier" — the same underlying storage is legitimately
    // reachable through both an exclusive and a shared reference at
    // different points in one program (e.g. `outer` is exclusively
    // owned, but a library call like `Vec::index_shared(&outer, 0)`
    // transiently reads through a *shared* reborrow of it internally;
    // that transient read must never taint a later, unrelated
    // `&mut outer`) — found the hard way, via a real regression, while
    // fixing this rule.
    pub(super) fn crosses_shared_ref(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<bool, Flow> {
        // Peeking evaluates, so an expression with a call is never peeked:
        // the call would run twice (`&mut *HashMap::entry(&mut m, k, 0)`
        // would move `k` twice). `*f(…)` crosses a shared reference when
        // `f` is declared to return one; any other such case is left to
        // the static check (`typecheck`'s `crosses_shared`).
        if crate::parser::has_call(e) {
            return Ok(match &e.kind {
                ExprKind::Deref(inner) => match &inner.kind {
                    ExprKind::Call(callee, _) => match &callee.kind {
                        ExprKind::Path(segs, _) => {
                            self.items.fns.get(&segs.join("::")).map_or(false, |f| matches!(&f.ret, Type::Ref(_, Mode::Shared)))
                        }
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            });
        }
        match &e.kind {
            ExprKind::Deref(inner) => {
                // Peek the raw value directly, like `eval_deref` itself
                // does — never through `eval_to_value`/`result_to_value`,
                // which reject `is-resource` values; a `guard<T>` is
                // `is-resource` but its own interior (what *this* peek is
                // checking the mode of) may legitimately be a resource
                // too (`&mut *g` on a `lock`ed resource-typed mutex).
                let ir = self.eval(inner, subst)?;
                let v = match ir {
                    EvalResult::Val(v) => v,
                    EvalResult::Place(o, p, _) => self.read_place(o, &p),
                    EvalResult::Temp(o) => self.objects.get(&o).unwrap().value.clone(),
                };
                Ok(matches!(v, Value::Ref { mode: Mode::Shared, .. }))
            }
            ExprKind::Field(base, _) | ExprKind::Index(base, _) => {
                let bv = self.eval(base, subst)?;
                let peeked = match &bv {
                    EvalResult::Val(v) => Some(v.clone()),
                    EvalResult::Place(o, p, _) => Some(self.read_place(*o, p)),
                    EvalResult::Temp(o) => self.objects.get(o).map(|obj| obj.value.clone()),
                };
                if matches!(peeked, Some(Value::Ref { mode: Mode::Shared, .. })) {
                    return Ok(true);
                }
                // Not shared at *this* hop — but an earlier hop further
                // down a longer chain (`r.a.b` where `r` is shared) must
                // still be seen.
                match &base.kind {
                    ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) => {
                        self.crosses_shared_ref(base, subst)
                    }
                    _ => Ok(false),
                }
            }
            ExprKind::Paren(inner) => self.crosses_shared_ref(inner, subst),
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                // A borrow-captured name is `*self.f_i`: crossing a
                // shared capture reference is crossing a shared ref.
                match self.lookup(&segs[0]) {
                    Some((obj, _)) if self.capture_cells.contains(&obj) => {
                        Ok(matches!(self.read_place(obj, &[]), Value::Ref { mode: Mode::Shared, .. }))
                    }
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }
}
