// C types and layouts: size_align, ctype, type ids.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn struct_decl(&self, name: &str) -> Option<std::sync::Arc<StructDecl>> {
        self.items.structs.get(name).cloned()
    }

    pub(super) fn enum_decl(&self, name: &str) -> Option<std::sync::Arc<EnumDecl>> {
        self.items.enums.get(name).cloned()
    }

    // `rule.type.is-resource`.
    // D-0192: an element type whose values a native `Vec` body may move as
    // bytes: plain data, or a resource holding no reference and no `fn`
    // value (`String`, a `Vec` of plain elements, a user resource of plain
    // fields), whose ownership travels with its bytes and whose object, once
    // moved into a cell, ends (`cb_raw_move_in`).
    pub(super) fn movable_elem(&self, t: &Type) -> bool {
        self.plain_data(t) || (self.is_resource(t) && !self.has_refs(t) && !self.holds_fn(t) && !matches!(t, Type::Ref(..)))
    }

    pub(super) fn is_resource(&self, t: &Type) -> bool {
        match t {
            Type::Closure(id) => self.closures.get(id).map_or(false, |c| c.fields.iter().any(|(_, t)| self.is_resource(t))),
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    sd.resource || sd.fields.iter().any(|f| self.is_resource(&apply(&f.ty, &sub)))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    ed.resource || ed.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| self.is_resource(&apply(p, &sub))))
                } else {
                    false
                }
            }
            Type::Array(inner, _) => self.is_resource(inner),
            Type::Handle(..) | Type::Mutex(..) | Type::Guard(..) => true,
            _ => false,
        }
    }

    // Does a value of this type hold references (slots the runtime tracks)?
    // D-0089: a value of `t` holds a `fn` value somewhere, whose closure
    // it owns: copied or moved slot by slot (`cb_fn_split`), and ended
    // with it.
    pub(super) fn holds_fn(&self, t: &Type) -> bool {
        match t {
            Type::Fn(..) => true,
            Type::Closure(id) => self.closures.get(id).map_or(false, |c| c.fields.iter().any(|(_, t)| self.holds_fn(t))),
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    sd.fields.iter().any(|f| self.holds_fn(&apply(&f.ty, &sub)))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    ed.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| self.holds_fn(&apply(p, &sub))))
                } else {
                    false
                }
            }
            Type::Array(inner, _) | Type::Mutex(inner) => self.holds_fn(inner),
            _ => false,
        }
    }

    pub(super) fn has_refs(&self, t: &Type) -> bool {
        match t {
            Type::Ref(..) | Type::Slice(..) => true,
            Type::Closure(id) => self.closures.get(id).map_or(false, |c| c.fields.iter().any(|(_, t)| self.has_refs(t))),
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    sd.fields.iter().any(|f| self.has_refs(&apply(&f.ty, &sub)))
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    ed.variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| self.has_refs(&apply(p, &sub))))
                } else {
                    false
                }
            }
            Type::Array(inner, _) => self.has_refs(inner),
            Type::Guard(..) => true,
            Type::Mutex(inner) => self.has_refs(inner),
            _ => false,
        }
    }

    // `[Sizeof-*]` and `rule.agg.layout`, recomputed from the spec.
    pub(super) fn size_align(&self, t: &Type) -> R<(u64, u64)> {
        Ok(match t {
            Type::Int(i) => {
                let n = (i.bitwidth(64) / 8) as u64;
                (n, n)
            }
            Type::F32 => (4, 4),
            Type::F64 => (8, 8),
            Type::Bool => (1, 1),
            Type::Void | Type::Never => (0, 1),
            Type::Str => (16, 8),
            Type::Ref(..) | Type::Rawptr(..) | Type::Fn(..) | Type::Handle(..) | Type::Guard(..) => (8, 8),
            Type::Slice(..) => (24, 8),
            // `[Sizeof-Mutex]`: the layout of `struct { τ inner; usize state; }`.
            Type::Mutex(inner) => {
                let (s, a) = self.size_align(inner)?;
                let align = a.max(8);
                (round_up(round_up(s, 8) + 8, align), align)
            }
            Type::Array(inner, n) => {
                let (s, a) = self.size_align(inner)?;
                (s * (*n as u64), a)
            }
            // D-0254 `[Layout-Simd]`: aligned to its whole size.
            Type::Simd(inner, n) => {
                let (s, _) = self.size_align(inner)?;
                (s * (*n as u64), s * (*n as u64))
            }
            Type::Named(name, args) => {
                if let Some(sd) = self.struct_decl(name) {
                    if let Some(b) = sd.bits {
                        return self.size_align(&Type::Int(b));
                    }
                    let sub = subst_of(&sd.type_params, args);
                    // D-0251: `align(N)` raises the struct's alignment.
                    let (mut off, mut align) = (0u64, sd.align.unwrap_or(1));
                    for f in &sd.fields {
                        let (s, a) = self.size_align(&apply(&f.ty, &sub))?;
                        off = round_up(off, a);
                        off += s;
                        align = align.max(a);
                    }
                    (round_up(off, align), align)
                } else if let Some(ed) = self.enum_decl(name) {
                    let sub = subst_of(&ed.type_params, args);
                    let (mut ps, mut pa) = (0u64, 1u64);
                    for v in &ed.variants {
                        if let Some(p) = &v.payload {
                            let (s, a) = self.size_align(&apply(p, &sub))?;
                            ps = ps.max(s);
                            pa = pa.max(a);
                        }
                    }
                    let dw = 4u64;
                    let payload_off = round_up(dw, pa);
                    let align = dw.max(pa);
                    (round_up(payload_off + ps, align), align)
                } else {
                    return internal(&format!("unknown type `{}`", name));
                }
            }
            Type::Closure(id) => {
                let fields = self.closures.get(id).map(|c| c.fields.clone()).unwrap_or_default();
                let (mut off, mut align) = (0u64, 1u64);
                for (_, fty) in &fields {
                    let (s, a) = self.size_align(fty)?;
                    off = round_up(off, a);
                    off += s;
                    align = align.max(a);
                }
                (round_up(off, align), align)
            }
        })
    }

    // The C spelling of a fully substituted type as stored in memory,
    // emitting its definition (fields first) on first use.
    pub(super) fn ctype(&mut self, t: &Type) -> R<String> {
        Ok(match t {
            Type::Int(i) => c_int_type(*i).to_string(),
            Type::F32 => "float".to_string(),
            Type::F64 => "double".to_string(),
            Type::Bool => "uint8_t".to_string(),
            Type::Void | Type::Never => "cb_unit".to_string(),
            Type::Str => "cb_str".to_string(),
            Type::Ref(..) => "void *".to_string(),
            // A slice (D-0047): the reference slot that holds the borrow of
            // its source, a pointer to its first element, its length.
            Type::Slice(inner, _) => {
                let name = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    let ic = self.ctype(inner)?;
                    writeln!(self.types, "struct {} {{ void * src; {} * data; uint64_t len; }};", name, ic).unwrap();
                    self.assert_layout(t, &name)?;
                }
                format!("struct {}", name)
            }
            Type::Rawptr(inner) => {
                let inner_c = match &**inner {
                    Type::Named(..) => format!("struct {}", self.forward_name(inner)?),
                    other => self.ctype(other)?,
                };
                format!("{} *", inner_c)
            }
            Type::Fn(..) => "void *".to_string(),
            // `[Repr-Handle]`: the thread id. `[Repr-Guard]`: as a reference,
            // the address of the mutex's interior.
            Type::Handle(..) => "uint64_t".to_string(),
            Type::Guard(..) => "void *".to_string(),
            // `[Sizeof-Mutex]`: `struct { τ inner; usize state; }`.
            Type::Mutex(inner) => {
                let name = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    let ic = self.ctype(inner)?;
                    writeln!(self.types, "struct {} {{ {} inner; uint64_t state; }};", name, ic).unwrap();
                    self.assert_layout(t, &name)?;
                }
                format!("struct {}", name)
            }
            Type::Closure(id) => {
                let name = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    let fields = self.closures.get(id).map(|c| c.fields.clone()).unwrap_or_default();
                    let mut body = String::new();
                    for (i, (_, fty)) in fields.iter().enumerate() {
                        let fc = self.ctype(fty)?;
                        write!(body, " {} f{};", fc, i).unwrap();
                    }
                    writeln!(self.types, "struct {} {{{} }};", name, body).unwrap();
                    self.assert_layout(t, &name)?;
                }
                format!("struct {}", name)
            }
            Type::Array(inner, n) => {
                let name = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    let ic = self.ctype(inner)?;
                    writeln!(self.types, "struct {} {{ {} a[{}]; }};", name, ic, n).unwrap();
                    self.assert_layout(t, &name)?;
                }
                format!("struct {}", name)
            }
            // D-0254: a GCC/Clang vector type, and for integer lanes its
            // unsigned twin (`cb_u…`), in which `cobc` does the arithmetic so
            // that a wrap is never undefined behaviour in C.
            Type::Simd(inner, n) => {
                let name = format!("cbv_{}", mangle(t));
                if self.defined.insert(t.clone()) {
                    let ic = self.ctype(inner)?;
                    let (s, _) = self.size_align(inner)?;
                    let bytes = s * (*n as u64);
                    writeln!(self.types, "typedef {} {} __attribute__((vector_size({})));", ic, name, bytes).unwrap();
                    if let Type::Int(it) = &**inner {
                        let ut = c_uint_type(*it);
                        writeln!(self.types, "typedef {} cbvu_{} __attribute__((vector_size({})));", ut, mangle(t), bytes).unwrap();
                    }
                }
                name
            }
            Type::Named(name, args) => {
                let cname = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    if let Some(sd) = self.struct_decl(name) {
                        if let Some(b) = sd.bits {
                            // D-0118: a bitstruct is its backing integer.
                            writeln!(self.types, "struct {} {{ {} bits; }};", cname, c_int_type(b)).unwrap();
                        } else {
                            let sub = subst_of(&sd.type_params, args);
                            let mut body = String::new();
                            for f in &sd.fields {
                                let fc = self.ctype(&apply(&f.ty, &sub))?;
                                write!(body, " {} {};", fc, san(&f.name)).unwrap();
                            }
                            // D-0251: GCC's and Clang's spelling of `alignas` on a type.
                            let al = sd.align.filter(|&a| a > 1).map_or(String::new(), |a| format!(" __attribute__((aligned({})))", a));
                            writeln!(self.types, "struct {} {{{} }}{};", cname, body, al).unwrap();
                        }
                    } else if let Some(ed) = self.enum_decl(name) {
                        let sub = subst_of(&ed.type_params, args);
                        let mut body = String::new();
                        for (i, v) in ed.variants.iter().enumerate() {
                            if let Some(p) = &v.payload {
                                let pc = self.ctype(&apply(p, &sub))?;
                                write!(body, " {} v{};", pc, i).unwrap();
                            }
                        }
                        if body.is_empty() {
                            writeln!(self.types, "struct {} {{ uint32_t tag; }};", cname).unwrap();
                        } else {
                            writeln!(self.types, "struct {} {{ uint32_t tag; union {{{} }} u; }};", cname, body).unwrap();
                        }
                    } else {
                        return internal(&format!("unknown type `{}`", name));
                    }
                    self.assert_layout(t, &cname)?;
                }
                format!("struct {}", cname)
            }
        })
    }

    pub(super) fn forward_name(&mut self, t: &Type) -> R<String> {
        if let Some(n) = self.type_names.get(t) {
            return Ok(n.clone());
        }
        let prefix = match t {
            Type::Array(..) => "a_",
            Type::Named(n, _) if self.items.enums.contains_key(n) => "e_",
            _ => "s_",
        };
        let name = format!("{}{}", prefix, mangle(t));
        writeln!(self.fwd, "struct {};", name).unwrap();
        self.type_names.insert(t.clone(), name.clone());
        Ok(name)
    }

    pub(super) fn assert_layout(&mut self, t: &Type, cname: &str) -> R<()> {
        let (s, a) = self.size_align(t)?;
        writeln!(
            self.types,
            "_Static_assert(sizeof(struct {0}) == {1} && _Alignof(struct {0}) == {2}, \"layout of {0}\");",
            cname, s, a
        )
        .unwrap();
        Ok(())
    }

    // The runtime type descriptor's index for a type (emitting the
    // descriptor and its C type on first use).
    pub(super) fn type_id(&mut self, t: &Type) -> R<u32> {
        if let Some(id) = self.type_ids.get(t) {
            return Ok(*id);
        }
        let name_for_id = mangle(t);
        let mut id = fnv1a32(name_for_id.as_bytes());
        while id == 0 || id >= 0xFFFF_FFF0 || self.type_pos.contains_key(&id) {
            id = id.wrapping_add(1);
        }
        self.type_pos.insert(id, self.descs.len());
        self.descs.push(None);
        self.desc_ids.push(id);
        self.type_ids.insert(t.clone(), id);
        let cty = self.ctype(t)?;
        let is_res = self.is_resource(t) as u8;
        let has_refs = self.has_refs(t) as u8;
        let size = if matches!(t, Type::Void | Type::Never) { "0".to_string() } else { format!("sizeof({})", cty) };
        let name = c_escape(&mangle(t));
        let desc = match t {
            Type::Ref(..) => format!("{{ \"{}\", {}, CB_K_REF, {}, {}, 0, 0, 0, 0, 0, 0, 0, 0 }}", name, size, is_res, has_refs),
            // A slice: a struct whose one tracked field is its reference slot.
            Type::Slice(inner, m) => {
                let rid = self.type_id(&Type::Ref(inner.clone(), m.clone()))?;
                let farr = format!("cb_f_{}", id);
                writeln!(self.desc_aux, "static const cb_field {}[] = {{ {{ offsetof({}, src), {}u }}, {{0, 0}} }};", farr, cty, rid).unwrap();
                format!("{{ \"{}\", {}, CB_K_STRUCT, 0, 1, 0, 1u, {}, 0, 0, 0, 0, 0 }}", name, size, farr)
            }
            Type::Fn(..) => format!("{{ \"{}\", {}, CB_K_FN, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }}", name, size),
            Type::Handle(..) => format!("{{ \"{}\", {}, CB_K_HANDLE, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0 }}", name, size),
            Type::Guard(..) => format!("{{ \"{}\", {}, CB_K_GUARD, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0 }}", name, size),
            // A mutex is destroyed as the struct it is laid out as: its
            // `inner` is destroyed (`destroy-composite`).
            Type::Mutex(inner) => {
                let fid = self.type_id(inner)?;
                let farr = format!("cb_f_{}", id);
                writeln!(self.desc_aux, "static const cb_field {}[] = {{ {{ offsetof({}, inner), {}u }}, {{0, 0}} }};", farr, cty, fid).unwrap();
                format!("{{ \"{}\", {}, CB_K_STRUCT, 1, {}, 0, 1u, {}, 0, 0, 0, 0, 0, 1 }}", name, size, has_refs, farr)
            }
            Type::Closure(id) => {
                let fields = self.closures.get(id).map(|c| c.fields.clone()).unwrap_or_default();
                let mut fs = String::new();
                for (i, (_, fty)) in fields.iter().enumerate() {
                    let fid = self.type_id(fty)?;
                    write!(fs, "{{ offsetof({}, f{}), {}u }}, ", cty, i, fid).unwrap();
                }
                let farr = format!("cb_f_{}", id_of(self, t));
                writeln!(self.desc_aux, "static const cb_field {}[] = {{ {}{{0, 0}} }};", farr, fs).unwrap();
                format!("{{ \"{}\", {}, CB_K_STRUCT, {}, {}, 0, {}u, {}, 0, 0, 0, 0, 0, 1 }}", name, size, is_res, has_refs, fields.len(), farr)
            }
            Type::Array(inner, n) => {
                let eid = self.type_id(inner)?;
                format!("{{ \"{}\", {}, CB_K_ARRAY, {}, {}, 0, 0, 0, 0, 0, 0, {}u, {}u }}", name, size, is_res, has_refs, eid, n)
            }
            Type::Named(n, args) => {
                if let Some(sd) = self.struct_decl(n) {
                    let sub = subst_of(&sd.type_params, args);
                    let mut fields = String::new();
                    // D-0118: a bitstruct's fields are bits of one integer,
                    // nothing the runtime tracks.
                    let tracked: Vec<&FieldDecl> = if sd.bits.is_some() { Vec::new() } else { sd.fields.iter().collect() };
                    for f in &tracked {
                        let fid = self.type_id(&apply(&f.ty, &sub))?;
                        write!(fields, "{{ offsetof({}, {}), {}u }}, ", cty, san(&f.name), fid).unwrap();
                    }
                    let farr = format!("cb_f_{}", id);
                    writeln!(self.desc_aux, "static const cb_field {}[] = {{ {}{{0, 0}} }};", farr, fields).unwrap();
                    let drop = {
                        let dname = format!("{}::drop", n);
                        if self.items.fns.contains_key(&dname) {
                            self.request_fn(&dname, args.clone())?
                        } else {
                            "0".to_string()
                        }
                    };
                    // A Box's `elem` is its referent's id + 1: the runtime
                    // destroys it natively (a Box chain in constant stack).
                    let box_mark = if n == "std::Box" && args.len() == 1 { self.type_id(&args[0])? + 1 } else { 0 };
                    // A `Vec<T>` whose `drop` is the native one for a plain,
                    // non-zero-sized `T` (`native_vec_drop`): its `count` is
                    // `sizeof(T)` + 1, and the runtime destroys it as that
                    // body does, without the call (`drop_in_place_tail`).
                    let vec_mark = match args.as_slice() {
                        [elem] if n == "std::Vec" && self.plain_data(elem) && self.size_align(elem)?.0 > 0 => self.size_align(elem)?.0 + 1,
                        _ => 0,
                    };
                    format!(
                        "{{ \"{}\", {}, CB_K_STRUCT, {}, {}, {}, {}u, {}, 0, 0, 0, {}u, {}u, {} }}",
                        name,
                        size,
                        is_res,
                        has_refs,
                        drop,
                        if sd.bits.is_some() { 0 } else { sd.fields.len() },
                        farr,
                        box_mark,
                        vec_mark,
                        coby::typecheck::type_is_owner(&self.items, n) as u8
                    )
                } else if let Some(ed) = self.enum_decl(n) {
                    let sub = subst_of(&ed.type_params, args);
                    let mut vars = String::new();
                    let mut any_payload = false;
                    for v in &ed.variants {
                        match &v.payload {
                            Some(p) => {
                                any_payload = true;
                                let pid = self.type_id(&apply(p, &sub))?;
                                write!(vars, "{}u, ", pid).unwrap();
                            }
                            None => vars.push_str("CB_NOTYPE, "),
                        }
                    }
                    let varr = format!("cb_v_{}", id);
                    writeln!(self.desc_aux, "static const uint32_t {}[] = {{ {}0 }};", varr, vars).unwrap();
                    let poff = if any_payload { format!("offsetof({}, u)", cty) } else { "0".to_string() };
                    format!(
                        "{{ \"{}\", {}, CB_K_ENUM, {}, {}, 0, 0, 0, {}u, {}, {}, 0, 0, {} }}",
                        name,
                        size,
                        is_res,
                        has_refs,
                        ed.variants.len(),
                        varr,
                        poff,
                        coby::typecheck::type_is_owner(&self.items, n) as u8
                    )
                } else {
                    return internal(&format!("unknown type `{}`", n));
                }
            }
            _ => format!("{{ \"{}\", {}, CB_K_SCALAR, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }}", name, size),
        };
        let pos = self.type_pos[&id];
        self.descs[pos] = Some(desc);
        Ok(id)
    }

    // The C type of a parameter: a register's, except that a parameter
    // of type `void` (a generic function instantiated at `T = void`,
    // `Result::unwrap_or(r, ())`) is the unit value `cb_unit`, since C
    // has no `void` parameter.
    pub(super) fn param_ctype(&mut self, t: &Type) -> R<String> {
        match t {
            Type::Void => Ok("cb_unit".to_string()),
            other => self.reg_ctype(other),
        }
    }

    // The C type of a value in a register: parameters, results, temps.
    pub(super) fn reg_ctype(&mut self, t: &Type) -> R<String> {
        Ok(match t {
            Type::Void | Type::Never => "void".to_string(),
            Type::Ref(..) => "cb_ref".to_string(),
            other => self.ctype(other)?,
        })
    }
}
