// Native C bodies for std's line reading, File and channels.

use super::*;

impl<'a> Gen<'a> {
    // D-0200: `read_line()` natively: the body's steps on its `Vec<u8>`
    // with none of its checks (a local no path ever reaches): `reserve`'s
    // growth (the same capacities, its arithmetic and allocation faults),
    // each read into the spare capacity (`cb_read_line_in`), the line end
    // dropped (`\n`, then a `\r` before it); then `String::from_utf8`'s
    // check, `utf8_error_at` over the bytes (called as its pure slice
    // parameter takes them). The buffer is freed where the body's `Vec`
    // would be destroyed, and the result given an object and sent as the
    // body's is.
    pub(super) fn native_read_line(&mut self) -> R<Option<String>> {
        if !self.pure_direct("std::utf8_error_at", 0) {
            return Ok(None);
        }
        let st = Type::Named("std::String".to_string(), vec![]);
        let ot = Type::Named("std::Option".to_string(), vec![st.clone()]);
        let fe = Type::Named("std::FileError".to_string(), vec![]);
        let rt = Type::Named("std::Result".to_string(), vec![ot, fe]);
        let rc = self.ctype(&rt)?;
        let rtid = self.type_id(&rt)?;
        let uea = self.request_fn("std::utf8_error_at", Vec::new())?;
        let slc = self.ctype(&Type::Slice(Box::new(Type::Int(IntTy::U8)), Mode::Shared))?;
        let mut b = String::new();
        let give = format!("uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro); return r;", rtid);
        let free = "if (cap > 0) cb_deallocate_plain(p, cap);";
        writeln!(b, "    uint8_t *p = (uint8_t *)1; uint64_t len = 0, cap = 0; uint8_t any = 0;").unwrap();
        writeln!(b, "    {} r;", rc).unwrap();
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t need; if (__builtin_add_overflow(len, (uint64_t)128, &need)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        if (need > cap) {{").unwrap();
        writeln!(b, "            uint64_t nc = need;").unwrap();
        writeln!(b, "            if (cap > need - cap && __builtin_mul_overflow(cap, (uint64_t)2, &nc)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "            uint8_t *np = (uint8_t *)cb_allocate(nc, 1u);").unwrap();
        writeln!(b, "            if (!np) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "            if (len > 0) memcpy(np, p, len);").unwrap();
        writeln!(b, "            {}", free).unwrap();
        writeln!(b, "            p = np; cap = nc;").unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "        int64_t n = cb_read_line_in((void *)(p + len), cap - len);").unwrap();
        writeln!(b, "        if (n < 0) {{ {} r.tag = 1u; r.u.v1.tag = 2u; {} }}", free, give).unwrap();
        writeln!(b, "        if (n == 0) break;").unwrap();
        writeln!(b, "        any = 1; len += (uint64_t)n;").unwrap();
        writeln!(b, "        if (p[len - 1] == 10) {{ len -= 1; if (len > 0 && p[len - 1] == 13) len -= 1; break; }}").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    if (!any) {{ {} r.tag = 0u; r.u.v0.tag = 1u; {} }}", free, give).unwrap();
        writeln!(b, "    {} sl; sl.src = NULL; sl.data = p; sl.len = len;", slc).unwrap();
        writeln!(b, "    uint64_t at = {}(sl);", uea).unwrap();
        writeln!(b, "    if (at < len) {{ {} r.tag = 1u; r.u.v1.tag = 3u; r.u.v1.u.v3.offset = at; {} }}", free, give).unwrap();
        writeln!(b, "    r.tag = 0u; r.u.v0.tag = 0u; r.u.v0.u.v0.bytes.ptr = p; r.u.v0.u.v0.bytes.len = len; r.u.v0.u.v0.bytes.cap = cap;").unwrap();
        writeln!(b, "    {}", give).unwrap();
        Ok(Some(b))
    }

    // D-0200: `File::read_line(f)` natively, as `native_read_line`: the
    // checked read of `f.id`, the line's length (`file_op` 4) and its
    // error, `File::take`'s growth of the new `Vec<u8>` (`Vec::grow`: 4,
    // then doubling, its arithmetic and allocation faults) and its read
    // (`file_op` 3, after its own read of `f.id`), the line end dropped,
    // then `String::from_utf8`'s check; the buffer freed where the body
    // would destroy it, the result given an object and sent.
    pub(super) fn native_file_read_line(&mut self) -> R<Option<String>> {
        if !self.pure_direct("std::utf8_error_at", 0) {
            return Ok(None);
        }
        let st = Type::Named("std::String".to_string(), vec![]);
        let ot = Type::Named("std::Option".to_string(), vec![st.clone()]);
        let fe = Type::Named("std::FileError".to_string(), vec![]);
        let rt = Type::Named("std::Result".to_string(), vec![ot, fe]);
        let rc = self.ctype(&rt)?;
        let rtid = self.type_id(&rt)?;
        let fc = self.ctype(&Type::Named("std::File".to_string(), vec![]))?;
        let uea = self.request_fn("std::utf8_error_at", Vec::new())?;
        let ferr = self.request_fn("std::file_error", Vec::new())?;
        let slc = self.ctype(&Type::Slice(Box::new(Type::Int(IntTy::U8)), Mode::Shared))?;
        let give = format!("uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro); return r;", rtid);
        let free = "if (cap > 0) cb_deallocate_plain(p, cap);";
        let mut b = String::new();
        writeln!(b, "    {} r;", rc).unwrap();
        writeln!(b, "    cb_read(p_f.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t id = (({} *)p_f.p)->id;", fc).unwrap();
        writeln!(b, "    int64_t r0 = cb_file_op(4, id, (void *)1, 0);").unwrap();
        writeln!(b, "    if (r0 < 0) {{ r.tag = 1u; r.u.v1 = {}(r0); {} }}", ferr, give).unwrap();
        writeln!(b, "    if (r0 == 0) {{ r.tag = 0u; r.u.v0.tag = 1u; {} }}", give).unwrap();
        writeln!(b, "    uint8_t *p = (uint8_t *)1; uint64_t len = 0, cap = 0, want = (uint64_t)r0;").unwrap();
        writeln!(b, "    while (cap - len < want) {{").unwrap();
        writeln!(b, "        uint64_t nc = 4;").unwrap();
        writeln!(b, "        if (cap != 0 && __builtin_mul_overflow(cap, (uint64_t)2, &nc)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        uint8_t *np = (uint8_t *)cb_allocate(nc, 1u);").unwrap();
        writeln!(b, "        if (!np) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        if (len > 0) memcpy(np, p, len);").unwrap();
        writeln!(b, "        {}", free).unwrap();
        writeln!(b, "        p = np; cap = nc;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_read(p_f.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    int64_t rr = cb_file_op(3, (({} *)p_f.p)->id, (void *)(p + len), want);", fc).unwrap();
        writeln!(b, "    if (rr < 0) {{ {} r.tag = 1u; r.u.v1 = {}(rr); {} }}", free, ferr, give).unwrap();
        writeln!(b, "    uint64_t n = (uint64_t)rr; len += n;").unwrap();
        writeln!(b, "    if (n > 0 && p[n - 1] == 10) {{ len -= 1; if (n > 1 && p[n - 2] == 13) len -= 1; }}").unwrap();
        writeln!(b, "    {} sl; sl.src = NULL; sl.data = p; sl.len = len;", slc).unwrap();
        writeln!(b, "    uint64_t at = {}(sl);", uea).unwrap();
        writeln!(b, "    if (at < len) {{ {} r.tag = 1u; r.u.v1.tag = 3u; r.u.v1.u.v3.offset = at; {} }}", free, give).unwrap();
        writeln!(b, "    r.tag = 0u; r.u.v0.tag = 0u; r.u.v0.u.v0.bytes.ptr = p; r.u.v0.u.v0.bytes.len = len; r.u.v0.u.v0.bytes.cap = cap;").unwrap();
        writeln!(b, "    {}", give).unwrap();
        Ok(Some(b))
    }

    // D-0200: `File::write(f, data)` and `File::write_text(f, text)`
    // natively: the bytes' datum received as the body's parameter receives
    // it (unless the callers send none: a pure slice or view), then the
    // body's steps -- nothing for no bytes; the checked read of `f.id`;
    // the first byte's index and element access (`&data[0]`, which
    // `rawptr_of` takes); the write (`file_op` 5) and its error. `write_text`
    // is `write` of its text's bytes, whose hand-off checks nothing. The
    // bindings and the datum's objects, which nothing else reaches, are
    // not made.
    pub(super) fn native_file_write(&mut self, name: &str) -> R<Option<String>> {
        let text = name == "std::File::write_text";
        let pure = if text { self.pure_view(name, 1) } else { self.pure_direct(name, 1) };
        let ft = Type::Named("std::File".to_string(), vec![]);
        let fc = self.ctype(&ft)?;
        let fe = Type::Named("std::FileError".to_string(), vec![]);
        let rt = Type::Named("std::Result".to_string(), vec![Type::Void, fe.clone()]);
        if self.is_resource(&rt) || self.has_refs(&rt) {
            return Ok(None);
        }
        let rc = self.ctype(&rt)?;
        let u8tid = self.type_id(&Type::Int(IntTy::U8))?;
        let ferr = self.request_fn("std::file_error", Vec::new())?;
        let d = if text { "p_text.bytes" } else { "p_data" };
        let mut b = String::new();
        if !pure {
            writeln!(b, "    (void)cb_recv_datum_tok();").unwrap();
        }
        writeln!(b, "    {} r;", rc).unwrap();
        writeln!(b, "    uint64_t n = {}.len;", d).unwrap();
        writeln!(b, "    if (n == 0) {{ r.tag = 0u; return r; }}").unwrap();
        writeln!(b, "    cb_read(p_f.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t id = (({} *)p_f.p)->id;", fc).unwrap();
        writeln!(b, "    cb_elem_access((void *)&{}.data[0], 1u, {}u, CB_SHARED, 0u, CB_NOLOC, 0);", d, u8tid).unwrap();
        writeln!(b, "    int64_t w = cb_file_op(5, id, (void *){}.data, n);", d).unwrap();
        writeln!(b, "    if (w < 0) {{ r.tag = 1u; r.u.v1 = {}(w); return r; }}", ferr).unwrap();
        writeln!(b, "    r.tag = 0u;").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // D-0200: `Channel::send` and `recv` natively, for values holding no
    // reference, no `fn` and no destructor of the program's own: the same
    // steps under the same lock and the same events (`event_op`: the count
    // a waiter watches read under the lock, the wait after it is
    // released, the signal under it). The body's `lock(&ch.state)` makes
    // a guard whose path every access to the state goes through; the
    // state is reachable through nothing else (a private field, locked
    // only by these bodies, each releasing it before it returns), and
    // the lock excludes every other thread, so no check through the guard
    // can fail: the first, of `ch` itself (`&ch.state`), is made, then the
    // ring is used directly between `cb_lock_bare` and `cb_unlock_bare`.
    // A value enters a slot as bytes (its record ending, as `cb_raw_move_in`
    // ends one) and leaves with a new one; a slot a value left holds `None`
    // as the body leaves it. Results of resource types are given objects
    // and sent, as the body's are.
    pub(super) fn native_channel(&mut self, send: bool, t: &Type) -> R<Option<String>> {
        let (size, _) = self.size_align(t)?;
        let res = self.is_resource(t) && self.objectless_type(t);
        if size == 0 || !(self.plain_data(t) || res) {
            return Ok(None);
        }
        let cc = self.ctype(&Type::Named("std::Channel".to_string(), vec![t.clone()]))?;
        let ot = Type::Named("std::Option".to_string(), vec![t.clone()]);
        let mut b = String::new();
        writeln!(b, "    {} *ch = ({} *)p_ch.p;", cc, cc).unwrap();
        if send {
            let rt = Type::Named("std::Result".to_string(), vec![Type::Void, t.clone()]);
            let rc = self.ctype(&rt)?;
            let robj = self.is_resource(&rt);
            let rtid = if robj { self.type_id(&rt)? } else { 0 };
            if res {
                writeln!(b, "    uint64_t vo = cb_recv(&p_v);").unwrap();
            }
            writeln!(b, "    cb_borrow_check(p_ch.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
            let fin = |b: &mut String, ind: &str| {
                if res {
                    writeln!(b, "{}cb_forget_obj(vo);", ind).unwrap();
                }
                if robj {
                    writeln!(b, "{}uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro);", ind, rtid).unwrap();
                }
                writeln!(b, "{}return r;", ind).unwrap();
            };
            writeln!(b, "    for (;;) {{").unwrap();
            writeln!(b, "        cb_lock_bare((void *)&ch->state, CB_NOLOC, 0);").unwrap();
            writeln!(b, "        if (ch->state.inner.closed) {{").unwrap();
            writeln!(b, "            cb_unlock_bare((void *)&ch->state);").unwrap();
            writeln!(b, "            {} r; r.tag = 1u; r.u.v1 = p_v;", rc).unwrap();
            fin(&mut b, "            ");
            writeln!(b, "        }}").unwrap();
            writeln!(b, "        if (ch->state.inner.count < ch->cap) {{").unwrap();
            writeln!(b, "            uint64_t at = (ch->state.inner.head + ch->state.inner.count) % ch->cap;").unwrap();
            writeln!(b, "            if (at >= ch->state.inner.slots.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
            writeln!(b, "            ch->state.inner.slots.ptr[at].tag = 0u; ch->state.inner.slots.ptr[at].u.v0 = p_v;").unwrap();
            writeln!(b, "            ch->state.inner.count += 1;").unwrap();
            writeln!(b, "            cb_unlock_signal((void *)&ch->state, ch->can_recv);").unwrap();
            writeln!(b, "            {} r; r.tag = 0u;", rc).unwrap();
            fin(&mut b, "            ");
            writeln!(b, "        }}").unwrap();
            writeln!(b, "        uint64_t seen = cb_event_op(1, ch->can_send, 0);").unwrap();
            writeln!(b, "        cb_unlock_bare((void *)&ch->state);").unwrap();
            writeln!(b, "        (void)cb_event_op(2, ch->can_send, seen);").unwrap();
            writeln!(b, "    }}").unwrap();
            // The `_raw` form, for a value made with no object
            // (`Fx::lower_channel_send_fresh`): nothing to receive or end.
            if res {
                if let Some((_, _, _, _, sig)) = self.native_ctx.clone() {
                    let open = sig.find('(').expect("a signature");
                    let raw_sig = format!("{}_raw{}", &sig[..open], &sig[open..]);
                    let raw: String = b.lines().filter(|l| !l.contains("cb_recv(&p_v)") && !l.contains("cb_forget_obj(vo)")).map(|l| format!("{}\n", l)).collect();
                    writeln!(self.protos, "{};", raw_sig).unwrap();
                    write!(self.fns, "{}\n{{\n{}}}\n\n", raw_sig, raw).unwrap();
                }
            }
        } else {
            let oc = self.ctype(&ot)?;
            let oobj = self.is_resource(&ot);
            let otid = if oobj { self.type_id(&ot)? } else { 0 };
            let give = if oobj { format!(" uint64_t ro = cb_new(&r, {}u, CB_NOLOC, 0); cb_send(ro);", otid) } else { String::new() };
            writeln!(b, "    cb_borrow_check(p_ch.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    for (;;) {{").unwrap();
            writeln!(b, "        cb_lock_bare((void *)&ch->state, CB_NOLOC, 0);").unwrap();
            writeln!(b, "        if (ch->state.inner.count > 0) {{").unwrap();
            writeln!(b, "            uint64_t at = ch->state.inner.head;").unwrap();
            writeln!(b, "            if (at >= ch->state.inner.slots.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
            writeln!(b, "            {} r = ch->state.inner.slots.ptr[at];", oc).unwrap();
            writeln!(b, "            ch->state.inner.slots.ptr[at].tag = 1u;").unwrap();
            writeln!(b, "            ch->state.inner.head = (at + 1) % ch->state.inner.slots.len;").unwrap();
            writeln!(b, "            ch->state.inner.count -= 1;").unwrap();
            writeln!(b, "            cb_unlock_signal((void *)&ch->state, ch->can_send);").unwrap();
            writeln!(b, "           {}", give).unwrap();
            writeln!(b, "            return r;").unwrap();
            writeln!(b, "        }}").unwrap();
            writeln!(b, "        if (ch->state.inner.closed) {{").unwrap();
            writeln!(b, "            cb_unlock_bare((void *)&ch->state);").unwrap();
            writeln!(b, "            {} r; r.tag = 1u;{}", oc, give).unwrap();
            writeln!(b, "            return r;").unwrap();
            writeln!(b, "        }}").unwrap();
            writeln!(b, "        uint64_t seen = cb_event_op(1, ch->can_recv, 0);").unwrap();
            writeln!(b, "        cb_unlock_bare((void *)&ch->state);").unwrap();
            writeln!(b, "        (void)cb_event_op(2, ch->can_recv, seen);").unwrap();
            writeln!(b, "    }}").unwrap();
        }
        Ok(Some(b))
    }
}
