// Access checks (clash, solitary), sub-values, places and their types.

use super::*;

impl Interp {
    // ---- aliasing: the live references to an object ----

    // The distinct live references (and guards) to `target`: one entry
    // per token — (target path, mode, token, is_guard) — however many
    // holders carry a copy. `clash` and `solitary` need no more; a
    // reference passed down a recursion is held by every frame.
    pub(super) fn distinct_refs_to(&self, target: u64) -> Vec<(Vec<Proj>, Mode, u64, bool)> {
        let mut out: Vec<(Vec<Proj>, Mode, u64, bool)> = Vec::new();
        let mut seen: IdSet = Default::default();
        if let Some(hs) = self.holders_by_target.get(&target) {
            for &holder in hs.iter() {
                if let Some(obj) = self.objects.get(&holder) {
                    Self::scan_distinct(&obj.value, target, &mut seen, &mut out);
                }
            }
        }
        for v in self.arena_side.values() {
            Self::scan_distinct(v, target, &mut seen, &mut out);
        }
        out
    }

    pub(super) fn scan_distinct(v: &Value, target: u64, seen: &mut IdSet, out: &mut Vec<(Vec<Proj>, Mode, u64, bool)>) {
        match v {
            Value::Ref { obj, path: tp, mode, token, range, .. } => {
                if *obj == target && seen.insert(*token) {
                    let mut tp = tp.clone();
                    if let Some((lo, hi)) = range {
                        tp.push(Proj::Range(*lo, *hi));
                    }
                    out.push((tp, mode.clone(), *token, false));
                }
            }
            Value::Guard { obj, path: tp, token, .. } => {
                if *obj == target && seen.insert(*token) {
                    out.push((tp.clone(), Mode::Exclusive, *token, true));
                }
            }
            Value::Struct(fields) => fields.iter().for_each(|f| Self::scan_distinct(f, target, seen, out)),
            Value::Array(items) => items.iter().for_each(|e| Self::scan_distinct(e, target, seen, out)),
            Value::Enum(_, payload) => Self::scan_distinct(payload, target, seen, out),
            _ => {}
        }
    }

    // Is `token` covered by `excludes`, either directly or because `token`
    // is a persistent ancestor (`record_token_ancestors`) of one of the
    // tokens in `excludes`? The latter is what makes a live occurrence of
    // `v`'s own borrow stop conflicting with a later access made through a
    // sub-borrow of `v` formed earlier (e.g. `v.len` inside a function
    // called with `&*v` as its argument, several frames after the
    // sub-borrow was minted) — `excludes` only ever names the *immediate*
    // chain for *this* access; `token_ancestors` is what remembers that an
    // earlier access's chain is part of this one's lineage too.
    pub(super) fn excluded(&self, excludes: &[u64], token: u64) -> bool {
        if excludes.contains(&token) {
            return true;
        }
        excludes.iter().any(|e| self.token_ancestors.get(e).and_then(|w| w.upgrade()).map_or(false, |anc| anc.binary_search(&token).is_ok()))
    }

    // ¬clash(target/path, mode), excluding the occurrences identified by
    // `excludes` (the access-path *tokens* used to reach this very access,
    // if any — spec/08 `ancestors`: a reference is always its own access's
    // ancestor). Token identity, not which object currently holds a copy
    // of the value, is what makes two occurrences "the same path" — e.g.
    // passing a `ref` argument into a callee copies the value but not its
    // token (spec/15 §3), so the caller's and callee's copies must both be
    // excluded together, not just whichever one is lexically "in scope".
    pub(super) fn clash(&self, target: u64, path: &[Proj], mode: Mode, excludes: &[u64]) -> bool {
        if self.verify_due() {
            self.verify_refs_to(target);
        }
        // `sync-exempt` (spec/19 §2, spec/04 §2): a lock-derived path (a
        // guard) never conflicts with a `shared` reference to the mutex
        // it locks, in either direction — that serialization is what
        // `lock`'s own blocking-on-`unlocked` already provides. A guard
        // always projects into the mutex's `inner` sub-range (a non-empty
        // path on a `mutex<_>` target), which no other rule ever reaches,
        // so "this access goes via a guard" is exactly that shape.
        //
        // A mutex may be a field of the object (a struct holding one, shared
        // by reference, CHG-0072): `lock-derived` is then decided by the
        // guard tokens, a path being lock-derived when a guard's token is
        // it or one of its ancestors.
        let refs = self.distinct_refs_to(target);
        let guards: Vec<u64> = refs.iter().filter(|r| r.3).map(|r| r.2).collect();
        let lock_derived = |t: u64| guards.contains(&t) || self.token_ancestors.get(&t).and_then(|w| w.upgrade()).map_or(false, |anc| guards.iter().any(|g| anc.binary_search(g).is_ok()));
        let target_is_mutex = matches!(self.objects.get(&target).map(|o| &o.ty), Some(Type::Mutex(_)));
        let this_access_is_guard_derived = (target_is_mutex && !path.is_empty()) || excludes.iter().any(|&e| lock_derived(e));
        for (tpath, m, token, is_guard) in refs.iter().cloned() {
            if self.excluded(excludes, token) {
                continue;
            }
            if !overlap(path, &tpath) {
                continue;
            }
            let other_lock_derived = is_guard || lock_derived(token);
            if (other_lock_derived && !this_access_is_guard_derived && mode == Mode::Shared) || (this_access_is_guard_derived && !other_lock_derived && m == Mode::Shared) {
                continue;
            }
            if !permitted(mode.clone(), m) {
                return true;
            }
        }
        false
    }

