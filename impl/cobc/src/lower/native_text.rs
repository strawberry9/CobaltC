// Native C bodies for std's String and StringView operations.

use super::*;

impl<'a> Gen<'a> {
    // `String::append_string(s, t)`: the body's checked read of `t.bytes.len`;
    // then, when there is a byte, the first iteration's checked read of
    // `t.bytes.ptr` and its push's borrow check of `s.bytes`. The later
    // iterations' checks are the same checks through the same paths with
    // nothing between that could change them (`s` and `t` cannot share a
    // `String`: the call holds both), so the loop runs unchecked, handing
    // `grow` a path formed for it when it needs one (as the inline push
    // does, `lower_push_place_once`).
    // `String::from_str(s)` and `String::clone(&t)` (`spec/21` §2): a new
    // `String` holding a copy of the bytes. The bodies build `Vec::new()`,
    // `Vec::reserve` it to exactly `n` (a fresh vector's capacity is 0, so
    // `max(n, 0)`), and push each byte, which never grows. Natively: the
    // checked reads `append_string` makes of `t` (for `clone`; a `str` is
    // a value), one `allocate` of `n` bytes with its failure, the copy,
    // and the result object, established and sent as the body's `return`
    // does. No other state the bodies touch is observable: their locals
    // end inside them, and the new cells have no object over them.
    pub(super) fn native_string_copy(&mut self, clone: bool) -> R<String> {
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let ty = self.type_id(&st)?;
        let _ = self.ctype(&Type::Named("std::Vec".to_string(), vec![Type::Int(IntTy::U8)]))?;
        let mut b = String::new();
        if clone {
            writeln!(b, "    cb_read(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} *t = ({} *)p_s.p;", sc, sc).unwrap();
            writeln!(b, "    uint64_t n = t->bytes.len;").unwrap();
            writeln!(b, "    const uint8_t *src = t->bytes.ptr;").unwrap();
        } else {
            writeln!(b, "    uint64_t n = p_s.n;").unwrap();
            writeln!(b, "    const uint8_t *src = (const uint8_t *)p_s.p;").unwrap();
        }
        writeln!(b, "    {} r;", sc).unwrap();
        writeln!(b, "    r.bytes.ptr = (uint8_t *)1;").unwrap();
        writeln!(b, "    r.bytes.len = 0;").unwrap();
        writeln!(b, "    r.bytes.cap = 0;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(n, 1u);").unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        if clone {
            writeln!(b, "        cb_read(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 0u }} }}, 2, CB_NOLOC, 0);").unwrap();
        }
        writeln!(b, "        memcpy(p, src, n);").unwrap();
        writeln!(b, "        r.bytes.ptr = p;").unwrap();
        writeln!(b, "        r.bytes.cap = n;").unwrap();
        writeln!(b, "        r.bytes.len = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t o = cb_new(&r, {}u, CB_NOLOC, 0);", ty).unwrap();
        writeln!(b, "    cb_send(o);").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(b)
    }

