// Mutexes, allocation, addresses, strings interned and conversions.

use super::*;

impl Interp {
    // `[Mutex-New]`: `hint` is `T` when an explicit type argument or the
    // expected `mutex<T>` fixes it, so the argument (a literal, say) is
    // typed from it (`rule.type.expected`, as for any parameter).
    pub(super) fn mutex_new(&mut self, arg: &Expr, subst: &HashMap<String, Type>, hint: Option<&Type>) -> Result<EvalResult, Flow> {
        let r = match hint {
            Some(t) => self.eval_expected(arg, subst, t)?,
            None => self.eval(arg, subst)?,
        };
        let inner_ty = hint.cloned().or_else(|| self.result_type_hint(&r)).unwrap_or(Type::Void);
        let v = if self.is_resource(&inner_ty) {
            self.store_into_field(r, &inner_ty)?
        } else {
            self.result_to_value(r)?
        };
        let ty = Type::Mutex(Box::new(inner_ty));
        let id = self.new_object(ty, Value::Struct(vec![v]));
        Ok(EvalResult::Temp(id))
    }

    // The index answers what a scan of every frame's bindings did; the
    // scan (O(frames) per `drop`, quadratic on deep teardown) was 87% of
    // a 120k-deep drop. A debug assert cross-checked index against scan
    // through one full debug suite run (verify-battery-20260929.log,
    // 1010 tests, 0 failures) and was then removed: the scan itself is
    // too costly to keep even as a debug check.
    pub(super) fn lookup_owns(&self, obj: u64) -> bool {
        self.bound_count_by_thread.get(&self.current_thread).and_then(|m| m.get(&obj)).is_some_and(|c| *c > 0)
    }

    pub(super) fn bump_alloc(&mut self, size: u64, align: u64) -> u64 {
        let align = align.max(1);
        self.next_addr = (self.next_addr + align - 1) / align * align;
        let addr = self.next_addr;
        self.next_addr += size.max(1);
        addr
    }

