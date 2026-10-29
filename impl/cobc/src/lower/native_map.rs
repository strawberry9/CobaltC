// Native C bodies for std's HashMap, HashSet, Queue and PriorityQueue.

use super::*;

impl<'a> Gen<'a> {
    // `HashMap::find` with `String` keys (`spec/21` §4, D-0041): the probe
    // loop over `m.slots` comparing keys with `key_eq`. Natively: the
    // body's checked reads of `m.slots` (once: the same path, and nothing
    // in the loop writes) and of `k` (`key_hash`'s, then `key_eq`'s, which
    // cannot fail after it), `len - 1`'s overflow, each slot's element
    // access, and for each candidate key the read of `m.keys` (on the
    // first), its bounds fault, and the element access `key_eq`'s read of
    // it makes (`cb_elem_access`: checked where an object lies over the
    // key, and otherwise unobservable, as for a plain element, D-0023).
    pub(super) fn native_map_find_string(&mut self, v: &Type) -> R<String> {
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let mc = self.ctype(&Type::Named("std::HashMap".to_string(), vec![st.clone(), v.clone()]))?;
        let sty = self.type_id(&st)?;
        let uty = self.type_id(&Type::Int(IntTy::Usize))?;
        let mut b = String::new();
        writeln!(b, "    {} *m = ({} *)p_m.p;", mc, mc).unwrap();
        writeln!(b, "    cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (m->slots.len == 0) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t mask = m->slots.len - 1;").unwrap();
        writeln!(b, "    cb_read(p_k.tok, NULL, 0, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *k = ({} *)p_k.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t i = cb_hash_bytes((const void *)k->bytes.ptr, k->bytes.len) & mask;").unwrap();
        writeln!(b, "    int keys_read = 0;").unwrap();
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t *se = m->slots.ptr + i;").unwrap();
        writeln!(b, "        cb_elem_access(se, sizeof(*se), {}u, CB_SHARED, 0, CB_NOLOC, 0);", uty).unwrap();
        writeln!(b, "        uint64_t e = *se;").unwrap();
        writeln!(b, "        if (e == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (!keys_read) {{ cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0); keys_read = 1; }}").unwrap();
        writeln!(b, "        if (e >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e;", sc).unwrap();
        writeln!(b, "        cb_elem_access(ke, sizeof(*ke), {}u, CB_SHARED, 0, CB_NOLOC, 0);", sty).unwrap();
        writeln!(b, "        if (cb_bytes_eq((const void *)ke->bytes.ptr, ke->bytes.len, (const void *)k->bytes.ptr, k->bytes.len)) break;").unwrap();
        writeln!(b, "        i = (i + 1) & mask;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    return i;").unwrap();
        Ok(b)
    }

    // D-0196: `HashMap::find_str` (`get_str`, `get_mut_str`,
    // `contains_str`, `remove_str`): `native_map_find_string`'s probe with
    // the key's bytes from the `str`, a plain value (no read of it to check;
    // `key_hash` of a `str` is its bytes' hash, as a `String`'s is).
    pub(super) fn native_map_find_str(&mut self, v: &Type) -> R<String> {
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let mc = self.ctype(&Type::Named("std::HashMap".to_string(), vec![st.clone(), v.clone()]))?;
        let sty = self.type_id(&st)?;
        let uty = self.type_id(&Type::Int(IntTy::Usize))?;
        let mut b = String::new();
        writeln!(b, "    {} *m = ({} *)p_m.p;", mc, mc).unwrap();
        writeln!(b, "    cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (m->slots.len == 0) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t mask = m->slots.len - 1;").unwrap();
        writeln!(b, "    uint64_t i = cb_hash_bytes((const void *)p_k.p, p_k.n) & mask;").unwrap();
        writeln!(b, "    int keys_read = 0;").unwrap();
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t *se = m->slots.ptr + i;").unwrap();
        writeln!(b, "        cb_elem_access(se, sizeof(*se), {}u, CB_SHARED, 0, CB_NOLOC, 0);", uty).unwrap();
        writeln!(b, "        uint64_t e = *se;").unwrap();
        writeln!(b, "        if (e == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (!keys_read) {{ cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0); keys_read = 1; }}").unwrap();
        writeln!(b, "        if (e >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e;", sc).unwrap();
        writeln!(b, "        cb_elem_access(ke, sizeof(*ke), {}u, CB_SHARED, 0, CB_NOLOC, 0);", sty).unwrap();
        writeln!(b, "        if (cb_bytes_eq((const void *)ke->bytes.ptr, ke->bytes.len, (const void *)p_k.p, p_k.n)) break;").unwrap();
        writeln!(b, "        i = (i + 1) & mask;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    return i;").unwrap();
        Ok(b)
    }

