// Native C bodies for std's Option and Result helpers and empty values.

use super::*;

impl<'a> Gen<'a> {
    // D-0200: `Result::unwrap_or(r, d)` natively, for a resource `T` whose
    // destruction is silent (`silent_drop`) holding no reference, and a
    // plain `E`: both arguments received in order, as the body's
    // parameters are; for `Ok(v)`, `d` destroyed (its record ended, its
    // value dropped in place) and `v` given an object and sent; for `Err`,
    // `r`'s record ended (its payload plain) and `d` sent back with the
    // object it came with.
    pub(super) fn native_result_unwrap_or(&mut self, t: &Type, e: &Type) -> R<Option<String>> {
        if !self.is_resource(t) || self.has_refs(t) || self.holds_fn(t) || !self.silent_drop(t) || !self.plain_data(e) || self.has_refs(e) {
            return Ok(None);
        }
        let rt = Type::Named("std::Result".to_string(), vec![t.clone(), e.clone()]);
        if !self.is_resource(&rt) {
            return Ok(None);
        }
        let tc = self.ctype(t)?;
        let ttid = self.type_id(t)?;
        let mut b = String::new();
        writeln!(b, "    uint64_t orr = cb_recv(&p_r);").unwrap();
        writeln!(b, "    uint64_t od = cb_recv(&p_d);").unwrap();
        writeln!(b, "    if (p_r.tag == 0u) {{").unwrap();
        writeln!(b, "        {} v = p_r.u.v0;", tc).unwrap();
        writeln!(b, "        cb_forget_obj(orr);").unwrap();
        writeln!(b, "        cb_forget_obj(od);").unwrap();
        writeln!(b, "        cb_drop_local(&p_d, {}u);", ttid).unwrap();
        writeln!(b, "        uint64_t ro = cb_new(&v, {}u, CB_NOLOC, 0);", ttid).unwrap();
        writeln!(b, "        cb_send(ro);").unwrap();
        writeln!(b, "        return v;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    cb_forget_obj(orr);").unwrap();
        writeln!(b, "    cb_send(od);").unwrap();
        writeln!(b, "    return p_d;").unwrap();
        Ok(Some(b))
    }

    // `Option::unwrap`/`expect` and `Result::unwrap`/`expect` natively, for a
    // value that is plain data or a reference (and, for a `Result`, an error
    // that is plain data, so a fault leaves nothing to destroy): the body
    // receives the argument (a datum holding the reference, if any), takes
    // the payload of `Some`/`Ok` or faults `diag.unwrap-failed` with its
    // message, and returns the payload (a reference sent back as the
    // caller's `cb_recv_ref` expects). Here the same, without the body's
    // frame, bindings and copies, which nothing else reaches.
    pub(super) fn native_unwrap(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let (t, e) = match targs {
            [t] => (t.clone(), None),
            [t, e] => (t.clone(), Some(e.clone())),
            _ => return Ok(None),
        };
        if let Some(e) = &e {
            if !self.plain_data(e) {
                return Ok(None);
            }
        }
        let is_ref = matches!(t, Type::Ref(..));
        if !(is_ref || self.plain_data(&t)) || matches!(t, Type::Void | Type::Never) {
            return Ok(None);
        }
        if !is_ref && self.size_align(&t)?.0 == 0 {
            return Ok(None);
        }
        let (param, msg) = match name {
            "std::Option::unwrap" => ("p_o", Some("`Option::unwrap` found `None`")),
            "std::Option::expect" => ("p_o", None),
            "std::Result::unwrap" => ("p_r", Some("`Result::unwrap` found `Err`")),
            _ => ("p_r", None),
        };
        let fault = match msg {
            Some(m) => format!("cb_fault_msg(\"diag.unwrap-failed\", (const uint8_t *)\"{}\", {}u, CB_NOLOC, 0);", m, m.len()),
            None => "cb_fault_msg(\"diag.unwrap-failed\", p_message.p, p_message.n, CB_NOLOC, 0);".to_string(),
        };
        let ety = match &e {
            Some(e) => Type::Named("std::Result".to_string(), vec![t.clone(), e.clone()]),
            None => Type::Named("std::Option".to_string(), vec![t.clone()]),
        };
        let not_ok = format!("!({})", self.tag_is(&ety, param, 0));
        let mut b = String::new();
        if is_ref {
            writeln!(b, "    uint64_t tk = cb_recv_datum_tok();").unwrap();
            writeln!(b, "    if ({}) {}", not_ok, fault).unwrap();
            writeln!(b, "    cb_send_ref(tk);").unwrap();
            writeln!(b, "    return (cb_ref){{ (void *){}.u.v0, tk }};", param).unwrap();
        } else {
            writeln!(b, "    if ({}) {}", not_ok, fault).unwrap();
            writeln!(b, "    return {}.u.v0;", param).unwrap();
        }
        Ok(Some(b))
    }

