// The intrinsics: raw pointers, raw strings, allocation and faults.

use super::*;

impl Interp {
    pub(super) fn intrinsic_raw(
        &mut self,
        name: &str,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let t0 = || -> Type { Type::Void };
        let targ = |i: usize, s: &Interp| -> Type {
            targs.get(i).map(|t| s.apply_subst(t, subst)).unwrap_or_else(t0)
        };
        macro_rules! ok { ($v:expr) => { return Ok(Some(EvalResult::Val($v))) } }
        match name {
            "rawptr_of" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path, pty) = match r {
                    EvalResult::Val(Value::Ref { obj, path, pointee, .. }) => (obj, path, pointee),
                    // A binding of reference type in value position is read
                    // (`[Rawptr-Of]`: `r : ref<τ, m>`, a value `a`): the result
                    // is its referent's address, not the binding's own cells
                    // (D-0206: `conf.ref-byte-image-width` found the latter).
                    EvalResult::Place(obj, path, _) => match self.read_place(obj, &path) {
                        Value::Ref { obj, path, pointee, .. } => (obj, path, pointee),
                        _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if let Some((addr, rty)) = self.reclaimed_addr.get(&obj).cloned() {
                    let (off, _) = self.path_offset(&rty, &path);
                    ok!(Value::Rawptr(addr + off, pty));
                }
                // `[Rawptr-Of]` yields `min(target(a))`: the address of the
                // object's own cells. A fresh-storage object has had no
                // address so far, so give it one now -- move its whole
                // value into the byte arena and record it as arena-backed
                // (`reclaimed_addr`), so that every later `read_place`/
                // `write_place` through the binding and every raw
                // `*p`/`*p = v` see the same storage. (Previously a
                // *snapshot* was written to a fresh address, so a raw
                // write through `rawptr_of(&mut x)` was invisible to `x`.)
                // Only types the arena can encode faithfully are backed
                // this way; a value carrying references or thread handles
                // keeps the snapshot behaviour.
                // The same address every time (`min(target(a))` is a
                // function of Σ), shared with the byte image of a `ref`
                // to the object (`object_addr`).
                let whole_ty = self.type_at(obj, &[]);
                let Some(base) = self.object_addr(obj) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let (off, _) = self.path_offset(&whole_ty, &path);
                if !self.reclaimed_addr.contains_key(&obj) {
                    let v = self.read_place(obj, &path);
                    self.arena_write(base + off, &pty, v);
                }
                ok!(Value::Rawptr(base + off, pty))
            }
            "reinterpret_ptr" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let dst = targ(0, self);
                if let Value::Rawptr(addr, _) = v {
                    ok!(Value::Rawptr(addr, dst));
                }
                return Err(Flow::Fault("diag.type-mismatch".into()));
            }
            // spec/21 rule.stdlib.str
            // `slice_len(s)` (D-0047): a slice's length, also through a reference.
            "slice_len" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path, chain) = match r {
                    EvalResult::Place(o, p, c) => (o, p, c),
                    EvalResult::Temp(o) => {
                        self.stmt_temp(o);
                        (o, vec![], vec![])
                    }
                    EvalResult::Val(Value::Ref { obj, path, token, mode, .. }) => {
                self.affine(token, mode == Mode::Exclusive)?;
                (obj, path, vec![token])
            }
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (obj, path, _) = self.resolve_through_refs(obj, path, chain)?;
                let n = self.slice_parts(obj, &path)?.2;
                ok!(Value::Int(IntTy::Usize, n as i128))
            }
            "str_len" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                ok!(Value::Int(IntTy::Usize, s.len() as i128))
            }
            "str_byte" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                let i = match self.eval_to_value(&args[1], subst)? {
                    Value::Int(_, i) => i,
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                // `[Str-Byte-Out-Of-Bounds]`
                if i < 0 || i as usize >= s.len() {
                    return Err(Flow::Fault("diag.index-out-of-bounds".into()));
                }
                ok!(Value::Int(IntTy::U8, s[i as usize] as i128))
            }
            "str_ptr" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                let addr = self.intern_str(&s);
                ok!(Value::Rawptr(addr, Type::Int(IntTy::U8)))
            }
            // D-0134 (`StringView::of`): the text's bytes as a shared
            // slice, of a `Vec<u8>` object over its interned bytes that is
            // made once per text and never ends.
            "str_slice" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                let n = s.len();
                let vty = Type::Named("std::Vec".into(), vec![Type::Int(IntTy::U8)]);
                let obj = match self.str_views.get(&s) {
                    Some(&o) => o,
                    None => {
                        let addr = self.intern_str(&s);
                        let v = Value::Struct(vec![Value::Rawptr(addr, Type::Int(IntTy::U8)), Value::Int(IntTy::Usize, n as i128), Value::Int(IntTy::Usize, n as i128)]);
                        let o = self.new_object(vty.clone(), v);
                        self.str_views.insert(s.clone(), o);
                        o
                    }
                };
                let token = self.fresh_id();
                let rv = Value::Ref { obj, path: vec![], mode: Mode::Shared, pointee: vty, token, range: Some((0, n)), anc: None };
                ok!(Value::Struct(vec![rv, Value::Int(IntTy::Usize, 0), Value::Int(IntTy::Usize, n as i128)]))
            }
            // D-0258: an address as a pointer (`[Rawptr-From-Address]`) and
            // back (`[Rawptr-Address]`). This interpreter's storage is
            // simulated, so a pointer is made only from an address it gave
            // out; any other names nothing here, and the call is refused
            // rather than reading something else.
            "rawptr_from_address" => {
                self.require_unsafe()?;
                let a = self.eval_to_value(&args[0], subst)?;
                let Value::Int(_, a) = a else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                let addr = a as u64;
                if addr != 0 && !self.knows_address(addr) {
                    return Err(Flow::Fault(format!(
                        "unsupported: rawptr_from_address({:#x}): coby's memory is simulated, and this address names none of it (a device's registers or mapped memory are cobc's to reach)",
                        addr
                    )));
                }
                ok!(Value::Rawptr(addr, targ(0, self)))
            }
            "rawptr_address" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let Value::Rawptr(addr, _) = p else { return Err(Flow::Fault("diag.type-mismatch".into())) };
                ok!(Value::Int(IntTy::Usize, addr as i128))
            }
            "reclaim" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let ty = targ(0, self);
                if let Value::Rawptr(addr, _) = p {
                    let id = self.intrinsic_reclaim(addr, ty);
                    return Ok(Some(EvalResult::Place(id, vec![], vec![])));
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
            "release" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let n = self.eval_to_value(&args[1], subst)?;
                if let (Value::Rawptr(addr, _), Value::Int(_, n)) = (p, n) {
                    let end = addr + n as u64;
                    let stale: Vec<u64> = self.reclaimed_addr.ids_in(addr, end);
                    self.check_not_foreign(&stale)?;
                    self.clear_side(addr, n as u64);
                    for id in stale {
                        if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                            self.reclaimed_by_addr.remove(&a);
                        }
                        self.remove_object(&id);
                    }
                }
                ok!(Value::Unit)
            }
            "copy_raw" => {
                self.require_unsafe()?;
                let dst = self.eval_to_value(&args[0], subst)?;
                let src = self.eval_to_value(&args[1], subst)?;
                let n = self.eval_to_value(&args[2], subst)?;
                if let (Value::Rawptr(d, _), Value::Rawptr(s, _), Value::Int(_, n)) = (dst, src, n) {
                    let bytes: Vec<u8> = (0..n as u64).map(|i| *self.arena.get(&(s + i)).unwrap_or(&0)).collect();
                    for (i, b) in bytes.into_iter().enumerate() {
                        self.arena.insert(d + i as u64, b);
                    }
                    let side: Vec<(u64, Value)> = self.arena_side.range(s..s + (n as u64).max(1)).map(|(a, v)| (*a - s, v.clone())).collect();
                    self.clear_side(d, n as u64);
                    for (off, v) in side {
                        self.arena_side.insert(d + off, v);
                    }
                }
                ok!(Value::Unit)
            }
            "allocate" => {
                let n = self.eval_to_value(&args[0], subst)?;
                let align = self.eval_to_value(&args[1], subst)?;
                let (Value::Int(_, n), Value::Int(_, align)) = (n, align) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let addr = if n == 0 { 1 } else { self.bump_alloc(n as u64, align.max(1) as u64) };
                if n > 0 {
                    self.allocations.insert(addr, (n as u64, align as u64));
                }
                let result_ty = Type::Named("std::Result".into(), vec![Type::Rawptr(Box::new(Type::Int(IntTy::U8))), Type::Named("std::AllocError".into(), vec![])]);
                let v = Value::Enum(0, Box::new(Value::Rawptr(addr, Type::Int(IntTy::U8))));
                let id = self.new_object(result_ty, v);
                return Ok(Some(EvalResult::Temp(id)));
            }
            "deallocate" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let n = self.eval_to_value(&args[1], subst)?;
                if let (Value::Rawptr(addr, _), Value::Int(_, n)) = (p, n) {
                    let end = addr + n.max(0) as u64;
                    let stale: Vec<u64> = self.reclaimed_addr.ids_in(addr, end);
                    self.check_not_foreign(&stale)?;
                    self.clear_side(addr, n.max(0) as u64);
                    for id in stale {
                        if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                            self.reclaimed_by_addr.remove(&a);
                        }
                        self.remove_object(&id);
                    }
                    self.allocations.remove(&addr);
                }
                ok!(Value::Unit)
            }
            "fault" => {
                if let ExprKind::Path(segs, _) = &args[0].kind {
                    let d = segs.last().unwrap().replace('_', "-");
                    // D-0083: an optional message travels with the fault.
                    if let Some(a1) = args.get(1) {
                        let Value::Str(m) = self.eval_to_value(a1, subst)? else {
                            return Err(Flow::Fault("diag.type-mismatch".into()));
                        };
                        let m = String::from_utf8_lossy(&m).into_owned();
                        return Err(Flow::Fault(format!("diag.{}{}{}", d, crate::diagnostics::DETAIL_SEP, m)));
                    }
                    return Err(Flow::Fault(format!("diag.{}", d)));
                }
                Err(Flow::Fault("diag.index-out-of-bounds".into()))
            }
            _ => self.intrinsic_threads(name, targs, args, subst),
        }
    }

    // Whether `addr` lies in storage this interpreter gave out: a byte of
    // the arena (a written cell, interned text), an allocation, or an
    // arena-backed object's extent (`rawptr_from_address`).
    pub(super) fn knows_address(&self, addr: u64) -> bool {
        if self.arena.contains_key(&addr) {
            return true;
        }
        if self.allocations.iter().any(|(a, (n, _))| *a <= addr && addr < *a + *n) {
            return true;
        }
        !self.reclaimed_addr.ids_in(addr, addr + 1).is_empty()
    }

}