    // The probe `HashMap::find` makes (`native_map_find_string`,
    // `native_map_find_scalar`), inline in another native body: `m` the
    // map, `kp` a `K *` to the key, `read_k` the key's checked read (none
    // for a key the body owns: a fresh value's read cannot fail). Leaves
    // the slot in `s`.
    pub(super) fn map_probe(&mut self, k: &Type, kp: &str, read_k: Option<&str>) -> R<String> {
        let kc = self.ctype(k)?;
        let kty = self.type_id(k)?;
        let uty = self.type_id(&Type::Int(IntTy::Usize))?;
        let text = matches!(k, Type::Named(n, _) if n == "std::String");
        let mut b = String::new();
        writeln!(b, "    cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (m->slots.len == 0) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t mask = m->slots.len - 1;").unwrap();
        if let Some(rk) = read_k {
            writeln!(b, "    cb_read({}, NULL, 0, CB_NOLOC, 0);", rk).unwrap();
        }
        writeln!(b, "    {} *kq = {};", kc, kp).unwrap();
        if text {
            writeln!(b, "    uint64_t s = cb_hash_bytes((const void *)kq->bytes.ptr, kq->bytes.len) & mask;").unwrap();
        } else {
            writeln!(b, "    uint64_t s = cb_hash_bytes((const void *)kq, sizeof(*kq)) & mask;").unwrap();
        }
        // The probe makes no runtime check: the reads of `m.keys` cannot
        // fail where the read of `m.slots` above passed (one path, held
        // for the call, nothing derived from it alive); no object outlives
        // a `std` call over a cell of `slots`, a private field only `std`
        // reaches; and a key element can be held only shared (`key_at`),
        // which a shared read never clashes with.
        let _ = (uty, kty);
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t e0 = m->slots.ptr[s];").unwrap();
        writeln!(b, "        if (e0 == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (e0 >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e0;", kc).unwrap();
        if text {
            writeln!(b, "        if (cb_bytes_eq((const void *)ke->bytes.ptr, ke->bytes.len, (const void *)kq->bytes.ptr, kq->bytes.len)) break;").unwrap();
        } else {
            writeln!(b, "        if (*ke == *kq) break;").unwrap();
        }
        writeln!(b, "        s = (s + 1) & mask;").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(b)
    }