    // D-0200: `String::new()` and `Vec::new()` natively: the empty value
    // (`dangling` pointer, no length, no capacity) as the body builds it,
    // given an object and sent, as the body's literal is.
    pub(super) fn native_empty(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
        let ty = if name == "std::String::new" {
            Type::Named("std::String".to_string(), vec![])
        } else {
            match targs {
                [t] => Type::Named("std::Vec".to_string(), vec![t.clone()]),
                _ => return Ok(None),
            }
        };
        let tc = self.ctype(&ty)?;
        let tid = self.type_id(&ty)?;
        let init = self.empty_value_init(&ty)?;
        let mut b = String::new();
        writeln!(b, "    {} r;", tc).unwrap();
        writeln!(b, "    {}", init.replace("@", "r")).unwrap();
        writeln!(b, "    uint64_t o = cb_new(&r, {}u, CB_NOLOC, 0);", tid).unwrap();
        writeln!(b, "    cb_send(o);").unwrap();
        writeln!(b, "    return r;").unwrap();
        Ok(Some(b))
    }

    // The C statements that make `@` an empty `String` or `Vec<T>`.
    pub(super) fn empty_value_init(&mut self, ty: &Type) -> R<String> {
        match ty {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => Ok("@.bytes.ptr = (uint8_t *)1; @.bytes.len = 0; @.bytes.cap = 0;".to_string()),
            Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => {
                let pc = self.ctype(&Type::Rawptr(Box::new(a[0].clone())))?;
                Ok(format!("@.ptr = ({})1; @.len = 0; @.cap = 0;", pc))
            }
            _ => internal("an empty value of an unexpected type"),
        }
    }