    pub(super) fn native_append_string(&mut self) -> R<String> {
        // The body: `Vec::reserve(&mut s.bytes, n)` -- the borrow formed for
        // the call (its exclusive check, made even when `n` is 0), then
        // `need = len + n` (its overflow fault) and, when it does not fit,
        // a buffer of `max(2 * cap, need)` -- then `t`'s bytes pushed one at
        // a time. Nothing runs between those checks and the pushes, so the
        // pushes' own checks cannot fail where the borrow's passed: the
        // bytes are copied at once. `reserve`'s growth rule is kept exactly.
        let ovf = "cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0)";
        let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_t.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *t = ({} *)p_t.p;", sc, sc).unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t n = t->bytes.len;").unwrap();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t need;").unwrap();
        writeln!(b, "    if (__builtin_add_overflow(s->bytes.len, n, &need)) {};", ovf).unwrap();
        writeln!(b, "    if (n == 0) return;").unwrap();
        writeln!(b, "    cb_read(p_t.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 0u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (need > s->bytes.cap) {{").unwrap();
        writeln!(b, "        uint64_t nc = need;").unwrap();
        writeln!(b, "        if (s->bytes.cap > need - s->bytes.cap && __builtin_mul_overflow(s->bytes.cap, (uint64_t)2, &nc)) {};", ovf).unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(nc, 1u);").unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        cb_copy_raw(p, (uint8_t *)s->bytes.ptr, s->bytes.len);").unwrap();
        writeln!(b, "        if (s->bytes.cap > 0) cb_deallocate((uint8_t *)s->bytes.ptr, s->bytes.cap, 1u);").unwrap();
        writeln!(b, "        s->bytes.ptr = p;").unwrap();
        writeln!(b, "        s->bytes.cap = nc;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    memcpy(s->bytes.ptr + s->bytes.len, t->bytes.ptr, n);").unwrap();
        writeln!(b, "    s->bytes.len += n;").unwrap();
        Ok(b)
    }

    // `String::append_str`, `append_view` and `append_text<T>` (`spec/21`
    // §2d) natively: each body pushes its bytes one at a time, each push
    // through a borrow of `s.bytes` formed and checked anew, each byte of a
    // view read through its checked path, and `append_text` formats into
    // 64 allocated bytes first. Nothing else runs in between, so the first
    // byte's checks are every byte's: they are made once (when there is a
    // byte), `s.bytes` grows as the pushes would grow it (`Vec::grow`
    // until the bytes fit, the same doublings), and the bytes are copied.
    // `append_text` formats into a buffer on the stack (`text_write`'s
    // runtime call); a view not sent to it (`pure_view`) is the caller's
    // to hold, and one sent is received as its body's binding would be,
    // its path ending with the caller's statement.
    pub(super) fn native_append_bytes(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let grow = self.request_fn("std::Vec::grow", vec![Type::Int(IntTy::U8)])?;
        let mut b = String::new();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        let src = match name {
            "std::String::append_str" => {
                writeln!(b, "    uint64_t n = p_t.n;").unwrap();
                writeln!(b, "    if (n == 0) return;").unwrap();
                writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
                "(const void *)p_t.p".to_string()
            }
            "std::String::append_view" => {
                let sent = !self.pure_view("std::String::append_view", 1);
                if sent {
                    writeln!(b, "    uint64_t vt = cb_recv_datum_tok();").unwrap();
                }
                writeln!(b, "    uint64_t n = p_v.bytes.len;").unwrap();
                writeln!(b, "    if (n == 0) return;").unwrap();
                writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
                if sent {
                    // Every byte's `[Read]` through the view's path.
                    writeln!(b, "    if (vt != 0) cb_borrow_range_unminted(vt, NULL, 0, 0u, n, CB_SHARED, CB_NOLOC, 0);").unwrap();
                }
                "(const void *)p_v.bytes.data".to_string()
            }
            _ => {
                let [t] = targs else { return internal("append_text's type argument") };
                let call = match t {
                    Type::Int(it) => {
                        let wide = if it.signed() { "(cb_u128)(cb_i128)(p_x)" } else { "(cb_u128)(p_x)" };
                        format!("cb_text_int((uint64_t)({} >> 64), (uint64_t)({}), {}u, (void *)buf)", wide, wide, it.signed() as u32)
                    }
                    Type::F64 => "cb_text_f64(p_x, (void *)buf)".to_string(),
                    Type::F32 => "cb_text_f32(p_x, (void *)buf)".to_string(),
                    Type::Bool => "cb_text_bool(p_x, (void *)buf)".to_string(),
                    _ => return Ok(None),
                };
                writeln!(b, "    uint8_t buf[64];").unwrap();
                writeln!(b, "    uint64_t n = {};", call).unwrap();
                writeln!(b, "    if (n == 0) return;").unwrap();
                writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
                "(const void *)buf".to_string()
            }
        };
        writeln!(b, "    while (s->bytes.cap - s->bytes.len < n) {{ uint64_t gt = cb_borrow_unstamped(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&s->bytes, gt }})); cb_call_done(gt); }}", grow).unwrap();
        writeln!(b, "    memcpy(s->bytes.ptr + s->bytes.len, {}, n);", src).unwrap();
        writeln!(b, "    s->bytes.len += n;").unwrap();
        Ok(Some(b))
    }

    // `StringView::sub(v, lo, hi)` natively: the body receives `v` (bound
    // in its frame), makes its bounds and character-boundary checks
    // (`StringView::boundary`, reading `v.bytes` through its fresh binding,
    // which cannot fail), borrows `[lo, hi)` of `v`'s path and returns the
    // view built over it as reference data. Here `v`'s path is received as
    // `split` receives it, the same checks are made in the same order, the
    // range borrowed from that path, and the new path sent as the result's
    // one reference (`cb_send_tok_datum`), as `cb_send_datum` would send it
    // from the body's local.
    pub(super) fn native_view_sub(&mut self) -> R<Option<String>> {
        // `boundary` is called with the view's bytes and nothing sent: the
        // form a pure direct slice parameter takes (D-0191). When it is not
        // one (`--all-checks`), the body is lowered as written.
        if self.pure_view("std::StringView::sub", 0) || !self.pure_direct("std::StringView::boundary", 0) {
            return Ok(None);
        }
        let svc = self.ctype(&Type::Named("std::StringView".to_string(), vec![]))?;
        let boundary = self.request_fn("std::StringView::boundary", vec![])?;
        let mut b = String::new();
        writeln!(b, "    uint64_t vt = cb_recv_datum_tok();").unwrap();
        writeln!(b, "    if (p_lo > p_hi || p_hi > p_v.bytes.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (!{b}(p_v.bytes, p_lo) || !{b}(p_v.bytes, p_hi)) cb_fault(\"diag.not-char-boundary\", CB_NOLOC, 0);", b = boundary).unwrap();
        writeln!(b, "    uint64_t tk = cb_borrow_range(vt, NULL, 0, p_lo, p_hi, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} r;", svc).unwrap();
        writeln!(b, "    r.bytes.src = p_v.bytes.src;").unwrap();
        writeln!(b, "    r.bytes.data = p_v.bytes.data + p_lo;").unwrap();
        writeln!(b, "    r.bytes.len = p_hi - p_lo;").unwrap();
        writeln!(b, "    cb_send_tok_datum(tk);").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `String::from_view(v)` natively: the body is `String { .bytes =
    // Vec::from_slice(v.bytes) }` -- `v` received and bound, its bytes
    // copied (`from_slice`'s native body: an exact allocation, the bytes,
    // no check per byte), the `String` built and returned. Here `v`'s path
    // is received as `sub` receives it, the bytes copied, and the result a
    // binding moved out and sent, as `from_slice`'s is.
    pub(super) fn native_string_from_view(&mut self) -> R<Option<String>> {
        if self.pure_view("std::String::from_view", 0) {
            return Ok(None);
        }
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let stid = self.type_id(&st)?;
        let mut b = String::new();
        writeln!(b, "    (void)cb_recv_datum_tok();").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", sc).unwrap();
        writeln!(b, "    out.bytes.ptr = (uint8_t *)1;").unwrap();
        writeln!(b, "    out.bytes.len = 0;").unwrap();
        writeln!(b, "    out.bytes.cap = 0;").unwrap();
        writeln!(b, "    uint64_t n = p_v.bytes.len;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(n, 1u);").unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        memcpy(p, p_v.bytes.data, n);").unwrap();
        writeln!(b, "        out.bytes.ptr = p;").unwrap();
        writeln!(b, "        out.bytes.len = n;").unwrap();
        writeln!(b, "        out.bytes.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", stid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", sc).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // `StringView::split(v, sep)` (`spec/21`, D-0113) natively: the body
    // builds `out` by pushing `StringView::sub(v, start, i)` for each piece,
    // and each push moves a temporary view (an object, sent and received
    // twice) into the vector. Here each piece's checks are `sub`'s (its
    // bounds hold by construction; both ends must be character
    // boundaries, `StringView::boundary`, else `diag.not-char-boundary`),
    // its path is `sub`'s -- the range borrowed from `v`'s path -- and it
    // is written straight into `out`'s cells with that path in its slot,
    // as `push`'s `cb_recv_datum` would leave it. `out` is a binding as in
    // the body (its object, bound in a frame, grown through its root, then
    // moved out and sent); `v` is received as the body's parameter is
    // (it is never a pure view: the result holds references). The search
    // reads `v`'s bytes as `StringView::at` does, unchecked (`at`'s view
    // is pure), with `i + m`'s overflow fault.
    pub(super) fn native_view_split(&mut self) -> R<Option<String>> {
        if self.pure_view("std::StringView::split", 0) || !self.pure_direct("std::StringView::boundary", 0) {
            return Ok(None);
        }
        let svt = Type::Named("std::StringView".to_string(), vec![]);
        let vect = Type::Named("std::Vec".to_string(), vec![svt.clone()]);
        let vc = self.ctype(&vect)?;
        let svc = self.ctype(&svt)?;
        let vtid = self.type_id(&vect)?;
        let grow = self.request_fn("std::Vec::grow", vec![svt.clone()])?;
        let boundary = self.request_fn("std::StringView::boundary", vec![])?;
        let ovf = "cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0)";
        let mut b = String::new();
        writeln!(b, "    uint64_t vt = cb_recv_datum_tok();").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", vc).unwrap();
        writeln!(b, "    out.ptr = ({} *)1;", svc).unwrap();
        writeln!(b, "    out.len = 0;").unwrap();
        writeln!(b, "    out.cap = 0;").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", vtid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t n = p_v.bytes.len;").unwrap();
        writeln!(b, "    uint64_t m = p_sep.n;").unwrap();
        // One piece, `[lo, hi)` of `v` with path `tk`, pushed.
        let piece = format!(
            "{{ if (out.len == out.cap) {{ {}(((cb_ref){{ (void *)&out, root }})); }} {} *el = out.ptr + out.len; el->bytes.src = p_v.bytes.src; el->bytes.data = p_v.bytes.data + (LO); el->bytes.len = (HI) - (LO); cb_store_ref(&el->bytes.src, TK); out.len += 1; }}",
            grow, svc
        );
        let cut = |lo: &str, hi: &str| -> String {
            format!(
                "    if (!{b}(p_v.bytes, {lo}) || !{b}(p_v.bytes, {hi})) cb_fault(\"diag.not-char-boundary\", CB_NOLOC, 0);\n    {{ uint64_t tk = cb_borrow_range(vt, NULL, 0, {lo}, {hi}, CB_SHARED, CB_NOLOC, 0);\n    {p} }}\n",
                b = boundary,
                lo = lo,
                hi = hi,
                p = piece.replace("(LO)", &format!("({})", lo)).replace("(HI)", &format!("({})", hi)).replace("TK", "tk")
            )
        };
        writeln!(b, "    if (m == 0) {{").unwrap();
        // `Vec::push(&mut out, v)`: `v` itself, its own path.
        writeln!(b, "    {}", piece.replace("(LO)", "(0)").replace("(HI)", "(n)").replace("TK", "vt")).unwrap();
        writeln!(b, "    }} else {{").unwrap();
        writeln!(b, "    uint64_t start = 0, i = 0;").unwrap();
        writeln!(b, "    for (;;) {{").unwrap();
        writeln!(b, "        uint64_t e;").unwrap();
        writeln!(b, "        if (__builtin_add_overflow(i, m, &e)) {};", ovf).unwrap();
        writeln!(b, "        if (!(e <= n)) break;").unwrap();
        writeln!(b, "        if (memcmp(p_v.bytes.data + i, p_sep.p, m) == 0) {{").unwrap();
        b.push_str(&cut("start", "i").replace("\n    ", "\n        "));
        writeln!(b, "            if (__builtin_add_overflow(i, m, &i)) {};", ovf).unwrap();
        writeln!(b, "            start = i;").unwrap();
        writeln!(b, "        }} else {{").unwrap();
        writeln!(b, "            if (__builtin_add_overflow(i, (uint64_t)1, &i)) {};", ovf).unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "    }}").unwrap();
        b.push_str(&cut("start", "n"));
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", vc).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // D-0193: `StringView::split_whitespace(v)` natively, as `split` is
    // (`native_view_split`): each word, `StringView::sub(v, start, i)` in
    // the body, borrowed from `v`'s path and written straight into `out`.
    // Its ends lie next to ASCII bytes (or at `v`'s ends), so `sub`'s
    // character-boundary checks cannot fail and are not repeated.
    //
    // `chars` (D-0128) the same way: a piece from each character's first
    // byte (one that is not `10xxxxxx`) to the next one, or the end.
    pub(super) fn native_view_split_whitespace(&mut self, chars: bool) -> R<Option<String>> {
        let key = if chars { "std::StringView::chars" } else { "std::StringView::split_whitespace" };
        if self.pure_view(key, 0) {
            return Ok(None);
        }
        let svt = Type::Named("std::StringView".to_string(), vec![]);
        let vect = Type::Named("std::Vec".to_string(), vec![svt.clone()]);
        let vc = self.ctype(&vect)?;
        let svc = self.ctype(&svt)?;
        let vtid = self.type_id(&vect)?;
        let grow = self.request_fn("std::Vec::grow", vec![svt.clone()])?;
        let mut b = String::new();
        writeln!(b, "    uint64_t vt = cb_recv_datum_tok();").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", vc).unwrap();
        writeln!(b, "    out.ptr = ({} *)1;", svc).unwrap();
        writeln!(b, "    out.len = 0;").unwrap();
        writeln!(b, "    out.cap = 0;").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", vtid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    const uint8_t *d = p_v.bytes.data;").unwrap();
        writeln!(b, "    uint64_t n = p_v.bytes.len, i = 0;").unwrap();
        writeln!(b, "    while (i < n) {{").unwrap();
        if chars {
            writeln!(b, "        uint64_t start = i;").unwrap();
            writeln!(b, "        i++;").unwrap();
            writeln!(b, "        while (i < n && (d[i] & 0xc0) == 0x80) i++;").unwrap();
        } else {
            writeln!(b, "        while (i < n && (d[i] == ' ' || (d[i] >= 9 && d[i] <= 13))) i++;").unwrap();
            writeln!(b, "        uint64_t start = i;").unwrap();
            writeln!(b, "        while (i < n && !(d[i] == ' ' || (d[i] >= 9 && d[i] <= 13))) i++;").unwrap();
        }
        writeln!(b, "        if (i > start) {{").unwrap();
        writeln!(b, "            uint64_t tk = cb_borrow_range(vt, NULL, 0, start, i, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "            if (out.len == out.cap) {{ {}(((cb_ref){{ (void *)&out, root }})); }}", grow).unwrap();
        writeln!(b, "            {} *el = out.ptr + out.len; el->bytes.src = p_v.bytes.src; el->bytes.data = p_v.bytes.data + start; el->bytes.len = i - start; cb_store_ref(&el->bytes.src, tk); out.len += 1;", svc).unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", vc).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // D-0193: `StringView::parse<T>` and `String::parse<T>` for an integer
    // `T`, natively: the body hands the text's bytes to `parse_check`, then
    // to `parse_value` when they are a `T`, and builds the `Result` from the
    // code (`-1` a number, `-2` `Empty`, `-3` `OutOfRange`, else
    // `Invalid(at)`). Here the same two calls on the same bytes: the view's
    // (received as the body's parameter is) or the `String`'s (after the
    // body's checked read of them).
    pub(super) fn native_parse_int(&mut self, name: &str, it: IntTy) -> R<Option<String>> {
        let t = Type::Int(it);
        let rc = self.ctype(&Type::Named("std::Result".to_string(), vec![t.clone(), Type::Named("std::ParseError".to_string(), vec![])]))?;
        let tc = self.ctype(&t)?;
        let mut b = String::new();
        if name == "std::StringView::parse" {
            if !self.pure_view(name, 0) {
                writeln!(b, "    (void)cb_recv_datum_tok();").unwrap();
            }
            writeln!(b, "    const uint8_t *p = p_v.bytes.data; uint64_t n = p_v.bytes.len;").unwrap();
        } else {
            let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
            writeln!(b, "    cb_read(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    const uint8_t *p = (({} *)p_s.p)->bytes.ptr; uint64_t n = (({} *)p_s.p)->bytes.len;", sc, sc).unwrap();
        }
        let head = format!("cb_parse_int((const void *)p, n, {}u, {}u", it.signed() as u32, it.bitwidth(64));
        writeln!(b, "    {} r;", rc).unwrap();
        writeln!(b, "    int64_t code = {}, 0);", head).unwrap();
        writeln!(b, "    if (code == -1) {{ uint64_t o[2]; {}, o); r.tag = 0u; r.u.v0 = ({})((((cb_u128)o[0]) << 64) | o[1]); return r; }}", head, tc).unwrap();
        writeln!(b, "    r.tag = 1u;").unwrap();
        writeln!(b, "    if (code == -2) r.u.v1.tag = 0u;").unwrap();
        writeln!(b, "    else if (code == -3) r.u.v1.tag = 2u;").unwrap();
        writeln!(b, "    else {{ r.u.v1.tag = 1u; r.u.v1.u.v1 = (uint64_t)code; }}").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // D-0193: `StringView::trim`, `trim_start`, `trim_end` natively: the
    // body reads `v`'s bytes through its fresh binding (which cannot fail),
    // then returns `StringView::sub(v, i, n)`, whose ends lie next to ASCII
    // whitespace or at `v`'s ends (boundaries): here `v` received as `sub`
    // receives it, the same range found, borrowed from `v`'s path and sent
    // as the result's one reference (`native_view_sub`).
    pub(super) fn native_view_trim(&mut self, name: &str) -> R<Option<String>> {
        if self.pure_view(name, 0) {
            return Ok(None);
        }
        let svc = self.ctype(&Type::Named("std::StringView".to_string(), vec![]))?;
        let mut b = String::new();
        writeln!(b, "    uint64_t vt = cb_recv_datum_tok();").unwrap();
        writeln!(b, "    const uint8_t *d = p_v.bytes.data;").unwrap();
        writeln!(b, "    uint64_t n = p_v.bytes.len, i = 0;").unwrap();
        if name != "std::StringView::trim_end" {
            writeln!(b, "    while (i < n && (d[i] == ' ' || (d[i] >= 9 && d[i] <= 13))) i++;").unwrap();
        }
        if name != "std::StringView::trim_start" {
            writeln!(b, "    while (n > i && (d[n - 1] == ' ' || (d[n - 1] >= 9 && d[n - 1] <= 13))) n--;").unwrap();
        }
        writeln!(b, "    uint64_t tk = cb_borrow_range(vt, NULL, 0, i, n, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} r;", svc).unwrap();
        writeln!(b, "    r.bytes.src = p_v.bytes.src;").unwrap();
        writeln!(b, "    r.bytes.data = p_v.bytes.data + i;").unwrap();
        writeln!(b, "    r.bytes.len = n - i;").unwrap();
        writeln!(b, "    cb_send_tok_datum(tk);").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `find`, `contains`, `starts_with` and `ends_with` of a `StringView`
    // or a `String` natively. The bodies read bytes and keep nothing: a
    // view's bytes through its fresh binding, which cannot fail (`trim`'s
    // note), and a `String`'s through the shared borrow `&(*s)[0..$]` of
    // its bytes, whose check is the one that can (`cb_borrow_check`). Here
    // the views are received as the bodies receive them (none for a pure
    // view parameter, which the caller keeps), the borrow's check made,
    // and the bytes compared at once: `StringView::at` position by position
    // gives the answer `cb_find_bytes` gives (an empty needle at 0).
    pub(super) fn native_text_search(&mut self, name: &str) -> R<Option<String>> {
        let on_string = name.starts_with("std::String::");
        let mut b = String::new();
        for (j, _) in ["v", "t"].iter().enumerate() {
            if (j == 1 || !on_string) && !self.pure_view(name, j) {
                writeln!(b, "    (void)cb_recv_datum_tok();").unwrap();
            }
        }
        if on_string {
            let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
            writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    const uint8_t *h = (({} *)p_s.p)->bytes.ptr;", sc).unwrap();
            writeln!(b, "    uint64_t hn = (({} *)p_s.p)->bytes.len;", sc).unwrap();
        } else {
            writeln!(b, "    const uint8_t *h = p_v.bytes.data;").unwrap();
            writeln!(b, "    uint64_t hn = p_v.bytes.len;").unwrap();
        }
        writeln!(b, "    const uint8_t *n = p_t.bytes.data;").unwrap();
        writeln!(b, "    uint64_t nn = p_t.bytes.len;").unwrap();
        let op = name.rsplit("::").next().unwrap_or("");
        match op {
            "find" => {
                let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Int(IntTy::Usize)]))?;
                writeln!(b, "    uint64_t i = cb_find_bytes(h, hn, n, nn);").unwrap();
                writeln!(b, "    {} r;", oc).unwrap();
                writeln!(b, "    if (i == UINT64_MAX) {{ r.tag = 1u; return r; }}").unwrap();
                writeln!(b, "    r.tag = 0u;").unwrap();
                writeln!(b, "    r.u.v0 = i;").unwrap();
                writeln!(b, "    return r;").unwrap();
            }
            "contains" => writeln!(b, "    return (uint8_t)(cb_find_bytes(h, hn, n, nn) != UINT64_MAX);").unwrap(),
            "starts_with" => writeln!(b, "    return (uint8_t)(nn <= hn && (nn == 0 || memcmp(h, n, (size_t)nn) == 0));").unwrap(),
            _ => writeln!(b, "    return (uint8_t)(nn <= hn && (nn == 0 || memcmp(h + (hn - nn), n, (size_t)nn) == 0));").unwrap(),
        }
        Ok(Some(b))
    }

    // `String::from_view(String::view(s, lo, hi))` as one call: the checks
    // `native_string_view` makes, in its order, then the bytes copied into a
    // new `String` returned as `native_string_from_view` returns its. The
    // view's path is never formed: `from_view` would receive it and it
    // would end, unheld, with the statement (D-0018: no check counts an
    // unheld path), and every check its borrow could fail is the whole
    // bytes' check made first.
    pub(super) fn substring_helper(&mut self) -> R<String> {
        let name = "cbn_string_substring".to_string();
        if self.substring_fn {
            return Ok(name);
        }
        self.substring_fn = true;
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let stid = self.type_id(&st)?;
        let sig = format!("{} {}(cb_ref p_s, uint64_t p_lo, uint64_t p_hi)", sc, name);
        let mut b = String::new();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t n = s->bytes.len;").unwrap();
        writeln!(b, "    if (p_lo > p_hi || p_hi > n) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (!cb_char_boundary(s->bytes.ptr, n, p_lo) || !cb_char_boundary(s->bytes.ptr, n, p_hi)) cb_fault(\"diag.not-char-boundary\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", sc).unwrap();
        writeln!(b, "    out.bytes.ptr = (uint8_t *)1;").unwrap();
        writeln!(b, "    out.bytes.len = 0;").unwrap();
        writeln!(b, "    out.bytes.cap = 0;").unwrap();
        writeln!(b, "    uint64_t k = p_hi - p_lo;").unwrap();
        writeln!(b, "    if (k > 0) {{").unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(k, 1u);").unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        memcpy(p, s->bytes.ptr + p_lo, k);").unwrap();
        writeln!(b, "        out.bytes.ptr = p;").unwrap();
        writeln!(b, "        out.bytes.len = k;").unwrap();
        writeln!(b, "        out.bytes.cap = k;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", stid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", sc).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        writeln!(self.protos, "{};", sig).unwrap();
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, b).unwrap();
        // D-0200: the `_raw` form, for a result with no object
        // (`Fx::lower_raw_string`): the same checks and copy, no frame.
        let at = b.find("    uint64_t oo = cb_new(&out").expect("the helper's result");
        let raw = format!("{}    return out;\n", b[..at].replacen("    cb_frame_push();\n", "", 1));
        let raw_sig = format!("{} {}_raw(cb_ref p_s, uint64_t p_lo, uint64_t p_hi)", sc, name);
        writeln!(self.protos, "{};", raw_sig).unwrap();
        write!(self.fns, "{}\n{{\n{}}}\n\n", raw_sig, raw).unwrap();
        Ok(name)
    }

    // `String::append(s, String::view(t, lo, hi))` as one call: the view's
    // checks (`native_string_view`), then `append_view`'s (`native_append_
    // bytes`): nothing when the view is empty; else `s.bytes` borrowed
    // exclusively, where the body's binding of the view (held while it
    // runs) clashes exactly when `s` is `t` -- the one way the view's path
    // and `s`'s can meet -- so that is the aliasing fault, made where the
    // check would make it; then `s` grown as the pushes would grow it and
    // the bytes copied. The view's path is never formed: unheld, it would
    // end with the statement, and its range's check is within the whole
    // bytes' check made first, with nothing formed in between.
    pub(super) fn append_substring_helper(&mut self) -> R<String> {
        let name = "cbn_append_substring".to_string();
        if self.append_substring_fn {
            return Ok(name);
        }
        self.append_substring_fn = true;
        let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let grow = self.request_fn("std::Vec::grow", vec![Type::Int(IntTy::U8)])?;
        let sig = format!("void {}(cb_ref p_s, cb_ref p_t, uint64_t p_lo, uint64_t p_hi)", name);
        let mut b = String::new();
        writeln!(b, "    cb_borrow_check(p_t.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *t = ({} *)p_t.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t tn = t->bytes.len;").unwrap();
        writeln!(b, "    if (p_lo > p_hi || p_hi > tn) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (!cb_char_boundary(t->bytes.ptr, tn, p_lo) || !cb_char_boundary(t->bytes.ptr, tn, p_hi)) cb_fault(\"diag.not-char-boundary\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t n = p_hi - p_lo;").unwrap();
        writeln!(b, "    if (n == 0) return;").unwrap();
        writeln!(b, "    if (p_s.p == p_t.p) cb_fault(\"diag.aliasing-conflict\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    while (s->bytes.cap - s->bytes.len < n) {{ uint64_t gt = cb_borrow_unstamped(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&s->bytes, gt }})); cb_call_done(gt); }}", grow).unwrap();
        writeln!(b, "    memcpy(s->bytes.ptr + s->bytes.len, t->bytes.ptr + p_lo, n);").unwrap();
        writeln!(b, "    s->bytes.len += n;").unwrap();
        writeln!(self.protos, "{};", sig).unwrap();
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, b).unwrap();
        Ok(name)
    }

    // D-0200: `replace(r, v)` natively for a value an objectless local may
    // hold (`objectless_type`): the body receives `v`, binds it as `old`,
    // checks that both places are still there (`[Swap-Places]`: `r`'s; the
    // fresh `old` is), exchanges them and returns `old` -- with the object
    // `v` came with, which is what the body's bindings hand on (`cb_take`
    // of a binding nothing borrowed cannot fail). The same here, without
    // the bindings.
    pub(super) fn native_replace(&mut self, t: &Type) -> R<Option<String>> {
        if !self.objectless_type(t) {
            return Ok(None);
        }
        let tc = self.ctype(t)?;
        let mut b = String::new();
        writeln!(b, "    uint64_t o = cb_recv(&p_v);").unwrap();
        writeln!(b, "    cb_valid(p_r.tok, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} old = *({} *)p_r.p;", tc, tc).unwrap();
        writeln!(b, "    *({} *)p_r.p = p_v;", tc).unwrap();
        writeln!(b, "    cb_move_to(o, &old);").unwrap();
        writeln!(b, "    cb_send(o);").unwrap();
        writeln!(b, "    return old;").unwrap();
        Ok(Some(b))
    }

    // `String::view(s, lo, hi)` natively: the body borrows `&s.bytes[0..$]`
    // (its check the first that can fail), faults on bounds and then on
    // character boundaries (`StringView::boundary`), borrows `[lo, hi)` of
    // that whole-bytes path and returns the view over it as reference data.
    // Here the same checks in the same order, the range borrowed from `s`'s
    // path to its bytes (the path any later access meets is the same: the
    // bytes' object, `lo .. hi`), and sent as the result's one reference
    // (`cb_send_tok_datum`), as `native_view_sub` sends its.
    pub(super) fn native_string_view(&mut self) -> R<Option<String>> {
        let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let svc = self.ctype(&Type::Named("std::StringView".to_string(), vec![]))?;
        let mut b = String::new();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t n = s->bytes.len;").unwrap();
        writeln!(b, "    if (p_lo > p_hi || p_hi > n) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (!cb_char_boundary(s->bytes.ptr, n, p_lo) || !cb_char_boundary(s->bytes.ptr, n, p_hi)) cb_fault(\"diag.not-char-boundary\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t tk = cb_borrow_range(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, p_lo, p_hi, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} r;", svc).unwrap();
        writeln!(b, "    r.bytes.src = (void *)&s->bytes;").unwrap();
        writeln!(b, "    r.bytes.data = s->bytes.ptr + p_lo;").unwrap();
        writeln!(b, "    r.bytes.len = p_hi - p_lo;").unwrap();
        writeln!(b, "    cb_send_tok_datum(tk);").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // D-0193: `String::to_ascii_lower` / `to_ascii_upper` natively: the
    // body borrows `s.bytes` for its `foreach` (the one check that can
    // fail), reads each byte through that borrow and pushes its mapping
    // onto a new `String`'s bytes. Here the same borrow's check, then the
    // bytes mapped into one exact allocation (a `String`'s capacity is not
    // observable); a byte with an object over it takes the prelude's body.
    // D-0200: a native body's `__raw` form, where it has one of its own:
    // `native_ascii_case`'s, the new `String` returned with no object and no
    // frame (which held nothing else); the prelude's body's result, when
    // that is taken, received and its record ended.
    pub(super) fn native_raw_body(&self, name: &str, body: &str) -> Option<String> {
        if !matches!(name, "std::String::to_ascii_lower" | "std::String::to_ascii_upper") {
            return None;
        }
        let at = body.find("    uint64_t oo = cb_new(&out")?;
        let fb_start = body.find("if (cb_live_in(s->bytes.ptr, s->bytes.len)) return ")?;
        let fb_end = body[fb_start..].find(';')? + fb_start;
        let call = &body[fb_start + "if (cb_live_in(s->bytes.ptr, s->bytes.len)) return ".len()..fb_end];
        let head = format!(
            "{}if (cb_live_in(s->bytes.ptr, s->bytes.len)) {{ struct s_std__String r = {}; cb_forget_obj(cb_recv(&r)); return r; }}{}",
            &body[..fb_start],
            call,
            &body[fb_end + 1..at]
        );
        // The frame held nothing but the result's object.
        Some(format!("{}    return out;\n", head.replacen("    cb_frame_push();\n", "", 1)))
    }

    pub(super) fn native_ascii_case(&mut self, upper: bool) -> R<Option<String>> {
        let st = Type::Named("std::String".to_string(), vec![]);
        let sc = self.ctype(&st)?;
        let stid = self.type_id(&st)?;
        let fb = self.std_fallback()?;
        let (lo, hi, delta) = if upper { ("'a'", "'z'", "- 32") } else { ("'A'", "'Z'", "+ 32") };
        let mut b = String::new();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    if (cb_live_in(s->bytes.ptr, s->bytes.len)) return {}(p_s);", fb).unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", sc).unwrap();
        writeln!(b, "    out.bytes.ptr = (uint8_t *)1;").unwrap();
        writeln!(b, "    out.bytes.len = 0;").unwrap();
        writeln!(b, "    out.bytes.cap = 0;").unwrap();
        writeln!(b, "    uint64_t n = s->bytes.len;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(n, 1u);").unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        for (uint64_t k = 0; k < n; k++) {{ uint8_t c = s->bytes.ptr[k]; p[k] = (c >= {} && c <= {}) ? (uint8_t)(c {}) : c; }}", lo, hi, delta).unwrap();
        writeln!(b, "        out.bytes.ptr = p;").unwrap();
        writeln!(b, "        out.bytes.len = n;").unwrap();
        writeln!(b, "        out.bytes.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", stid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", sc).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }
}
