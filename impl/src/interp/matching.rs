// match and its patterns.

use super::*;

impl Interp {
    pub(super) fn eval_match(&mut self, scrut: &Expr, arms: &[Arm], subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        let r = self.eval(scrut, subst)?;
        // `[Match-By-Ref]` (D-0046): a scrutinee whose value is a reference.
        let refv = match &r {
            EvalResult::Val(v @ Value::Ref { .. }) => Some(v.clone()),
            EvalResult::Place(o, p, _) => match self.read_place(*o, p) {
                v @ Value::Ref { .. } => Some(v),
                _ => None,
            },
            EvalResult::Temp(o) => match self.objects.get(o).map(|x| x.value.clone()) {
                Some(v @ Value::Ref { .. }) => {
                    self.stmt_temp(*o);
                    Some(v)
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(Value::Ref { obj, path, mode, token, .. }) = refv {
            return self.eval_match_by_ref(obj, path, mode, token, arms, subst, expected);
        }
        // D-0058: a first arm that matches everything (`_`, or a binder of
        // the whole value) needs no discriminant: the scrutinee may be of
        // any type.
        if let Some(a0) = arms.first().filter(|a| a.variant.is_none() && a.lit.is_none()) {
            let bound: Option<(String, Type, Value)> = match (&a0.binder, r) {
                (None, EvalResult::Temp(o)) => {
                    self.stmt_temp(o);
                    None
                }
                (None, _) => None,
                (Some(b), EvalResult::Val(v)) => Some((b.clone(), self.type_of_value(&v), v)),
                (Some(b), EvalResult::Temp(o)) => {
                    let ty = self.type_at(o, &[]);
                    let v = self.take_whole(o, &[])?;
                    Some((b.clone(), ty, v))
                }
                (Some(b), EvalResult::Place(o, p, excl)) => {
                    if self.clash(o, &p, Mode::Shared, &excl) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    let ty = self.type_at(o, &p);
                    let v = self.take_whole(o, &p)?;
                    Some((b.clone(), ty, v))
                }
            };
            self.push_frame();
            if let Some((b, ty, v)) = bound {
                let id = self.new_object(ty, v);
                self.bind(&b, id);
            }
            let body_r = match expected {
                Some(ty) => self.eval_expected(&a0.body, subst, ty),
                None => self.eval(&a0.body, subst),
            }
            .and_then(|r| self.into_result_form(r));
            return match body_r {
                Ok(res) => {
                    let keep = match &res {
                        EvalResult::Temp(o) => Some(*o),
                        _ => None,
                    };
                    self.pop_frame(keep)?;
                    Ok(res)
                }
                Err(flow) => {
                    let keep = returned_temp(&flow);
                    self.pop_frame(keep)?;
                    Err(flow)
                }
            };
        }
        // D-0057: an integer or `bool` scrutinee, matched against literals;
        // D-0127: a text scrutinee (`str`, `String`, `StringView`), matched
        // by its bytes. `text` is the bytes of a text scrutinee, and
        // `whole` where the value sits, for a binder arm to take it whole
        // (moved out when it is a `String`, as D-0058's binder does).
        let scalar: Option<(Value, Type, Option<Vec<u8>>, Option<(u64, Vec<Proj>)>)> = match &r {
            EvalResult::Val(v @ (Value::Int(..) | Value::Bool(_))) => Some((v.clone(), self.type_of_value(v), None, None)),
            EvalResult::Val(Value::Str(b)) => Some((Value::Str(b.clone()), Type::Str, Some(b.to_vec()), None)),
            EvalResult::Place(o, p, _) => {
                let ty = self.type_at(*o, p);
                match self.read_place(*o, p) {
                    v @ (Value::Int(..) | Value::Bool(_)) => Some((v, ty, None, None)),
                    v => match self.text_bytes_of(&v, &ty) {
                        Some(b) => Some((v, ty, Some(b), Some((*o, p.clone())))),
                        None => None,
                    },
                }
            }
            EvalResult::Temp(o) => match self.objects.get(o).map(|x| (x.ty.clone(), x.value.clone())) {
                Some((ty, v @ (Value::Int(..) | Value::Bool(_)))) => {
                    self.stmt_temp(*o);
                    Some((v, ty, None, None))
                }
                Some((ty, v)) => match self.text_bytes_of(&v, &ty) {
                    Some(b) => {
                        self.stmt_temp(*o);
                        Some((v, ty, Some(b), Some((*o, Vec::new()))))
                    }
                    None => None,
                },
                None => None,
            },
            _ => None,
        };
        if let Some((v, ty, text, whole)) = scalar {
            let arm = arms
                .iter()
                .find(|a| {
                    a.variant.is_none()
                        && a.lit.as_ref().map_or(true, |l| match (&l.kind, &text) {
                            (ExprKind::StrLit(t), Some(b)) => t.as_bytes() == b.as_slice(),
                            (ExprKind::StrLit(_), None) => false,
                            _ => lit_equals(l, &v),
                        })
                })
                .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
            self.push_frame();
            if let (Some(b), None) = (&arm.binder, &arm.lit) {
                let v = match &whole {
                    Some((o, p)) if self.is_resource(&ty) => self.take_whole(*o, p)?,
                    _ => v.clone(),
                };
                let id = self.new_object(ty.clone(), v);
                self.bind(b, id);
            }
            let body_r = self.eval_stmt_scoped(&arm.body, subst, expected).and_then(|r| self.into_result_form(r));
            return match body_r {
                Ok(res) => {
                    let keep = match &res {
                        EvalResult::Temp(o) => Some(*o),
                        _ => None,
                    };
                    self.pop_frame(keep)?;
                    Ok(res)
                }
                Err(flow) => {
                    let keep = returned_temp(&flow);
                    self.pop_frame(keep)?;
                    Err(flow)
                }
            };
        }
        let (obj, path, excl) = match r {
            EvalResult::Place(o, p, excl) => (o, p, excl),
            EvalResult::Temp(o) => {
                // A temporary scrutinee ends with the statement ([Stmt-Exit]),
                // like any other; a resource one is consumed below instead.
                // Left alone, an `Option<ref<…>>` from a call kept its
                // reference live, and destroying the referent later failed.
                self.stmt_temp(o);
                (o, vec![], vec![])
            }
            EvalResult::Val(_) => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if self.clash(obj, &path, Mode::Shared, &excl) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let scrutv = self.read_place(obj, &path);
        let (vi, is_temp_root) = match &scrutv {
            Value::Enum(vi, _) => (*vi, path.is_empty() && matches!(r_is_temp(obj, &self.objects), true)),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let sty = self.type_at(obj, &path);
        let enum_name = if let Type::Named(n, _) = &sty { n.clone() } else { String::new() };
        let e = self.items.enums.get(&enum_name).cloned();
        let variant_name = e.as_ref().and_then(|e| e.variants.get(vi)).map(|v| v.name.clone());
        let _ = (vi, &variant_name, &e);
        // D-0056: the first arm whose pattern the value matches.
        let arm = arms
            .iter()
            .find(|a| self.arm_matches(obj, &path, a))
            .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
        self.push_frame();
        let mut moved_out = false;
        let mut bpath = path.clone();
        for _ in 0..arm.chain().len() {
            bpath.push(Proj::Payload);
        }
        // D-0109: a pattern that looked through a reference binds a
        // reference to what it reached.
        let crossed = match (&arm.binder, self.pattern_target(obj, &path, arm)) {
            (Some(_), Some((o, p, Some((m, t))))) => Some((o, p, m, t)),
            _ => None,
        };
        if let (Some(binder), Some((o, p, m, t))) = (&arm.binder, crossed) {
            if let Err(f) = self.bind_ref_binder(binder, o, p, m, t) {
                self.pop_frame(None)?;
                return Err(f);
            }
        } else if let (Some(binder), true) = (&arm.binder, arm.chain().is_empty()) {
            // D-0058: the whole value, moved out when it is a resource.
            let ty = self.type_at(obj, &path);
            let whole = match self.take_whole(obj, &path) {
                Ok(v) => v,
                Err(f) => {
                    self.pop_frame(None)?;
                    return Err(f);
                }
            };
            let id = self.new_object(ty, whole);
            self.bind(binder, id);
        } else if let Some(binder) = &arm.binder {
            let payload_ty = self.type_at(obj, &bpath);
            let payload = self.read_place(obj, &bpath);
            if self.is_resource(&payload_ty) {
                if !path.is_empty() {
                    self.pop_frame(None)?;
                    return Err(Flow::Fault("diag.move-out-of-field".into()));
                }
                // [Relocate-Out]: the payload is *moved* out — vacate the
                // scrutinee's own copy so the destroy-composite pass below
                // (or the enum object's eventual end, for the
                // through-a-reference case rejected above) does not also
                // destroy this same value a second time.
                let moved_payload = payload;
                self.write_place(obj, &bpath, Value::Unit);
                let id = self.new_object(payload_ty, moved_payload);
                self.bind(binder, id);
                moved_out = true;
            } else {
                let id = self.new_object(payload_ty, payload);
                self.bind(binder, id);
            }
        }
        // D-0049: the scrutinee is consumed only when this arm moved its
        // resource payload out; otherwise it stays with its owner (a
        // temporary one ends with the statement).
        let consumed_ok = {
            let ety = self.type_at(obj, &[]);
            if moved_out && self.is_resource(&ety) && path.is_empty() {
                self.invalidate_binding_pointing_at(obj);
                self.destroy_object(obj)
            } else {
                Ok(())
            }
        };
        let body_r = self.eval_stmt_scoped(&arm.body, subst, expected).and_then(|r| self.into_result_form(r));
        let _ = is_temp_root;
        match body_r {
            Ok(res) => {
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                consumed_ok?;
                Ok(res)
            }
            Err(flow) => {
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                consumed_ok?;
                Err(flow)
            }
        }
    }

    // D-0058: the whole value at `path` in `obj` for a binder: moved out
    // of a whole binding or temporary when it is a resource (the object
    // ends), copied otherwise; a resource through a projection is
    // `[Match-Move-Through-Ref]`.
    pub(super) fn take_whole(&mut self, obj: u64, path: &[Proj]) -> Result<Value, Flow> {
        let ty = self.type_at(obj, path);
        if !self.is_resource(&ty) {
            return Ok(self.read_place(obj, path));
        }
        if !path.is_empty() {
            return Err(Flow::Fault("diag.move-out-of-field".into()));
        }
        self.result_to_value_resource_aware(EvalResult::Place(obj, Vec::new(), Vec::new()))
    }

    // D-0056: whether the enum at `path` in `obj` has, level by level
    // down its payloads, the variants arm `a`'s pattern names.
    pub(super) fn arm_matches(&self, obj: u64, path: &[Proj], a: &Arm) -> bool {
        self.pattern_target(obj, path, a).is_some()
    }

    // The place arm `a`'s binder would bind, when the value matches its
    // pattern, and the mode and token of the references the pattern
    // looked through below its first level (D-0109; the weaker mode).
    pub(super) fn pattern_target(&self, obj: u64, path: &[Proj], a: &Arm) -> Option<(u64, Vec<Proj>, Option<(Mode, u64)>)> {
        let (mut o, mut p) = (obj, path.to_vec());
        let mut cross: Option<(Mode, u64)> = None;
        let chain = a.chain();
        let through = |me: &Self, o: &mut u64, p: &mut Vec<Proj>, cross: &mut Option<(Mode, u64)>| {
            if let Value::Ref { obj: o2, path: p2, mode, token, .. } = me.read_place(*o, p) {
                let m = if mode == Mode::Shared || matches!(cross, Some((Mode::Shared, _))) { Mode::Shared } else { Mode::Exclusive };
                *cross = Some((m, token));
                *o = o2;
                *p = p2;
            }
        };
        for (k, vn) in chain.iter().enumerate() {
            if k > 0 {
                through(self, &mut o, &mut p, &mut cross);
            }
            let Value::Enum(vi, _) = self.read_place(o, &p) else {
                return None;
            };
            let name = match self.type_at(o, &p) {
                Type::Named(en, _) => self.items.enums.get(&en).and_then(|e| e.variants.get(vi)).map(|v| v.name.clone()),
                _ => None,
            };
            if name.as_deref() != Some(vn.as_str()) {
                return None;
            }
            p.push(Proj::Payload);
        }
        if let Some(l) = &a.lit {
            if !chain.is_empty() {
                through(self, &mut o, &mut p, &mut cross);
            }
            let v = self.read_place(o, &p);
            let hit = match &l.kind {
                // D-0127: a text literal against the payload's bytes.
                ExprKind::StrLit(t) => self.text_bytes_of(&v, &self.type_at(o, &p)).map_or(false, |b| b == t.as_bytes()),
                _ => lit_equals(l, &v),
            };
            if !hit {
                return None;
            }
        }
        Some((o, p, cross))
    }

    // A binder bound as a reference of `mode` to `path` in `obj`, derived
    // from the reference whose token is `from` (D-0046, D-0109).
    pub(super) fn bind_ref_binder(&mut self, binder: &str, obj: u64, path: Vec<Proj>, mode: Mode, from: u64) -> Result<(), Flow> {
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        let pty = self.type_at(obj, &path);
        let ancestors = vec![from];
        if self.clash(obj, &path, mode.clone(), &ancestors) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let t = self.mint_token(&mode);
        let anc = self.record_token_ancestors(t, &ancestors);
        let rv = Value::Ref { obj, path, mode: mode.clone(), pointee: pty.clone(), token: t, range: None, anc };
        let id = self.new_object(Type::Ref(Box::new(pty), mode), rv);
        self.bind(binder, id);
        Ok(())
    }

    // `match` through a reference (D-0046): the arm is chosen by the
    // referent's variant, and a binder is a reference of the scrutinee's
    // mode to the payload, derived from the scrutinee's reference.
    pub(super) fn eval_match_by_ref(
        &mut self,
        obj: u64,
        path: Vec<Proj>,
        mode: Mode,
        token: u64,
        arms: &[Arm],
        subst: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> EvalOutcome {
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        let catch_all_first = arms.first().map_or(false, |a| a.variant.is_none() && a.lit.is_none());
        if !catch_all_first && !matches!(self.read_place(obj, &path), Value::Enum(..)) {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        // D-0056: the first arm whose pattern the referent matches.
        let arm = arms
            .iter()
            .find(|a| self.arm_matches(obj, &path, a))
            .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
        self.push_frame();
        if let Some(binder) = &arm.binder {
            let (o, ppath, cross) = self.pattern_target(obj, &path, arm).unwrap_or((obj, path.clone(), None));
            let (m, from) = match cross {
                Some((cm, ct)) => (if cm == Mode::Shared || mode == Mode::Shared { Mode::Shared } else { Mode::Exclusive }, ct),
                None => (mode, token),
            };
            if let Err(f) = self.bind_ref_binder(binder, o, ppath, m, from) {
                self.pop_frame(None)?;
                return Err(f);
            }
        }
        let body_r = self.eval_stmt_scoped(&arm.body, subst, expected).and_then(|r| self.into_result_form(r));
        match body_r {
            Ok(res) => {
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(flow) => {
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                Err(flow)
            }
        }
    }
}