    pub(super) fn eval_str_arg(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<Arc<[u8]>, Flow> {
        match self.eval_to_value(e, subst)? {
            Value::Str(s) => Ok(s),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // The address of a `str` value's bytes in program data, minted on
    // first use; the same content always yields the same address.
    pub(super) fn intern_str(&mut self, s: &Arc<[u8]>) -> u64 {
        if let Some(&addr) = self.str_data.get(s) {
            return addr;
        }
        let addr = self.bump_alloc(s.len() as u64, 1);
        for (i, b) in s.iter().enumerate() {
            self.arena.insert(addr + i as u64, *b);
        }
        self.str_data.insert(s.clone(), addr);
        addr
    }

    // `encode` is `&self`, so every `str` inside a value bound for the
    // arena gets its program-data address minted here first.
    pub(super) fn intern_strs_in(&mut self, v: &Value) {
        match v {
            Value::Str(s) => {
                self.intern_str(s);
            }
            Value::Struct(fs) | Value::Array(fs) => {
                for f in fs {
                    self.intern_strs_in(f);
                }
            }
            Value::Enum(_, p) => self.intern_strs_in(p),
            _ => {}
        }
    }

    pub(super) fn path_offset(&self, ty: &Type, path: &[Proj]) -> (u64, Type) {
        self.path_offset_at(None, ty, path)
    }

    // The offset and type of `path` within a value of type `ty`. With the
    // value's address (`base`), a `Payload` step reads the discriminant
    // there and goes to the active variant's payload (D-0046: a reference
    // into a payload of an enum on the heap, as in a `Box`).
    pub(super) fn path_offset_at(&self, base: Option<u64>, ty: &Type, path: &[Proj]) -> (u64, Type) {
        let sub = |off: u64| base.map(|b| b + off);
        match path.split_first() {
            None => (0, ty.clone()),
            Some((Proj::Field(i), rest)) => {
                if let Type::Named(name, args) = ty {
                    if let Some(layout) = self.struct_layout(name, args) {
                        let s = self.items.structs.get(name).unwrap().clone();
                        let subst = self.subst_map(&s.type_params, args);
                        let fty = self.apply_subst(&s.fields[*i].ty, &subst);
                        let (roff, rty) = self.path_offset_at(sub(layout.offsets[*i]), &fty, rest);
                        return (layout.offsets[*i] + roff, rty);
                    }
                }
                if let Type::Mutex(inner) = ty {
                    if *i == 0 {
                        return self.path_offset_at(base, inner, rest);
                    }
                }
                (0, ty.clone())
            }
            Some((Proj::Index(i), rest)) => {
                if let Type::Array(inner, _) | Type::Simd(inner, _) = ty {
                    let sz = self.sizeof_ty(inner);
                    let (roff, rty) = self.path_offset_at(sub(sz * (*i as u64)), inner, rest);
                    return (sz * (*i as u64) + roff, rty);
                }
                (0, ty.clone())
            }
            Some((Proj::Range(..), rest)) => self.path_offset_at(base, ty, rest),
            Some((Proj::Payload, rest)) => {
                if let (Some(b), Type::Named(name, args)) = (base, ty) {
                    if let Some(layout) = self.enum_layout(name, args) {
                        let mut vi: u64 = 0;
                        if let Some((some, none)) = layout.niche {
                            // D-0298: the pointer's being null names the variant.
                            vi = if (0..layout.size).any(|k| *self.arena.get(&(b + k)).unwrap_or(&0) != 0) { some as u64 } else { none as u64 };
                        }
                        for k in 0..layout.dw {
                            vi |= (*self.arena.get(&(b + k)).unwrap_or(&0) as u64) << (8 * k);
                        }
                        if let Some(pty) = layout.payload_types.get(vi as usize) {
                            let (roff, rty) = self.path_offset_at(Some(b + layout.payload_off), pty, rest);
                            return (layout.payload_off + roff, rty);
                        }
                    }
                }
                self.path_offset_at(base, ty, rest)
            }
        }
    }

    // Can the arena's byte encoding round-trip a value of this type?
    // References, guards, handles, fn values and closures are stored as
    // opaque tokens the encoding cannot recover.
    // The address of `obj`'s cells (`[Rawptr-Of]`'s `min(target(a))`,
    // `[Repr-Ref]`'s byte image). An arena-backed object has one; an
    // object the arena can carry is moved into it now, so that every
    // later access through the binding and every raw access see one
    // storage; any other object is given a stable address whose cells
    // hold a snapshot (D-0206, `conf.ref-byte-image-width`).
    pub(super) fn object_addr(&mut self, obj: u64) -> Option<u64> {
        if let Some((addr, _)) = self.reclaimed_addr.get(&obj) {
            return Some(*addr);
        }
        if let Some(a) = self.image_addr.get(&obj) {
            return Some(*a);
        }
        if !self.objects.contains_key(&obj) {
            return None;
        }
        let whole_ty = self.type_at(obj, &[]);
        let base = self.bump_alloc(self.sizeof_ty(&whole_ty).max(1), self.alignof_ty(&whole_ty).max(1));
        if self.arena_encodable(&whole_ty) {
            let whole = self.read_place(obj, &[]);
            self.arena_write(base, &whole_ty, whole);
            self.reclaimed_addr.insert(obj, (base, whole_ty));
            self.reclaimed_by_addr.insert(base, obj);
        } else {
            self.image_addr.insert(obj, base);
        }
        Some(base)
    }

    // `[Repr-Ref]`: the AddrWidth/8-byte image of the referent's address,
    // or zeros for a reference whose object is gone (its image is then
    // never read by a rule).
    pub(super) fn ref_image(&self, obj: u64, path: &[Proj]) -> Vec<u8> {
        let base = match (self.reclaimed_addr.get(&obj), self.image_addr.get(&obj)) {
            (Some((a, _)), _) => *a,
            (None, Some(a)) => *a,
            (None, None) => return 0u64.to_le_bytes().to_vec(),
        };
        if !self.objects.contains_key(&obj) {
            return base.to_le_bytes().to_vec();
        }
        let whole_ty = self.type_at(obj, &[]);
        let (off, _) = self.path_offset(&whole_ty, path);
        (base + off).to_le_bytes().to_vec()
    }

    pub(super) fn arena_encodable(&self, ty: &Type) -> bool {
        match ty {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Rawptr(_) => true,
            Type::Array(inner, _) | Type::Simd(inner, _) => self.arena_encodable(inner),
            Type::Named(name, args) => {
                if let Some(s) = self.items.structs.get(name) {
                    let subst = self.subst_map(&s.type_params, args);
                    s.fields.iter().all(|f| self.arena_encodable(&self.apply_subst(&f.ty, &subst)))
                } else if let Some(e) = self.items.enums.get(name) {
                    let subst = self.subst_map(&e.type_params, args);
                    e.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| self.arena_encodable(&self.apply_subst(p, &subst))))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(super) fn intrinsic_reclaim(&mut self, addr: u64, ty: Type) -> u64 {
        self.intrinsic_reclaim_inner(addr, ty)
    }
    pub(super) fn intrinsic_reclaim_inner(&mut self, addr: u64, ty: Type) -> u64 {
        if let Some(&id) = self.reclaimed_by_addr.get(&addr) {
            // [Reclaim] (CHG-0030): re-attaching a resource hands its destroy
            // authority to the reclaiming thread -- an element a worker
            // reached through a shared `Vec` is the owner's again when the
            // owner's `Vec::drop` reclaims it.
            let me = self.current_thread;
            if let Some(o) = self.objects.get_mut(&id) {
                if o.is_resource {
                    o.owner_thread = me;
                }
            }
            return id;
        }
        let is_res = self.is_resource(&ty);
        let id = self.fresh_id();
        if trace_on() {
            eprintln!("TRACE reclaim addr={} ty={:?} -> id={}", addr, ty, id);
        }
        let owner_thread = self.current_thread;
        self.objects.insert(id, Object { ty: ty.clone(), value: Value::Unit, is_resource: is_res, owner_thread });
        self.reclaimed_addr.insert(id, (addr, ty));
        self.reclaimed_by_addr.insert(addr, id);
        id
    }

    pub(super) fn convert(&self, name: &str, v: Value, dst: &Type) -> Result<Value, Flow> {
        match (name, v, dst) {
            ("widen", Value::Int(st, x), Type::Int(dt)) => {
                // Every value of the source fits the target; the payload
                // only changes representation for `u128`'s bit pattern.
                narrow(st, *dt, x).map(|r| Value::Int(*dt, r)).map_err(|_| Flow::Fault("diag.type-mismatch".into()))
            }
            ("narrow", Value::Int(st, x), Type::Int(dt)) => narrow(st, *dt, x).map(|r| Value::Int(*dt, r)).map_err(|_| Flow::Fault("diag.narrowing-overflow".into())),
            ("narrow_wrapping", Value::Int(_, x), Type::Int(dt)) => Ok(Value::Int(*dt, narrow_wrapping(*dt, x))),
            ("reinterpret", Value::Int(st, x), Type::Int(dt)) => Ok(Value::Int(*dt, reinterpret_sign(*dt, x, st.bitwidth(ADDR_WIDTH)))),
            // D-0051: `f32` into `f64` exactly, and the float conversions
            // IEEE-754 rounds (nearest, ties to even; beyond the range, an
            // infinity), as for an integer source.
            ("widen", Value::F32(f), Type::F64) | ("to_float", Value::F32(f), Type::F64) => Ok(Value::F64(f as f64)),
            ("widen", Value::F32(f), Type::F32) | ("to_float", Value::F32(f), Type::F32) => Ok(Value::F32(f)),
            ("widen", Value::F64(f), Type::F64) | ("to_float", Value::F64(f), Type::F64) => Ok(Value::F64(f)),
            ("to_float", Value::F64(f), Type::F32) => Ok(Value::F32(f as f32)),
            // D-0051: a float's bits as an integer of its width, and back.
            ("reinterpret", Value::F32(f), Type::Int(dt)) => {
                let b = f.to_bits();
                Ok(Value::Int(*dt, if dt.signed() { b as i32 as i128 } else { b as i128 }))
            }
            ("reinterpret", Value::F64(f), Type::Int(dt)) => {
                let b = f.to_bits();
                Ok(Value::Int(*dt, if dt.signed() { b as i64 as i128 } else { b as i128 }))
            }
            ("reinterpret", Value::Int(_, x), Type::F32) => Ok(Value::F32(f32::from_bits(x as u32))),
            ("reinterpret", Value::Int(_, x), Type::F64) => Ok(Value::F64(f64::from_bits(x as u64))),
            ("to_float", Value::Int(st, x), Type::F64) => Ok(Value::F64(if is_u128(st) { x as u128 as f64 } else { x as f64 })),
            ("to_float", Value::Int(st, x), Type::F32) => Ok(Value::F32(if is_u128(st) { x as u128 as f32 } else { x as f32 })),
            ("to_int", Value::F64(f), Type::Int(dt)) => float_to_int(f, *dt),
            ("to_int", Value::F32(f), Type::Int(dt)) => float_to_int(f as f64, *dt),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }
}
