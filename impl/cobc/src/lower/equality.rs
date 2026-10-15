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
    pub(super) fn thunk(&mut self, fty: &Type) -> R<String> {
        if let Some(n) = self.thunks.get(fty) {
            return Ok(n.clone());
        }
        let Type::Fn(ps, r) = fty else { return internal("thunk for a non-fn type") };
        let name = format!("cb_call_{}", mangle(fty));
        let rc = self.reg_ctype(r)?;
        let mut params = vec!["void *h".to_string()];
        let mut args = Vec::new();
        let mut ptypes = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let pc = self.reg_ctype(p)?;
            params.push(format!("{} a{}", pc, i));
            args.push(format!("a{}", i));
            ptypes.push(pc);
        }
        let ret = if rc == "void" { "" } else { "return " };
        let item_sig = if ptypes.is_empty() { "void".to_string() } else { ptypes.join(", ") };
        let closure_sig = std::iter::once("cb_ref".to_string()).chain(ptypes.iter().cloned()).collect::<Vec<_>>().join(", ");
        let closure_args = std::iter::once("self".to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(", ");
        writeln!(
            self.thunks_out,
            "static {rc} {name}__i({params})\n{{\n    if (!h) cb_fn_moved();\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    if (b->closure == 2u) {{\n        cb_ref self = {{ b->data, 0 }};\n        {ret}(({rc} (*)({closure_sig}))b->code)({closure_args});\n    }}\n    else if (b->closure) {{\n        cb_ref self = {{ b->data, cb_call_self(b->root) }};\n        {keep}(({rc} (*)({closure_sig}))b->code)({closure_args});\n        cb_call_done(self.tok);\n        {give}\n    }}\n    {ret2}(({rc} (*)({item_sig}))b->code)({item_args});\n}}\n{rc} {name}({params})\n{{\n    uint32_t wf, wl;\n    cb_where(&wf, &wl);\n    {keep}{name}__i({inner_args});\n    cb_set_where(wf, wl);\n    {give}\n}}",
            rc = rc,
            name = name,
            params = params.join(", "),
            inner_args = std::iter::once("h".to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(", "),
            ret = ret,
            closure_sig = closure_sig,
            closure_args = closure_args,
            ret2 = if rc == "void" { "else " } else { "return " },
            keep = if rc == "void" { String::new() } else { format!("{} r = ", rc) },
            give = if rc == "void" { "return;" } else { "return r;" },
            item_sig = item_sig,
            item_args = args.join(", ")
        )
        .unwrap();
        self.thunks.insert(fty.clone(), name.clone());
        Ok(name)
    }
}