    pub(super) fn solitary(&self, target: u64, excludes: &[u64]) -> bool {
        for (_tpath, _m, token, _is_guard) in self.distinct_refs_to(target) {
            if self.excluded(excludes, token) {
                continue;
            }
            return false;
        }
        true
    }

    // ---- value navigation ----

    pub(super) fn get_sub<'a>(v: &'a Value, path: &[Proj]) -> &'a Value {
        match path.split_first() {
            None => v,
            Some((Proj::Field(i), rest)) => match v {
                Value::Struct(fs) => Self::get_sub(&fs[*i], rest),
                _ => panic!("field projection on non-struct"),
            },
            Some((Proj::Index(i), rest)) => match v {
                Value::Array(es) => Self::get_sub(&es[*i], rest),
                _ => panic!("index projection on non-array"),
            },
            Some((Proj::Payload, rest)) => match v {
                Value::Enum(_, p) => Self::get_sub(p, rest),
                _ => panic!("payload projection on non-enum"),
            },
            // D-0070: a range is compared, never navigated.
            Some((Proj::Range(..), rest)) => Self::get_sub(v, rest),
        }
    }
    pub(super) fn get_sub_mut<'a>(v: &'a mut Value, path: &[Proj]) -> &'a mut Value {
        match path.split_first() {
            None => v,
            Some((Proj::Field(i), rest)) => match v {
                Value::Struct(fs) => Self::get_sub_mut(&mut fs[*i], rest),
                _ => panic!("field projection on non-struct"),
            },
            Some((Proj::Index(i), rest)) => match v {
                Value::Array(es) => Self::get_sub_mut(&mut es[*i], rest),
                _ => panic!("index projection on non-array"),
            },
            Some((Proj::Payload, rest)) => match v {
                Value::Enum(_, p) => Self::get_sub_mut(p, rest),
                _ => panic!("payload projection on non-enum"),
            },
            Some((Proj::Range(..), rest)) => Self::get_sub_mut(v, rest),
        }
    }

    pub fn read_place(&self, obj: u64, path: &[Proj]) -> Value {
        if let Some((addr, ty)) = self.reclaimed_addr.get(&obj) {
            // D-0118: a bit field of a bitstruct in raw memory is read out
            // of the whole backing integer.
            if let Some(k) = self.bitstruct_split(*addr, ty, path) {
                let (off, bt) = self.path_offset_at(Some(*addr), ty, &path[..k]);
                let whole = self.arena_read(addr + off, &bt);
                return Self::get_sub(&whole, &path[k..]).clone();
            }
            let (off, subty) = self.path_offset_at(Some(*addr), ty, path);
            return self.arena_read(addr + off, &subty);
        }
        Self::get_sub(&self.objects.get(&obj).expect("stale object").value, path).clone()
    }

    pub fn write_place(&mut self, obj: u64, path: &[Proj], v: Value) {
        if let Some((addr, ty)) = self.reclaimed_addr.get(&obj).cloned() {
            if let Some(k) = self.bitstruct_split(addr, &ty, path) {
                let (off, bt) = self.path_offset_at(Some(addr), &ty, &path[..k]);
                let mut whole = self.arena_read(addr + off, &bt);
                *Self::get_sub_mut(&mut whole, &path[k..]) = v;
                self.arena_write(addr + off, &bt, whole);
                return;
            }
            let (off, subty) = self.path_offset_at(Some(addr), &ty, path);
            self.arena_write(addr + off, &subty, v);
            return;
        }
        if Self::value_has_ref(&v) {
            self.index_refs(obj, &v);
        }
        let o = self.objects.get_mut(&obj).expect("stale object");
        *Self::get_sub_mut(&mut o.value, path) = v;
    }

    // The capture types of the closure in `obj`: its own entry, else its
    // literal's (`closure_cid_types`).
    pub(super) fn closure_ftys(&self, obj: u64) -> Option<Vec<Type>> {
        if let Some(v) = self.closure_field_types.get(&obj) {
            return Some(v.clone());
        }
        match self.objects.get(&obj).map(|o| &o.ty) {
            Some(Type::Closure(cid)) => self.closure_cid_types.get(cid).cloned(),
            _ => None,
        }
    }

    pub(super) fn type_at(&self, obj: u64, path: &[Proj]) -> Type {
        self.with_type_at(obj, path, |t| t.clone())
    }

    // `type_at`'s type, lent to `f` rather than cloned: what a caller that
    // only looks at it (is it a reference, a resource) needs.
    pub(super) fn with_type_at<R>(&self, obj: u64, path: &[Proj], f: impl FnOnce(&Type) -> R) -> R {
        // A closure's own `.ty` is the opaque `Type::Closure(cid)` tag; a
        // projection into one of its capture fields is resolved from the
        // per-instance capture-type table recorded at closure formation,
        // then the walk below continues normally from there.
        if let Some((Proj::Field(i), _)) = path.split_first() {
            if matches!(self.objects.get(&obj).map(|o| &o.ty), Some(Type::Closure(_))) {
                if let Some(fty) = self.closure_ftys(obj).and_then(|v| v.get(*i).cloned()) {
                    return walk(self, obj, path, 1, &fty, f);
                }
            }
        }
        // `pos` is how much of `path` has been walked to reach `ty`.
        fn walk<R>(interp: &Interp, obj: u64, path: &[Proj], pos: usize, ty: &Type, f: impl FnOnce(&Type) -> R) -> R {
            let Some(step) = path.get(pos) else {
                return f(ty);
            };
            match step {
                Proj::Payload => {
                    // The active variant's payload type: the variant is
                    // read from the enum value this far along the path
                    // (a reference into a payload, D-0046, walks on).
                    if let (Type::Named(name, args), Value::Enum(vi, _)) = (ty, interp.read_place(obj, &path[..pos])) {
                        if let Some(e) = interp.items.enums.get(name) {
                            if let Some(pt) = e.variants.get(vi).and_then(|v| v.payload.as_ref()) {
                                let pt = interp.apply_subst_pos(pt, &e.type_params, args);
                                return walk(interp, obj, path, pos + 1, &pt, f);
                            }
                        }
                    }
                    f(ty)
                }
                Proj::Field(i) => {
                    if let Type::Named(name, args) = ty {
                        if let Some(s) = interp.items.structs.get(name) {
                            // A struct without type parameters: its field's
                            // declared type is the type, with nothing to clone.
                            if s.type_params.is_empty() {
                                return walk(interp, obj, path, pos + 1, &s.fields[*i].ty, f);
                            }
                            let ft = interp.apply_subst_pos(&s.fields[*i].ty, &s.type_params, args);
                            return walk(interp, obj, path, pos + 1, &ft, f);
                        }
                    }
                    if let Type::Mutex(inner) = ty {
                        if *i == 0 {
                            return walk(interp, obj, path, pos + 1, inner, f);
                        }
                    }
                    f(ty)
                }
                Proj::Index(_) => {
                    if let Type::Array(inner, _) | Type::Simd(inner, _) = ty {
                        return walk(interp, obj, path, pos + 1, inner, f);
                    }
                    f(ty)
                }
                Proj::Range(..) => walk(interp, obj, path, pos + 1, ty, f),
            }
        }
        match self.objects.get(&obj) {
            Some(o) => walk(self, obj, path, 0, &o.ty, f),
            None => walk(self, obj, path, 0, &Type::Void, f),
        }
    }

    pub(super) fn type_of_value(&self, v: &Value) -> Type {
        match v {
            Value::Int(t, _) => Type::Int(*t),
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            Value::Bool(_) => Type::Bool,
            Value::Str(_) => Type::Str,
            Value::Unit => Type::Void,
            Value::Ref { mode, pointee, .. } => Type::Ref(Box::new(pointee.clone()), mode.clone()),
            Value::Rawptr(_, t) => Type::Rawptr(Box::new(t.clone())),
            Value::FnVal(_) => Type::Void,
            Value::Closure(h) => self.objects.get(&h.id()).map(|o| o.ty.clone()).unwrap_or(Type::Void),
            Value::Struct(_) => Type::Void,
            Value::Array(_) => Type::Void,
            Value::Enum(_, _) => Type::Void,
            Value::Handle(_, t) => Type::Handle(Box::new(t.clone())),
            Value::Guard { inner, .. } => Type::Guard(Box::new(inner.clone())),
        }
    }
}
