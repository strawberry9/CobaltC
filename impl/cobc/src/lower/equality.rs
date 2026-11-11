// Generated equality, key hashing and comparison functions, and thunks.

use super::*;

impl<'a> Gen<'a> {
    // A value with nothing for the runtime to track when it is copied or
    // dropped: no resource, no reference, no fn value, at any depth.
    pub(super) fn plain_data(&self, t: &Type) -> bool {
        match t {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Rawptr(_) => true,
            Type::Array(inner, _) => self.plain_data(inner),
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    !sd.resource && sd.fields.iter().all(|f| self.plain_data(&apply(&f.ty, &sub)))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    !ed.resource && ed.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| self.plain_data(&apply(p, &sub))))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // D-0110/D-0271: a key type (an integer, `bool`, `str`, `String`, or a
    // struct or enum of key types), as the checker decides it.
    pub(super) fn is_key_type(&self, t: &Type) -> bool {
        coby::typecheck::is_key_type(self.items, t)
    }

    // `[Eq-Struct]`: equality of two values of type `t` held in memory
    // at the C lvalues `a` and `b`, as a C expression. Scalars compare
    // in place (a reference in memory is its bare address, `[Eq-Ref]`);
    // an aggregate calls its generated `cb_eq_…` function.
    pub(super) fn eq_expr(&mut self, t: &Type, a: &str, b: &str) -> R<String> {
        Ok(match t {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Ref(..) | Type::Rawptr(_) | Type::Fn(..) => {
                format!("({} == {})", a, b)
            }
            Type::Str => format!("cb_str_eq({}, {})", a, b),
            Type::Void => "1".to_string(),
            // D-0271: a `String` inside a key type compares by its bytes
            // (`[Eq-Str]`), as `[Key-Bytes]` and `key_cmp_part` read it.
            Type::Named(n, _) if n == "std::String" => format!("(cb_bytes_cmp(({a}).bytes.ptr, ({a}).bytes.len, ({b}).bytes.ptr, ({b}).bytes.len) == 0)"),
            Type::Named(..) | Type::Array(..) => {
                let f = self.eq_fn(t)?;
                format!("{}(&({}), &({}))", f, a, b)
            }
            _ => return internal("`==` on a type with no equality"),
        })
    }