    // The native bodies of std's functions on types other than `Vec`
    // (`native_vec_index` dispatches to this first): `None` when the
    // function has none and is lowered as written.
    pub(super) fn native_std_body(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str) -> R<Option<String>> {
        if matches!(name, "std::Queue::front" | "std::Queue::back") && in_prelude(f) {
            if let Some(b) = self.native_queue_peek(name, targs)? {
                return Ok(Some(b));
            }
        }
        if matches!(name, "std::Queue::pop_front" | "std::Queue::pop_back") && in_prelude(f) {
            if let Some(b) = self.native_queue_op(name, targs)? {
                return Ok(Some(b));
            }
        }
        if name == "std::String::append_string" && in_prelude(f) {
            return self.native_append_string().map(Some);
        }
        if matches!(name, "std::String::append_str" | "std::String::append_view" | "std::String::append_text") && in_prelude(f) {
            return self.native_append_bytes(name, targs);
        }
        if name == "std::StringView::split" && in_prelude(f) {
            return self.native_view_split();
        }
        if matches!(name, "std::StringView::split_whitespace" | "std::StringView::chars") && in_prelude(f) {
            return self.native_view_split_whitespace(name == "std::StringView::chars");
        }
        if matches!(name, "std::PriorityQueue::push" | "std::PriorityQueue::pop") && in_prelude(f) {
            if let [t] = targs {
                return self.native_pq(name, t);
            }
        }
        if name == "std::Result::unwrap_or" && in_prelude(f) {
            if let [t, e] = targs {
                return self.native_result_unwrap_or(t, e);
            }
        }
        if name == "std::read_line" && in_prelude(f) && targs.is_empty() {
            return self.native_read_line();
        }
        if name == "std::File::read_line" && in_prelude(f) && targs.is_empty() {
            return self.native_file_read_line();
        }
        if matches!(name, "std::File::write" | "std::File::write_text") && in_prelude(f) && targs.is_empty() {
            return self.native_file_write(name);
        }
        if matches!(name, "std::Channel::send" | "std::Channel::recv") && in_prelude(f) {
            if let [t] = targs {
                return self.native_channel(name == "std::Channel::send", t);
            }
        }
        if matches!(name, "std::StringView::parse" | "std::String::parse") && in_prelude(f) {
            if let [Type::Int(it)] = targs {
                return self.native_parse_int(name, *it);
            }
        }
        if name == "std::Drain::take" && in_prelude(f) {
            if let [t] = targs {
                return self.native_drain_take(t);
            }
        }
        if matches!(name, "std::StringView::trim" | "std::StringView::trim_start" | "std::StringView::trim_end") && in_prelude(f) {
            return self.native_view_trim(name);
        }
        if matches!(name, "std::String::to_ascii_lower" | "std::String::to_ascii_upper") && in_prelude(f) {
            return self.native_ascii_case(name == "std::String::to_ascii_upper");
        }
        if name == "std::StringView::sub" && in_prelude(f) {
            return self.native_view_sub();
        }
        if matches!(
            name,
            "std::StringView::find" | "std::StringView::contains" | "std::StringView::starts_with" | "std::StringView::ends_with"
                | "std::String::find" | "std::String::contains" | "std::String::starts_with" | "std::String::ends_with"
        ) && in_prelude(f)
        {
            return self.native_text_search(name);
        }
        if name == "std::String::view" && in_prelude(f) {
            return self.native_string_view();
        }
        if matches!(name, "std::Option::unwrap" | "std::Option::expect" | "std::Result::unwrap" | "std::Result::expect") && in_prelude(f) {
            return self.native_unwrap(name, targs);
        }
        if name == "std::replace" && in_prelude(f) {
            if let [t] = targs {
                return self.native_replace(t);
            }
        }
        if matches!(name, "std::String::new" | "std::Vec::new") && in_prelude(f) {
            return self.native_empty(name, targs);
        }
        if name == "std::HashSet::insert" && in_prelude(f) && matches!(targs, [Type::Named(n, a)] if n == "std::String" && a.is_empty()) {
            return self.native_set_insert_string();
        }
        if name == "std::Vec::sorted_order" && in_prelude(f) {
            if let [t] = targs {
                return self.native_sorted_order(t);
            }
        }
        if name == "std::StringView::of" && in_prelude(f) {
            // The body: `StringView { .bytes = str_slice(s) }` -- a view
            // of the text's bytes, which are no object's cells (`type.str`),
            // so with no path (0), sent as the result's one reference
            // (`cb_send_tok_datum`) as `str_slice` makes it.
            let svc = self.ctype(&Type::Named("std::StringView".to_string(), vec![]))?;
            let mut b = String::new();
            writeln!(b, "    uint64_t tk = 0;").unwrap();
            writeln!(b, "    {} r;", svc).unwrap();
            writeln!(b, "    r.bytes.src = (void *)p_s.p;").unwrap();
            writeln!(b, "    r.bytes.data = (uint8_t *)p_s.p;").unwrap();
            writeln!(b, "    r.bytes.len = p_s.n;").unwrap();
            writeln!(b, "    cb_send_tok_datum(tk);").unwrap();
            writeln!(b, "    return r;").unwrap();
            return Ok(Some(b));
        }
        if name == "std::String::from_view" && in_prelude(f) {
            let b = self.native_string_from_view()?;
            if b.is_some() {
                // The `_raw` variant (`lower_map_fresh_key`,
                // `lower_push_fresh`): the copy alone, for a call that hands
                // the new `String` straight to a native body which stores
                // or frees it; the view is not sent to it.
                let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
                let svc = self.ctype(&Type::Named("std::StringView".to_string(), vec![]))?;
                let sig = format!("{} {}_raw({} p_v)", sc, cname, svc);
                let mut raw = String::new();
                writeln!(raw, "    {} r;", sc).unwrap();
                writeln!(raw, "    r.bytes.ptr = (uint8_t *)1;").unwrap();
                writeln!(raw, "    r.bytes.len = 0;").unwrap();
                writeln!(raw, "    r.bytes.cap = 0;").unwrap();
                writeln!(raw, "    uint64_t n = p_v.bytes.len;").unwrap();
                writeln!(raw, "    if (n > 0) {{").unwrap();
                writeln!(raw, "        uint8_t *p = cb_allocate(n, 1u);").unwrap();
                writeln!(raw, "        if (!p) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
                writeln!(raw, "        memcpy(p, p_v.bytes.data, n);").unwrap();
                writeln!(raw, "        r.bytes.ptr = p;").unwrap();
                writeln!(raw, "        r.bytes.cap = n;").unwrap();
                writeln!(raw, "        r.bytes.len = n;").unwrap();
                writeln!(raw, "    }}").unwrap();
                writeln!(raw, "    return r;").unwrap();
                writeln!(self.protos, "{};", sig).unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, raw).unwrap();
            }
            return Ok(b);
        }
        if matches!(name, "std::HashMap::get" | "std::HashMap::get_mut" | "std::HashMap::contains" | "std::HashMap::entry" | "std::HashMap::insert") && in_prelude(f) {
            if let Some(b) = self.native_map_op(name, targs)? {
                // `entry`/`insert` with `String` keys: the `_rawk` variant
                // for a key no object tracks (`lower_map_fresh_key`): not
                // received; freed directly when not stored (fresh bytes have
                // no object over them), stored with nothing to move in.
                if matches!(name, "std::HashMap::entry" | "std::HashMap::insert") && matches!(targs.first(), Some(Type::Named(n, _)) if n == "std::String") {
                    let raw: String = b
                        .lines()
                        .filter(|l| !l.contains("uint64_t ok = cb_recv(&p_k);") && !l.contains("cb_raw_move_in(ok,"))
                        .map(|l| {
                            if l.contains("cb_consume(ok, CB_NOLOC, 0);") {
                                "        if (p_k.bytes.cap > 0) cb_deallocate_plain((uint8_t *)p_k.bytes.ptr, p_k.bytes.cap);\n".to_string()
                            } else {
                                format!("{}\n", l)
                            }
                        })
                        .collect();
                    let [k, v] = targs else { return internal("map arity") };
                    let mc = self.ctype(&Type::Named("std::HashMap".to_string(), vec![k.clone(), v.clone()]))?;
                    let _ = mc;
                    let kc = self.ctype(k)?;
                    let vc = self.ctype(v)?;
                    let sig = if name == "std::HashMap::entry" {
                        format!("cb_ref {}_rawk(cb_ref p_m, {} p_k, {} p_fresh)", cname, kc, vc)
                    } else {
                        let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![v.clone()]))?;
                        format!("{} {}_rawk(cb_ref p_m, {} p_k, {} p_v)", oc, cname, kc, vc)
                    };
                    writeln!(self.protos, "{};", sig).unwrap();
                    write!(self.fns, "{}\n{{\n{}}}\n\n", sig, raw).unwrap();
                    if name == "std::HashMap::entry" {
                        let acc = Self::entry_acc_body(&raw, self.type_id(v)?);
                        let sig = format!("{} *{}_rawk_acc(cb_ref p_m, {} p_k, {} p_fresh)", vc, cname, kc, vc);
                        writeln!(self.protos, "{};", sig).unwrap();
                        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, acc).unwrap();
                    }
                }
                if name == "std::HashMap::entry" {
                    if let [k, v] = targs {
                        let kc = self.ctype(k)?;
                        let vc = self.ctype(v)?;
                        let acc = Self::entry_acc_body(&b, self.type_id(v)?);
                        let sig = format!("{} *{}_acc(cb_ref p_m, {} p_k, {} p_fresh)", vc, cname, kc, vc);
                        writeln!(self.protos, "{};", sig).unwrap();
                        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, acc).unwrap();
                    }
                }
                if matches!(name, "std::HashMap::get_mut" | "std::HashMap::get") {
                    if let [_, v] = targs {
                        if self.plain_data(v) {
                            let vc = self.ctype(v)?;
                            let acc = Self::get_mut_acc_body(&b, self.type_id(v)?, name == "std::HashMap::get_mut");
                            let sig = format!("{} *{}_acc(cb_ref p_m, cb_ref p_k)", vc, cname);
                            writeln!(self.protos, "{};", sig).unwrap();
                            write!(self.fns, "{}\n{{\n{}}}\n\n", sig, acc).unwrap();
                            if name == "std::HashMap::get_mut" {
                                let sig = format!("{} *{}_acc_x(cb_ref p_m, cb_ref p_k)", vc, cname);
                                writeln!(self.protos, "{};", sig).unwrap();
                                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, Self::map_excl_unchecked(&acc)).unwrap();
                            }
                        }
                    }
                }
                if name == "std::HashMap::get_mut" {
                    // The body for a map checked exclusively at the call
                    // (`lower_user_call`'s `map_excl_checked`).
                    let [_, v] = targs else { return internal("map arity") };
                    let oc = self.ctype(&Type::Named("std::Option".to_string(), vec![Type::Ref(Box::new(v.clone()), Mode::Exclusive)]))?;
                    let sig = format!("{} {}_x(cb_ref p_m, cb_ref p_k)", oc, cname);
                    writeln!(self.protos, "{};", sig).unwrap();
                    write!(self.fns, "{}\n{{\n{}}}\n\n", sig, Self::map_excl_unchecked(&b)).unwrap();
                }
                return Ok(Some(b));
            }
        }
        if name == "std::HashMap::find_str" && in_prelude(f) {
            if let [v] = targs {
                return self.native_map_find_str(v).map(Some);
            }
        }
        if name == "std::HashMap::find" && in_prelude(f) {
            if let [Type::Named(k, ka), v] = targs {
                if k == "std::String" && ka.is_empty() {
                    return self.native_map_find_string(v).map(Some);
                }
            }
            if let [k @ (Type::Int(_) | Type::Bool), v] = targs {
                return self.native_map_find_scalar(k, v).map(Some);
            }
        }
        if (name == "std::String::from_str" || name == "std::String::clone") && in_prelude(f) {
            let b = self.native_string_copy(name == "std::String::clone")?;
            // The `_raw` variant (`lower_map_fresh_key`): the same copy, its
            // result untracked, for a call that hands it straight to a
            // native map body which stores or frees it.
            let raw: String = b.lines().filter(|l| !l.contains("cb_new(&r") && !l.contains("cb_send(o)")).map(|l| format!("{}\n", l)).collect();
            let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
            let param = if name == "std::String::clone" { "cb_ref p_s" } else { "cb_str p_s" };
            let sig = format!("{} {}_raw({})", sc, cname, param);
            writeln!(self.protos, "{};", sig).unwrap();
            write!(self.fns, "{}\n{{\n{}}}\n\n", sig, raw).unwrap();
            return Ok(Some(b));
        }
        if (name == "std::Box::get" || name == "std::Box::get_mut") && in_prelude(f) {
            // `&reclaim<T>(b.ptr)` / `&mut reclaim<T>(b.ptr)`: the checked
            // read of `b.ptr`, then `[Reclaim]`, `[Borrow]` and the result's
            // return in one step, as native `Vec::index_*` makes them.
            let [elem] = targs else { return Ok(None) };
            let bc = self.ctype(&Type::Named("std::Box".to_string(), vec![elem.clone()]))?;
            let tid = self.type_id(elem)?;
            let m = if name == "std::Box::get" { "CB_SHARED" } else { "CB_EXCLUSIVE" };
            let mut b = String::new();
            if name == "std::Box::get_mut" {
                // D-0222: `_ = &mut *b;`, the box exclusively through `b`.
                writeln!(b, "    cb_borrow_check(p_b.tok, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
            }
            writeln!(b, "    cb_read(p_b.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    void *a = (void *)(({} *)p_b.p)->ptr;", bc).unwrap();
            writeln!(b, "    return (cb_ref){{ a, cb_elem_borrow(a, {}u, {}) }};", tid, m).unwrap();
            return Ok(Some(b));
        }
        Ok(None)
    }
}
