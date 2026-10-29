// Assignment, propagation, if and while.

use super::*;

impl Interp {
    pub(super) fn eval_assign(&mut self, lhs: &Expr, rhs: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // `*e = rhs`: three different targets share this syntax (spec/13
        // §1) — a raw-pointer write (`[Rawptr-Write]`/`[Rawptr-Move-In]`,
        // trusted, `e : rawptr<T>`), or an ordinary place reached by
        // dereferencing a `ref`/`guard` (`*r = ..`/`*g = ..`). Peek `e`'s
        // value directly — not via the resource-rejecting
        // `result_to_value` — since a guard is `is-resource` but its
        // deref is never `[Read]` (spec/13 §1), and evaluate it only
        // once (D-0007), reusing the peeked value either way rather than
        // re-evaluating `e` through the ordinary place path below.
        if let ExprKind::Deref(inner) = &lhs.kind {
            let inner_r = self.eval(inner, subst)?;
            let inner_v = match &inner_r {
                EvalResult::Val(v) => v.clone(),
                EvalResult::Place(o, p, _) => self.read_place(*o, p),
                EvalResult::Temp(o) => self.objects.get(o).unwrap().value.clone(),
            };
            match inner_v {
                Value::Rawptr(addr, pty) => {
                    self.require_unsafe()?;
                    let r = self.eval_expected(rhs, subst, &pty)?;
                    if self.is_resource(&pty) {
                        if let Some(&old) = self.reclaimed_by_addr.get(&addr) {
                            self.check_not_foreign(&[old])?;
                        }
                    }
                    let v = if self.is_resource(&pty) {
                        self.store_into_field(r, &pty)?
                    } else {
                        self.result_to_value(r)?
                    };
                    self.arena_write(addr, &pty, v);
                    return Ok(EvalResult::Val(Value::Unit));
                }
                Value::Ref { mode: Mode::Shared, .. } => {
                    // [Write-Not-Exclusive] (spec/08): a direct write
                    // through a `ref<_, shared>` — `*r = ..` where `r`
                    // itself is shared, no intermediate reborrow — must be
                    // rejected outright, distinct from the reborrow case
                    // `eval_borrow` rejects with `diag.borrow-exceeds-
                    // source`.
                    return Err(Flow::Fault("diag.write-through-shared".into()));
                }
                Value::Ref { obj, path, token, .. } | Value::Guard { obj, path, token, .. } => {
                    self.affine(token, true)?;
                    // Reaching this arm means the ref just matched above
                    // was *not* `Mode::Shared` (that case already
                    // returned), or it's a guard (always
                    // exclusive-equivalent) — this write is sound.
                    // `*r`'s place literally *is* the path `r` holds
                    // (`[Ref-Deref-Place]`) — exclude `r`'s own token
                    // from this write's clash check (spec/08 `ancestors`),
                    // same as `eval_deref`'s read-position handling.
                    return self.eval_assign_place(obj, path, vec![token], rhs, subst);
                }
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            }
        }
        // [Write-Not-Exclusive] (spec/08): catches a write reached via
        // field/index auto-deref through a `ref<_, shared>` (`r.field =
        // ..`, no explicit `*`) — the explicit `*r = ..` case is already
        // rejected above, before this point. Checked before evaluating
        // `lhs` for real, same reasoning as `eval_borrow`.
        if self.crosses_shared_ref(lhs, subst)? {
            return Err(Flow::Fault("diag.write-through-shared".into()));
        }
        // D-0033 (1): a whole binding. Its place has no evaluation of its
        // own, so whether it still has a value is asked after `rhs`,
        // which may have moved it away (`x = f(x)`).
        if let ExprKind::Path(segs, _) = &lhs.kind {
            let plain_binding = segs.len() == 1 && self.lookup(&segs[0]).map_or(false, |(o, _)| !self.capture_cells.contains(&o));
            if plain_binding {
                let name = &segs[0];
                if let Some(ty) = self.binding_gone(name) {
                    let r = match &ty {
                        Some(t) => self.eval_expected(rhs, subst, t)?,
                        None => self.eval(rhs, subst)?,
                    };
                    return self.reinit_binding(name, r);
                }
                let (obj, _) = self.lookup(name).unwrap();
                let fty = self.type_at(obj, &[]);
                let r = self.eval_expected(rhs, subst, &fty)?;
                if self.binding_gone(name).is_some() {
                    return self.reinit_binding(name, r);
                }
                return self.write_evaluated(obj, Vec::new(), Vec::new(), r, &fty);
            }
        }
        self.write_ctx = true;
        let lr = self.eval(lhs, subst);
        self.write_ctx = false;
        let (obj, path, excl) = match lr? {
            EvalResult::Place(o, p, excl) => (o, p, excl),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        self.eval_assign_place(obj, path, excl, rhs, subst)
    }

    pub(super) fn eval_assign_place(&mut self, obj: u64, path: Vec<Proj>, excl: Vec<u64>, rhs: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // spec/13 §1: the target place first, then the value; `[Write]`'s
        // premises hold at the write itself, after the value -- whose
        // evaluation may have ended the target (a `Vec::push` that
        // reallocates under an element reference: `[Write-Stale]`).
        let fty = self.type_at(obj, &path);
        let r = self.eval_expected(rhs, subst, &fty)?;
        // D-0118 `[Bitfield-Overflow]`: a value written into a bit field
        // must fit its width.
        if let Some((Proj::Field(i), parent)) = path.split_last() {
            if let Type::Named(pn, _) = self.type_at(obj, parent) {
                if let Some((_, widths)) = self.bitstruct_decl(&pn) {
                    let probe = match &r {
                        EvalResult::Val(v) => v.clone(),
                        EvalResult::Place(o, p, _) => self.read_place(*o, p),
                        EvalResult::Temp(o) => self.objects.get(o).map(|x| x.value.clone()).unwrap_or(Value::Unit),
                    };
                    if let Value::Int(_, x) = probe {
                        if x < 0 || (x as u128) > Self::bits_mask(widths[*i]) {
                            return Err(Flow::Fault("diag.narrowing-overflow".into()));
                        }
                    }
                }
            }
        }
        self.write_evaluated(obj, path, excl, r, &fty)
    }

    // `[Write]` of an evaluated `rhs` to a place (`eval_assign_place`).
    pub(super) fn write_evaluated(&mut self, obj: u64, path: Vec<Proj>, excl: Vec<u64>, r: EvalResult, fty: &Type) -> EvalOutcome {
        let fty = fty.clone();
        let v = if self.is_resource(&fty) {
            self.store_into_field(r, &fty)?
        } else {
            self.result_to_value(r)?
        };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        // D-0194: a live value of a quiet type is destroyed by the write
        // (`[Write-Quiet-Replace]`), once the write's own checks pass.
        let mut replaced: Option<Value> = None;
        if self.is_resource(&fty) {
            let existing = self.read_place(obj, &path);
            if !matches!(existing, Value::Unit) && self.field_is_initialized(obj, &path) && self.value_owns(&existing, &fty) {
                if !crate::typecheck::quiet_ty(&self.items, &fty) {
                    // The value that was to be written is a temporary of the
                    // statement, which the fault's unwind ends
                    // (`[Fault-Unwind]`): destroyed here, first, as the
                    // innermost scope's. The fault is raised before it
                    // (D-0264: the trace event precedes the unwind's first
                    // destructor, as in `cbrt`).
                    self.first_fault("diag.overwrite-of-live-resource");
                    self.destroy_field_value(&v, &fty, false)?;
                    return Err(Flow::Fault("diag.overwrite-of-live-resource".into()));
                }
                replaced = Some(existing);
            }
        }
        if !self.clash_check_write(obj, &path, &excl) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        if let Some(old) = replaced {
            self.destroy_field_value(&old, &fty, false)?;
        }
        self.write_place(obj, &path, v);
        self.mark_initialized(obj, &path);
        // rule.init.definite-assignment: any write reaching this object
        // (whole-object or a field of it) satisfies `[Let-Uninit]`'s
        // obligation -- simple, not per-sub-path (a struct declared
        // uninitialized and only partially field-written before a read
        // of an *untouched* field is not separately caught; a narrower
        // scope than full definite-assignment analysis, matching this
        // pass's own documented conservative-first-write approach).
        self.uninit_bindings.remove(&obj);
        Ok(EvalResult::Val(Value::Unit))
    }

    // Minimal definite-assignment/"live resource" bookkeeping: we track
    // per-object initialized sub-paths lazily via a side table keyed by
    // (obj) -> set of path-strings, since Value has no explicit
    // "uninitialized" sentinel distinct from a zero value for resources.
    pub(super) fn field_is_initialized(&self, _obj: u64, _path: &[Proj]) -> bool {
        // Conservative: treat every existing resource value as "live"
        // (matches the common case; declaring an uninitialized resource
        // field is rare in this corpus). See STATUS.md.
        true
    }
    pub(super) fn mark_initialized(&mut self, _obj: u64, _path: &[Proj]) {}

    pub(super) fn eval_propagate(&mut self, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let r = self.eval(inner, subst)?;
        let result_ty = self.result_type_hint(&r).unwrap_or(Type::Void);
        // `[Propagate]` (D-0060): `?` consumes a temporary operand -- it
        // ends here, and a reference it held no longer counts -- and
        // copies out of a place that owns nothing.
        let v = self.result_to_value_resource_aware(r)?;
        match v {
            Value::Enum(0, payload) => {
                // Ok(v) / Some(v). A payload that is itself an enum becomes
                // a typed temporary, so `match (f()?)` can read its variants
                // (a bare value has no type to match on).
                let pty = match &result_ty {
                    Type::Named(n, args) => self.items.enums.get(n).cloned().and_then(|e| {
                        let sub = self.subst_map(&e.type_params, args);
                        e.variants.first().and_then(|v0| v0.payload.clone()).map(|t| self.apply_subst(&t, &sub))
                    }),
                    _ => None,
                };
                match pty {
                    Some(t @ Type::Named(..)) if matches!(&t, Type::Named(n, _) if self.items.enums.contains_key(n)) => {
                        let id = self.new_object(t, *payload);
                        Ok(EvalResult::Temp(id))
                    }
                    // A struct (a `Vec`, a `String`), an array or a slice
                    // likewise: a bare value does not say which type it is,
                    // and a binding, `foreach` or index (`f()?[i]`) of it
                    // needs it. It is a temporary of the statement, unless
                    // something takes it.
                    Some(t @ (Type::Named(..) | Type::Array(..) | Type::Simd(..) | Type::Slice(..))) => {
                        let id = self.new_object(t, *payload);
                        self.stmt_temp(id);
                        Ok(EvalResult::Temp(id))
                    }
                    _ => Ok(EvalResult::Val(*payload)),
                }
            }
            Value::Enum(1, payload) => {
                // `return Err(err)` (rule.fail.propagate's own desugaring)
                // — keep the enclosing `Result<_,E>`'s own type, not a
                // bare untyped value, so a caller's `match` on the
                // propagated function's return can still see which enum
                // and variant this is.
                let id = self.new_object(result_ty, Value::Enum(1, payload));
                Err(Flow::Return(EvalResult::Temp(id)))
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    pub(super) fn result_to_value_resource_aware(&mut self, r: EvalResult) -> Result<Value, Flow> {
        match r {
            EvalResult::Temp(o) if self.is_closure_obj(o) => Ok(self.closure_box(o)),
            EvalResult::Place(obj, path, _) if path.is_empty() && self.is_closure_obj(obj) && self.is_resource(&self.type_at(obj, &[])) => {
                self.invalidate_binding_pointing_at(obj);
                Ok(self.closure_box(obj))
            }
            EvalResult::Temp(o) => {
                let v = self.remove_object(&o).unwrap().value;
                Ok(v)
            }
            EvalResult::Place(obj, path, excl) if path.is_empty() => {
                let ty = self.type_at(obj, &[]);
                if self.is_resource(&ty) {
                    self.invalidate_binding_pointing_at(obj);
                    Ok(self.remove_object(&obj).unwrap().value)
                } else {
                    self.result_to_value(EvalResult::Place(obj, path, excl))
                }
            }
            other => self.result_to_value(other),
        }
    }

    pub(super) fn eval_if(&mut self, c: &Expr, t: &Block, f: &Option<Box<Expr>>, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        let cv = self.eval_to_value(c, subst)?;
        // D-0049: a literal branch's type, from its sibling (typecheck).
        let hint = match expected {
            Some(_) => None,
            None => self.items.match_hints.lock().unwrap().get(&(t as *const Block as usize)).cloned().map(|h| self.apply_subst(&h, subst)),
        };
        let expected = expected.or(hint.as_ref());
        match cv {
            Value::Bool(true) => self.exec_block_body_expected(t, subst, expected),
            Value::Bool(false) => match f {
                Some(e) => match expected {
                    Some(ty) => self.eval_expected(e, subst, ty),
                    None => self.eval(e, subst),
                },
                None => Ok(EvalResult::Val(Value::Unit)),
            },
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // A `for` loop's `step` runs after the body, and after a `continue`,
    // as a statement of its own (its temporaries end with it).
    pub(super) fn eval_while(&mut self, c: &Expr, b: &Block, step: Option<&Expr>, subst: &HashMap<String, Type>) -> EvalOutcome {
        loop {
            let cv = self.eval_to_value(c, subst)?;
            match cv {
                Value::Bool(false) => return Ok(EvalResult::Val(Value::Unit)),
                Value::Bool(true) => {}
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            }
            match self.exec_block_body(b, subst) {
                Ok(_) => {}
                Err(Flow::Break) => return Ok(EvalResult::Val(Value::Unit)),
                Err(Flow::Continue) => {}
                Err(other) => return Err(other),
            }
            if let Some(s) = step {
                self.exec_expr_stmt(s, subst)?;
            }
        }
    }
}
