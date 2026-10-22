// Native C bodies for std's Vec operations.

use super::*;

impl<'a> Gen<'a> {
    // `Vec::index_shared`/`Vec::index_exclusive` realized natively, as
    // `spec/21` §0 permits ("an implementation may realize them
    // differently provided every conformance case holds"; D-0023): the
    // body's checked read of `v.len`, its bounds fault, then one runtime
    // call for `[Reclaim]` + `[Borrow]` + the return (`cb_elem_borrow`).
    // Only the prelude's own declaration qualifies — a program may
    // declare a function of the same name, and that one is lowered as
    // written.
    pub(super) fn native_vec_index(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str) -> R<Option<String>> {
        // The natives of the other std types first (`native_std_body`).
        if let Some(b) = self.native_std_body(name, f, targs, cname)? {
            return Ok(Some(b));
        }
        let mode = match name {
            "std::Vec::index_shared" => "CB_SHARED",
            "std::Vec::index_exclusive" => "CB_EXCLUSIVE",
            "std::Vec::drop" | "std::Vec::push" | "std::Vec::pop" | "std::Vec::len" | "std::Vec::clear" | "std::Vec::sort" | "std::Vec::grow" | "std::Vec::reserve" | "std::Vec::remove" | "std::Vec::index_of" | "std::Vec::extend_from" | "std::Vec::from_slice" | "std::Vec::filled"
            | "std::Vec::swap" | "std::Vec::reverse" | "std::Vec::insert" | "std::Vec::truncate" | "std::Vec::clone" | "std::Vec::append" | "std::Vec::binary_search" | "std::Vec::contains" | "std::Vec::dedup" => "",
            _ => return Ok(None),
        };
        let [elem] = targs else { return Ok(None) };
        if !in_prelude(f) {
            return Ok(None);
        }
        let vec_c = self.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        if name == "std::Vec::len" {
            // The body is one checked read of `v.len`. Its frame and the
            // parameter binding holding the reference are unobservable:
            // nothing else runs while the path is held.
            let mut b = String::new();
            writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    return (({} *)p_v.p)->len;", vec_c).unwrap();
            return Ok(Some(b));
        }
        if name == "std::Vec::drop" {
            return self.native_vec_drop(elem, &vec_c);
        }
        if name == "std::Vec::clear" {
            return self.native_vec_clear(elem, &vec_c);
        }
        if name == "std::Vec::sort" {
            return self.native_vec_sort(elem, &vec_c);
        }
        if name == "std::Vec::grow" {
            return self.native_vec_grow(elem, &vec_c);
        }
        if name == "std::Vec::reserve" {
            return self.native_vec_reserve(elem, &vec_c);
        }
        if name == "std::Vec::remove" {
            return self.native_vec_remove(elem, &vec_c);
        }
        if name == "std::Vec::index_of" {
            return self.native_vec_index_of(elem, &vec_c);
        }
        if name == "std::Vec::extend_from" {
            return self.native_vec_extend_from(elem, &vec_c);
        }
        if name == "std::Vec::from_slice" {
            return self.native_vec_from_slice(elem, &vec_c);
        }
        if name == "std::Vec::filled" {
            return self.native_vec_filled(elem, &vec_c);
        }
        match name {
            "std::Vec::swap" => return self.native_vec_swap(elem, &vec_c),
            "std::Vec::reverse" => return self.native_vec_reverse(elem, &vec_c),
            "std::Vec::insert" => return self.native_vec_insert(elem, &vec_c),
            "std::Vec::truncate" => return self.native_vec_truncate(elem, &vec_c),
            "std::Vec::clone" => return self.native_vec_clone(elem, &vec_c),
            "std::Vec::append" => return self.native_vec_append(elem, &vec_c),
            "std::Vec::binary_search" => return self.native_vec_binary_search(elem, &vec_c),
            "std::Vec::contains" => return self.native_vec_contains(elem, &vec_c),
            "std::Vec::dedup" => return self.native_vec_dedup(elem, &vec_c),
            _ => {}
        }
        if name == "std::Vec::pop" && !self.plain_data(elem) {
            return self.native_vec_pop_res(elem, &vec_c);
        }
        if name == "std::Vec::clear" && !self.plain_data(elem) {
            return self.native_vec_clear_res(elem, &vec_c);
        }
        if name == "std::Vec::pop" {
            let b = self.native_vec_pop(elem, &vec_c, true)?;
            if b.is_some() {
                // For a confined vector (`Fx::bind_confined`): no checks.
                let nc = self.native_vec_pop(elem, &vec_c, false)?.expect("the same element type");
                let opt_c = self.ctype(&Type::Named("std::Option".to_string(), vec![elem.clone()]))?;
                let sig = format!("{} {}_nc(cb_ref p_v)", opt_c, cname);
                writeln!(self.protos, "{};", sig).unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, nc).unwrap();
            }
            return Ok(b);
        }
        if name == "std::Vec::push" {
            let b = self.native_vec_push(elem, &vec_c)?;
            // `Vec<String>::push` of a fresh `String` (`lower_push_fresh`):
            // the `_raw` variant, which neither receives nor moves an object.
            // D-0200: and any resource holding no reference and no `fn`
            // (`Vec<String>`: a taken objectless local, `lower_push_fresh`).
            if let (Some(body), true) = (&b, self.is_resource(elem) && !self.has_refs(elem) && !self.holds_fn(elem)) {
                let raw: String = body.lines().filter(|l| !l.contains("cb_recv(&p_x)") && !l.contains("cb_raw_move_in(o,")).map(|l| format!("{}\n", l)).collect();
                let elem_c = self.reg_ctype(elem)?;
                let sig = format!("void {}_raw(cb_ref p_v, {} p_x)", cname, elem_c);
                writeln!(self.protos, "{};", sig).unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, raw).unwrap();
            }
            if b.is_some() && self.plain_data(elem) {
                // For a confined vector (`Fx::bind_confined`): no checks.
                let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
                let elem_c = self.reg_ctype(elem)?;
                let sig = format!("void {}_nc(cb_ref p_v, {} p_x)", cname, elem_c);
                writeln!(self.protos, "{};", sig).unwrap();
                let mut nc = String::new();
                writeln!(nc, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
                writeln!(nc, "    if (v->len == v->cap) {{ {}(p_v); }}", grow).unwrap();
                writeln!(nc, "    v->ptr[v->len] = p_x;").unwrap();
                // `v.len + 1` cannot overflow: here `len < cap`, and a
                // capacity is bounded by the storage `allocate` gave.
                writeln!(nc, "    uint64_t n = v->len + 1;").unwrap();
                writeln!(nc, "    v->len = n;").unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, nc).unwrap();
            }
            return Ok(b);
        }
        let elem_c = self.ctype(elem)?;
        let ty = self.type_id(elem)?;
        // The `…_pre` variant, for a call whose argument is `&place` or
        // `&mut place` (`Fx::lower_vec_index_pre`): the call site has made
        // the borrow's check and the read's (`cb_borrow_check`).
        let mut pre = String::new();
        writeln!(pre, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(pre, "    if (p_i >= v->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(pre, "    {} *e = v->ptr + (int64_t)p_i;", elem_c).unwrap();
        writeln!(pre, "    return (cb_ref){{ (void *)e, cb_elem_borrow_here(e, {}u, {}) }};", ty, mode).unwrap();
        let pre_sig = format!("cb_ref {}_pre(cb_ref p_v, uint64_t p_i)", cname);
        writeln!(self.protos, "{};", pre_sig).unwrap();
        write!(self.fns, "{}\n{{\n{}}}\n\n", pre_sig, pre).unwrap();
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (p_i >= v->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        if mode == "CB_EXCLUSIVE" {
            // D-0222: the exclusive access to element i of `*v` that
            // `[Index-Vec]` checks, through `v` itself: a live shared path
            // to the vector (or a slice holding i) meets it; a slice of
            // other elements does not.
            writeln!(b, "    cb_borrow_range_unminted(p_v.tok, NULL, 0, p_i, p_i + 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        }
        writeln!(b, "    {} *e = v->ptr + (int64_t)p_i;", elem_c).unwrap();
        writeln!(b, "    return (cb_ref){{ (void *)e, cb_elem_borrow(e, {}u, {}) }};", ty, mode).unwrap();
        Ok(Some(b))
    }

    // `Vec<String>::drop`: the body's loop `drop(reclaim<String>(…))` over
    // the elements, then the buffer's `deallocate`. An element with a live
    // object over its cells (`cb_live_at`) takes the body's own steps:
    // `[Reclaim]`, the move out, and its destruction. For any other,
    // `[Reclaim]` would establish a fresh object no path reaches, which
    // the destruction ends at once; what is observable of that is the
    // destruction itself, the `String`'s `Vec<u8>` field dropped as its
    // native body does (`cb_vec_drop_plain` over the bytes, then their
    // `deallocate`). The reads of `self` go through one path that nothing
    // in the loop writes, so the first of each field stands for all.
    pub(super) fn native_vec_drop_strings(&mut self, elem: &Type, vec_c: &str, opt: bool) -> R<String> {
        let sc = self.ctype(elem)?;
        let ty = self.type_id(elem)?;
        let (size, align) = self.size_align(elem)?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_self.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_self.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (v->len > 0) cb_read(p_self.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    for (uint64_t i = 0; i < v->len; i++) {{").unwrap();
        writeln!(b, "        {} *e = v->ptr + i;", sc).unwrap();
        writeln!(b, "        if (cb_live_at(e)) {{").unwrap();
        writeln!(b, "            uint64_t root = cb_reclaim(e, {}u);", ty).unwrap();
        writeln!(b, "            uint64_t o = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "            {} t = *e;", sc).unwrap();
        writeln!(b, "            cb_move_to(o, &t);").unwrap();
        writeln!(b, "            cb_consume(o, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        }} else {{").unwrap();
        // `Option<String>`: only `Some` (variant 0) holds bytes.
        let (guard, sb) = if opt { ("if (e->tag == 0u) ", "e->u.v0.bytes") } else { ("", "e->bytes") };
        writeln!(b, "            {}{{ if ({sb}.len > 0) cb_vec_drop_plain({sb}.ptr, {sb}.len, 1u);", guard, sb = sb).unwrap();
        // A `String`'s bytes, and a buffer of `String`s, never hold a
        // reference (`cb_deallocate_plain`).
        writeln!(b, "            if ({sb}.cap > 0) cb_deallocate_plain((uint8_t *){sb}.ptr, {sb}.cap); }}", sb = sb).unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_read(p_self.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (v->cap > 0) {{").unwrap();
        writeln!(b, "        cb_read(p_self.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        uint64_t n = ({{ uint64_t _r; if (__builtin_mul_overflow(v->cap, (uint64_t){}u, &_r)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0); _r; }});", size).unwrap();
        let _ = align;
        writeln!(b, "        cb_deallocate_plain((uint8_t *)v->ptr, n);").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(b)
    }

    // `Vec::drop` for a plain element type (`cb_vec_drop_plain`): the
    // body's checked read of `self.len`, the element loop, then the
    // buffer's `deallocate`. Its other reads of `self` are through the
    // same path with no write in between and cannot fail where the first
    // passed. A resource, reference-holding, fn-valued or zero-sized
    // element type keeps the prelude body.
    pub(super) fn native_vec_drop(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let is_string = |t: &Type| matches!(t, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if is_string(elem) {
            return self.native_vec_drop_strings(elem, vec_c, false).map(Some);
        }
        // `Vec<Option<String>>` (a database row's cells): the same, an
        // element's bytes dropped only when it is `Some`.
        if matches!(elem, Type::Named(n, a) if n == "std::Option" && a.len() == 1 && is_string(&a[0])) {
            return self.native_vec_drop_strings(elem, vec_c, true).map(Some);
        }
        let (size, align) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let mut b = String::new();
        writeln!(b, "    cb_read(p_self.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *s = ({} *)p_self.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (s->len > 0) cb_vec_drop_plain(s->ptr, s->len, {}u);", size).unwrap();
        writeln!(b, "    if (s->cap > 0) {{").unwrap();
        writeln!(b, "        uint64_t n = ({{ uint64_t _r; if (__builtin_mul_overflow(s->cap, (uint64_t){}u, &_r)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0); _r; }});", size).unwrap();
        // Plain elements: no reference in the cells (`cb_deallocate_plain`).
        let _ = align;
        writeln!(b, "        cb_deallocate_plain((uint8_t *)s->ptr, n);").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // `Vec::clear` for a plain element type (its loop of `take_raw`s, each
    // a `[Reclaim]`, a read and a `[Release]` of one element, with nothing
    // to destroy): the elements that already have an object are checked
    // as `Vec::drop`'s are (`cb_vec_drop_plain`), the whole range released
    // at once, and `len` written through the same path. The same checks,
    // in the same order, without an object per element.
    pub(super) fn native_vec_clear(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size > 0 && !self.plain_data(elem) {
            return self.native_vec_clear_res(elem, vec_c);
        }
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (v->len > 0) {{").unwrap();
        writeln!(b, "        cb_vec_drop_plain(v->ptr, v->len, {}u);", size).unwrap();
        writeln!(b, "        cb_release_plain((uint8_t *)v->ptr, v->len * {}u);", size).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->len = 0;").unwrap();
        Ok(Some(b))
    }

    // `Vec::sort` (D-0062) for an integer or `bool` element type: the
    // prelude's form reads `v`'s fields, moves every element out
    // (`take_raw`: a `[Release]` of its cells) into a new buffer in order,
    // and writes `v` whole. Natively: the same read and write checks on
    // `v`, the elements' objects ended as their release would, and the
    // values sorted in place, stably -- the order and the values the
    // prelude's form leaves, without an object per comparison.
    pub(super) fn native_vec_sort(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let kind: u32 = match elem {
            Type::Bool => 0,
            Type::Int(t) => {
                let bytes = t.bitwidth(64) / 8;
                bytes + if t.signed() { 256 } else { 0 }
            }
            _ => return self.native_vec_sort_keys(elem, vec_c),
        };
        let (size, _) = self.size_align(elem)?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (v->len < 2) return;").unwrap();
        writeln!(b, "    cb_vec_drop_plain(v->ptr, v->len, {}u);", size).unwrap();
        writeln!(b, "    cb_release((uint8_t *)v->ptr, v->len * {}u);", size).unwrap();
        writeln!(b, "    cb_write(p_v.tok, NULL, 0, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_sort_plain(v->ptr, v->len, {}u);", kind).unwrap();
        Ok(Some(b))
    }

    // `Vec::sort` for the other key types (`String`, `str`, and struct or
    // enum keys, D-0110) holding no reference: the prelude's merge sort
    // over positions (`sorted_order_keys`), natively, comparing with what
    // `key_less_at` compares; its element reads, through the vector the
    // call holds exclusively, cannot fail. Then `apply_order`'s moves as
    // native `sort` makes them: the elements that have objects read in
    // order, their cells released, the vector written.
    pub(super) fn native_vec_sort_keys(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || self.has_refs(elem) || self.holds_fn(elem) {
            return Ok(None);
        }
        // Text: the elements themselves, merged stably (the same order
        // as the merge over positions below: a stable sort's result is
        // determined), each comparison an inline `memcmp`.
        let text = match elem {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => Some(("bytes.ptr", "bytes.len")),
            Type::Str => Some(("p", "n")),
            _ => None,
        };
        if let Some((pf, nf)) = text {
            let elem_c = self.reg_ctype(elem)?;
            let mut b = String::new();
            writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
            writeln!(b, "    uint64_t n = v->len;").unwrap();
            writeln!(b, "    if (n < 2) return;").unwrap();
            writeln!(b, "    {} *x = ({} *)malloc(n * sizeof({}));", elem_c, elem_c, elem_c).unwrap();
            writeln!(b, "    {} *y = ({} *)malloc(n * sizeof({}));", elem_c, elem_c, elem_c).unwrap();
            writeln!(b, "    if (!x || !y) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
            writeln!(b, "    memcpy(x, v->ptr, n * sizeof({}));", elem_c).unwrap();
            writeln!(b, "    for (uint64_t width = 1; width < n; width = width > n / 2 ? n : width * 2) {{").unwrap();
            writeln!(b, "        for (uint64_t lo = 0; lo < n; ) {{").unwrap();
            writeln!(b, "            uint64_t mid = n - lo > width ? lo + width : n;").unwrap();
            writeln!(b, "            uint64_t hi = n - mid > width ? mid + width : n;").unwrap();
            writeln!(b, "            uint64_t l = lo, r = mid, k = lo;").unwrap();
            writeln!(b, "            while (l < mid && r < hi) {{").unwrap();
            writeln!(b, "                if (cb_bytes_lt((const void *)x[r].{p}, x[r].{n}, (const void *)x[l].{p}, x[l].{n})) y[k++] = x[r++]; else y[k++] = x[l++];", p = pf, n = nf).unwrap();
            writeln!(b, "            }}").unwrap();
            writeln!(b, "            while (l < mid) y[k++] = x[l++];").unwrap();
            writeln!(b, "            while (r < hi) y[k++] = x[r++];").unwrap();
            writeln!(b, "            lo = hi;").unwrap();
            writeln!(b, "        }}").unwrap();
            writeln!(b, "        {} *t = x; x = y; y = t;", elem_c).unwrap();
            writeln!(b, "    }}").unwrap();
            writeln!(b, "    cb_vec_drop_plain(v->ptr, n, {}u);", size).unwrap();
            writeln!(b, "    cb_release((uint8_t *)v->ptr, n * {}u);", size).unwrap();
            writeln!(b, "    memcpy(v->ptr, x, n * sizeof({}));", elem_c).unwrap();
            writeln!(b, "    free(x);").unwrap();
            writeln!(b, "    free(y);").unwrap();
            writeln!(b, "    cb_write(p_v.tok, NULL, 0, 0u, CB_NOLOC, 0);").unwrap();
            return Ok(Some(b));
        }
        let cmp = match elem {
            Type::Named(n, _) if n == "std::String" => "cb_bytes_less((const void *)v->ptr[@1].bytes.ptr, v->ptr[@1].bytes.len, (const void *)v->ptr[@2].bytes.ptr, v->ptr[@2].bytes.len)".to_string(),
            Type::Str => "cb_bytes_less((const void *)v->ptr[@1].p, v->ptr[@1].n, (const void *)v->ptr[@2].p, v->ptr[@2].n)".to_string(),
            Type::Named(..) if composite_key(&Type::Ref(Box::new(elem.clone()), Mode::Shared)).is_some() => {
                let f = self.key_cmp_fn(elem)?;
                format!("((uint8_t)({}(&v->ptr[@1], &v->ptr[@2]) < 0))", f)
            }
            _ => return Ok(None),
        };
        let elem_c = self.reg_ctype(elem)?;
        let lt = |x: &str, y: &str| cmp.replace("@1", x).replace("@2", y);
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    if (n < 2) return;").unwrap();
        writeln!(b, "    uint64_t *a = (uint64_t *)malloc(n * 2 * sizeof(uint64_t));").unwrap();
        writeln!(b, "    {} *out = ({} *)malloc(n * sizeof({}));", elem_c, elem_c, elem_c).unwrap();
        writeln!(b, "    if (!a || !out) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t *bb = a + n;").unwrap();
        writeln!(b, "    for (uint64_t i = 0; i < n; i++) {{ a[i] = i; bb[i] = 0; }}").unwrap();
        writeln!(b, "    uint64_t width = 1;").unwrap();
        writeln!(b, "    while (width < n) {{").unwrap();
        writeln!(b, "        uint64_t lo = 0;").unwrap();
        writeln!(b, "        while (lo < n) {{").unwrap();
        writeln!(b, "            uint64_t mid = n - lo > width ? lo + width : n;").unwrap();
        writeln!(b, "            uint64_t hi = n - mid > width ? mid + width : n;").unwrap();
        writeln!(b, "            uint64_t l = lo, r = mid, k = lo;").unwrap();
        writeln!(b, "            while (k < hi) {{").unwrap();
        writeln!(b, "                int take_left = l < mid && (r >= hi || !{});", lt("a[r]", "a[l]")).unwrap();
        writeln!(b, "                if (take_left) bb[k] = a[l++]; else bb[k] = a[r++];").unwrap();
        writeln!(b, "                k++;").unwrap();
        writeln!(b, "            }}").unwrap();
        writeln!(b, "            lo = hi;").unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "        uint64_t *t = a; a = bb; bb = t;").unwrap();
        writeln!(b, "        width = width > n ? n : width * 2;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    for (uint64_t i = 0; i < n; i++) out[i] = v->ptr[a[i]];").unwrap();
        writeln!(b, "    cb_vec_drop_plain(v->ptr, n, {}u);", size).unwrap();
        writeln!(b, "    cb_release((uint8_t *)v->ptr, n * {}u);", size).unwrap();
        writeln!(b, "    memcpy(v->ptr, out, n * sizeof({}));", elem_c).unwrap();
        writeln!(b, "    free(a < bb ? a : bb);").unwrap();
        writeln!(b, "    free(out);").unwrap();
        writeln!(b, "    cb_write(p_v.tok, NULL, 0, 0u, CB_NOLOC, 0);").unwrap();
        Ok(Some(b))
    }

    // D-0193: `Drain::take` (each step of `foreach` over a `Vec` it
    // consumes) natively, for plain elements and `StringView`s: the body
    // borrows `d.v` exclusively and reads `d.next` (both checked), then
    // `take_raw` copies the element out (a view's path with it,
    // `cb_copy_datum`) and releases its cells, then `d.next` is written
    // (checked, its `+ 1` with the overflow fault) and the element is
    // returned (a view sent as reference data). Here the same steps, without
    // the bodies' frames and bindings, which nothing else reaches. An
    // element with an object over it takes the prelude's body.
    pub(super) fn native_drain_take(&mut self, t: &Type) -> R<Option<String>> {
        let view = matches!(t, Type::Named(n, a) if n == "std::StringView" && a.is_empty());
        if !(self.plain_data(t) || view) {
            return Ok(None);
        }
        let (size, _) = self.size_align(t)?;
        if size == 0 {
            return Ok(None);
        }
        let dc = self.ctype(&Type::Named("std::Drain".to_string(), vec![t.clone()]))?;
        let tc = self.ctype(t)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_borrow_check(p_d.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_read(p_d.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *d = ({} *)p_d.p;", dc, dc).unwrap();
        writeln!(b, "    uint64_t i = d->next;").unwrap();
        writeln!(b, "    {} *p = d->v.ptr + i;", tc).unwrap();
        writeln!(b, "    if (cb_live_in(p, {}u)) return {}(p_d);", size, fb).unwrap();
        writeln!(b, "    {} x = *p;", tc).unwrap();
        if view {
            let tid = self.type_id(t)?;
            writeln!(b, "    cb_copy_datum(&x, p, {}u);", tid).unwrap();
            writeln!(b, "    cb_release((uint8_t *)p, {}u);", size).unwrap();
        } else {
            writeln!(b, "    cb_release_plain((uint8_t *)p, {}u);", size).unwrap();
        }
        writeln!(b, "    uint64_t ni;").unwrap();
        writeln!(b, "    if (__builtin_add_overflow(i, (uint64_t)1, &ni)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_write(p_d.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    d->next = ni;").unwrap();
        if view {
            let tid = self.type_id(t)?;
            writeln!(b, "    cb_send_datum(&x, {}u);", tid).unwrap();
        }
        writeln!(b, "    return x;").unwrap();
        Ok(Some(b))
    }

    // D-0200: `Vec::sorted_order(v, less)` natively, for elements holding no
    // reference and no `fn`, when `less` keeps nothing it is given
    // (`cb_fn_pure`) and no object lies over an element (`cb_live_in`):
    // the body's merge over positions, the same comparisons in the same
    // order (`less` may print or fault), each called with the elements'
    // bare addresses. The body's index calls would each borrow `v` (whose
    // path the call holds shared) and an element, establishing an
    // ephemeral object whose one path no check counts (D-0018, D-0023);
    // the comparator, keeping nothing and reaching nothing else, cannot
    // write either meanwhile. Its result, a `Vec<usize>` of the positions,
    // given an object and sent as the body's is. Anything else takes the
    // prelude's body.
    pub(super) fn native_sorted_order(&mut self, t: &Type) -> R<Option<String>> {
        let (size, _) = self.size_align(t)?;
        if size == 0 || self.has_refs(t) || self.holds_fn(t) {
            return Ok(None);
        }
        let vec_c = self.ctype(&Type::Named("std::Vec".to_string(), vec![t.clone()]))?;
        let ut = Type::Named("std::Vec".to_string(), vec![Type::Int(IntTy::Usize)]);
        let uc = self.ctype(&ut)?;
        let utid = self.type_id(&ut)?;
        let ft = Type::Fn(vec![Type::Ref(Box::new(t.clone()), Mode::Shared), Type::Ref(Box::new(t.clone()), Mode::Shared)], Box::new(Type::Bool));
        let thunk = self.thunk(&ft)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (!cb_fn_pure(p_less) || cb_live_in(v->ptr, v->len * {}u)) return {}(p_v, p_less);", size, fb).unwrap();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    {} res;", uc).unwrap();
        writeln!(b, "    res.ptr = (uint64_t *)8; res.len = 0; res.cap = 0;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint64_t nb; if (__builtin_mul_overflow(n, (uint64_t)8, &nb)) cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        uint64_t *a = (uint64_t *)cb_allocate(nb, 8u);").unwrap();
        writeln!(b, "        uint64_t *bb = (uint64_t *)cb_allocate(nb, 8u);").unwrap();
        writeln!(b, "        if (!a || !bb) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        for (uint64_t i = 0; i < n; i++) {{ a[i] = i; bb[i] = 0; }}").unwrap();
        writeln!(b, "        uint64_t width = 1;").unwrap();
        writeln!(b, "        while (width < n) {{").unwrap();
        writeln!(b, "            uint64_t lo = 0;").unwrap();
        writeln!(b, "            while (lo < n) {{").unwrap();
        writeln!(b, "                uint64_t mid = n - lo > width ? lo + width : n;").unwrap();
        writeln!(b, "                uint64_t hi = n - mid > width ? mid + width : n;").unwrap();
        writeln!(b, "                uint64_t l = lo, r = mid, k = lo;").unwrap();
        writeln!(b, "                while (k < hi) {{").unwrap();
        writeln!(b, "                    int take_left = l < mid && (r >= hi || !{}(p_less, ((cb_ref){{ (void *)&v->ptr[a[r]], 0 }}), ((cb_ref){{ (void *)&v->ptr[a[l]], 0 }})));", thunk).unwrap();
        writeln!(b, "                    if (take_left) bb[k] = a[l++]; else bb[k] = a[r++];").unwrap();
        writeln!(b, "                    k++;").unwrap();
        writeln!(b, "                }}").unwrap();
        writeln!(b, "                lo = hi;").unwrap();
        writeln!(b, "            }}").unwrap();
        writeln!(b, "            uint64_t *tt = a; a = bb; bb = tt;").unwrap();
        writeln!(b, "            width = width > n ? n : width * 2;").unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "        cb_deallocate(bb, nb, 8u);").unwrap();
        writeln!(b, "        res.ptr = a; res.len = n; res.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        // `less` was given by value: its box (and the paths its captures
        // hold) ends with the call, as the body's binding of it does.
        writeln!(b, "    cb_drop_fn(p_less);").unwrap();
        writeln!(b, "    uint64_t o = cb_new(&res, {}u, CB_NOLOC, 0);", utid).unwrap();
        writeln!(b, "    cb_send(o);").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // `Vec::grow`: the body's first checked read (of `v.cap`), its
    // arithmetic and faults, its two checked writes' checks, `allocate`,
    // `copy_raw` of the elements, `deallocate` of the old cells, then the
    // two stores (the body's order, CHG-0332: the writes before the old
    // cells go). Its other reads go through the same path with nothing in
    // between that touches `v`, and cannot fail where the first passed.
    pub(super) fn native_vec_grow(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, align) = self.size_align(elem)?;
        if size == 0 {
            return Ok(None);
        }
        let ovf = "cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0)";
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t nc = 4;").unwrap();
        writeln!(b, "    if (v->cap != 0 && __builtin_mul_overflow(v->cap, (uint64_t)2, &nc)) {};", ovf).unwrap();
        writeln!(b, "    uint64_t bytes;").unwrap();
        writeln!(b, "    if (__builtin_mul_overflow(nc, (uint64_t){}u, &bytes)) {};", size, ovf).unwrap();
        // The two checked writes' checks (`v.ptr`, `v.cap`) before the old
        // cells go: a check that faults (the exclusive path's first write
        // meeting a shared path of the same `Vec`, D-0022) must leave the
        // `Vec` as it was, or the unwind's `Vec::drop` frees the old cells
        // a second time (the fuzzer's finding, CHG-0332). Nothing between
        // the checks and the stores touches `v`.
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        if self.plain_data(elem) {
            // Plain elements: allocate, copy and deallocate as one
            // `cb_reallocate_plain` (no cell holds a reference to move).
            writeln!(b, "    uint8_t *p = cb_reallocate_plain((uint8_t *)v->ptr, v->cap * {}u, bytes, {}u);", size, align).unwrap();
            writeln!(b, "    if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        } else {
            writeln!(b, "    uint8_t *p = cb_allocate(bytes, {}u);", align).unwrap();
            writeln!(b, "    if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
            writeln!(b, "    uint64_t lb;").unwrap();
            writeln!(b, "    if (__builtin_mul_overflow(v->len, (uint64_t){}u, &lb)) {};", size, ovf).unwrap();
            writeln!(b, "    cb_copy_raw(p, (uint8_t *)v->ptr, lb);").unwrap();
            writeln!(b, "    if (v->cap > 0) {{").unwrap();
            writeln!(b, "        uint64_t ob;").unwrap();
            writeln!(b, "        if (__builtin_mul_overflow(v->cap, (uint64_t){}u, &ob)) {};", size, ovf).unwrap();
            writeln!(b, "        cb_deallocate((uint8_t *)v->ptr, ob, {}u);", align).unwrap();
            writeln!(b, "    }}").unwrap();
        }
        writeln!(b, "    v->ptr = ({} *)p;", self.ctype(elem)?).unwrap();
        writeln!(b, "    v->cap = nc;").unwrap();
        Ok(Some(b))
    }

    // `Vec::reserve` (D-0129): the body's first checked read (of `v.len`),
    // its checked `len + n`, then nothing when the room is there, or the
    // cells moved as `native_vec_grow` moves them, to the body's capacity,
    // with its two checked writes. Its other reads go through the same path
    // with nothing between that touches `v`.
    pub(super) fn native_vec_reserve(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, align) = self.size_align(elem)?;
        if size == 0 {
            return Ok(None);
        }
        let ovf = "cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0)";
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t need;").unwrap();
        writeln!(b, "    if (__builtin_add_overflow(v->len, p_n, &need)) {};", ovf).unwrap();
        writeln!(b, "    if (need <= v->cap) return;").unwrap();
        writeln!(b, "    uint64_t nc = need;").unwrap();
        writeln!(b, "    if (v->cap > need - v->cap && __builtin_mul_overflow(v->cap, (uint64_t)2, &nc)) {};", ovf).unwrap();
        writeln!(b, "    uint64_t bytes;").unwrap();
        writeln!(b, "    if (__builtin_mul_overflow(nc, (uint64_t){}u, &bytes)) {};", size, ovf).unwrap();
        // The two checked writes' checks (`v.ptr`, `v.cap`) before the old
        // cells go: a check that faults (the exclusive path's first write
        // meeting a shared path of the same `Vec`, D-0022) must leave the
        // `Vec` as it was, or the unwind's `Vec::drop` frees the old cells
        // a second time (the fuzzer's finding, CHG-0332). Nothing between
        // the checks and the stores touches `v`.
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        if self.plain_data(elem) {
            // Plain elements: allocate, copy and deallocate as one
            // `cb_reallocate_plain` (no cell holds a reference to move).
            writeln!(b, "    uint8_t *p = cb_reallocate_plain((uint8_t *)v->ptr, v->cap * {}u, bytes, {}u);", size, align).unwrap();
            writeln!(b, "    if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        } else {
            writeln!(b, "    uint8_t *p = cb_allocate(bytes, {}u);", align).unwrap();
            writeln!(b, "    if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
            writeln!(b, "    uint64_t lb;").unwrap();
            writeln!(b, "    if (__builtin_mul_overflow(v->len, (uint64_t){}u, &lb)) {};", size, ovf).unwrap();
            writeln!(b, "    cb_copy_raw(p, (uint8_t *)v->ptr, lb);").unwrap();
            writeln!(b, "    if (v->cap > 0) {{").unwrap();
            writeln!(b, "        uint64_t ob;").unwrap();
            writeln!(b, "        if (__builtin_mul_overflow(v->cap, (uint64_t){}u, &ob)) {};", size, ovf).unwrap();
            writeln!(b, "        cb_deallocate((uint8_t *)v->ptr, ob, {}u);", align).unwrap();
            writeln!(b, "    }}").unwrap();
        }
        writeln!(b, "    v->ptr = ({} *)p;", self.ctype(elem)?).unwrap();
        writeln!(b, "    v->cap = nc;").unwrap();
        Ok(Some(b))
    }

    // `Vec::extend_from(a, s)` for a plain element type: the body binds
    // `a` and `s` (holding both paths for the call), then pushes
    // `clone(&s[i])` one at a time, each element read through the slice's
    // checked path and each push through `a` (its checked read of `len`,
    // `grow` when full, its checked write of `len`). Natively: the same
    // bindings, the first iteration's checks in its order -- `s[0]`'s
    // read, `a.len`'s read, the growth, `a.len`'s write -- then every
    // later element's read at once (`cb_borrow_range_unminted` over
    // `1 .. n`): nothing else runs in between that could change what they
    // find. `a` grows by the pushes' own doublings (`Vec::grow` until the
    // elements fit), then the elements are copied.
    pub(super) fn native_vec_extend_from(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
        let vt = Type::Named("std::Vec".to_string(), vec![elem.clone()]);
        let rtid = self.type_id(&Type::Ref(Box::new(vt), Mode::Exclusive))?;
        let stid = self.type_id(&Type::Slice(Box::new(elem.clone()), Mode::Shared))?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        // A source element with a live object: the body's element reads.
        writeln!(b, "    if (cb_live_in(p_s.data, p_s.len * {}u)) {{ {}(p_a, p_s); return; }}", size, fb).unwrap();
        // The body's bindings of `a` and `s`, which hold their paths for
        // the call: a slice of the very vector being extended clashes with
        // `a` at its first element's read, as in the body.
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    void *l_a = p_a.p;").unwrap();
        writeln!(b, "    uint64_t oa = cb_new(&l_a, {}u, CB_NOLOC, 0);", rtid).unwrap();
        writeln!(b, "    cb_store_ref(&l_a, p_a.tok);").unwrap();
        writeln!(b, "    (void)cb_bind(oa);").unwrap();
        writeln!(b, "    cb_recv_datum(&p_s, {}u);", stid).unwrap();
        writeln!(b, "    uint64_t os = cb_new(&p_s, {}u, CB_NOLOC, 0);", stid).unwrap();
        writeln!(b, "    (void)cb_bind(os);").unwrap();
        writeln!(b, "    uint64_t n = p_s.len;").unwrap();
        writeln!(b, "    if (n == 0) {{ cb_frame_pop(); return; }}").unwrap();
        writeln!(b, "    uint64_t st = cb_load_ref(&p_s.src);").unwrap();
        writeln!(b, "    cb_read(st, (cb_proj[]){{ {{ CB_INDEX, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    uint64_t at = cb_load_ref(&l_a);").unwrap();
        writeln!(b, "    cb_read(at, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_a.p;", vec_c, vec_c).unwrap();
        // `len + n` cannot overflow: `n` elements are in memory already.
        writeln!(b, "    while (v->cap - v->len < n) {{ {}(((cb_ref){{ p_a.p, at }})); }}", grow).unwrap();
        writeln!(b, "    cb_write(at, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (n > 1) cb_borrow_range_unminted(st, NULL, 0, 1u, n, CB_SHARED, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    memcpy(v->ptr + v->len, p_s.data, n * sizeof(*v->ptr));").unwrap();
        writeln!(b, "    v->len += n;").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        Ok(Some(b))
    }

    // The prelude's own body of the function a native body is being made
    // for, emitted as `NAME__std` beside it: the native body calls it
    // where its shortcut does not hold (an element with a live object,
    // `cb_live_in`), so that there every check and fault is the body's.
    pub(super) fn std_fallback(&mut self) -> R<String> {
        let (name, f, targs, ret_c, sig) = self.native_ctx.clone().expect("a native body is being made");
        let open = sig.find('(').expect("a signature");
        let cname = sig[..open].rsplit(' ').next().expect("a function name").to_string();
        let fb = format!("{}__std", cname);
        let fsig = format!("{} {}{}", ret_c, fb, &sig[open..]);
        writeln!(self.protos, "{};", fsig).unwrap();
        self.uid = 0;
        self.emit_fn_body(&name, &f, &targs, 0, &fsig, None)?;
        Ok(fb)
    }

    // `Vec::swap` for a plain element type: the body's checked read of
    // `v.len`, its bounds fault, and, for two positions, the exclusive
    // borrow of `v` its element borrows are formed from (`cb_borrow_check`;
    // the second borrow, from the same path with nothing in between,
    // cannot fail where the first passed), then the values exchanged in
    // place. The element paths themselves are not formed: nothing holds
    // them, and through the vector the call holds exclusively no access
    // to an element can fail.
    pub(super) fn native_vec_swap(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        let ec = self.ctype(elem)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (p_i >= v->len || p_j >= v->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (p_i != p_j) {{").unwrap();
        writeln!(b, "        if (cb_live_in2(&v->ptr[p_i], &v->ptr[p_j], {}u)) {{ {}(p_v, p_i, p_j); return; }}", size, fb).unwrap();
        writeln!(b, "        cb_borrow_check(p_v.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} t = v->ptr[p_i];", ec).unwrap();
        writeln!(b, "        v->ptr[p_i] = v->ptr[p_j];").unwrap();
        writeln!(b, "        v->ptr[p_j] = t;").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // `Vec::reverse` for a plain element type: the body's checked read of
    // `v.len`, then its `n / 2` swaps, each of two distinct positions in
    // bounds, so each makes the exclusive borrow check of `swap`
    // (`native_vec_swap`) and nothing else that can fail: the check once,
    // the elements reversed in place.
    pub(super) fn native_vec_reverse(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        let ec = self.ctype(elem)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    if (n >= 2 && cb_live_in(v->ptr, n * {}u)) {{ {}(p_v); return; }}", size, fb).unwrap();
        writeln!(b, "    if (n >= 2) {{").unwrap();
        writeln!(b, "        cb_borrow_check(p_v.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        for (uint64_t i = 0, j = n - 1; i < j; i++, j--) {{ {} t = v->ptr[i]; v->ptr[i] = v->ptr[j]; v->ptr[j] = t; }}", ec).unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // `Vec::insert` for a plain element type: the body's checked read of
    // `v.len` and its bounds fault, then `Vec::push(v, x)` (native: `grow`
    // when full, the checked write of `v.len`), then the swaps that carry
    // `x` down to `i`, each making `swap`'s exclusive borrow check: once,
    // and the elements moved up by one in a single `memmove`.
    pub(super) fn native_vec_insert(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        // A resource `x` is received first, as the body's binding of it is,
        // and moved into its cell (`cb_raw_move_in`) once placed; where the
        // body runs instead, it is sent back for the body to receive.
        let res = !self.plain_data(elem);
        let ec = self.ctype(elem)?;
        let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        if res {
            writeln!(b, "    uint64_t ox = cb_recv(&p_x);").unwrap();
        }
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (p_i > v->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        let resend = if res { "cb_send(ox); " } else { "" };
        writeln!(b, "    if (p_i < v->len && cb_live_in(&v->ptr[p_i], (v->len - p_i) * {}u)) {{ {}{}(p_v, p_i, p_x); return; }}", size, resend, fb).unwrap();
        writeln!(b, "    if (v->len == v->cap) {{ {}(p_v); }}", grow).unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    if (n > p_i) {{").unwrap();
        writeln!(b, "        cb_borrow_check(p_v.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        memmove(v->ptr + p_i + 1, v->ptr + p_i, (size_t)(n - p_i) * sizeof({}));", ec).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    v->ptr[p_i] = p_x;").unwrap();
        if res {
            writeln!(b, "    cb_raw_move_in(ox, &v->ptr[p_i]);").unwrap();
        }
        writeln!(b, "    v->len = n + 1;").unwrap();
        Ok(Some(b))
    }

    // `Vec::truncate` for a plain element type: the body pops from the end
    // while `v.len > n`, each pop a checked read and write of `v.len` and a
    // `[Release]` of the element's cells, the value dropped (plain: no
    // effect). Natively the first read and write, and one release of all
    // the cells: their objects end, as the pops' releases end them.
    pub(super) fn native_vec_truncate(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        // A resource's dropped elements are destroyed as the body's pops
        // drop them, last first (`cb_drop_local`: no object over them); the
        // prelude's body where one has an object.
        let res = !self.plain_data(elem);
        let tid = self.type_id(elem)?;
        let fb = if res { Some(self.std_fallback()?) } else { None };
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        if let Some(fb) = &fb {
            writeln!(b, "    if (v->len > p_n && cb_live_in(v->ptr + p_n, (v->len - p_n) * {}u)) {{ {}(p_v, p_n); return; }}", size, fb).unwrap();
        }
        writeln!(b, "    if (v->len > p_n) {{").unwrap();
        writeln!(b, "        cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        uint64_t old = v->len;").unwrap();
        if res {
            writeln!(b, "        for (uint64_t k = old; k > p_n; k--) {{ v->len = k - 1; cb_drop_local((uint8_t *)&v->ptr[k - 1], {}u); }}", tid).unwrap();
        }
        writeln!(b, "        cb_release_plain((uint8_t *)(v->ptr + p_n), (old - p_n) * {}u);", size).unwrap();
        writeln!(b, "        v->len = p_n;").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // D-0192: `Vec::pop` for a movable resource element type
    // (`movable_elem`): the body's checked read and write of `v.len`, the
    // last element moved out (`[Rawptr-Move-Out]`: no object lay over it)
    // and its cells released, and the `Option` returned as the body returns
    // a resource value, `None` included: an object, bound, taken and sent.
    // The prelude's body where the last element has an object.
    pub(super) fn native_vec_pop_res(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        let opt = Type::Named("std::Option".to_string(), vec![elem.clone()]);
        let opt_c = self.ctype(&opt)?;
        let otid = self.type_id(&opt)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    {} r;", opt_c).unwrap();
        writeln!(b, "    if (v->len == 0) {{").unwrap();
        writeln!(b, "        r.tag = 1u;").unwrap();
        writeln!(b, "    }} else {{").unwrap();
        writeln!(b, "        uint64_t n = v->len - 1;").unwrap();
        writeln!(b, "        if (cb_live_in(&v->ptr[n], {}u)) return {}(p_v);", size, fb).unwrap();
        writeln!(b, "        cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        v->len = n;").unwrap();
        writeln!(b, "        r.tag = 0u;").unwrap();
        writeln!(b, "        r.u.v0 = v->ptr[n];").unwrap();
        writeln!(b, "        cb_release_plain((uint8_t *)(v->ptr + n), {}u);", size).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&r, {}u, CB_NOLOC, 0);", otid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = r;", opt_c).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // D-0192: `Vec::clear` for a movable resource element type: the body
    // destroys each element in order, first to last (`take_raw`, then
    // `drop`: here `cb_drop_local`, no object lying over them), then writes
    // `v.len` (checked). The prelude's body where an element has an object.
    pub(super) fn native_vec_clear_res(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        let tid = self.type_id(elem)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    if (cb_live_in(v->ptr, n * {}u)) {{ {}(p_v); return; }}", size, fb).unwrap();
        writeln!(b, "    for (uint64_t k = 0; k < n; k++) cb_drop_local((uint8_t *)&v->ptr[k], {}u);", tid).unwrap();
        writeln!(b, "    cb_release_plain((uint8_t *)v->ptr, n * {}u);", size).unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->len = 0;").unwrap();
        Ok(Some(b))
    }

    // `Vec::clone` for a plain element type (a plain value's clone is its
    // copy): the body's checked read of `v.len`, then each element read
    // through the vector the call holds shared, which cannot fail where
    // that read passed, and pushed onto a confined `out`. Natively one
    // allocation and one copy; `out` bound, moved out and sent as the
    // body's is (as `native_vec_from_slice`).
    pub(super) fn native_vec_clone(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, align) = self.size_align(elem)?;
        let string = matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if size == 0 || !(self.plain_data(elem) || string) {
            return Ok(None);
        }
        let ec = self.ctype(elem)?;
        let vtid = self.type_id(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (cb_live_in(v->ptr, v->len * {}u)) return {}(p_v);", size, fb).unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", vec_c).unwrap();
        writeln!(b, "    out.ptr = ({} *){};", ec, align).unwrap();
        writeln!(b, "    out.len = 0;").unwrap();
        writeln!(b, "    out.cap = 0;").unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        {} *p = ({} *)cb_allocate(n * {}u, {}u);", ec, ec, size, align).unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        if string {
            // D-0192: each `String` cloned as `String::clone` makes it: a
            // buffer of its bytes, its own (capacity unobservable).
            writeln!(b, "        for (uint64_t k = 0; k < n; k++) {{").unwrap();
            writeln!(b, "            uint64_t bl = v->ptr[k].bytes.len;").unwrap();
            writeln!(b, "            uint8_t *bp = (uint8_t *)1;").unwrap();
            writeln!(b, "            if (bl > 0) {{ bp = cb_allocate(bl, 1u); if (!bp) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0); memcpy(bp, v->ptr[k].bytes.ptr, bl); }}").unwrap();
            writeln!(b, "            p[k].bytes.ptr = bp; p[k].bytes.len = bl; p[k].bytes.cap = bl;").unwrap();
            writeln!(b, "        }}").unwrap();
        } else {
            writeln!(b, "        memcpy(p, v->ptr, (size_t)n * {}u);", size).unwrap();
        }
        writeln!(b, "        out.ptr = p;").unwrap();
        writeln!(b, "        out.len = n;").unwrap();
        writeln!(b, "        out.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", vtid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", vec_c).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // `Vec::append(a, b)` for a plain element type: the body receives and
    // binds `b`, then moves each element out of it and pushes it onto `a`
    // (the first push the first checked read of `a.len`; `grow` when full;
    // a checked write of `a.len` each), and `b`, emptied, ends with the
    // frame. Natively: `b` received and bound, `a` grown as the pushes
    // would grow it, the elements copied at once, their cells in `b`
    // released as the moves out release them, and `b` left empty for its
    // destructor at the frame's end.
    pub(super) fn native_vec_append(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
        let mut b = String::new();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    uint64_t ob = cb_recv(&p_b);").unwrap();
        writeln!(b, "    (void)cb_bind(ob);").unwrap();
        writeln!(b, "    uint64_t n = p_b.len;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        cb_read(p_a.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *a = ({} *)p_a.p;", vec_c, vec_c).unwrap();
        writeln!(b, "        while (a->cap - a->len < n) {{ {}(p_a); }}", grow).unwrap();
        writeln!(b, "        memcpy(a->ptr + a->len, p_b.ptr, (size_t)n * {}u);", size).unwrap();
        writeln!(b, "        cb_write(p_a.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        a->len += n;").unwrap();
        writeln!(b, "        cb_release_plain((uint8_t *)p_b.ptr, n * {}u);", size).unwrap();
        writeln!(b, "        p_b.len = 0;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        Ok(Some(b))
    }

    // `Vec::dedup` for an integer or `bool` element type, whose `==` is the
    // C one: the body reads `v.len` (checked), compares each element with
    // the last kept through the vector it holds exclusively, swaps a kept
    // one into place (`swap`'s exclusive borrow check, when the first such
    // swap happens) and truncates (a checked write of `v.len`, the dropped
    // cells released). The same, in place; the prelude's body where an
    // element has a live object.
    pub(super) fn native_vec_dedup(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        if !matches!(elem, Type::Int(_) | Type::Bool) {
            return Ok(None);
        }
        let size = self.size_align(elem)?.0;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t n = v->len;").unwrap();
        writeln!(b, "    if (n < 2) return;").unwrap();
        writeln!(b, "    if (cb_live_in(v->ptr, n * {}u)) {{ {}(p_v); return; }}", size, fb).unwrap();
        writeln!(b, "    uint64_t w = 1;").unwrap();
        writeln!(b, "    uint8_t swapped = 0;").unwrap();
        writeln!(b, "    for (uint64_t i = 1; i < n; i++) {{").unwrap();
        writeln!(b, "        if (v->ptr[i] != v->ptr[w - 1]) {{").unwrap();
        writeln!(b, "            if (w != i) {{").unwrap();
        writeln!(b, "                if (!swapped) {{ cb_borrow_check(p_v.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0); swapped = 1; }}").unwrap();
        writeln!(b, "                v->ptr[w] = v->ptr[i];").unwrap();
        writeln!(b, "            }}").unwrap();
        writeln!(b, "            w++;").unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    if (w < n) {{").unwrap();
        writeln!(b, "        cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        cb_release_plain((uint8_t *)(v->ptr + w), (n - w) * {}u);", size).unwrap();
        writeln!(b, "        v->len = w;").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // D-0192: `==` and `<` for the element types the native searches take:
    // C's own for integers and `bool`, the bytes for `String` (its `==` and
    // its order, `cb_bytes_less` as `Vec::sort` compares).
    pub(super) fn search_ops(&self, elem: &Type) -> Option<(fn(&str, &str) -> String, fn(&str, &str) -> String)> {
        match elem {
            Type::Int(_) | Type::Bool => Some((|a, b| format!("({} == {})", a, b), |a, b| format!("({} < {})", a, b))),
            Type::Named(n, a) if n == "std::String" && a.is_empty() => Some((
                |a, b| format!("cb_bytes_eq((const void *)({a}).bytes.ptr, ({a}).bytes.len, (const void *)({b}).bytes.ptr, ({b}).bytes.len)", a = a, b = b),
                |a, b| format!("cb_bytes_less((const void *)({a}).bytes.ptr, ({a}).bytes.len, (const void *)({b}).bytes.ptr, ({b}).bytes.len)", a = a, b = b),
            )),
            _ => None,
        }
    }

    // `Vec::contains` for an integer or `bool` element type: the body is
    // `index_of` (native, `native_vec_index_of`) and a test of its result;
    // the same checks, the same fallback where an element has an object.
    pub(super) fn native_vec_contains(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let Some((eq, _)) = self.search_ops(elem) else { return Ok(None) };
        let elem_c = self.ctype(elem)?;
        let size = self.size_align(elem)?.0;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (cb_live_in(v->ptr, v->len * {}u)) return {}(p_v, p_x);", size, fb).unwrap();
        writeln!(b, "    if (v->len > 0) {{").unwrap();
        writeln!(b, "        cb_read(p_x.tok, NULL, 0, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} xv = *({} *)p_x.p;", elem_c, elem_c).unwrap();
        writeln!(b, "        for (uint64_t k = 0; k < v->len; k++) if ({}) return 1;", eq("v->ptr[k]", "xv")).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    return 0;").unwrap();
        Ok(Some(b))
    }

    // `Vec::binary_search` for an integer or `bool` element type, whose
    // `<` (`key_before`) is the C one: the body (`binary_search_by` with
    // `key_before`) reads `v.len` checked; for a non-empty vector it reads
    // the key (checked) and the elements through the vector the call holds
    // shared, which cannot fail where the first read passed. The same
    // lower bound, compared in place.
    pub(super) fn native_vec_binary_search(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let Some((_, lt)) = self.search_ops(elem) else { return Ok(None) };
        let ec = self.ctype(elem)?;
        let res_c = self.ctype(&Type::Named("std::Result".to_string(), vec![Type::Int(IntTy::Usize), Type::Int(IntTy::Usize)]))?;
        let fb = self.std_fallback()?;
        let size = self.size_align(elem)?.0;
        let mut b = String::new();
        writeln!(b, "    {} r;", res_c).unwrap();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    uint64_t lo = 0, hi = v->len;").unwrap();
        writeln!(b, "    if (hi > 0 && cb_live_in(v->ptr, hi * {}u)) return {}(p_v, p_key);", size, fb).unwrap();
        writeln!(b, "    if (hi > 0) {{").unwrap();
        writeln!(b, "        cb_read(p_key.tok, NULL, 0, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} k = *({} *)p_key.p;", ec, ec).unwrap();
        writeln!(b, "        while (lo < hi) {{ uint64_t mid = lo + (hi - lo) / 2; if ({}) lo = mid + 1; else hi = mid; }}", lt("v->ptr[mid]", "k")).unwrap();
        writeln!(b, "        if (lo < v->len && !({})) {{ r.tag = 0u; r.u.v0 = lo; return r; }}", lt("k", "v->ptr[lo]")).unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    r.tag = 1u;").unwrap();
        writeln!(b, "    r.u.v1 = lo;").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `Vec::filled(n, x)` for a plain element type, as `from_slice`: the
    // body pushes `n` clones of `x` (a plain value's clone is its copy)
    // onto a confined `out`, reading `x` checked each time though nothing
    // can reach it but the body. Here one allocation of exactly `n`
    // elements, filled at once; `out` bound, moved out and sent as the
    // body's is. A size that overflows is the allocation the body's
    // growth could never have made: `diag.alloc-failure`, as there.
    pub(super) fn native_vec_filled(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, align) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let ec = self.ctype(elem)?;
        let vtid = self.type_id(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let fail = "cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0)";
        let mut b = String::new();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", vec_c).unwrap();
        writeln!(b, "    out.ptr = ({} *){};", ec, align).unwrap();
        writeln!(b, "    out.len = 0;").unwrap();
        writeln!(b, "    out.cap = 0;").unwrap();
        writeln!(b, "    uint64_t n = p_n;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint64_t bytes;").unwrap();
        writeln!(b, "        if (__builtin_mul_overflow(n, (uint64_t){}u, &bytes)) {};", size, fail).unwrap();
        writeln!(b, "        {} *p = ({} *)cb_allocate(bytes, {}u);", ec, ec, align).unwrap();
        writeln!(b, "        if (!p) {};", fail).unwrap();
        if size == 1 {
            writeln!(b, "        memset(p, *(uint8_t *)&p_x, n);").unwrap();
        } else {
            writeln!(b, "        for (uint64_t i = 0; i < n; i++) p[i] = p_x;").unwrap();
        }
        writeln!(b, "        out.ptr = p;").unwrap();
        writeln!(b, "        out.len = n;").unwrap();
        writeln!(b, "        out.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", vtid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", vec_c).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // `Vec::from_slice(s)` for a plain element type: the body binds `out`
    // (`Vec::new`), reserves `s`'s length -- from an empty vector, a buffer
    // of exactly that many elements, its overflow fault -- then pushes
    // each element, unchecked: the slice is a direct parameter and `out`
    // a confined local. Here the elements are copied at once; `out` is a
    // binding as in the body (its object, bound in a frame, then moved
    // out and sent), and the slice is received as `split` receives its
    // view, its path ending with the caller's statement.
    pub(super) fn native_vec_from_slice(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, align) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let ec = self.ctype(elem)?;
        let vtid = self.type_id(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let ovf = "cb_fault(\"diag.arith-overflow\", CB_NOLOC, 0)";
        let fb = self.std_fallback()?;
        let mut b = String::new();
        // An element with a live object: the body's element reads, each
        // checked against the paths that may hold it.
        writeln!(b, "    if (cb_live_in(p_s.data, p_s.len * {}u)) return {}(p_s);", size, fb).unwrap();
        writeln!(b, "    (void)cb_recv_datum_tok();").unwrap();
        writeln!(b, "    cb_frame_push();").unwrap();
        writeln!(b, "    {} out;", vec_c).unwrap();
        writeln!(b, "    out.ptr = ({} *){};", ec, align).unwrap();
        writeln!(b, "    out.len = 0;").unwrap();
        writeln!(b, "    out.cap = 0;").unwrap();
        writeln!(b, "    uint64_t n = p_s.len;").unwrap();
        writeln!(b, "    if (n > 0) {{").unwrap();
        writeln!(b, "        uint64_t bytes;").unwrap();
        writeln!(b, "        if (__builtin_mul_overflow(n, (uint64_t){}u, &bytes)) {};", size, ovf).unwrap();
        writeln!(b, "        uint8_t *p = cb_allocate(bytes, {}u);", align).unwrap();
        writeln!(b, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        memcpy(p, p_s.data, bytes);").unwrap();
        writeln!(b, "        out.ptr = ({} *)p;", ec).unwrap();
        writeln!(b, "        out.len = n;").unwrap();
        writeln!(b, "        out.cap = n;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    uint64_t oo = cb_new(&out, {}u, CB_NOLOC, 0);", vtid).unwrap();
        writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
        writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} res = out;", vec_c).unwrap();
        writeln!(b, "    cb_move_to(ot, &res);").unwrap();
        writeln!(b, "    cb_send(ot);").unwrap();
        writeln!(b, "    cb_frame_pop();").unwrap();
        writeln!(b, "    return res;").unwrap();
        Ok(Some(b))
    }

    // `Vec::pop` for a plain element type: the body's checked read of
    // `v.len`, `None` when it is 0, the checked write of `len - 1` (which
    // cannot overflow there), then the element's `[Rawptr-Read]` and the
    // `[Release]` of its cells, as the prelude's body does. Its other
    // reads of `v` go through the same path with nothing in between but
    // that write, and cannot fail where the first passed. `checked`
    // false: the confined form (`_nc`), without the two checks.
    pub(super) fn native_vec_pop(&mut self, elem: &Type, vec_c: &str, checked: bool) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let opt_c = self.ctype(&Type::Named("std::Option".to_string(), vec![elem.clone()]))?;
        let mut b = String::new();
        if checked {
            writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        }
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    {} r;", opt_c).unwrap();
        writeln!(b, "    if (v->len == 0) {{ r.tag = 1u; return r; }}").unwrap();
        writeln!(b, "    uint64_t n = v->len - 1;").unwrap();
        if checked {
            writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        }
        writeln!(b, "    v->len = n;").unwrap();
        writeln!(b, "    r.tag = 0u;").unwrap();
        writeln!(b, "    r.u.v0 = v->ptr[n];").unwrap();
        writeln!(b, "    cb_release_plain((uint8_t *)(v->ptr + n), {}u);", size).unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `Vec::remove` for a plain element type: the body moves elements
    // `i + 1 ..` down one place by `Vec::swap`s, which for plain data copy
    // bytes only (`swap_places`), then `pop`s the last. Here the same bytes
    // move at once: the checked read of `v.len`, the bounds fault, the
    // element out, the move, then `pop`'s checked write of `len - 1` and the
    // `[Release]` of the last cells. The swaps' own checks are through the
    // exclusive path the call was given, with nothing in between that could
    // change their outcome, and cannot fail where the first read passed.
    pub(super) fn native_vec_remove(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.movable_elem(elem) {
            return Ok(None);
        }
        let res = !self.plain_data(elem);
        let elem_c = self.ctype(elem)?;
        let fb = self.std_fallback()?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (p_i >= v->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        // An element the body's swaps would borrow, with a live object.
        writeln!(b, "    if (cb_live_in(&v->ptr[p_i], (v->len - p_i) * {}u)) return {}(p_v, p_i);", size, fb).unwrap();
        writeln!(b, "    {} x = v->ptr[p_i];", elem_c).unwrap();
        writeln!(b, "    memmove(v->ptr + p_i, v->ptr + p_i + 1, (size_t)(v->len - p_i - 1) * sizeof({}));", elem_c).unwrap();
        writeln!(b, "    uint64_t n = v->len - 1;").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->len = n;").unwrap();
        writeln!(b, "    cb_release_plain((uint8_t *)(v->ptr + n), {}u);", size).unwrap();
        if res {
            // The value moved out (`[Rawptr-Move-Out]`) and returned as the
            // body returns it: an object, bound, taken and sent.
            let tid = self.type_id(elem)?;
            writeln!(b, "    cb_frame_push();").unwrap();
            writeln!(b, "    uint64_t oo = cb_new(&x, {}u, CB_NOLOC, 0);", tid).unwrap();
            writeln!(b, "    uint64_t root = cb_bind(oo);").unwrap();
            writeln!(b, "    uint64_t ot = cb_take(root, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} res = x;", elem_c).unwrap();
            writeln!(b, "    cb_move_to(ot, &res);").unwrap();
            writeln!(b, "    cb_send(ot);").unwrap();
            writeln!(b, "    cb_frame_pop();").unwrap();
            writeln!(b, "    return res;").unwrap();
        } else {
            writeln!(b, "    return x;").unwrap();
        }
        Ok(Some(b))
    }

    // `Vec::index_of` for an integer or `bool` element type, whose `==` is
    // the C one: the body's checked reads of `v.len` and of `*x`, then the
    // elements compared in place. The vector is borrowed shared for the
    // call, so no path can write it meanwhile, and each element's read
    // through it cannot fail where the first passed.
    pub(super) fn native_vec_index_of(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let Some((eq, _)) = self.search_ops(elem) else { return Ok(None) };
        let elem_c = self.ctype(elem)?;
        let size = self.size_align(elem)?.0;
        let fb = self.std_fallback()?;
        let opt_c = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Int(IntTy::Usize)]))?;
        let mut b = String::new();
        writeln!(b, "    {} r;", opt_c).unwrap();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (cb_live_in(v->ptr, v->len * {}u)) return {}(p_v, p_x);", size, fb).unwrap();
        writeln!(b, "    if (v->len > 0) {{").unwrap();
        writeln!(b, "        cb_read(p_x.tok, NULL, 0, CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} xv = *({} *)p_x.p;", elem_c, elem_c).unwrap();
        writeln!(b, "        for (uint64_t k = 0; k < v->len; k++) {{").unwrap();
        writeln!(b, "            if ({}) {{ r.tag = 0u; r.u.v0 = k; return r; }}", eq("v->ptr[k]", "xv")).unwrap();
        writeln!(b, "        }}").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    r.tag = 1u;").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // `Vec::push` for a plain element type: the body's checked read of
    // `v.len` (its reads of `v.cap` and `v.ptr` go through the same path
    // with nothing in between but `grow`'s own checked writes through it,
    // and cannot fail where that passed), `grow` as written in the
    // prelude when full, the element's `[Rawptr-Write]` (no reference in
    // the value, so no holder object: CHG-0031), and the checked write of
    // `v.len`.
    pub(super) fn native_vec_push(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 {
            return Ok(None);
        }
        if self.is_resource(elem) && !self.has_refs(elem) && !self.holds_fn(elem) && !matches!(elem, Type::Ref(..)) {
            // A resource holding no reference (`String`, D-0129's bench):
            // the body's receipt of `x` (`[Param]`), its checked reads of
            // `v` (`len` first; the rest through the same path), `grow`
            // when full, the move into the buffer (`[Rawptr-Move-In]`,
            // `cb_raw_move_in`), then the checked write of `v.len`. The
            // body's bind and take of `x` are not made: `x`'s object was
            // received just now, no path into it exists, so the take's
            // solitude check cannot fail, and its root path ends unheld.
            let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
            let mut b = String::new();
            writeln!(b, "    uint64_t o = cb_recv(&p_x);").unwrap();
            writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
            writeln!(b, "    if (v->len == v->cap) {{ {}(p_v); }}", grow).unwrap();
            writeln!(b, "    v->ptr[v->len] = p_x;").unwrap();
            writeln!(b, "    cb_raw_move_in(o, &v->ptr[v->len]);").unwrap();
            writeln!(b, "    uint64_t n = v->len + 1;").unwrap();
            writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    v->len = n;").unwrap();
            return Ok(Some(b));
        }
        if !self.plain_data(elem) {
            // Reference data with no resource and no `fn` value: the body's
            // receipt of `x` (`[Param]`), then its write into the buffer
            // (`[Rawptr-Write]`: the bytes, and the references' paths held
            // by the element's slots) -- here one step: the in-flight
            // paths are received straight into those slots, stamped where
            // the parameter's receipt stamps them (the calling statement's
            // scope), held once, as the body leaves them once `x` ends.
            // Its checks and their order as for a plain element.
            // A bare reference travels as a `cb_ref`, not in flight.
            if self.is_resource(elem) || self.holds_fn(elem) || !self.has_refs(elem) || matches!(elem, Type::Ref(..)) {
                return Ok(None);
            }
            let tid = self.type_id(elem)?;
            let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
            let mut b = String::new();
            writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
            writeln!(b, "    if (v->len == v->cap) {{ {}(p_v); }}", grow).unwrap();
            writeln!(b, "    v->ptr[v->len] = p_x;").unwrap();
            writeln!(b, "    cb_recv_datum(&v->ptr[v->len], {}u);", tid).unwrap();
            writeln!(b, "    uint64_t n = v->len + 1;").unwrap();
            writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    v->len = n;").unwrap();
            return Ok(Some(b));
        }
        let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (v->len == v->cap) {{ {}(p_v); }}", grow).unwrap();
        writeln!(b, "    v->ptr[v->len] = p_x;").unwrap();
        // As in the `_nc` body: `len < cap` here, so `len + 1` cannot overflow.
        writeln!(b, "    uint64_t n = v->len + 1;").unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->len = n;").unwrap();
        Ok(Some(b))
    }
}