    // The `cb_eq_…` function for a struct (every field), an array (every
    // element) or an enum (same variant, then equal payload). A value
    // type cannot contain itself, so the functions for its parts are
    // written out first and no prototypes are needed.
    pub(super) fn eq_fn(&mut self, t: &Type) -> R<String> {
        if let Some(n) = self.eqs.get(t) {
            return Ok(n.clone());
        }
        let ct = self.ctype(t)?;
        let name = format!("cb_eq_{}", mangle(t));
        let body = match t {
            Type::Array(inner, n) => {
                let e = self.eq_expr(inner, "a->a[i]", "b->a[i]")?;
                format!("    for (size_t i = 0; i < {}u; i++)\n        if (!{}) return 0;\n    return 1;\n", n, e)
            }
            Type::Named(tn, args) => {
                if let Some(sd) = self.struct_decl(tn) {
                    let sub = subst_of(&sd.type_params, args);
                    let mut body = String::new();
                    if sd.bits.is_some() {
                        // D-0118: equal bits.
                        body.push_str("    return a->bits == b->bits;\n");
                    } else {
                    for f in &sd.fields {
                        let fname = san(&f.name);
                        let e = self.eq_expr(&apply(&f.ty, &sub), &format!("a->{}", fname), &format!("b->{}", fname))?;
                        writeln!(body, "    if (!{}) return 0;", e).unwrap();
                    }
                    body.push_str("    return 1;\n");
                    }
                    body
                } else if let Some(ed) = self.enum_decl(tn) {
                    let sub = subst_of(&ed.type_params, args);
                    let mut body = String::from("    if (a->tag != b->tag) return 0;\n    switch (a->tag) {\n");
                    for (i, v) in ed.variants.iter().enumerate() {
                        if let Some(p) = &v.payload {
                            let e = self.eq_expr(&apply(p, &sub), &format!("a->u.v{}", i), &format!("b->u.v{}", i))?;
                            writeln!(body, "    case {}u: return {};", i, e).unwrap();
                        }
                    }
                    body.push_str("    default: return 1;\n    }\n");
                    body
                } else {
                    return internal(&format!("unknown type `{}`", tn));
                }
            }
            _ => return internal("eq_fn for a non-aggregate"),
        };
        writeln!(self.thunks_out, "uint8_t {}(const {} *a, const {} *b)\n{{\n{}}}", name, ct, ct, body).unwrap();
        self.eqs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // D-0315 (step 5): the drop glue of `t` in a decided program: a C function
    // `cb_dg_…(void *p, uint32_t d)` that ends a value of `t` as
    // `cb_drop_local` would (`[Destroy-Composite]`: a struct's fields last
    // first, an enum's payload; a `Vec`'s elements, then its buffer; a
    // `Box`'s referent, then its block), for a value whose end frees memory
    // alone (`objectless_type`) of a type with no destructor of its own: a
    // `String`, a `Vec`, a `Box`, a struct or an enum. None for any other
    // type, whose end stays the runtime's. Below a depth of 1000 the rest is
    // the runtime's too, whose walk ends a `Box` chain of any length in
    // constant stack. CHG-0427: a buffer the value owns (a `String`'s or a
    // `Vec`'s, a `Box`'s block) with an object over its cells
    // (`cb_cells_busy`) is ended by the runtime's path, the `String`, `Vec` or
    // `Box` with it; every other buffer is freed here, wherever else objects
    // lie over raw cells.
    pub(super) fn drop_glue(&mut self, t: &Type) -> R<Option<String>> {
        if !self.decided || self.boot(1) {
            return Ok(None);
        }
        if let Some(g) = self.glues.get(t) {
            return Ok(g.clone());
        }
        let kind_ok = match t {
            Type::Named(n, a) if n == "std::String" && a.is_empty() => true,
            Type::Named(n, a) if (n == "std::Vec" || n == "std::Box") && a.len() == 1 => true,
            Type::Named(n, _) if n == "std::Queue" => false,
            Type::Named(n, _) => (self.struct_decl(n).map_or(false, |sd| sd.bits.is_none()) || self.enum_decl(n).is_some()) && !self.items.fns.contains_key(&format!("{}::drop", n)),
            _ => false,
        };
        if !kind_ok || !self.is_resource(t) || !self.objectless_type(t) {
            self.glues.insert(t.clone(), None);
            return Ok(None);
        }
        let name = format!("cb_dg_{}", mangle(t));
        self.glues.insert(t.clone(), Some(name.clone()));
        writeln!(self.thunks_out, "static void {}(void *p, uint32_t d);", name).unwrap();
        let ct = self.ctype(t)?;
        let tid = self.type_id(t)?;
        let mut body = String::new();
        writeln!(body, "    if (d > 1000u) {{ cb_drop_local(p, {}u); return; }}", tid).unwrap();
        writeln!(body, "    {} *v = ({} *)p;", ct, ct).unwrap();
        let Type::Named(n, args) = t else { return internal("drop glue of a non-named type") };
        if n == "std::String" {
            writeln!(body, "    cb_drop_plain_buf(p, {}u, (void *)v->bytes.ptr, v->bytes.cap);", tid).unwrap();
        } else if n == "std::Vec" {
            let e = args[0].clone();
            let (size, _) = self.size_align(&e)?;
            if size > 0 {
                writeln!(body, "    if (v->cap && cb_cells_busy(v->ptr, v->cap * {}u)) {{ cb_drop_local(p, {}u); return; }}", size, tid).unwrap();
            }
            let part = self.glue_part(&e, "v->ptr[i]")?;
            if !part.is_empty() {
                writeln!(body, "    for (uint64_t i = 0; i < v->len; i++) {}", part).unwrap();
            }
            body.push_str(&self.glue_free(&e, "v->ptr", "v->cap")?);
        } else if n == "std::Box" {
            let e = args[0].clone();
            let part = self.glue_part(&e, "*v->ptr")?;
            let free = self.glue_free(&e, "v->ptr", "1u")?;
            let (size, _) = self.size_align(&e)?;
            writeln!(body, "    if (v->ptr && cb_cells_busy(v->ptr, {}u)) {{ cb_drop_local(p, {}u); return; }}", size.max(1), tid).unwrap();
            body.push_str("    if (v->ptr) {\n");
            if !part.is_empty() {
                writeln!(body, "    {}", part).unwrap();
            }
            body.push_str(&free);
            body.push_str("    }\n");
        } else if let Some(sd) = self.struct_decl(n) {
            let sub = subst_of(&sd.type_params, args);
            for f in sd.fields.iter().rev() {
                let part = self.glue_part(&apply(&f.ty, &sub), &format!("v->{}", san(&f.name)))?;
                if !part.is_empty() {
                    writeln!(body, "    {}", part).unwrap();
                }
            }
        } else if let Some(ed) = self.enum_decl(n) {
            let sub = subst_of(&ed.type_params, args);
            for (i, v) in ed.variants.iter().enumerate() {
                if let Some(p) = &v.payload {
                    let part = self.glue_part(&apply(p, &sub), &format!("v->u.v{}", i))?;
                    if !part.is_empty() {
                        writeln!(body, "    if ({}) {}", self.tag_is(t, "(*v)", i), part).unwrap();
                    }
                }
            }
        }
        writeln!(self.thunks_out, "static void {}(void *p, uint32_t d)\n{{\n{}}}", name, body).unwrap();
        Ok(Some(name))
    }

    // D-0315: the end of a part of type `t` at the C place `place`, for a drop
    // glue: nothing for plain data; its own glue, or the runtime's path.
    fn glue_part(&mut self, t: &Type, place: &str) -> R<String> {
        if !self.is_resource(t) && !self.holds_fn(t) {
            return Ok(String::new());
        }
        match self.drop_glue(t)? {
            Some(g) => Ok(format!("{}((void *)&({}), d + 1u);", g, place)),
            None => {
                let tid = self.type_id(t)?;
                Ok(format!("cb_drop_local((void *)&({}), {}u);", place, tid))
            }
        }
    }

    // D-0315: the block of `n` elements of `e` at `ptr` freed, as `cb_deallocate`
    // frees it (`malloc`'s own alignment: `free` at once).
    fn glue_free(&mut self, e: &Type, ptr: &str, n: &str) -> R<String> {
        let (size, align) = self.size_align(e)?;
        if size == 0 {
            return Ok(String::new());
        }
        Ok(if align <= 16 { format!("    if ({}) free({});\n", n, ptr) } else { format!("    if ({}) cb_deallocate({}, {} * {}u, {}u);\n", n, ptr, n, size, align) })
    }

    // D-0110: the fields (or an enum's tag and payloads) of a struct or
    // enum key type, with their C access and type, for `key_hash_fn` and
    // `key_cmp_fn`: `(None, …)` for a struct's fields, `(Some(i), …)` for
    // variant i's payload.
    pub(super) fn key_parts(&mut self, t: &Type) -> R<(bool, Vec<(Option<usize>, String, Type)>)> {
        let Type::Named(tn, args) = t else { return internal("key parts of a non-aggregate") };
        if let Some(sd) = self.struct_decl(tn) {
            if let Some(b) = sd.bits {
                // D-0118: `[Key-Bytes]` of a bitstruct is its backing integer's.
                return Ok((false, vec![(None, "bits".to_string(), Type::Int(b))]));
            }
            let sub = subst_of(&sd.type_params, args);
            return Ok((false, sd.fields.iter().map(|f| (None, san(&f.name), apply(&f.ty, &sub))).collect()));
        }
        if let Some(ed) = self.enum_decl(tn) {
            let sub = subst_of(&ed.type_params, args);
            return Ok((true, ed.variants.iter().enumerate().filter_map(|(i, v)| v.payload.as_ref().map(|p| (Some(i), format!("u.v{}", i), apply(p, &sub)))).collect()));
        }
        internal(&format!("unknown type `{}`", tn))
    }

    // D-0110: `h` folded over one key part's bytes (`[Key-Bytes]`).
    pub(super) fn key_hash_part(&mut self, t: &Type, e: &str) -> R<String> {
        Ok(match t {
            Type::Int(_) | Type::Bool => format!("h = cb_fnv_more(h, &({e}), sizeof({e}));"),
            Type::Str => format!("{{ uint64_t n = ({e}).n; h = cb_fnv_more(h, &n, 8); h = cb_fnv_more(h, ({e}).p, n); }}"),
            Type::Named(n, _) if n == "std::String" => format!("{{ uint64_t n = ({e}).bytes.len; h = cb_fnv_more(h, &n, 8); h = cb_fnv_more(h, ({e}).bytes.ptr, n); }}"),
            _ => {
                let f = self.key_hash_fn(t)?;
                format!("h = {}(h, &({}));", f, e)
            }
        })
    }

    // D-0110: the key types' order on one part: negative, zero, positive.
    pub(super) fn key_cmp_part(&mut self, t: &Type, a: &str, b: &str) -> R<String> {
        Ok(match t {
            Type::Int(_) | Type::Bool => format!("(({a}) < ({b}) ? -1 : ({a}) > ({b}) ? 1 : 0)"),
            Type::Str => format!("cb_bytes_cmp(({a}).p, ({a}).n, ({b}).p, ({b}).n)"),
            Type::Named(n, _) if n == "std::String" => format!("cb_bytes_cmp(({a}).bytes.ptr, ({a}).bytes.len, ({b}).bytes.ptr, ({b}).bytes.len)"),
            _ => {
                let f = self.key_cmp_fn(t)?;
                format!("{}(&({}), &({}))", f, a, b)
            }
        })
    }

    // D-0110: `cb_kh_…`, a struct or enum key's hash folded into `h`
    // (FNV-1a over `[Key-Bytes]`). A key type cannot contain itself, so
    // the functions for its parts come first.
    pub(super) fn key_hash_fn(&mut self, t: &Type) -> R<String> {
        if let Some(n) = self.khs.get(t) {
            return Ok(n.clone());
        }
        let ct = self.ctype(t)?;
        let name = format!("cb_kh_{}", mangle(t));
        let (is_enum, parts) = self.key_parts(t)?;
        let mut body = String::new();
        if is_enum {
            body.push_str("    uint32_t tag = (uint32_t)k->tag;\n    h = cb_fnv_more(h, &tag, 4);\n    switch (k->tag) {\n");
            for (i, acc, pt) in &parts {
                let st = self.key_hash_part(pt, &format!("k->{}", acc))?;
                writeln!(body, "    case {}u: {} break;", i.unwrap(), st).unwrap();
            }
            body.push_str("    default: break;\n    }\n");
        } else {
            for (_, acc, pt) in &parts {
                let st = self.key_hash_part(pt, &format!("k->{}", acc))?;
                writeln!(body, "    {}", st).unwrap();
            }
        }
        body.push_str("    return h;\n");
        writeln!(self.thunks_out, "uint64_t {}(uint64_t h, const {} *k)\n{{\n{}}}", name, ct, body).unwrap();
        self.khs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // D-0110: `cb_kc_…`, the key types' order on a struct or enum key:
    // field by field in declaration order; an enum by variant, then
    // payload.
    pub(super) fn key_cmp_fn(&mut self, t: &Type) -> R<String> {
        if let Some(n) = self.kcs.get(t) {
            return Ok(n.clone());
        }
        let ct = self.ctype(t)?;
        let name = format!("cb_kc_{}", mangle(t));
        let (is_enum, parts) = self.key_parts(t)?;
        let mut body = String::new();
        if is_enum {
            body.push_str("    if (a->tag != b->tag) return a->tag < b->tag ? -1 : 1;\n    switch (a->tag) {\n");
            for (i, acc, pt) in &parts {
                let e = self.key_cmp_part(pt, &format!("a->{}", acc), &format!("b->{}", acc))?;
                writeln!(body, "    case {}u: return {};", i.unwrap(), e).unwrap();
            }
            body.push_str("    default: return 0;\n    }\n");
        } else {
            for (_, acc, pt) in &parts {
                let e = self.key_cmp_part(pt, &format!("a->{}", acc), &format!("b->{}", acc))?;
                writeln!(body, "    {{ int c = {}; if (c) return c; }}", e).unwrap();
            }
            body.push_str("    return 0;\n");
        }
        writeln!(self.thunks_out, "int {}(const {} *a, const {} *b)\n{{\n{}}}", name, ct, ct, body).unwrap();
        self.kcs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // A per-signature thunk calling a fn value: an item directly, a
    // closure through an exclusive borrow of its boxed object. The fault
    // location (`cb_at`) is the caller's again when the call returns: a
    // program function called from `std` (a comparator, a predicate) sets
    // its own lines, and a fault in `std` after it reports the program's
    // line that called into `std`, as `coby` does.
    // The key of a `fn` type's targets: its thunk's name. `fn` types are
    // nominal, with no conversion between two of them (D-0006), so a box made
    // for one type is only ever called through that type's thunk (CHG-0436).
    pub(super) fn fn_sig_key(&mut self, fty: &Type) -> R<String> {
        if !matches!(fty, Type::Fn(..)) {
            return internal("a target key for a non-fn type");
        }
        Ok(format!("cb_call_{}", mangle(fty)))
    }

    // CHG-0436: a box of `fty` may hold `code`, of `kind` (the box's
    // `closure` field: 0 a fn item, 1 a closure, 2 one capturing nothing, 3
    // one writing its captures).
    pub(super) fn note_fn_target(&mut self, fty: &Type, code: &str, kind: u32) -> R<()> {
        let k = self.fn_sig_key(fty)?;
        self.fn_targets.entry(k).or_default().insert((code.to_string(), kind));
        Ok(())
    }

    pub(super) fn thunk(&mut self, fty: &Type) -> R<String> {
        if let Some(n) = self.thunks.get(fty) {
            return Ok(n.clone());
        }
        let Type::Fn(ps, r) = fty else { return internal("thunk for a non-fn type") };
        let name = format!("cb_call_{}", mangle(fty));
        let rc = self.reg_ctype(r)?;
        let mut params = vec!["void *h".to_string()];
        let mut ptypes = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let pc = self.reg_ctype(p)?;
            params.push(format!("{} a{}", pc, i));
            ptypes.push(pc);
        }
        let key = self.fn_sig_key(fty)?;
        let spec = ThunkSpec { name: name.clone(), key, rc, params, ptypes };
        writeln!(self.thunks_out, "{}", thunk_text(&spec, None)).unwrap();
        self.thunk_specs.push(spec);
        self.thunks.insert(fty.clone(), name.clone());
        Ok(name)
    }

    // CHG-0436: each thunk whose `fn` type has exactly one target code
    // (`fn_targets`, every box the program makes: `box_closure`'s three forms
    // and a fn item's handle) calls that code, through the same cast, with
    // the self argument its kind gives; the null test (a moved-out value) and
    // the location kept around the call stay. The code is now a direct call,
    // which the C compiler may inline.
    pub(super) fn single_target_thunks(&self, text: &str) -> String {
        let mut out = text.to_string();
        for spec in &self.thunk_specs {
            let Some(t) = self.fn_targets.get(&spec.key) else { continue };
            if t.len() != 1 {
                continue;
            }
            let (code, kind) = t.iter().next().unwrap();
            let generic = thunk_text(spec, None);
            let one = thunk_text(spec, Some((code.as_str(), *kind)));
            out = out.replacen(&generic, &one, 1);
        }
        out
    }
}

// A thunk's parts (`Gen::thunk`).
pub(super) struct ThunkSpec {
    pub(super) name: String,
    pub(super) key: String,
    pub(super) rc: String,
    pub(super) params: Vec<String>,
    pub(super) ptypes: Vec<String>,
}

// The two functions of a thunk: `NAME__i`, the call through the box (by its
// kind; or, given its one target, that code), and `NAME`, which keeps the
// caller's location around it.
pub(super) fn thunk_text(t: &ThunkSpec, target: Option<(&str, u32)>) -> String {
    let rc = &t.rc;
    let name = &t.name;
    let args: Vec<String> = (0..t.ptypes.len()).map(|i| format!("a{}", i)).collect();
    let ret = if rc == "void" { "" } else { "return " };
    let item_sig = if t.ptypes.is_empty() { "void".to_string() } else { t.ptypes.join(", ") };
    let closure_sig = std::iter::once("cb_ref".to_string()).chain(t.ptypes.iter().cloned()).collect::<Vec<_>>().join(", ");
    let closure_args = std::iter::once("self".to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(", ");
    let keep = if rc == "void" { String::new() } else { format!("{} r = ", rc) };
    let give = if rc == "void" { "return;" } else { "return r;" };
    let params = t.params.join(", ");
    let inner_args = std::iter::once("h".to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(", ");
    let inner = match target {
        None => format!(
            "static {rc} {name}__i({params})\n{{\n    if (!h) cb_fn_moved();\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    if (b->closure == 2u) {{\n        cb_ref self = {{ b->data, 0 }};\n        {ret}(({rc} (*)({closure_sig}))b->code)({closure_args});\n    }}\n    else if (b->closure) {{\n        cb_ref self = {{ b->data, cb_call_self(b->root) }};\n        {keep}(({rc} (*)({closure_sig}))b->code)({closure_args});\n        cb_call_done(self.tok);\n        {give}\n    }}\n    {ret2}(({rc} (*)({item_sig}))b->code)({item_args});\n}}",
            ret2 = if rc == "void" { "else " } else { "return " },
            item_args = args.join(", ")
        ),
        Some((code, 0)) => format!("static {rc} {name}__i({params})\n{{\n    if (!h) cb_fn_moved();\n    {ret}(({rc} (*)({item_sig})){code})({item_args});\n}}", item_args = args.join(", ")),
        Some((code, 2)) => format!("static {rc} {name}__i({params})\n{{\n    if (!h) cb_fn_moved();\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    cb_ref self = {{ b->data, 0 }};\n    {ret}(({rc} (*)({closure_sig})){code})({closure_args});\n}}"),
        Some((code, _)) => format!("static {rc} {name}__i({params})\n{{\n    if (!h) cb_fn_moved();\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    cb_ref self = {{ b->data, cb_call_self(b->root) }};\n    {keep}(({rc} (*)({closure_sig})){code})({closure_args});\n    cb_call_done(self.tok);\n    {give}\n}}"),
    };
    format!("{inner}\n{rc} {name}({params})\n{{\n    uint32_t wf, wl;\n    cb_where(&wf, &wl);\n    {keep}{name}__i({inner_args});\n    cb_set_where(wf, wl);\n    {give}\n}}")
}