    // `HashMap::get`, `get_mut`, `contains`, `entry` and `insert` (`spec/21`
    // §4) for `String`, integer and `bool` keys, natively: each body's
    // checked reads of `m`'s fields through the path the call holds (one
    // per field: nothing in between writes them), its arithmetic faults,
    // the probe (`map_probe`), the element accesses and borrows the
    // `Vec::index_*` calls make, and the result as the body returns it.
    // `entry` and `insert` only for a plain value type (a pushed value
    // needs no move). A key passed by value is received (`cb_recv` for a
    // `String`) and, when not stored, destroyed as the body's frame would.
    // `Queue::front`/`Queue::back` (`rule.stdlib.queue`, D-0137) realized
    // natively, as `HashMap::get` is: the body's checked reads of the two
    // stacks' lengths, then the element's `[Reclaim]` + `[Borrow]` and the
    // `Option`'s return in one runtime call (`cb_elem_borrow_datum`)
    // rather than the general construction of an `Option` holding a
    // reference. The front is `head`'s last element, else `tail`'s first;
    // the back is `tail`'s last, else `head`'s first. Element types as
    // for the native `get`: none holding a reference or a `fn` value.
    pub(super) fn native_queue_peek(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let [v] = targs else { return Ok(None) };
        let vsize = self.size_align(v)?.0;
        if vsize == 0 || self.has_refs(v) || self.holds_fn(v) {
            return Ok(None);
        }
        let qc = self.ctype(&Type::Named("std::Queue".to_string(), vec![v.clone()]))?;
        let vc = self.ctype(v)?;
        let vty = self.type_id(v)?;
        let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Ref(Box::new(v.clone()), Mode::Shared)]))?;
        // field 0 is `head`, 1 is `tail`; a `Vec`'s field 1 is `len`.
        let (near, far, near_at, far_at) = if name == "std::Queue::front" { (0u32, 1u32, "len - 1", "0") } else { (1u32, 0u32, "len - 1", "0") };
        let fld = |k: u32| if k == 0 { "head" } else { "tail" };
        let mut b = String::new();
        writeln!(b, "    {} *q = ({} *)p_q.p;", qc, qc).unwrap();
        writeln!(b, "    {} r;", oc).unwrap();
        writeln!(b, "    {} *ve;", vc).unwrap();
        writeln!(b, "    cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);", near).unwrap();
        writeln!(b, "    if (q->{n}.len > 0) {{ ve = q->{n}.ptr + (q->{n}.{a}); }}", n = fld(near), a = near_at).unwrap();
        writeln!(b, "    else {{").unwrap();
        writeln!(b, "        cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);", far).unwrap();
        writeln!(b, "        if (q->{}.len == 0) {{ r.tag = 1u; cb_send_datum_none(); return r; }}", fld(far)).unwrap();
        let far_ix = if far_at == "0" { "0".to_string() } else { format!("q->{}.{}", fld(far), far_at) };
        writeln!(b, "        ve = q->{}.ptr + ({});", fld(far), far_ix).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    r.tag = 0u;").unwrap();
        writeln!(b, "    r.u.v0 = (void *)ve;").unwrap();
        writeln!(b, "    cb_elem_borrow_datum(ve, {}u, CB_SHARED);", vty).unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `Queue::pop_front`/`pop_back` (`rule.stdlib.queue`, D-0137) realized
    // natively for a plain element type, as `Vec::pop` is: the body's pop
    // of one of its two stacks made through the queue's own path (`q.head`
    // is field 0, `q.tail` field 1; a `Vec`'s field 1 is `len`), with the
    // native `Vec::pop`'s checked read and write of `len`, so no path is
    // formed for the stack itself. `shift`, when the stack is empty, is
    // called as the body calls it, on paths to the stacks formed for the
    // call. (The pushes stay as written: the body's call of the native
    // `Vec::push` measured faster than a native `Queue::push_back`.)
    pub(super) fn native_queue_op(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let [v] = targs else { return Ok(None) };
        let (size, _) = self.size_align(v)?;
        if size == 0 || !self.plain_data(v) {
            return Ok(None);
        }
        let qc = self.ctype(&Type::Named("std::Queue".to_string(), vec![v.clone()]))?;
        let fld = |k: u32| if k == 0 { "head" } else { "tail" };
        let mut b = String::new();
        writeln!(b, "    {} *q = ({} *)p_q.p;", qc, qc).unwrap();
        // `pop_front` takes from `head` (field 0), refilling it from `tail`,
        // and records `took_front` (field 2); `pop_back` the reverse.
        let (near, far, took, other) = if name == "std::Queue::pop_front" { (0u32, 1u32, 2u32, 3u32) } else { (1u32, 0u32, 3u32, 2u32) };
        let took_f = |k: u32| if k == 2 { "took_front" } else { "took_back" };
        let shift = self.request_fn("std::Queue::shift", vec![v.clone()])?;
        let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![v.clone()]))?;
        writeln!(b, "    {} r;", oc).unwrap();
        writeln!(b, "    cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);", near).unwrap();
        writeln!(b, "    if (q->{}.len == 0) {{", fld(near)).unwrap();
        writeln!(b, "        cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);", far).unwrap();
        writeln!(b, "        if (q->{}.len == 0) {{ r.tag = 1u; return r; }}", fld(far)).unwrap();
        writeln!(b, "        cb_write(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }} }}, 1, 0u, CB_NOLOC, 0);", took).unwrap();
        writeln!(b, "        q->{} = 1;", took_f(took)).unwrap();
        writeln!(b, "        cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }} }}, 1, CB_NOLOC, 0);", other).unwrap();
        writeln!(b, "        uint8_t half = q->{};", took_f(other)).unwrap();
        writeln!(b, "        uint64_t ta = cb_borrow_unstamped(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);", far).unwrap();
        writeln!(b, "        uint64_t tb = cb_borrow_unstamped(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);", near).unwrap();
        writeln!(b, "        {}(((cb_ref){{ (void *)&q->{}, ta }}), ((cb_ref){{ (void *)&q->{}, tb }}), half);", shift, fld(far), fld(near)).unwrap();
        writeln!(b, "        cb_call_done(tb);").unwrap();
        writeln!(b, "        cb_call_done(ta);").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t n = q->{}.len - 1;", fld(near)).unwrap();
        writeln!(b, "    cb_write(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, {}u }}, {{ CB_FIELD, 1u }} }}, 2, 0u, CB_NOLOC, 0);", near).unwrap();
        writeln!(b, "    q->{}.len = n;", fld(near)).unwrap();
        writeln!(b, "    r.tag = 0u;").unwrap();
        writeln!(b, "    r.u.v0 = q->{}.ptr[n];", fld(near)).unwrap();
        writeln!(b, "    cb_release((uint8_t *)(q->{}.ptr + n), {}u);", fld(near), size).unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    pub(super) fn native_map_op(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let [k, v] = targs else { return Ok(None) };
        let text = matches!(k, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if !text && !matches!(k, Type::Int(_) | Type::Bool) {
            return Ok(None);
        }
        let vsize = self.size_align(v)?.0;
        if vsize == 0 || self.has_refs(v) || self.holds_fn(v) {
            return Ok(None);
        }
        let plain_v = self.plain_data(v);
        if matches!(name, "std::HashMap::entry" | "std::HashMap::insert") && !plain_v {
            return Ok(None);
        }
        let mc = self.ctype(&Type::Named("std::HashMap".to_string(), vec![k.clone(), v.clone()]))?;
        let kc = self.ctype(k)?;
        let vc = self.ctype(v)?;
        let vty = self.type_id(v)?;
        let uty = self.type_id(&Type::Int(IntTy::Usize))?;
        let mut b = String::new();
        writeln!(b, "    {} *m = ({} *)p_m.p;", mc, mc).unwrap();
        match name {
            "std::HashMap::get" | "std::HashMap::get_mut" | "std::HashMap::contains" => {
                let probe = self.map_probe(k, &format!("({} *)p_k.p", kc), Some("p_k.tok"))?;
                b.push_str(&probe);
                writeln!(b, "    uint64_t e = m->slots.ptr[s];").unwrap();
                if name == "std::HashMap::contains" {
                    writeln!(b, "    return (uint8_t)(e != UINT64_MAX);").unwrap();
                    return Ok(Some(b));
                }
                let excl = name == "std::HashMap::get_mut";
                let mode = if excl { Mode::Exclusive } else { Mode::Shared };
                let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Ref(Box::new(v.clone()), mode)]))?;
                let m = if excl { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                writeln!(b, "    {} r;", oc).unwrap();
                writeln!(b, "    if (e == UINT64_MAX) {{ r.tag = 1u; cb_send_datum_none(); return r; }}").unwrap();
                if excl {
                    // `&mut m.values`: an exclusive borrow through `m`, which a
                    // live shared path to the map meets (D-0222).
                    writeln!(b, "    cb_borrow_check(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
                }
                writeln!(b, "    if (e >= m->values.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
                writeln!(b, "    {} *ve = m->values.ptr + e;", vc).unwrap();
                writeln!(b, "    r.tag = 0u;").unwrap();
                writeln!(b, "    r.u.v0 = (void *)ve;").unwrap();
                writeln!(b, "    cb_elem_borrow_datum(ve, {}u, {});", vty, m).unwrap();
                writeln!(b, "    return r;").unwrap();
                Ok(Some(b))
            }
            _ => {
                // `entry(m, k, fresh)` / `insert(m, k, v)`.
                let insert = name == "std::HashMap::insert";
                let vp = if insert { "p_v" } else { "p_fresh" };
                if text {
                    writeln!(b, "    uint64_t ok = cb_recv(&p_k);").unwrap();
                }
                // `reserve_one(m)`.
                let reslot = self.request_fn("std::HashMap::reslot", vec![k.clone(), v.clone()])?;
                // Every path the text takes makes an exclusive borrow through
                // `m` — `&mut m.slots` and the pushes of `keys` and `values`
                // for a new key, `&mut m.values` for an existing one — and
                // each faults where any would, at the call: one exclusive
                // check of the map, in place of `find`'s first read, stands
                // for them all (D-0222; the fields are private, so no path
                // outside `std` reaches one field alone).
                writeln!(b, "    cb_borrow_check(p_m.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
                writeln!(b, "    uint64_t n = m->slots.len, a, c;").unwrap();
                writeln!(b, "    if (__builtin_add_overflow(m->keys.len, (uint64_t)1, &a) || __builtin_mul_overflow(a, (uint64_t)4, &a) || __builtin_mul_overflow(n, (uint64_t)3, &c)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
                writeln!(b, "    if (a > c) {{ uint64_t n2; if (__builtin_mul_overflow(n, (uint64_t)2, &n2)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0); {}(((cb_ref){{ p_m.p, p_m.tok }}), n2); }}", reslot).unwrap();
                // `find`'s own first read of `m`, made just above.
                let probe = self.map_probe(k, "&p_k", None)?.replacen("    cb_read(p_m.tok, (cb_proj[]){ { CB_FIELD, 2u }, { CB_FIELD, 1u } }, 2, CB_NOLOC, 0);\n", "", 1);
                b.push_str(&probe);
                writeln!(b, "    uint64_t e = m->slots.ptr[s];").unwrap();
                let growk = self.request_fn("std::Vec::grow", vec![k.clone()])?;
                let growv = self.request_fn("std::Vec::grow", vec![v.clone()])?;
                let oc = if insert { Some(self.ctype(&Type::Named("std::Option".to_string(), vec![v.clone()]))?) } else { None };
                if let Some(oc) = &oc {
                    writeln!(b, "    {} r;", oc).unwrap();
                }
                writeln!(b, "    if (e == UINT64_MAX) {{").unwrap();
                writeln!(b, "        e = m->keys.len;").unwrap();
                // The exclusive borrows of `slots`, `keys` and `values` were
                // checked at entry. No object lies over a cell of `slots`.
                let _ = uty;
                writeln!(b, "        m->slots.ptr[s] = e;").unwrap();
                writeln!(b, "        if (m->keys.len == m->keys.cap) {{ uint64_t gt = cb_borrow_unstamped(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&m->keys, gt }})); cb_call_done(gt); }}", growk).unwrap();
                writeln!(b, "        m->keys.ptr[m->keys.len] = p_k;").unwrap();
                if text {
                    writeln!(b, "        cb_raw_move_in(ok, &m->keys.ptr[m->keys.len]);").unwrap();
                }
                writeln!(b, "        m->keys.len += 1;").unwrap();
                writeln!(b, "        if (m->values.len == m->values.cap) {{ uint64_t gt = cb_borrow_unstamped(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&m->values, gt }})); cb_call_done(gt); }}", growv).unwrap();
                writeln!(b, "        m->values.ptr[m->values.len] = {};", vp).unwrap();
                writeln!(b, "        m->values.len += 1;").unwrap();
                if insert {
                    writeln!(b, "        r.tag = 1u;").unwrap();
                    writeln!(b, "        return r;").unwrap();
                }
                writeln!(b, "    }} else {{").unwrap();
                if text {
                    writeln!(b, "        cb_consume(ok, CB_NOLOC, 0);").unwrap();
                }
                if insert {
                    // `Some(Vec::replace(&mut m.values, e, v))`, its borrow checked at entry.
                    writeln!(b, "        if (e >= m->values.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
                    writeln!(b, "        {} *ve = m->values.ptr + e;", vc).unwrap();
                    writeln!(b, "        r.tag = 0u;").unwrap();
                    writeln!(b, "        r.u.v0 = *ve;").unwrap();
                    writeln!(b, "        cb_release((uint8_t *)ve, {}u);", vsize).unwrap();
                    writeln!(b, "        *ve = {};", vp).unwrap();
                    writeln!(b, "        return r;").unwrap();
                }
                writeln!(b, "    }}").unwrap();
                if !insert {
                    // `Vec::index_exclusive(&mut m.values, e)`, its borrow checked at entry.
                    writeln!(b, "    if (e >= m->values.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
                    writeln!(b, "    {} *ve = m->values.ptr + e;", vc).unwrap();
                    writeln!(b, "    return (cb_ref){{ (void *)ve, cb_elem_borrow(ve, {}u, CB_EXCLUSIVE) }};", vty).unwrap();
                }
                Ok(Some(b))
            }
        }
    }

    // A native `get_mut` body without the found branch's exclusive check
    // of `m.values` (D-0222), for a map the call checked exclusively.
    pub(super) fn map_excl_unchecked(b: &str) -> String {
        let chk = "    cb_borrow_check(p_m.tok, (cb_proj[]){ { CB_FIELD, 1u } }, 1, CB_EXCLUSIVE, CB_NOLOC, 0);\n";
        assert!(b.contains(chk), "get_mut's check");
        b.replacen(chk, "", 1)
    }

    // A native `entry` body returning the value's address instead of a
    // reference to it: the element access its caller's read and write
    // through that reference would make (`cb_elem_access`, exclusive, a
    // write: the read after it cannot fail on an initialized element).
    pub(super) fn entry_acc_body(b: &str, vty: u32) -> String {
        let ret = format!("    return (cb_ref){{ (void *)ve, cb_elem_borrow(ve, {}u, CB_EXCLUSIVE) }};", vty);
        assert!(b.contains(&ret), "entry's return");
        b.replace(&ret, &format!("    cb_elem_access(ve, sizeof(*ve), {}u, CB_EXCLUSIVE, 1, CB_NOLOC, 0);\n    return ve;", vty))
    }

    // A native `get_mut` body returning the value's address (NULL for no
    // entry) instead of an `Option` holding a reference to it: the element
    // access its caller's read and write through that reference would
    // make, as `entry_acc_body`'s.
    // D-0200: and `get`'s, the access shared, a read.
    pub(super) fn get_mut_acc_body(b: &str, vty: u32, excl: bool) -> String {
        let (m, w) = if excl { ("CB_EXCLUSIVE", 1) } else { ("CB_SHARED", 0) };
        let none = "{ r.tag = 1u; cb_send_datum_none(); return r; }";
        let some = format!("    r.tag = 0u;\n    r.u.v0 = (void *)ve;\n    cb_elem_borrow_datum(ve, {}u, {});\n    return r;", vty, m);
        assert!(b.contains(none) && b.contains(&some), "get_mut's returns");
        b.replace(none, "return NULL;").replace(&some, &format!("    cb_elem_access(ve, sizeof(*ve), {}u, {}, {}, CB_NOLOC, 0);\n    return ve;", vty, m, w))
    }

    // `HashMap::find` with an integer or `bool` key: as for `String` keys
    // (`native_map_find_string`), `key_hash`'s read of `*k` and its hash
    // of the value's bytes (`cb_hash_bytes`, as `lower_key_bytes` makes
    // it), and for each candidate key its element access and `key_eq`'s
    // comparison, which for these types is equality of the values.
    pub(super) fn native_map_find_scalar(&mut self, k: &Type, v: &Type) -> R<String> {
        let kc = self.ctype(k)?;
        let mc = self.ctype(&Type::Named("std::HashMap".to_string(), vec![k.clone(), v.clone()]))?;
        let kty = self.type_id(k)?;
        let uty = self.type_id(&Type::Int(IntTy::Usize))?;
        let mut b = String::new();
        writeln!(b, "    {} *m = ({} *)p_m.p;", mc, mc).unwrap();
        writeln!(b, "    cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (m->slots.len == 0) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t mask = m->slots.len - 1;").unwrap();
        writeln!(b, "    cb_read(p_k.tok, NULL, 0, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} kv = *({} *)p_k.p;", kc, kc).unwrap();
        writeln!(b, "    uint64_t i = cb_hash_bytes((const void *)&kv, sizeof(kv)) & mask;").unwrap();
        writeln!(b, "    int keys_read = 0;").unwrap();
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t *se = m->slots.ptr + i;").unwrap();
        writeln!(b, "        cb_elem_access(se, sizeof(*se), {}u, CB_SHARED, 0, CB_NOLOC, 0);", uty).unwrap();
        writeln!(b, "        uint64_t e = *se;").unwrap();
        writeln!(b, "        if (e == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (!keys_read) {{ cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0); keys_read = 1; }}").unwrap();
        writeln!(b, "        if (e >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e;", kc).unwrap();
        writeln!(b, "        cb_elem_access(ke, sizeof(*ke), {}u, CB_SHARED, 0, CB_NOLOC, 0);", kty).unwrap();
        writeln!(b, "        if (*ke == kv) break;").unwrap();
        writeln!(b, "        i = (i + 1) & mask;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    return i;").unwrap();
        Ok(b)
    }

    // D-0193: `PriorityQueue::push` and `pop` for plain elements, natively:
    // the same binary heap and the same comparisons in the same order
    // (`before`: the earlier arrival unless the later goes first). The body
    // reaches everything through `q`, borrowed exclusively by the call: its
    // first check (`&mut q.items` for `push`, the read of `q.items.len` for
    // `pop`, then its borrow) is made, and the rest cannot fail. The order
    // is the key types' (`keyed`, compared in C where `search_ops` has the
    // type) or `less`, called with bare element addresses when its closure
    // keeps nothing it is given (`cb_fn_pure`, D-0191). Any other queue, or
    // one with an element borrowed, takes the prelude's body.
    pub(super) fn native_pq(&mut self, name: &str, t: &Type) -> R<Option<String>> {
        let (size, _) = self.size_align(t)?;
        // D-0200: or a resource holding no reference, no `fn` and no
        // destructor of the program's own (`objectless_type`): moved within
        // the heap as bytes, as a plain element is (no element has an object
        // where the entry test finds none); received and sent at the ends.
        let res = self.is_resource(t) && self.objectless_type(t);
        if size == 0 || !(self.plain_data(t) || res) {
            return Ok(None);
        }
        let qc = self.ctype(&Type::Named("std::PriorityQueue".to_string(), vec![t.clone()]))?;
        let tc = self.ctype(t)?;
        let ft = Type::Fn(vec![Type::Ref(Box::new(t.clone()), Mode::Shared), Type::Ref(Box::new(t.clone()), Mode::Shared)], Box::new(Type::Bool));
        let thunk = self.thunk(&ft)?;
        let lt = self.search_ops(t).map(|(_, lt)| lt);
        let grow_t = self.request_fn("std::Vec::grow", vec![t.clone()])?;
        let grow_s = self.request_fn("std::Vec::grow", vec![Type::Int(IntTy::U64)])?;
        let fb = self.std_fallback()?;
        let push = name == "std::PriorityQueue::push";
        let args = if push { "p_q, p_x" } else { "p_q" };
        let less = |a: &str, b: &str| -> String {
            let keyed = match &lt {
                Some(lt) => lt(&format!("q->items.ptr[{}]", a), &format!("q->items.ptr[{}]", b)),
                None => "0".to_string(),
            };
            format!("(q->keyed ? ({}) : {}(q->less, ((cb_ref){{ (void *)&q->items.ptr[{}], 0 }}), ((cb_ref){{ (void *)&q->items.ptr[{}], 0 }})))", keyed, thunk, a, b)
        };
        let before = |i: &str, j: &str| -> String {
            format!("((q->seqs.ptr[{i}] < q->seqs.ptr[{j}]) ? !{lji} : {lij})", i = i, j = j, lji = less(j, i), lij = less(i, j))
        };
        let sift_up = |at: &str| -> String {
            format!(
                "    {{ uint64_t i = {at}, t = i; while (t > 0) {{ uint64_t p = (t - 1) / 2; if (!{b}) break; t = p; }}\n      if (t != i) {{ {tc} x = q->items.ptr[i]; uint64_t xs = q->seqs.ptr[i]; uint64_t j = i; while (j != t) {{ uint64_t p = (j - 1) / 2; q->items.ptr[j] = q->items.ptr[p]; q->seqs.ptr[j] = q->seqs.ptr[p]; j = p; }} q->items.ptr[t] = x; q->seqs.ptr[t] = xs; }} }}\n",
                at = at,
                b = before("i", "p"),
                tc = tc
            )
        };
        let mut b = String::new();
        writeln!(b, "    {} *q = ({} *)p_q.p;", qc, qc).unwrap();
        writeln!(b, "    if ((q->keyed && {}) || (!q->keyed && !cb_fn_pure(q->less)) || cb_live_in(q->items.ptr, q->items.len * {}u)) return {}({});", if lt.is_some() { "0" } else { "1" }, size, fb, args).unwrap();
        if push {
            if res {
                // The element's record ends as it enters the heap's cells,
                // as `cb_raw_move_in` ends a reference-free value's.
                writeln!(b, "    uint64_t xo = cb_recv(&p_x);").unwrap();
            }
            writeln!(b, "    cb_borrow_check(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
            if res {
                writeln!(b, "    cb_forget_obj(xo);").unwrap();
            }
            writeln!(b, "    if (q->items.len == q->items.cap) {}(((cb_ref){{ (void *)&q->items, 0 }}));", grow_t).unwrap();
            writeln!(b, "    q->items.ptr[q->items.len] = p_x; q->items.len += 1;").unwrap();
            writeln!(b, "    if (q->seqs.len == q->seqs.cap) {}(((cb_ref){{ (void *)&q->seqs, 0 }}));", grow_s).unwrap();
            writeln!(b, "    q->seqs.ptr[q->seqs.len] = q->next; q->seqs.len += 1;").unwrap();
            writeln!(b, "    if (__builtin_add_overflow(q->next, (uint64_t)1, &q->next)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
            b.push_str(&sift_up("q->items.len - 1"));
        } else {
            let opt_c = self.ctype(&Type::Named("std::Option".to_string(), vec![t.clone()]))?;
            writeln!(b, "    cb_read(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} r;", opt_c).unwrap();
            writeln!(b, "    uint64_t n = q->items.len;").unwrap();
            if res {
                let otid = self.type_id(&Type::Named("std::Option".to_string(), vec![t.clone()]))?;
                writeln!(b, "    if (n == 0) {{ r.tag = 1u; uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro); return r; }}", otid).unwrap();
            } else {
                writeln!(b, "    if (n == 0) {{ r.tag = 1u; return r; }}").unwrap();
            }
            writeln!(b, "    cb_borrow_check(p_q.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} top = q->items.ptr[0];", tc).unwrap();
            writeln!(b, "    uint64_t last = n - 1, hole = 0;").unwrap();
            writeln!(b, "    while (2 * hole + 1 < last) {{ uint64_t c = 2 * hole + 1; if (c + 1 < last && {}) c = c + 1; q->items.ptr[hole] = q->items.ptr[c]; q->seqs.ptr[hole] = q->seqs.ptr[c]; hole = c; }}", before("c + 1", "c")).unwrap();
            writeln!(b, "    if (hole != last) {{ q->items.ptr[hole] = q->items.ptr[last]; q->seqs.ptr[hole] = q->seqs.ptr[last]; }}").unwrap();
            writeln!(b, "    q->items.len = last; q->seqs.len = last;").unwrap();
            writeln!(b, "    if (hole != last)").unwrap();
            b.push_str(&sift_up("hole"));
            if res {
                let otid = self.type_id(&Type::Named("std::Option".to_string(), vec![t.clone()]))?;
                writeln!(b, "    r.tag = 0u; r.u.v0 = top; uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro); return r;", otid).unwrap();
            } else {
                writeln!(b, "    r.tag = 0u; r.u.v0 = top; return r;").unwrap();
            }
        }
        Ok(Some(b))
    }

    // D-0200: whether destroying a `t` runs no code of the program's and
    // no check can fail in it: plain data, a container an objectless local
    // may be (`objectless_type`), or an `Option` of either.
    pub(super) fn silent_drop(&self, t: &Type) -> bool {
        (self.plain_data(t) && !self.has_refs(t) && !self.holds_fn(t))
            || self.objectless_type(t)
            || matches!(t, Type::Named(n, a) if n == "std::Option" && a.len() == 1 && self.silent_drop(&a[0]))
    }

    // D-0200: `HashSet<String>::insert(s, k)` natively: the body receives
    // `k`, borrows `s.map` exclusively (the one check that can fail), moves
    // `k` into `HashMap::insert(&mut s.map, k, true)` and answers whether
    // that found none. Here the same borrow (unstamped, ended after the
    // call, as `native_append_bytes`' `grow` borrow is), and the key handed
    // to the map's `_rawk` form with no object: its record ends at once
    // (`cb_forget_obj`), as the map's own keys' do.
    pub(super) fn native_set_insert_string(&mut self) -> R<Option<String>> {
        let st = Type::Named("std::String".to_string(), vec![]);
        let setc = self.ctype(&Type::Named("std::HashSet".to_string(), vec![st.clone()]))?;
        let ins = self.request_fn("std::HashMap::insert", vec![st.clone(), Type::Bool])?;
        let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Bool]))?;
        let mut b = String::new();
        writeln!(b, "    uint64_t o = cb_recv(&p_k);").unwrap();
        writeln!(b, "    uint64_t tk = cb_borrow_unstamped(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_forget_obj(o);").unwrap();
        writeln!(b, "    {} r = {}_rawk(((cb_ref){{ (void *)&(({} *)p_s.p)->map, tk }}), p_k, 1);", oc, ins, setc).unwrap();
        writeln!(b, "    cb_call_done(tk);").unwrap();
        writeln!(b, "    return (uint8_t)(r.tag != 0u);").unwrap();
        // D-0200: the `_raw` form, for a key made with no object
        // (`Fx::lower_set_insert_fresh`): nothing to receive or end.
        if let Some((_, _, _, _, sig)) = self.native_ctx.clone() {
            let open = sig.find('(').expect("a signature");
            let raw_sig = format!("{}_raw{}", &sig[..open], &sig[open..]);
            let raw: String = b.lines().filter(|l| !l.contains("cb_recv(&p_k)") && !l.contains("cb_forget_obj(o)")).map(|l| format!("{}\n", l)).collect();
            writeln!(self.protos, "{};", raw_sig).unwrap();
            write!(self.fns, "{}\n{{\n{}}}\n\n", raw_sig, raw).unwrap();
        }
        Ok(Some(b))
    }
}
