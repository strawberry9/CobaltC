// Typed lowering of an accepted program to C (impl/COBC-PLAN.md §3).
//
// One recursive walk computes each expression's type and emits C for
// it, in three-address form: every intermediate value is a named C
// local. The program was accepted by `typecheck`, so nothing here
// rejects; a type this walk cannot determine is an internal error, and
// a construct this stage does not compile yet is `Unsupported` (exit 3
// at the driver).
//
// Every binding is a stack local *and* a runtime object with a root
// access path; every place access names its path (a binding's root, or
// the token of the reference it goes through) so the runtime can run
// `[Read]`/`[Write]`/`[Borrow]`'s checks. A reference is fat in
// registers (`cb_ref`: pointer + token) and thin in memory (the pointer;
// the token in the runtime's slot table). A resource-typed value always
// travels with its object id: moved into a binding (`cb_move_to` +
// `cb_bind`), absorbed into a container (`cb_absorb`), or sent across
// a call (`cb_send`/`cb_recv`). Temporaries that hold references or
// resources are registered so a statement's exit can end them.
//
// Layouts: C's struct layout for the same scalars *is* `[Layout-Struct]`
// and `struct { uint32_t tag; union {…} u; }` *is* `[Layout-Enum]` with
// `DW = 4`; `size_align` recomputes both from the spec's rules and the
// emitted C asserts they agree.

use coby::ast::*;
use coby::interp::Items;
use coby::loader::SourceMap;
use coby::prelude;
use coby::value::int_min_max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

pub struct Unsupported(pub String);
type R<T> = Result<T, Unsupported>;

fn unsupported<T>(what: &str) -> R<T> {
    Err(Unsupported(what.to_string()))
}

fn internal<T>(what: &str) -> R<T> {
    Err(Unsupported(format!("internal error in cobc: {}", what)))
}

const HEADER: &str = include_str!("../../cbrt/include/cbrt.h");
const NOLOC: &str = "CB_NOLOC, 0";

// A value: a C expression of the value's register representation
// (`cb_ref` for a reference), its type, and — when the value lives in
// a registered temporary — the C local holding that object's id.
#[derive(Clone)]
struct V {
    c: String,
    ty: Type,
    obj: Option<String>,
}

// A place: a C lvalue, its type, the token of the path it is reached
// through (`None` for a temporary or a raw pointer: unchecked), the
// projection from that path, and whether it is a binding itself.
#[derive(Clone)]
struct P {
    c: String,
    ty: Type,
    base: Option<String>,
    projs: Vec<String>,
    is_binding: bool,
    // The pointer expression when this place is `*p` on a raw pointer:
    // reads and writes are trusted, moves relocate through it.
    raw: Option<String>,
}

#[derive(Clone)]
enum BindKind {
    Local,
    // A borrow-captured name inside a closure body: `*self.f_i`.
    CaptureRef(usize),
    // A move-captured name: `self.f_i`.
    CaptureVal(usize),
}

#[derive(Clone)]
struct Binding {
    c: String,
    ty: Type,
    root: String,
    kind: BindKind,
}

#[derive(Clone)]
struct ClosureInfo {
    fields: Vec<(String, Type)>,
    is_move: bool,
    params: Vec<Type>,
    ret: Type,
    cname: String,
}

fn unit() -> V {
    V { c: "(cb_unit){}".to_string(), ty: Type::Void, obj: None }
}

fn never() -> V {
    V { c: "(cb_unit){}".to_string(), ty: Type::Never, obj: None }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cleanup {
    Frame(u32),
    Stmt(u32),
}

pub struct Gen<'a> {
    items: &'a Items,
    map: &'a SourceMap,
    files: Vec<String>,
    file_ids: HashMap<String, u32>,
    fwd: String,
    types: String,
    type_names: HashMap<Type, String>,
    defined: HashSet<Type>,
    type_ids: HashMap<Type, u32>,
    descs: Vec<Option<String>>,
    desc_aux: String,
    strings: String,
    nstr: u32,
    protos: String,
    fns: String,
    fn_names: HashMap<(String, Vec<Type>), String>,
    queue: VecDeque<(String, Vec<Type>, u64)>,
    uid: u32,
    closures: HashMap<u64, ClosureInfo>,
    thunks: HashMap<Type, String>,
    thunks_out: String,
    eqs: HashMap<Type, String>,
    externs_out: HashMap<String, String>,   // an extern's symbol -> its C-side name
    // While probing (lowering into a discarded buffer for a type), no
    // function body may be emitted.
    probing: u32,
    // Ids for statement scopes and frames (`resolve_scopes`).
    nscope: u32,
    // For each program function, which parameters are *confining*
    // (`confining_table`), and the C names of the variants compiled for
    // confined arguments, by the mask of those parameters.
    confining: HashMap<String, Vec<bool>>,
    variants: HashMap<(String, Vec<Type>, u64), String>,
}

impl<'a> Gen<'a> {
    pub fn new(items: &'a Items, map: &'a SourceMap) -> Self {
        Gen {
            items,
            map,
            files: Vec::new(),
            file_ids: HashMap::new(),
            fwd: String::new(),
            types: String::new(),
            type_names: HashMap::new(),
            defined: HashSet::new(),
            type_ids: HashMap::new(),
            descs: Vec::new(),
            desc_aux: String::new(),
            strings: String::new(),
            nstr: 0,
            protos: String::new(),
            fns: String::new(),
            fn_names: HashMap::new(),
            queue: VecDeque::new(),
            uid: 0,
            closures: HashMap::new(),
            thunks: HashMap::new(),
            thunks_out: String::new(),
            eqs: HashMap::new(),
            externs_out: HashMap::new(),
            probing: 0,
            nscope: 0,
            confining: HashMap::new(),
            variants: HashMap::new(),
        }
    }

    pub fn program(mut self) -> R<String> {
        self.confining = self.confining_table();
        let main_c = self.request_fn("main", vec![])?;
        let main_u8 = self.items.fns.get("main").map_or(false, |m| m.ret != Type::Void);
        while let Some((name, targs, mask)) = self.queue.pop_front() {
            self.lower_fn(&name, &targs, mask)?;
        }
        let mut out = String::new();
        out.push_str(HEADER);
        out.push_str("\n/* ---- types ---- */\n");
        out.push_str(&self.fwd);
        out.push_str(&self.types);
        out.push_str("\n/* ---- string literals ---- */\n");
        out.push_str(&self.strings);
        out.push_str("\n/* ---- functions ---- */\n");
        out.push_str(&self.protos);
        out.push_str("\n/* ---- call thunks for fn values ---- */\n");
        out.push_str(&self.thunks_out);
        out.push_str("\n/* ---- type descriptors ---- */\n");
        out.push_str(&self.desc_aux);
        out.push_str("static const cb_type cb_types[] = {\n");
        for d in &self.descs {
            writeln!(out, "    {},", d.as_deref().unwrap_or("{0}")).unwrap();
        }
        out.push_str("    {0}\n};\n");
        out.push_str(&self.fns);
        out.push_str("\n__thread uint32_t cb_at_file = CB_NOLOC, cb_at_line = 0;\n");
        out.push_str("void cb_where(uint32_t *file, uint32_t *line) { *file = cb_at_file; *line = cb_at_line; }\n");
        out.push_str("void cb_set_where(uint32_t file, uint32_t line) { cb_at_file = file; cb_at_line = line; }\n");
        out.push_str("\nstatic const char *const cb_files[] = {");
        for f in &self.files {
            write!(out, "\"{}\", ", c_escape(f)).unwrap();
        }
        out.push_str("0};\n");
        writeln!(
            out,
            "int main(int argc, char **argv)\n{{\n    cb_init(cb_files, {}, cb_types, {});\n    cb_set_args(argc, argv);\n    cb_frame_push();\n    {}\n    cb_frame_pop();\n    cb_terminate_ok({});\n}}",
            self.files.len(),
            self.descs.len(),
            // `ok(s)`: `fn main() : u8`'s value, or 0 (`rule.fn.program`).
            if main_u8 { format!("uint8_t cb_status = {}();", main_c) } else { format!("{}();", main_c) },
            if main_u8 { "cb_status" } else { "0" }
        )
        .unwrap();
        Ok(out)
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.uid += 1;
        format!("{}{}", prefix, self.uid)
    }

    fn loc(&mut self, line: usize) -> String {
        let user = line.checked_sub(prelude::line_count()).filter(|&n| n >= 1);
        match user.and_then(|n| self.map.resolve(n)) {
            Some((file, l)) => {
                let id = match self.file_ids.get(file) {
                    Some(id) => *id,
                    None => {
                        let id = self.files.len() as u32;
                        self.files.push(file.to_string());
                        self.file_ids.insert(file.to_string(), id);
                        id
                    }
                };
                format!("{}u, {}u", id, l)
            }
            None => NOLOC.to_string(),
        }
    }

    fn request_fn(&mut self, name: &str, targs: Vec<Type>) -> R<String> {
        let key = (name.to_string(), targs.clone());
        if let Some(c) = self.fn_names.get(&key) {
            return Ok(c.clone());
        }
        if !self.items.fns.contains_key(name) {
            return internal(&format!("no function `{}`", name));
        }
        let c = fn_cname(name, &targs);
        self.fn_names.insert(key.clone(), c.clone());
        self.queue.push_back((key.0, key.1, 0));
        Ok(c)
    }

    // The variant of a function compiled for confined arguments in the
    // parameters `mask` names (`Fx::bind_confined`, `confining_table`).
    fn request_variant(&mut self, name: &str, targs: Vec<Type>, mask: u64) -> R<String> {
        if mask == 0 {
            return self.request_fn(name, targs);
        }
        let key = (name.to_string(), targs, mask);
        if let Some(c) = self.variants.get(&key) {
            return Ok(c.clone());
        }
        let c = format!("{}_c{}", fn_cname(name, &key.1), mask);
        self.variants.insert(key.clone(), c.clone());
        self.queue.push_back(key);
        Ok(c)
    }

    fn struct_decl(&self, name: &str) -> Option<std::sync::Arc<StructDecl>> {
        self.items.structs.get(name).cloned()
    }

    fn enum_decl(&self, name: &str) -> Option<std::sync::Arc<EnumDecl>> {
        self.items.enums.get(name).cloned()
    }

    // `rule.type.is-resource`.
    fn is_resource(&self, t: &Type) -> bool {
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
    fn has_refs(&self, t: &Type) -> bool {
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
    fn size_align(&self, t: &Type) -> R<(u64, u64)> {
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
            Type::Named(name, args) => {
                if let Some(sd) = self.struct_decl(name) {
                    let sub = subst_of(&sd.type_params, args);
                    let (mut off, mut align) = (0u64, 1u64);
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
    fn ctype(&mut self, t: &Type) -> R<String> {
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
            Type::Named(name, args) => {
                let cname = self.forward_name(t)?;
                if self.defined.insert(t.clone()) {
                    if let Some(sd) = self.struct_decl(name) {
                        let sub = subst_of(&sd.type_params, args);
                        let mut body = String::new();
                        for f in &sd.fields {
                            let fc = self.ctype(&apply(&f.ty, &sub))?;
                            write!(body, " {} {};", fc, san(&f.name)).unwrap();
                        }
                        writeln!(self.types, "struct {} {{{} }};", cname, body).unwrap();
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

    fn forward_name(&mut self, t: &Type) -> R<String> {
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

    fn assert_layout(&mut self, t: &Type, cname: &str) -> R<()> {
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
    fn type_id(&mut self, t: &Type) -> R<u32> {
        if let Some(id) = self.type_ids.get(t) {
            return Ok(*id);
        }
        let id = self.descs.len() as u32;
        self.descs.push(None);
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
                    for f in &sd.fields {
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
                    format!(
                        "{{ \"{}\", {}, CB_K_STRUCT, {}, {}, {}, {}u, {}, 0, 0, 0, 0, 0, {} }}",
                        name,
                        size,
                        is_res,
                        has_refs,
                        drop,
                        sd.fields.len(),
                        farr,
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
        self.descs[id as usize] = Some(desc);
        Ok(id)
    }

    fn lower_fn(&mut self, name: &str, targs: &[Type], mask: u64) -> R<()> {
        let f = self.items.fns.get(name).cloned().expect("requested fn exists");
        let cname = if mask == 0 {
            self.fn_names[&(name.to_string(), targs.to_vec())].clone()
        } else {
            self.variants[&(name.to_string(), targs.to_vec(), mask)].clone()
        };
        let subst = subst_of(&f.type_params, targs);
        let ret = apply(&f.ret, &subst);
        let ret_c = self.reg_ctype(&ret)?;
        let mut params_c = Vec::new();
        for p in &f.params {
            let pty = apply(&p.ty, &subst);
            let pc = self.param_ctype(&pty)?;
            params_c.push(format!("{} p_{}", pc, san(&p.name)));
        }
        let sig = format!("{} {}({})", ret_c, cname, if params_c.is_empty() { "void".to_string() } else { params_c.join(", ") });
        writeln!(self.protos, "{};", sig).unwrap();

        if mask == 0 {
            if let Some(body) = self.native_vec_index(name, &f, targs, &cname)? {
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, body).unwrap();
                return Ok(());
            }
        }

        let pinned = pinned_names(&f.body);
        let confined_names = self.confined_vecs(&f.body);
        let mut fx = Fx { g: self, subst, scopes: vec![HashMap::new()], out: String::new(), ret: ret.clone(), cleanup: Vec::new(), loops: Vec::new(), ind: 1, self_ref: None, assigned: assigned_names(&f.body), rebind_frames: HashMap::new(), no_overflow: HashSet::new(), proven_next: false, pinned, confined_names, confined: HashSet::new(), confined_params: HashMap::new(), probes: HashMap::new(), kept: Vec::new(), then_exprs: HashMap::new(), dollar: Vec::new(), place_write: false, vec_index_exprs: HashMap::new() };
        fx.push_frame();
        for (j, p) in f.params.iter().enumerate() {
            if mask & (1 << j) != 0 {
                // A confined argument (`Fx::bind_confined`): no binding
                // object; its uses reach the vector through `p.p`.
                let pty = fx.sub(&p.ty);
                let Type::Ref(inner, _) = &pty else { return internal("confined parameter of a non-reference type") };
                let Type::Named(_, a) = &**inner else { return internal("confined parameter of a non-Vec type") };
                let vec_c = fx.g.ctype(inner)?;
                let pc = format!("p_{}", san(&p.name));
                fx.confined_params.insert(p.name.clone(), (format!("(*({} *){}.p)", vec_c, pc), format!("{}.tok", pc), a[0].clone()));
            } else {
                fx.bind_param(p)?;
            }
        }
        let v = fx.lower_block_body(&f.body, Some(&ret), None)?;
        if matches!(v.ty, Type::Never) {
            fx.emit("__builtin_unreachable();");
        } else {
            fx.emit_return(v)?;
        }
        while !fx.cleanup.is_empty() {
            fx.end_diverged();
        }
        let body = drop_dead_locations(&resolve_scopes(&fx.out));
        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, body).unwrap();
        Ok(())
    }

    // `Vec::index_shared`/`Vec::index_exclusive` realized natively, as
    // `spec/21` §0 permits ("an implementation may realize them
    // differently provided every conformance case holds"; D-0023): the
    // body's checked read of `v.len`, its bounds fault, then one runtime
    // call for `[Reclaim]` + `[Borrow]` + the return (`cb_elem_borrow`).
    // Only the prelude's own declaration qualifies — a program may
    // declare a function of the same name, and that one is lowered as
    // written.
    fn native_vec_index(&mut self, name: &str, f: &FnDecl, targs: &[Type], cname: &str) -> R<Option<String>> {
        let mode = match name {
            "std::Vec::index_shared" => "CB_SHARED",
            "std::Vec::index_exclusive" => "CB_EXCLUSIVE",
            "std::Vec::drop" | "std::Vec::push" | "std::Vec::len" => "",
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
        if name == "std::Vec::push" {
            let b = self.native_vec_push(elem, &vec_c)?;
            if b.is_some() {
                // For a confined vector (`Fx::bind_confined`): no checks.
                let grow = self.request_fn("std::Vec::grow", vec![elem.clone()])?;
                let elem_c = self.reg_ctype(elem)?;
                let sig = format!("static inline void {}_nc(cb_ref p_v, {} p_x)", cname, elem_c);
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
        writeln!(b, "    {} *e = v->ptr + (int64_t)p_i;", elem_c).unwrap();
        writeln!(b, "    return (cb_ref){{ (void *)e, cb_elem_borrow(e, {}u, {}) }};", ty, mode).unwrap();
        Ok(Some(b))
    }

    // `Vec::drop` for a plain element type (`cb_vec_drop_plain`): the
    // body's checked read of `self.len`, the element loop, then the
    // buffer's `deallocate`. Its other reads of `self` are through the
    // same path with no write in between and cannot fail where the first
    // passed. A resource, reference-holding, fn-valued or zero-sized
    // element type keeps the prelude body.
    fn native_vec_drop(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
        writeln!(b, "        cb_deallocate((uint8_t *)s->ptr, n, {}u);", align).unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(Some(b))
    }

    // `Vec::push` for a plain element type: the body's checked read of
    // `v.len` (its reads of `v.cap` and `v.ptr` go through the same path
    // with nothing in between but `grow`'s own checked writes through it,
    // and cannot fail where that passed), `grow` as written in the
    // prelude when full, the element's `[Rawptr-Write]` (no reference in
    // the value, so no holder object: CHG-0031), and the checked write of
    // `v.len`.
    fn native_vec_push(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
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

    // Stage 3 (`COBC-PLAN.md` §10): the names of locals that may be
    // *confined* `Vec`s — declared with an initializer, and used only as
    // `&x`/`&mut x` passed straight to the prelude's `Vec::push`,
    // `Vec::len`, or `Vec::index_*` whose result is dereferenced at once
    // (a read in a value position, or the target of an assignment whose
    // value pushes nothing onto `x`), and at most moved out as the
    // function body's own result. Nothing then ever holds a path to such
    // a local, or to one of its elements, but its root: see
    // `bind_confined`. By name, across scopes and shadowing, like
    // `pinned_names`: a use of any binding of the name that is not one of
    // these rules the name out, which only errs toward checking.
    fn confined_vecs(&self, body: &Block) -> HashSet<String> {
        let (declared, bad) = self.scan_vec_uses(body, &self.confining, true);
        declared.difference(&bad).cloned().collect()
    }

    // Which parameters of each program function are *confining*: typed
    // `ref<Vec<_>, _>`, never re-bound in the body, and used only as a
    // confined local would be (`confined_vecs`) — the vector argument of
    // `Vec::push`/`Vec::len`/`*Vec::index_*` (`p`, `&*p`, `&mut *p`), or
    // passed on to a confining parameter, with no other argument of that
    // call passing it too — never returned, stored, captured or copied. A
    // confined vector passed there stays confined: for the call's
    // duration the parameter is the only path to it (its caller is
    // suspended, nothing else was handed it, no thread holds it), so the
    // variant compiled for such arguments (`Gen::request_variant`) checks
    // nothing through it. The greatest fixed point: parameters that only
    // pass the vector to one another (recursion) are confining.
    fn confining_table(&self) -> HashMap<String, Vec<bool>> {
        let is_vec_ref = |t: &Type| matches!(t, Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, a) if n == "std::Vec" && a.len() == 1));
        let mut t: HashMap<String, Vec<bool>> =
            self.items.fns.iter().filter(|(_, f)| !in_prelude(f)).map(|(n, f)| (n.clone(), f.params.iter().map(|p| is_vec_ref(&p.ty)).collect())).collect();
        loop {
            let mut changed = false;
            for (n, f) in self.items.fns.iter() {
                let Some(cur) = t.get(n) else { continue };
                if !cur.iter().any(|&b| b) {
                    continue;
                }
                let (declared, bad) = self.scan_vec_uses(&f.body, &t, false);
                let new: Vec<bool> = f.params.iter().zip(cur).map(|(p, &c)| c && !bad.contains(&p.name) && !declared.contains(&p.name)).collect();
                if &new != cur {
                    t.insert(n.clone(), new);
                    changed = true;
                }
            }
            if !changed {
                return t;
            }
        }
    }

    // The scan behind `confined_vecs` and `confining_table`: the names a
    // body declares with an initializer, and the names it uses in any way
    // but the allowed ones. `fn_body`: a bare name as the body's own
    // result is allowed (a local moved out as the result); not for
    // parameters, where it would return the reference.
    fn scan_vec_uses(&self, body: &Block, table: &HashMap<String, Vec<bool>>, fn_body: bool) -> (HashSet<String>, HashSet<String>) {
        #[derive(Clone, Copy, PartialEq)]
        enum Ctx {
            Value,
            Place,
        }
        struct Scan<'s, 'a> {
            g: &'s Gen<'a>,
            table: &'s HashMap<String, Vec<bool>>,
            declared: HashSet<String>,
            bad: HashSet<String>,
        }
        const PUSH: &str = "std::Vec::push";
        const LEN: &str = "std::Vec::len";
        fn call_name(e: &Expr) -> Option<(String, &[Expr])> {
            match &e.kind {
                ExprKind::Call(callee, args) => match &callee.kind {
                    ExprKind::Path(segs, _) => Some((segs.join("::"), args.as_slice())),
                    _ => None,
                },
                _ => None,
            }
        }
        fn bare(e: &Expr) -> Option<&str> {
            match &e.kind {
                ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(&segs[0]),
                _ => None,
            }
        }
        // A vector passed as a reference argument: `&x`/`&mut x` (a
        // local), or `p`/`&*p`/`&mut *p` (a reference parameter).
        fn vec_arg(e: &Expr) -> Option<&str> {
            match &e.kind {
                ExprKind::Borrow(_, inner) => match &inner.kind {
                    ExprKind::Deref(p) => bare(p),
                    _ => bare(inner),
                },
                _ => bare(e),
            }
        }
        // `x[i]` (`[Index-Vec]`, D-0047): `*Vec::index_*(&x, i)` for a `Vec`
        // `x` (which `bind_confined` checks); `x` a local or a reference
        // parameter (`p[i]`, `(*p)[i]`), and the index.
        fn index_expr_on_local(e: &Expr) -> Option<(&str, &Expr)> {
            let ExprKind::Index(a, i) = &e.kind else { return None };
            let mut a: &Expr = a;
            while let ExprKind::Paren(x) = &a.kind {
                a = x;
            }
            let x = match &a.kind {
                ExprKind::Deref(p) => bare(p)?,
                _ => bare(a)?,
            };
            Some((x, i))
        }
        // `Vec::index_*(&x, i)`: `x` and the index.
        fn index_on_local(e: &Expr) -> Option<(&str, &Expr)> {
            let (name, args) = call_name(e)?;
            if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
                return None;
            }
            let [a0, a1] = args else { return None };
            Some((vec_arg(a0)?, a1))
        }
        fn pushes_onto(e: &Expr, x: &str) -> bool {
            let mut found = false;
            walk(e, &mut |e| {
                if let Some((name, args)) = call_name(e) {
                    if name == PUSH && args.first().and_then(vec_arg) == Some(x) {
                        found = true;
                    }
                }
            });
            found
        }
        // Every expression in `e`, closures' bodies included.
        fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
            f(e);
            match &e.kind {
                ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => walk(a, f),
                ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                    walk(a, f);
                    walk(b, f);
                }
                ExprKind::SliceOf(_, a, lo, hi) => {
                    walk(a, f);
                    walk(lo, f);
                    walk(hi, f);
                }
                ExprKind::Call(c, args) => {
                    walk(c, f);
                    args.iter().for_each(|a| walk(a, f));
                }
                ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| walk(a, f)),
                ExprKind::ArrayLit(es) => es.iter().for_each(|a| walk(a, f)),
                ExprKind::Block(b) | ExprKind::Unsafe(b) => walk_block(b, f),
                ExprKind::If(c, t, e2) => {
                    walk(c, f);
                    walk_block(t, f);
                    if let Some(e2) = e2 {
                        walk(e2, f);
                    }
                }
                ExprKind::While(c, b, step) => {
                    walk(c, f);
                    walk_block(b, f);
                    if let Some(st) = step {
                        walk(st, f);
                    }
                }
                ExprKind::Match(sc, arms) => {
                    walk(sc, f);
                    arms.iter().for_each(|a| walk(&a.body, f));
                }
                ExprKind::Return(Some(a)) => walk(a, f),
                ExprKind::Closure { body, .. } => walk_block(body, f),
                _ => {}
            }
        }
        fn walk_block(b: &Block, f: &mut dyn FnMut(&Expr)) {
            for st in &b.stmts {
                match st {
                    Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => walk(e, f),
                    Stmt::Destructure { init, .. } => walk(init, f),
                    Stmt::Let { init: None, .. } => {}
                }
            }
            if let Some(t) = &b.tail {
                walk(t, f);
            }
        }
        impl Scan<'_, '_> {
            fn args_ctx(&self, name: &str) -> Ctx {
                // Arguments of a function or extern are lowered as values,
                // and so are those of the conversions and the integer
                // arithmetic intrinsics (`lower_intrinsic`); any other
                // intrinsic's may be places.
                const VALUE_INTRINSICS: &[&str] = &[
                    "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul",
                    "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem",
                ];
                let local = |n: &str| self.g.items.fns.contains_key(n) || self.g.items.externs.contains_key(n);
                if local(name) || VALUE_INTRINSICS.contains(&name) {
                    Ctx::Value
                } else {
                    Ctx::Place
                }
            }
            fn expr(&mut self, e: &Expr, ctx: Ctx) {
                match &e.kind {
                    ExprKind::Path(segs, _) => {
                        if segs.len() == 1 {
                            self.bad.insert(segs[0].clone());
                        }
                    }
                    ExprKind::Deref(inner) => match index_on_local(inner) {
                        Some((_, i)) if ctx == Ctx::Value => self.expr(i, Ctx::Value),
                        _ => self.expr(inner, Ctx::Place),
                    },
                    ExprKind::Assign(l, r) => {
                        match &l.kind {
                            ExprKind::Deref(inner) => match index_on_local(inner) {
                                Some((x, i)) if !pushes_onto(r, x) => self.expr(i, Ctx::Value),
                                _ => self.expr(l, Ctx::Place),
                            },
                            ExprKind::Index(..) => match index_expr_on_local(l) {
                                Some((x, i)) if !pushes_onto(r, x) => self.expr(i, Ctx::Value),
                                _ => self.expr(l, Ctx::Place),
                            },
                            _ => self.expr(l, Ctx::Place),
                        }
                        self.expr(r, Ctx::Value);
                    }
                    ExprKind::Call(callee, args) => {
                        if let Some((name, _)) = call_name(e) {
                            if (name == PUSH || name == LEN) && args.first().and_then(vec_arg).is_some() {
                                args[1..].iter().for_each(|a| self.expr(a, Ctx::Value));
                                return;
                            }
                            if let Some(conf) = self.table.get(&name) {
                                // A vector passed to a confining parameter,
                                // and passed as no other argument of the
                                // call: the parameters would alias. Other
                                // uses in other arguments (reads, `len`,
                                // even pushes, which move the buffer but
                                // not the `Vec`) are over before the call
                                // begins and leave no held path; they are
                                // scanned like any other use.
                                for (j, a) in args.iter().enumerate() {
                                    let passed = conf.get(j) == Some(&true)
                                        && vec_arg(a).map_or(false, |x| !args.iter().enumerate().any(|(k, b)| k != j && vec_arg(b) == Some(x)));
                                    if !passed {
                                        self.expr(a, Ctx::Value);
                                    }
                                }
                                return;
                            }
                            let c = self.args_ctx(&name);
                            self.expr(callee, Ctx::Place);
                            args.iter().for_each(|a| self.expr(a, c));
                        } else {
                            self.expr(callee, Ctx::Place);
                            args.iter().for_each(|a| self.expr(a, Ctx::Value));
                        }
                    }
                    ExprKind::Borrow(_, a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) => self.expr(a, Ctx::Place),
                    ExprKind::Index(a, i) => match index_expr_on_local(e) {
                        Some((_, i)) if ctx == Ctx::Value => self.expr(i, Ctx::Value),
                        _ => {
                            self.expr(a, Ctx::Place);
                            self.expr(i, Ctx::Value);
                        }
                    },
                    ExprKind::SliceOf(_, a, lo, hi) => {
                        self.expr(a, Ctx::Place);
                        self.expr(lo, Ctx::Value);
                        self.expr(hi, Ctx::Value);
                    }
                    ExprKind::Unary(_, a) => self.expr(a, Ctx::Value),
                    ExprKind::Binary(_, a, b) => {
                        self.expr(a, Ctx::Value);
                        self.expr(b, Ctx::Value);
                    }
                    ExprKind::Paren(a) => self.expr(a, ctx),
                    ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| self.expr(a, Ctx::Value)),
                    ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a, Ctx::Value)),
                    ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b, false),
                    ExprKind::If(c, t, e2) => {
                        self.expr(c, Ctx::Value);
                        self.block(t, false);
                        if let Some(e2) = e2 {
                            self.expr(e2, Ctx::Value);
                        }
                    }
                    ExprKind::While(c, b, step) => {
                        self.expr(c, Ctx::Value);
                        self.block(b, false);
                        if let Some(st) = step {
                            self.expr(st, Ctx::Value);
                        }
                    }
                    ExprKind::Match(sc, arms) => {
                        self.expr(sc, Ctx::Place);
                        for a in arms {
                            if let Some(b) = &a.binder {
                                self.bad.insert(b.clone());
                            }
                            self.expr(&a.body, Ctx::Value);
                        }
                    }
                    ExprKind::Return(Some(a)) => self.expr(a, Ctx::Value),
                    ExprKind::Closure { captures, params, body, .. } => {
                        // Nothing a closure names or captures is confined.
                        self.bad.extend(captures.iter().cloned());
                        self.bad.extend(params.iter().map(|p| p.name.clone()));
                        let bad = &mut self.bad;
                        walk_block(body, &mut |e| {
                            if let ExprKind::Path(segs, _) = &e.kind {
                                if segs.len() == 1 {
                                    bad.insert(segs[0].clone());
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
            fn block(&mut self, b: &Block, fn_body: bool) {
                for st in &b.stmts {
                    match st {
                        Stmt::Let { name, init: Some(e), .. } => {
                            self.declared.insert(name.clone());
                            self.expr(e, Ctx::Value);
                        }
                        Stmt::Let { name, init: None, .. } => {
                            self.bad.insert(name.clone());
                        }
                        Stmt::Destructure { fields, init, .. } => {
                            for field in fields {
                                self.bad.insert(field.clone());
                            }
                            self.expr(init, Ctx::Value);
                        }
                        Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e, Ctx::Value),
                    }
                }
                match &b.tail {
                    // The function's own result may move the local out:
                    // nothing of the function runs after it.
                    Some(t) if fn_body && matches!(&t.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty()) => {}
                    Some(t) => self.expr(t, Ctx::Value),
                    None => {}
                }
            }
        }
        let mut sc = Scan { g: self, table, declared: HashSet::new(), bad: HashSet::new() };
        sc.block(body, fn_body);
        (sc.declared, sc.bad)
    }

    // A value with nothing for the runtime to track when it is copied or
    // dropped: no resource, no reference, no fn value, at any depth.
    fn plain_data(&self, t: &Type) -> bool {
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
    fn eq_expr(&mut self, t: &Type, a: &str, b: &str) -> R<String> {
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
    fn eq_fn(&mut self, t: &Type) -> R<String> {
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
                    for f in &sd.fields {
                        let fname = san(&f.name);
                        let e = self.eq_expr(&apply(&f.ty, &sub), &format!("a->{}", fname), &format!("b->{}", fname))?;
                        writeln!(body, "    if (!{}) return 0;", e).unwrap();
                    }
                    body.push_str("    return 1;\n");
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
        writeln!(self.thunks_out, "static uint8_t {}(const {} *a, const {} *b)\n{{\n{}}}", name, ct, ct, body).unwrap();
        self.eqs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // A per-signature thunk calling a fn value: an item directly, a
    // closure through an exclusive borrow of its boxed object.
    fn thunk(&mut self, fty: &Type) -> R<String> {
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
            "static {rc} {name}({params})\n{{\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    if (b->closure) {{\n        cb_ref self = {{ b->data, cb_borrow(b->root, NULL, 0, CB_EXCLUSIVE, CB_NOLOC, 0) }};\n        {ret}(({rc} (*)({closure_sig}))b->code)({closure_args});\n    }}\n    {ret2}(({rc} (*)({item_sig}))b->code)({item_args});\n}}",
            rc = rc,
            name = name,
            params = params.join(", "),
            ret = ret,
            closure_sig = closure_sig,
            closure_args = closure_args,
            ret2 = if rc == "void" { "else " } else { "return " },
            item_sig = item_sig,
            item_args = args.join(", ")
        )
        .unwrap();
        self.thunks.insert(fty.clone(), name.clone());
        Ok(name)
    }

    // The C type of a parameter: a register's, except that a parameter
    // of type `void` (a generic function instantiated at `T = void`,
    // `Result::unwrap_or(r, ())`) is the unit value `cb_unit`, since C
    // has no `void` parameter.
    fn param_ctype(&mut self, t: &Type) -> R<String> {
        match t {
            Type::Void => Ok("cb_unit".to_string()),
            other => self.reg_ctype(other),
        }
    }

    // The C type of a value in a register: parameters, results, temps.
    fn reg_ctype(&mut self, t: &Type) -> R<String> {
        Ok(match t {
            Type::Void | Type::Never => "void".to_string(),
            Type::Ref(..) => "cb_ref".to_string(),
            other => self.ctype(other)?,
        })
    }
}

struct Fx<'g, 'a> {
    g: &'g mut Gen<'a>,
    subst: HashMap<String, Type>,
    scopes: Vec<HashMap<String, Binding>>,
    out: String,
    ret: Type,
    cleanup: Vec<Cleanup>,
    // Each open loop: the cleanup depth at its head, and for a `for` loop
    // (D-0035) the label before its step, which `continue` jumps to.
    loops: Vec<(usize, Option<String>)>,
    ind: usize,
    // Inside a closure body: the `self` parameter and the closure's id.
    self_ref: Option<(String, u64)>,
    // Names the body borrows, captures, or drops (`pinned_names`): a
    // plain local not among them is never checked at run time.
    pinned: HashSet<String>,
    // Names the body assigns to as a whole (`assigned_names`), and, for
    // each binding of one, its root's C name -> the C name of its frame
    // (`cb_frame_here`), which `cb_rebind` needs.
    assigned: HashSet<String>,
    rebind_frames: HashMap<String, String>,
    // `x + 1` nodes (by address) that cannot overflow (`loop_increment`),
    // and whether the `lower_binary` about to run is one.
    no_overflow: HashSet<usize>,
    proven_next: bool,
    // Names `confined_vecs` found confined, and the C names of the
    // bindings of them that qualified (`bind_confined`).
    confined_names: HashSet<String>,
    confined: HashSet<String>,
    // In a variant for confined arguments: each such parameter's vector
    // (a C lvalue), the path its caller passed, and its element type.
    confined_params: HashMap<String, (String, String, Type)>,
    // `probe_type`'s answers in this body, by expression and expected type.
    // An expression's type is fixed by where it is, so probing it again --
    // as every enclosing `if`, `match` and block does before lowering it
    // for real -- need not lower it again: without this, nesting of depth
    // n was lowered 2^n times.
    probes: HashMap<(usize, String), Type>,
    // Expressions this body synthesizes (a cloned `then` block, `?`'s
    // match, a closure's captures), kept alive with it: `probes` is keyed
    // by address, which must not be reused by a later expression.
    kept: Vec<std::rc::Rc<dyn std::any::Any>>,
    // One block expression per `then` block, so that lowering an `if` a
    // second time (after an enclosing probe) finds its probes cached.
    then_exprs: HashMap<usize, std::rc::Rc<Expr>>,
    // D-0047: the C expressions `$` stands for, innermost `[…]` last.
    dollar: Vec<String>,
    // `v[i]` on a `Vec` (D-0047 `[Index-Vec]`): the place is written (an
    // assignment's target, `&mut`'s operand), so it is `index_exclusive`.
    place_write: bool,
    // `v[i]` on a `Vec` as the `*Vec::index_*(&v, i)` it stands for, kept
    // (address-stable for `probe_type`'s cache) per expression and mode.
    vec_index_exprs: HashMap<(usize, bool), std::rc::Rc<Expr>>,
}

impl<'g, 'a> Fx<'g, 'a> {
    // After `cb_bind` of binding `name` with root `root`: when the body
    // assigns to `name` as a whole, remember the frame it was bound in.
    fn note_frame(&mut self, name: &str, root: &str) {
        if !root.is_empty() && self.assigned.contains(name) {
            let f = self.g.fresh("frame");
            self.emit(&format!("uint64_t {} = cb_frame_here();", f));
            self.rebind_frames.insert(root.to_string(), f);
        }
    }

    fn emit(&mut self, s: &str) {
        for _ in 0..self.ind {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn sub(&self, t: &Type) -> Type {
        apply(t, &self.subst)
    }

    fn is_res(&self, t: &Type) -> bool {
        self.g.is_resource(t)
    }

    fn has_refs(&self, t: &Type) -> bool {
        self.g.has_refs(t)
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn loc(&mut self, line: usize) -> String {
        self.g.loc(line)
    }

    // The location a runtime fault without one of its own reports
    // (`cb_at`): set only at the program's own lines. Code inside `std`
    // leaves it at the program's line that called into `std`, as `coby`
    // reports such a fault there.
    fn at(&mut self, line: usize) {
        let loc = self.loc(line);
        if loc != NOLOC {
            self.emit(&format!("cb_at({});", loc));
        }
    }

    fn tid(&mut self, t: &Type) -> R<u32> {
        self.g.type_id(t)
    }

    fn projs_args(projs: &[String]) -> String {
        if projs.is_empty() {
            "NULL, 0".to_string()
        } else {
            format!("(cb_proj[]){{ {} }}, {}", projs.join(", "), projs.len())
        }
    }

    // ---- cleanup stack ----

    // Scopes are emitted as markers and settled by `resolve_scopes`
    // once the whole function is lowered.
    fn scope_marker(&mut self, kind: char, id: u32) {
        self.emit(&format!("\u{1}{}{}", kind, id));
    }

    fn push_frame(&mut self) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        self.scope_marker('F', id);
        self.cleanup.push(Cleanup::Frame(id));
    }

    fn pop_frame(&mut self) {
        let Some(Cleanup::Frame(id)) = self.cleanup.pop() else { panic!("cobc: frame pop closes a statement scope") };
        self.scope_marker('f', id);
    }

    fn push_stmt(&mut self) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        self.scope_marker('S', id);
        self.cleanup.push(Cleanup::Stmt(id));
    }

    fn pop_stmt(&mut self) {
        let Some(Cleanup::Stmt(id)) = self.cleanup.pop() else { panic!("cobc: statement pop closes a frame") };
        self.scope_marker('s', id);
    }

    // The innermost scope or frame, left by a jump already emitted (its
    // pop is part of the jump's unwind): its extent in the text ends here.
    fn end_diverged(&mut self) {
        let (Some(Cleanup::Frame(id)) | Some(Cleanup::Stmt(id))) = self.cleanup.pop() else { panic!("cobc: no scope to end") };
        self.scope_marker('e', id);
    }

    // Emits the pops that leave every scope and frame above `depth`,
    // without changing the lowering's own stack (the jump leaves them).
    fn emit_unwind_to(&mut self, depth: usize) {
        for k in self.cleanup[depth..].to_vec().into_iter().rev() {
            match k {
                Cleanup::Frame(id) => self.scope_marker('g', id),
                Cleanup::Stmt(id) => self.scope_marker('t', id),
            }
        }
    }

    // ---- values ----

    // A value in a named C local; a reference as a `cb_ref`; a value
    // that needs an object gets one (or keeps the one it has).
    fn into_temp(&mut self, v: V, line: usize) -> R<V> {
        if matches!(v.ty, Type::Never) {
            return Ok(v);
        }
        if matches!(v.ty, Type::Void) {
            return Ok(v);
        }
        if matches!(v.ty, Type::Ref(..)) {
            let t = self.g.fresh("r");
            self.emit(&format!("cb_ref {} = {};", t, v.c));
            return Ok(V { c: t, ty: v.ty, obj: None });
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&v.ty)?;
        self.emit(&format!("{} {} = {};", c, t, v.c));
        let obj = if self.is_res(&v.ty) {
            let Some(o) = v.obj else { return internal("a resource value without an object") };
            self.emit(&format!("cb_move_to({}, &{});", o, t));
            Some(o)
        } else if self.has_refs(&v.ty) {
            let tid = self.tid(&v.ty)?;
            match v.obj {
                Some(o) => {
                    self.emit(&format!("cb_move_to({}, &{});", o, t));
                    Some(o)
                }
                None => {
                    let loc = self.loc(line);
                    self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", t, v.c, tid));
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                    Some(o)
                }
            }
        } else {
            None
        };
        Ok(V { c: t, ty: v.ty, obj })
    }

    // Stores a materialized value into an lvalue of the same type: the
    // C store plus the reference data. Object handling is the caller's.
    fn store(&mut self, dst: &str, ty: &Type, v: &V) -> R<()> {
        if matches!(ty, Type::Void | Type::Never) {
            return Ok(());
        }
        if matches!(ty, Type::Ref(..)) {
            self.emit(&format!("{} = ({}).p;", dst, v.c));
            self.emit(&format!("cb_store_ref(&{}, ({}).tok);", dst, v.c));
            return Ok(());
        }
        self.emit(&format!("{} = {};", dst, v.c));
        if self.has_refs(ty) && v.obj.is_none() {
            let tid = self.tid(ty)?;
            self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", dst, v.c, tid));
        } else if self.has_refs(ty) {
            let tid = self.tid(ty)?;
            self.emit(&format!("cb_copy_datum(&{}, &({}), {}u);", dst, v.c, tid));
        }
        Ok(())
    }

    // A value becomes a binding's initial contents: the binding's
    // object is the moved resource, or a fresh object at its address.
    // Stage 3 (`COBC-PLAN.md` §10, T1): `rule.control.flow-analysis`
    // proves every check on a local of a plain type (no resource, no
    // reference inside, not a callable) that the function never borrows,
    // captures or drops: `valid(x)` holds while it is in scope (a plain
    // value is never consumed), `init(x)` is definite assignment's own
    // static guarantee, and with `escaped(x)` and every `deriv(·, x, ·)`
    // false, `¬clash` is proven. Proven checks are not performed
    // (`spec/14` §6 "Discharge"), and such a local needs no runtime
    // object at all.
    fn unchecked(&self, name: &str, ty: &Type) -> bool {
        !self.is_res(ty) && !self.has_refs(ty) && !matches!(ty, Type::Fn(..) | Type::Closure(_)) && !self.pinned.contains(name)
    }

    // The same proof for a local that holds references (a reference, or
    // plain data with one inside): a reference is never consumed, and
    // nothing else reaches the variable's own storage. Its runtime
    // object stays — the slots it holds are what make its references
    // held — but accesses to the variable itself are not checked.
    fn unchecked_holder(&self, name: &str, ty: &Type) -> bool {
        !self.is_res(ty) && self.has_refs(ty) && !matches!(ty, Type::Fn(..) | Type::Closure(_)) && !self.pinned.contains(name)
    }

    fn bind_value(&mut self, name: &str, cname: String, ty: &Type, v: &V, line: usize) -> R<()> {
        self.store(&cname, ty, v)?;
        if self.unchecked(name, ty) {
            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        let loc = self.loc(line);
        let tid = self.tid(ty)?;
        let o = self.g.fresh("o");
        if self.is_res(ty) {
            let Some(src) = &v.obj else { return internal("a resource binding without an object") };
            self.emit(&format!("cb_move_to({}, &{});", src, cname));
            self.emit(&format!("uint64_t {} = {};", o, src));
        } else {
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
        }
        let root = self.g.fresh("root");
        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
        let root = if self.unchecked_holder(name, ty) { String::new() } else { root };
        self.note_frame(name, &root);
        self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root, kind: BindKind::Local });
        Ok(())
    }

    // A binding of a confined name (`Gen::confined_vecs`) whose type is
    // `Vec<T>` for a plain, non-zero-sized `T` is confined. Its borrows are
    // unheld and end with their calls (`Vec::push` keeps only `T`,
    // `Vec::len` returns a number, an element reference is dereferenced
    // at once), it is never moved but as the function's result, and it
    // is initialized where it is declared; so no path but its root ever
    // holds it, and neither `[Borrow]`'s check nor the checked reads and
    // writes in `push`, `len` and `index_*` can fail on it. No element of
    // it ever has an object: elements are reached only through these
    // calls, which establish none (`cb_elem_access`'s first case), and
    // `grow` only copies bytes and releases the old buffer. So none of
    // those checks is made (`confined_local`).
    fn bind_confined(&mut self, name: &str, cname: &str, ty: &Type) -> R<()> {
        if !self.confined_names.contains(name) {
            return Ok(());
        }
        let Type::Named(n, a) = ty else { return Ok(()) };
        let [elem] = a.as_slice() else { return Ok(()) };
        if n != "std::Vec" || self.g.size_align(elem)?.0 == 0 || !self.g.plain_data(elem) {
            return Ok(());
        }
        if self.lookup(name).map_or(false, |b| !b.root.is_empty()) {
            self.confined.insert(cname.to_string());
        }
        Ok(())
    }

    // A confined vector passed as a `Vec` function's reference argument:
    // `&x` / `&mut x` for a confined local `x`, or `p` / `&*p` / `&mut *p`
    // for a confined parameter `p` of this variant. The vector as a C
    // lvalue, the path to hand on (the local's root; the parameter's
    // own), and the element type.
    fn confined_local(&self, e: &Expr) -> Option<(String, String, Type)> {
        let name = |e: &Expr| match &e.kind {
            ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(segs[0].clone()),
            _ => None,
        };
        match &e.kind {
            ExprKind::Borrow(_, inner) => {
                if let ExprKind::Deref(p) = &inner.kind {
                    return self.confined_params.get(&name(p)?).cloned();
                }
                let b = self.lookup(&name(inner)?)?;
                if !self.confined.contains(&b.c) {
                    return None;
                }
                let Type::Named(_, a) = &b.ty else { return None };
                Some((b.c.clone(), b.root.clone(), a.first()?.clone()))
            }
            ExprKind::Path(..) => self.confined_params.get(&name(e)?).cloned(),
            _ => None,
        }
    }

    fn bind_param(&mut self, p: &Param) -> R<()> {
        let ty = self.sub(&p.ty);
        let pc = format!("p_{}", san(&p.name));
        if self.unchecked(&p.name, &ty) {
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        let tid = self.tid(&ty)?;
        let o = self.g.fresh("o");
        let cname = if matches!(ty, Type::Ref(..)) {
            let l = format!("l_{}", pc);
            self.emit(&format!("void *{} = {}.p;", l, pc));
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, l, tid, NOLOC));
            self.emit(&format!("cb_store_ref(&{}, {}.tok);", l, pc));
            l
        } else if self.is_res(&ty) {
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, pc));
            pc
        } else {
            if self.has_refs(&ty) {
                self.emit(&format!("cb_recv_datum(&{}, {}u);", pc, tid));
            }
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, pc, tid, NOLOC));
            pc
        };
        let root = self.g.fresh("root");
        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
        let root = if self.unchecked_holder(&p.name, &ty) { String::new() } else { root };
        self.note_frame(&p.name, &root);
        self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: cname, ty, root, kind: BindKind::Local });
        Ok(())
    }

    // Leaves the function with `v` as its result: the resource or
    // reference data goes in flight first, then every open scope and
    // frame ends, then the C return.
    fn emit_return(&mut self, v: V) -> R<()> {
        let ret = self.ret.clone();
        if matches!(v.ty, Type::Never) {
            return Ok(());
        }
        let hold = if matches!(ret, Type::Void) {
            None
        } else {
            let t = self.into_temp(v, 0)?;
            if self.is_res(&ret) {
                let o = t.obj.clone().expect("resource temp has object");
                self.emit(&format!("cb_send({});", o));
            } else if matches!(ret, Type::Ref(..)) {
                self.emit(&format!("cb_send_ref({}.tok);", t.c));
            } else if self.has_refs(&ret) {
                let tid = self.tid(&ret)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
            Some(t.c)
        };
        self.emit_unwind_to(0);
        match hold {
            Some(t) => self.emit(&format!("return {};", t)),
            None => self.emit("return;"),
        }
        Ok(())
    }

    // ---- blocks and statements ----

    // The statements and tail of a block inside an already-opened
    // frame. With a sink, the tail's value is stored there inside the
    // tail's own statement scope (so its reference data is still live)
    // and its object, if any, is re-stamped to the enclosing statement.
    fn lower_block_body(&mut self, b: &Block, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        self.scopes.push(HashMap::new());
        let mut result = unit();
        for s in &b.stmts {
            if let Some(l) = stmt_line(s) {
                self.at(l);
            }
            self.push_stmt();
            let diverged = self.lower_stmt(s)?;
            if diverged {
                self.end_diverged();
                result = never();
                break;
            }
            self.pop_stmt();
        }
        if !matches!(result.ty, Type::Never) {
            if let Some(t) = &b.tail {
                self.at(t.line);
                self.push_stmt();
                let v = self.lower_expr(t, expected)?;
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                    result = never();
                } else if matches!(v.ty, Type::Void) {
                    self.emit(&format!("(void)({});", v.c));
                    self.pop_stmt();
                    result = unit();
                } else {
                    match sink {
                        Some((dst, dty, ov)) => {
                            let obj = self.assign_sink(dst, dty, v, t.line, ov)?;
                            if let Some(o) = &obj {
                                self.emit(&format!("cb_result({});", o));
                            }
                            self.pop_stmt();
                            result = V { c: dst.to_string(), ty: dty.clone(), obj };
                        }
                        None => {
                            // Value flows out to the function's return.
                            let t2 = self.into_temp(v, t.line)?;
                            if let Some(o) = &t2.obj {
                                self.emit(&format!("cb_result({});", o));
                            } else if matches!(t2.ty, Type::Ref(..)) {
                                self.emit(&format!("cb_result_ref({}.tok);", t2.c));
                            }
                            self.pop_stmt();
                            result = t2;
                        }
                    }
                }
            }
        }
        self.scopes.pop();
        Ok(result)
    }

    // Stores a value into a result temporary declared outside the
    // current C block. The object now living there, if any, is recorded
    // in `obj_var` — a `uint64_t` also declared outside the block, since
    // the id computed here is a local of the block being closed.
    fn assign_sink(&mut self, dst: &str, dty: &Type, v: V, line: usize, obj_var: Option<&str>) -> R<Option<String>> {
        if matches!(v.ty, Type::Never | Type::Void) {
            return Ok(None);
        }
        if matches!(dty, Type::Ref(..)) {
            // A reference result stays in register form (`cb_ref`) and
            // outlives the statement that formed it.
            self.emit(&format!("{} = {};", dst, v.c));
            self.emit(&format!("cb_result_ref({}.tok);", dst));
            return Ok(None);
        }
        self.store(dst, dty, &v)?;
        let o = if self.is_res(dty) {
            let Some(o) = v.obj else { return internal("a resource result without an object") };
            self.emit(&format!("cb_move_to({}, &{});", o, dst));
            o
        } else if self.has_refs(dty) {
            match v.obj {
                Some(o) => {
                    self.emit(&format!("cb_move_to({}, &{});", o, dst));
                    o
                }
                None => {
                    let tid = self.tid(dty)?;
                    let loc = self.loc(line);
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, dst, tid, loc));
                    o
                }
            }
        } else {
            return Ok(None);
        };
        match obj_var {
            Some(ov) => {
                self.emit(&format!("{} = {};", ov, o));
                Ok(Some(ov.to_string()))
            }
            None => Ok(Some(o)),
        }
    }

    // The holder for a sink's object id, declared beside the sink.
    fn sink_obj_var(&mut self, ty: &Type) -> Option<String> {
        if matches!(ty, Type::Ref(..)) || !(self.is_res(ty) || self.has_refs(ty)) {
            return None;
        }
        let ov = self.g.fresh("ob");
        self.emit(&format!("uint64_t {} = 0;", ov));
        Some(ov)
    }

    // A block as a value: opens a frame; its tail lands in a temporary
    // declared outside the braces, whose type is known only afterwards.
    fn lower_block(&mut self, b: &Block, expected: Option<&Type>) -> R<V> {
        // First pass into a buffer to learn the tail's type, then again
        // with the sink? Two passes would emit twice. Instead: lower the
        // body without a sink into a buffer, and if it produced a value,
        // re-lower with the sink. Bodies are small; the cost is compile time.
        // The probe's answer is cached like `probe_type`'s (a nested block
        // was otherwise lowered twice per level of nesting).
        let key = (b as *const Block as usize, format!("block {:?}", expected));
        let value_ty = match self.probes.get(&key) {
            Some(t) => t.clone(),
            None => {
                let saved_out = std::mem::take(&mut self.out);
                let saved_uid = self.g.uid;
                let saved_nstr = self.g.nstr;
                let saved_strings = self.g.strings.clone();
                let saved_cleanup = self.cleanup.len();
                let saved_scopes = self.scopes.len();
                let probe_ind = self.ind;
                self.ind += 1;
                self.push_frame();
                self.g.probing += 1;
                let probe = self.lower_block_body(b, expected, None);
                self.g.probing -= 1;
                let probe = probe?;
                self.cleanup.truncate(saved_cleanup);
                self.scopes.truncate(saved_scopes);
                self.ind = probe_ind;
                self.out = saved_out;
                self.g.uid = saved_uid;
                self.g.nstr = saved_nstr;
                self.g.strings = saved_strings;
                self.probes.insert(key, probe.ty.clone());
                probe.ty
            }
        };
        let (sink, ov) = if matches!(value_ty, Type::Never | Type::Void) {
            (None, None)
        } else {
            let r = self.g.fresh("b");
            let c = self.g.reg_ctype(&value_ty)?;
            self.emit(&format!("{} {};", c, r));
            let ov = self.sink_obj_var(&value_ty);
            (Some(r), ov)
        };
        self.emit("{");
        self.ind += 1;
        self.push_frame();
        let v = self.lower_block_body(b, expected, sink.as_deref().map(|s| (s, &value_ty, ov.as_deref())))?;
        if !matches!(v.ty, Type::Never) {
            self.pop_frame();
        } else {
            self.end_diverged();
        }
        self.ind -= 1;
        self.emit("}");
        Ok(match sink {
            Some(s) => V { c: s, ty: value_ty, obj: v.obj },
            None => v,
        })
    }

    // Returns true when the statement diverges.
    fn lower_stmt(&mut self, s: &Stmt) -> R<bool> {
        match s {
            Stmt::Let { ty, name, init } => {
                let declared = ty.as_ref().map(|t| self.sub(t));
                let cname = format!("l_{}_{}", san(name), self.g.fresh(""));
                match init {
                    Some(e) => {
                        let v = self.lower_expr(e, declared.as_ref())?;
                        let ty = declared.unwrap_or_else(|| v.ty.clone());
                        if matches!(v.ty, Type::Never) {
                            return Ok(true);
                        }
                        let v = if matches!(ty, Type::Ref(..)) { self.into_temp(v, e.line)? } else { v };
                        let c = self.g.ctype(&ty)?;
                        self.emit(&format!("{} {};", c, cname));
                        self.bind_value(name, cname.clone(), &ty, &v, e.line)?;
                        self.bind_confined(name, &cname, &ty)?;
                    }
                    None => {
                        let ty = declared.expect("`auto x;` has no type");
                        let c = self.g.ctype(&ty)?;
                        self.emit(&format!("{} {};", c, cname));
                        if self.unchecked(name, &ty) {
                            // Definite assignment (`spec/11` §3) proves every read.
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root: String::new(), kind: BindKind::Local });
                            return Ok(false);
                        }
                        let tid = self.tid(&ty)?;
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new_uninit(&{}, {}u, {});", o, cname, tid, NOLOC));
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                        self.note_frame(name, &root);
                        self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: cname, ty, root, kind: BindKind::Local });
                    }
                }
                Ok(false)
            }
            Stmt::Destructure { struct_name, fields, init } => {
                let v = self.lower_expr(init, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(true);
                }
                let (sname, args) = match &v.ty {
                    Type::Named(n, a) => (n.clone(), a.clone()),
                    _ => return internal("destructuring a non-struct"),
                };
                if &sname != struct_name && !sname.ends_with(&format!("::{}", struct_name)) {
                    return internal("destructuring struct name mismatch");
                }
                let sd = self.g.struct_decl(&sname).expect("struct");
                let sub = subst_of(&sd.type_params, &args);
                let whole = self.into_temp(v, init.line)?;
                for field in fields {
                    let fd = sd.fields.iter().find(|f| &f.name == field).expect("field");
                    let fty = apply(&fd.ty, &sub);
                    let cname = format!("l_{}_{}", san(field), self.g.fresh(""));
                    let c = self.g.ctype(&fty)?;
                    self.emit(&format!("{} {};", c, cname));
                    let fv = V { c: format!("{}.{}", whole.c, san(field)), ty: fty.clone(), obj: None };
                    let fv = if matches!(fty, Type::Ref(..)) { self.read_ref_slot(&fv.c, &fty) } else { fv };
                    // [Relocate-Out]: each field becomes its own object; the
                    // container ends without destroying what moved out.
                    if self.is_res(&fty) {
                        self.store(&cname, &fty, &fv)?;
                        let tid = self.tid(&fty)?;
                        let loc = self.loc(init.line);
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                        self.note_frame(field, &root);
                        self.scopes.last_mut().unwrap().insert(field.clone(), Binding { c: cname, ty: fty, root, kind: BindKind::Local });
                    } else {
                        self.bind_value(field, cname, &fty, &fv, init.line)?;
                    }
                }
                if let Some(o) = &whole.obj {
                    self.emit(&format!("cb_end_moved_out({});", o));
                }
                Ok(false)
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => {
                let v = self.lower_expr(e, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(true);
                }
                if !matches!(v.ty, Type::Void) {
                    self.emit(&format!("(void)({});", v.c));
                }
                Ok(false)
            }
        }
    }

    // ---- places ----

    fn lower_place(&mut self, e: &Expr) -> R<P> {
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 && self.lookup(&segs[0]).is_some() => {
                let b = self.lookup(&segs[0]).unwrap();
                match b.kind {
                    // An unchecked local has no access path: its accesses are
                    // all proven (`unchecked`), so none is checked.
                    BindKind::Local => {
                        let base = if b.root.is_empty() { None } else { Some(b.root) };
                        Ok(P { c: b.c, ty: b.ty, base, projs: Vec::new(), is_binding: true, raw: None })
                    }
                    BindKind::CaptureVal(i) => {
                        // `self.f_i`: a field of the closure through `self`.
                        let (sp, cid) = self.self_ref.clone().expect("capture outside a closure body");
                        let cty = self.g.ctype(&Type::Closure(cid))?;
                        Ok(P {
                            c: format!("(*({} *){}.p).f{}", cty, sp, i),
                            ty: b.ty,
                            base: Some(format!("{}.tok", sp)),
                            projs: vec![format!("{{ CB_FIELD, {}u }}", i)],
                            is_binding: false,
                            raw: None,
                        })
                    }
                    BindKind::CaptureRef(i) => {
                        // `*self.f_i`: through the captured reference.
                        let (sp, cid) = self.self_ref.clone().expect("capture outside a closure body");
                        let cty = self.g.ctype(&Type::Closure(cid))?;
                        let slot = format!("(*({} *){}.p).f{}", cty, sp, i);
                        let Type::Ref(pointee, _) = &b.ty else { return internal("borrow capture is not a reference") };
                        let tk = self.g.fresh("tk");
                        self.emit(&format!("uint64_t {} = cb_load_ref(&{});", tk, slot));
                        let pc = self.g.ctype(pointee)?;
                        Ok(P { c: format!("(*({} *){})", pc, slot), ty: (**pointee).clone(), base: Some(tk), projs: Vec::new(), is_binding: false, raw: None })
                    }
                }
            }
            ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "reclaim") => {
                // `[Reclaim]`: the object at a raw address, as a place
                // rooted in its own (reclaim) path.
                let ExprKind::Path(_, targs) = &callee.kind else { unreachable!() };
                let ty = targs.get(0).map(|t| self.sub(t)).expect("reclaim's type argument");
                let pv = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(pv, e.line)?;
                let tid = self.tid(&ty)?;
                let c = self.g.ctype(&ty)?;
                let root = self.g.fresh("root");
                self.emit(&format!("uint64_t {} = cb_reclaim({}, {}u);", root, pt.c, tid));
                Ok(P { c: format!("(*({} *){})", c, pt.c), ty, base: Some(root), projs: Vec::new(), is_binding: true, raw: None })
            }
            ExprKind::Paren(inner) => self.lower_place(inner),
            ExprKind::Field(base, f) => {
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                let (sname, args) = match &pb.ty {
                    Type::Named(n, a) => (n.clone(), a.clone()),
                    _ => return internal("field access on a non-struct"),
                };
                let sd = self.g.struct_decl(&sname).expect("struct");
                let sub = subst_of(&sd.type_params, &args);
                let idx = sd.fields.iter().position(|x| &x.name == f).expect("field");
                let mut projs = pb.projs;
                projs.push(format!("{{ CB_FIELD, {}u }}", idx));
                Ok(P { c: format!("{}.{}", pb.c, san(f)), ty: apply(&sd.fields[idx].ty, &sub), base: pb.base, projs, is_binding: false, raw: None })
            }
            ExprKind::Index(base, idx) => {
                if let Some(x) = self.vec_index_call(e, self.place_write)? {
                    self.place_write = false;
                    return self.lower_place(&x);
                }
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                // D-0047: a `Vec`'s element is checked as an access to the
                // `Vec` (its base and path), in the mode of the use; a
                // slice's through the slice's own borrow.
                match pb.ty.clone() {
                    Type::Named(n, args) if n == "std::Vec" => {
                        let elem = args.first().cloned().unwrap_or(Type::Void);
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        // The element's own index on the `Vec`'s path: two
                        // elements are disjoint, the whole `Vec` overlaps each.
                        let mut projs = pb.projs;
                        projs.push(format!("{{ CB_INDEX, (uint64_t){} }}", it.c));
                        return Ok(P { c: format!("({}).ptr[{}]", pb.c, it.c), ty: elem, base: pb.base, projs, is_binding: false, raw: None });
                    }
                    Type::Slice(elem, _) => {
                        let tok = self.g.fresh("tok");
                        self.emit(&format!("uint64_t {} = cb_load_ref(&({}).src);", tok, pb.c));
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        return Ok(P { c: format!("({}).data[{}]", pb.c, it.c), ty: (*elem).clone(), base: Some(tok), projs: vec![format!("{{ CB_INDEX, (uint64_t){} }}", it.c)], is_binding: false, raw: None });
                    }
                    _ => {}
                }
                let (elem, n) = match &pb.ty {
                    Type::Array(inner, n) => ((**inner).clone(), *n),
                    _ => return internal("index on a non-array"),
                };
                self.dollar.push(format!("{}u", n));
                let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                self.dollar.pop();
                let i = i?;
                let it = self.into_temp(i, e.line)?;
                let loc = self.loc(e.line);
                self.emit(&format!("cb_index_check((uint64_t){}, {}u, {});", it.c, n, loc));
                let mut projs = pb.projs;
                projs.push(format!("{{ CB_INDEX, (uint64_t){} }}", it.c));
                Ok(P { c: format!("{}.a[{}]", pb.c, it.c), ty: elem, base: pb.base, projs, is_binding: false, raw: None })
            }
            ExprKind::Deref(inner) => {
                if matches!(self.probe_type(inner, None)?, Type::Guard(_)) {
                    return self.guard_place(inner, e.line);
                }
                let v = self.lower_expr(inner, None)?;
                match &v.ty {
                    Type::Ref(pointee, _) => {
                        let r = self.into_temp(v.clone(), e.line)?;
                        let c = self.g.ctype(pointee)?;
                        Ok(P { c: format!("(*({} *){}.p)", c, r.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", r.c)), projs: Vec::new(), is_binding: false, raw: None })
                    }
                    Type::Rawptr(pointee) => {
                        let t = self.into_temp(v.clone(), e.line)?;
                        Ok(P { c: format!("(*{})", t.c), ty: (**pointee).clone(), base: None, projs: Vec::new(), is_binding: false, raw: Some(t.c) })
                    }
                    _ => internal("deref of a non-pointer"),
                }
            }
            _ => {
                // A temporary used as a place (`make().f`): unchecked.
                let v = self.lower_expr(e, None)?;
                let t = self.into_temp(v, e.line)?;
                // A reference temporary (`index_shared(&w, 0).r`) is held in
                // register form (`cb_ref`), not in a memory slot: the place
                // it stands for is its referent, reached through that token.
                if let Type::Ref(pointee, _) = &t.ty {
                    let c = self.g.ctype(pointee)?;
                    let inner = P { c: format!("(*({} *){}.p)", c, t.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", t.c)), projs: Vec::new(), is_binding: false, raw: None };
                    return self.auto_deref(inner);
                }
                Ok(P { c: t.c, ty: t.ty, base: None, projs: Vec::new(), is_binding: false, raw: None })
            }
        }
    }

    // `[Field-Access-Auto-Deref]`: a place of reference type used as a
    // struct/array place goes through the reference it holds.
    fn auto_deref(&mut self, p: P) -> R<P> {
        match &p.ty {
            Type::Ref(pointee, _) => {
                let tk = self.g.fresh("tk");
                self.emit(&format!("uint64_t {} = cb_load_ref(&{});", tk, p.c));
                let c = self.g.ctype(pointee)?;
                let inner = P { c: format!("(*({} *){})", c, p.c), ty: (**pointee).clone(), base: Some(tk), projs: Vec::new(), is_binding: false, raw: None };
                self.auto_deref(inner)
            }
            _ => Ok(p),
        }
    }

    // A reference stored in memory, read back as a register value.
    fn read_ref_slot(&self, slot: &str, ty: &Type) -> V {
        V { c: format!("((cb_ref){{ {}, cb_load_ref(&{}) }})", slot, slot), ty: ty.clone(), obj: None }
    }

    // `[Read]` of a place as a value; a resource whole binding moves.
    fn read_place(&mut self, p: P, line: usize) -> R<V> {
        let loc = self.loc(line);
        // A whole binding's move is `[Authority-Transfer]`, which
        // `cb_take` checks (temporally valid, then solitary: an alias of
        // either mode is `diag.move-while-aliased`); it is not a `[Read]`.
        let whole_move = self.is_res(&p.ty) && p.raw.is_none() && p.is_binding && p.projs.is_empty();
        if let Some(base) = p.base.as_ref().filter(|_| !whole_move) {
            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
        }
        if self.is_res(&p.ty) {
            if let Some(ptr) = &p.raw {
                // `[Rawptr-Move-Out]`: the value leaves the raw cells; the
                // reclaimed identity there, if any, ends.
                let t = self.g.fresh("t");
                let c = self.g.ctype(&p.ty)?;
                self.emit(&format!("{} {} = {};", c, t, p.c));
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("cb_raw_move_out({}, &{}, {}u);", ptr, t, tid));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                return Ok(V { c: t, ty: p.ty, obj: Some(o) });
            }
            if !(p.is_binding && p.projs.is_empty()) {
                return internal("move out of a projection");
            }
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_take({}, {});", o, p.base.as_deref().unwrap(), loc));
            return Ok(V { c: p.c, ty: p.ty, obj: Some(o) });
        }
        if matches!(p.ty, Type::Ref(..)) {
            return Ok(self.read_ref_slot(&p.c, &p.ty));
        }
        if matches!(p.ty, Type::Fn(..)) {
            // `[Read]` of a fn value: a closure box is cloned.
            return Ok(V { c: format!("cb_fn_copy({}, {})", p.c, loc), ty: p.ty, obj: None });
        }
        Ok(V { c: p.c, ty: p.ty, obj: None })
    }

    // ---- expressions ----

    // A value where a `fn` type is expected: a closure moves into a
    // callable box ([Callable-Closure]); anything else is itself.
    fn lower_expr(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        let saved = std::mem::replace(&mut self.place_write, false);
        let v = self.lower_expr_inner(e, expected);
        self.place_write = saved;
        let v = v?;
        if let (Some(dst @ Type::Fn(..)), Type::Closure(_)) = (expected, &v.ty) {
            return self.box_closure(v, dst.clone(), e.line);
        }
        Ok(v)
    }

    // `[Callable-Closure]`: a closure value moves into a callable box.
    fn box_closure(&mut self, v: V, dst: Type, line: usize) -> R<V> {
        let Type::Closure(id) = v.ty else { return internal("boxing a non-closure") };
        let info = self.g.closures[&id].clone();
        // A closure read out of a binding is copied (non-resource) or
        // moved (resource), as any struct; either way the box takes
        // an object of its own.
        let v = if v.obj.is_none() { self.into_temp(v, line)? } else { v };
        let o = match &v.obj {
            Some(o) => o.clone(),
            None => {
                let tid = self.tid(&v.ty)?;
                let loc = self.loc(line);
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, v.c, tid, loc));
                o
            }
        };
        let tid = self.tid(&v.ty)?;
        let h = self.g.fresh("h");
        self.emit(&format!("void *{} = cb_fn_box({}, (void *){}, {}u);", h, o, info.cname, tid));
        Ok(V { c: h, ty: dst, obj: None })
    }

    fn lower_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        match &e.kind {
            ExprKind::IntLit(v, suf) => {
                let ty = int_lit_type(suf.as_deref(), expected);
                Ok(V { c: c_int(ty, *v as i128), ty: Type::Int(ty), obj: None })
            }
            ExprKind::FloatLit(f, suf) => {
                let ty = match (suf.as_deref(), expected) {
                    (Some("f32"), _) => Type::F32,
                    (Some(_), _) => Type::F64,
                    (None, Some(Type::F32)) => Type::F32,
                    _ => Type::F64,
                };
                Ok(V { c: c_float(*f, &ty), ty, obj: None })
            }
            ExprKind::StrLit(s) => {
                let name = format!("cb_s{}", self.g.nstr);
                self.g.nstr += 1;
                let bytes = s.as_bytes();
                let mut lit = String::new();
                for b in bytes {
                    write!(lit, "{},", b).unwrap();
                }
                writeln!(self.g.strings, "static const uint8_t {}[] = {{{}0}};", name, lit).unwrap();
                Ok(V { c: format!("((cb_str){{ {}, {}u }})", name, bytes.len()), ty: Type::Str, obj: None })
            }
            ExprKind::BoolLit(b) => Ok(V { c: if *b { "1".into() } else { "0".into() }, ty: Type::Bool, obj: None }),
            ExprKind::Unit => Ok(unit()),
            ExprKind::Paren(inner) => self.lower_expr(inner, expected),
            ExprKind::Path(segs, targs) => {
                if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
                    let p = self.lower_place(e)?;
                    return self.read_place(p, e.line);
                }
                if let Some((ename, vi)) = self.variant_of(segs) {
                    return self.construct_variant(&ename, vi, targs, None, expected, e.line);
                }
                let name = segs.join("::");
                if let Some(f) = self.g.items.fns.get(&name).cloned() {
                    // `[T-Item]`: a fn item as a value — an interned callable box.
                    // A generic item is instantiated from its explicit type
                    // arguments, else from the expected `fn` type.
                    let mut m: HashMap<String, Type> = HashMap::new();
                    for (tp, t) in f.type_params.iter().zip(targs.iter()) {
                        m.insert(tp.clone(), self.sub(t));
                    }
                    if let Some(Type::Fn(ps, r)) = expected {
                        for (p, a) in f.params.iter().zip(ps.iter()) {
                            unify(&p.ty, a, &f.type_params, &mut m);
                        }
                        unify(&f.ret, r, &f.type_params, &mut m);
                    }
                    let Some(inst) = f.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<Vec<Type>>>() else {
                        return internal(&format!("cannot infer type arguments of `{}` as a value", name));
                    };
                    let cname = self.g.request_fn(&name, inst.clone())?;
                    let fsub = subst_of(&f.type_params, &inst);
                    let fty = Type::Fn(f.params.iter().map(|p| self.sub(&apply(&p.ty, &fsub))).collect(), Box::new(self.sub(&apply(&f.ret, &fsub))));
                    return Ok(V { c: format!("cb_fn_item((void *){})", cname), ty: fty, obj: None });
                }
                internal(&format!("unresolved path `{}`", name))
            }
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                if let Some(x) = self.vec_index_call(e, false)? {
                    return self.lower_expr(&x, expected);
                }
                if let ExprKind::Deref(inner) = &e.kind {
                    if let Some(p) = self.elem_access_place(inner, false, e.line)? {
                        if matches!(p.ty, Type::Never) {
                            return Ok(never());
                        }
                        // Loaded now: the element's address need not
                        // outlive this expression.
                        let v = self.read_place(p, e.line)?;
                        return self.into_temp(v, e.line);
                    }
                }
                let p = self.lower_place(e)?;
                self.read_place(p, e.line)
            }
            ExprKind::Call(callee, _) if matches!(&callee.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == "reclaim") => {
                let p = self.lower_place(e)?;
                self.read_place(p, e.line)
            }
            ExprKind::StructLit(segs, targs, fields) => self.lower_struct_lit(segs, targs, fields, expected, e.line),
            ExprKind::ArrayLit(es) => {
                let mut elem_ty = match expected {
                    Some(Type::Array(inner, _)) => Some((**inner).clone()),
                    _ => None,
                };
                let mut vals = Vec::new();
                for x in es {
                    let v = self.lower_expr(x, elem_ty.as_ref())?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    if elem_ty.is_none() {
                        elem_ty = Some(v.ty.clone());
                    }
                    vals.push(self.into_temp(v, x.line)?);
                }
                let Some(elem) = elem_ty else { return internal("empty array literal without an expected type") };
                let ty = Type::Array(Box::new(elem.clone()), es.len() as u128);
                let c = self.g.ctype(&ty)?;
                let t = self.g.fresh("arr");
                self.emit(&format!("{} {};", c, t));
                for (i, v) in vals.iter().enumerate() {
                    self.store(&format!("{}.a[{}]", t, i), &elem, v)?;
                    if self.is_res(&elem) {
                        self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource element has object")));
                    }
                }
                self.register_temp(t, ty, e.line)
            }
            ExprKind::Unary(op, inner) => self.lower_unary(*op, inner, expected, e.line),
            ExprKind::Binary(op, l, r) => {
                if let Some(vo) = self.view_op(e) {
                    return self.lower_view_op(e, vo);
                }
                self.proven_next = self.no_overflow.contains(&(e as *const Expr as usize));
                self.lower_binary(*op, l, r, expected, e.line)
            }
            ExprKind::Dollar => match self.dollar.last() {
                Some(c) => Ok(V { c: c.clone(), ty: Type::Int(IntTy::Usize), obj: None }),
                None => internal("`$` outside an index"),
            },
            ExprKind::SliceOf(mode, base, lo, hi) => match self.view_op(e) {
                Some(vo) => self.lower_view_op(e, vo),
                None => self.lower_slice_of(mode, base, lo, hi, e.line),
            },
            ExprKind::Borrow(mode, inner) => {
                self.place_write = *mode == Mode::Exclusive;
                let p = self.lower_place(inner);
                self.place_write = false;
                let p = p?;
                let Some(base) = &p.base else { return internal("borrow of a temporary") };
                let loc = self.loc(e.line);
                let tok = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {});", tok, base, Self::projs_args(&p.projs), m, loc));
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None })
            }
            ExprKind::Call(callee, args) => self.lower_call(callee, args, expected, e.line),
            ExprKind::Assign(place, val) => {
                // `v[i] = x` on a `Vec` is `*Vec::index_exclusive(&mut v, i) = x`.
                let synth = self.vec_index_call(place, true)?;
                let place: &Expr = synth.as_deref().unwrap_or(place);
                // A stateless value leaves nothing between the element's
                // borrow and the write for `cb_elem_access` to skip.
                // A confined vector's element (`bind_confined`) has nothing
                // to check at all; the analysis saw that `val` pushes
                // nothing onto it, so the address stays good.
                let confined = match &place.kind {
                    ExprKind::Deref(inner) => matches!(&inner.kind, ExprKind::Call(_, a) if a.first().and_then(|a0| self.confined_local(a0)).is_some()),
                    _ => false,
                };
                let fused = match &place.kind {
                    ExprKind::Deref(inner) if confined || self.stateless(val) => self.elem_access_place(inner, true, e.line)?,
                    _ => None,
                };
                let p = match fused {
                    Some(p) if matches!(p.ty, Type::Never) => return Ok(never()),
                    Some(p) => p,
                    None => {
                        self.place_write = true;
                        let p = self.lower_place(place);
                        self.place_write = false;
                        p?
                    }
                };
                let v = self.lower_expr(val, Some(&p.ty))?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, val.line)?;
                let loc = self.loc(e.line);
                if let Some(base) = &p.base {
                    // D-0033 (1): a whole binding whose value moved away or
                    // ended (before or during `val`) gets a new object in
                    // its own frame, as its first value did.
                    if let Some(frame) = self.rebind_frames.get(base).cloned().filter(|_| p.is_binding && p.projs.is_empty()) {
                        let tid = self.tid(&p.ty)?;
                        self.emit(&format!("{} = cb_rebind({}, &{}, {}u, {}, {});", base, base, p.c, tid, frame, loc));
                    }
                    // D-0049: a resource target is live only if its value owns
                    // something now, which the runtime decides unless every
                    // value of the type does.
                    let target = if !self.is_res(&p.ty) {
                        "0u".to_string()
                    } else if coby::typecheck::always_owns(&self.g.items, &p.ty) {
                        "1u".to_string()
                    } else {
                        let tid = self.tid(&p.ty)?;
                        format!("cb_owns(&({}), {}u)", p.c, tid)
                    };
                    self.emit(&format!("cb_write({}, {}, {}, {});", base, Self::projs_args(&p.projs), target, loc));
                }
                self.store(&p.c, &p.ty, &v)?;
                if self.is_res(&p.ty) {
                    let o = v.obj.as_ref().expect("resource value has object");
                    match &p.raw {
                        // `[Rawptr-Move-In]`: the object now lives at the raw address.
                        Some(ptr) => self.emit(&format!("cb_raw_move_in({}, {});", o, ptr)),
                        None => self.emit(&format!("cb_absorb({});", o)),
                    }
                }
                Ok(unit())
            }
            ExprKind::Propagate(inner) => {
                // `rule.fail.propagate`'s own desugaring:
                // `match (e) { Ok(v) : v, Err(err) : return Err(err) }`.
                let l = e.line;
                let path = |n: &str| Expr { kind: ExprKind::Path(vec![n.to_string()], vec![]), line: l };
                let is_option = matches!(self.probe_type(inner, None)?, Type::Named(ref n, _) if n == "std::Option");
                let arms = if is_option {
                    // On an `Option`: `match (e) { Some(v) : v, None : return None }`.
                    vec![
                        Arm { variant: Some("Some".into()), nested: Vec::new(), lit: None, binder: Some("__pv".into()), body: Box::new(path("__pv")) },
                        Arm { variant: Some("None".into()), nested: Vec::new(), lit: None, binder: None, body: Box::new(Expr { kind: ExprKind::Return(Some(Box::new(path("None")))), line: l }) },
                    ]
                } else {
                    let err_call = Expr { kind: ExprKind::Call(Box::new(path("Err")), vec![path("__pe")]), line: l };
                    vec![
                        Arm { variant: Some("Ok".into()), nested: Vec::new(), lit: None, binder: Some("__pv".into()), body: Box::new(path("__pv")) },
                        Arm { variant: Some("Err".into()), nested: Vec::new(), lit: None, binder: Some("__pe".into()), body: Box::new(Expr { kind: ExprKind::Return(Some(Box::new(err_call))), line: l }) },
                    ]
                };
                let arms = std::rc::Rc::new(arms);
                self.kept.push(arms.clone());
                self.lower_match(inner, &arms, expected, l)
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.lower_block(b, expected),
            ExprKind::If(c, then_b, else_e) => self.lower_if(c, then_b, else_e.as_deref(), expected, e.line),
            ExprKind::While(c, b, step) => {
                if let Some(inc) = loop_increment(c, b, step.as_deref(), |x| self.lookup(x).map_or(false, |b| b.root.is_empty() && matches!(b.kind, BindKind::Local) && matches!(b.ty, Type::Int(_)))) {
                    self.no_overflow.insert(inc);
                }
                // A `for` loop's `continue` goes to its step (a C `continue`
                // would skip it), through a label placed before it.
                let cont = step.as_ref().map(|_| self.g.fresh("cont"));
                self.loops.push((self.cleanup.len(), cont.clone()));
                self.emit("for (;;) {");
                self.ind += 1;
                // The condition is evaluated in a scope of its own each
                // iteration, so what it forms ends with it.
                self.push_stmt();
                let cv = self.lower_expr(c, Some(&Type::Bool))?;
                let ct = self.g.fresh("c");
                self.emit(&format!("uint8_t {} = {};", ct, cv.c));
                self.pop_stmt();
                self.emit(&format!("if (!{}) break;", ct));
                self.lower_block(b, None)?;
                if let (Some(st), Some(label)) = (step, &cont) {
                    self.emit(&format!("{}: ;", label));
                    self.at(st.line);
                    self.push_stmt();
                    self.lower_expr(st, None)?;
                    self.pop_stmt();
                }
                self.ind -= 1;
                self.emit("}");
                self.loops.pop();
                Ok(unit())
            }
            ExprKind::Match(scrut, arms) => self.lower_match(scrut, arms, expected, e.line),
            ExprKind::Return(opt) => {
                let ret = self.ret.clone();
                let v = match opt {
                    Some(x) => self.lower_expr(x, Some(&ret))?,
                    None => unit(),
                };
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                self.emit_return(v)?;
                Ok(never())
            }
            ExprKind::Break | ExprKind::Continue => {
                let (target, cont) = self.loops.last().cloned().expect("static pass: break inside a loop");
                self.emit_unwind_to(target);
                match (&e.kind, cont) {
                    (ExprKind::Break, _) => self.emit("break;"),
                    (_, Some(label)) => self.emit(&format!("goto {};", label)),
                    (_, None) => self.emit("continue;"),
                }
                Ok(never())
            }
            ExprKind::Closure { is_move, captures, params, body } => self.lower_closure(*is_move, captures, params, body, e.line),
        }
    }

    // `[Closure-Form-Borrow]`/`[Closure-Form-Move]`: the capture struct,
    // then the body as a C function taking `self`.
    fn lower_closure(&mut self, is_move: bool, captures: &[String], params: &[Param], body: &Block, line: usize) -> R<V> {
        self.g.uid += 1;
        let id = self.g.uid as u64;
        let mut fields: Vec<(String, Type)> = Vec::new();
        let mut inits: Vec<V> = Vec::new();
        for c in captures {
            let Some(b) = self.lookup(c) else { return internal(&format!("unbound capture `{}`", c)) };
            let vty = match &b.kind {
                BindKind::CaptureRef(_) => match &b.ty {
                    Type::Ref(t, _) => (**t).clone(),
                    _ => return internal("borrow capture is not a reference"),
                },
                _ => b.ty.clone(),
            };
            let path = Expr { kind: ExprKind::Path(vec![c.clone()], vec![]), line };
            if is_move {
                let path = std::rc::Rc::new(path);
                self.kept.push(path.clone());
                let v = self.lower_expr(&path, None)?;
                let v = self.into_temp(v, line)?;
                fields.push((c.clone(), vty));
                inits.push(v);
            } else {
                let mode = if body_writes(body, c) { Mode::Exclusive } else { Mode::Shared };
                let bexpr = std::rc::Rc::new(Expr { kind: ExprKind::Borrow(mode.clone(), Box::new(path)), line });
                self.kept.push(bexpr.clone());
                let v = self.lower_expr(&bexpr, None)?;
                let v = self.into_temp(v, line)?;
                fields.push((c.clone(), Type::Ref(Box::new(vty), mode)));
                inits.push(v);
            }
        }
        let ptypes: Vec<Type> = params.iter().map(|p| self.sub(&p.ty)).collect();
        let cname = format!("cl_{}", id);
        self.g.closures.insert(id, ClosureInfo { fields: fields.clone(), is_move, params: ptypes, ret: Type::Void, cname: cname.clone() });
        let ty = Type::Closure(id);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("cl");
        self.emit(&format!("{} {};", c, t));
        for (i, v) in inits.iter().enumerate() {
            let fty = fields[i].1.clone();
            self.store(&format!("{}.f{}", t, i), &fty, v)?;
            if self.is_res(&fty) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource capture has object")));
            }
        }
        // Always an object: a call borrows it exclusively.
        let tid = self.tid(&ty)?;
        let loc = self.loc(line);
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
        let ret = self.lower_closure_body(id, params, body)?;
        self.g.closures.get_mut(&id).unwrap().ret = ret;
        Ok(V { c: t, ty, obj: Some(o) })
    }

    // The body as `ret cl_N(cb_ref p_self, params…)`, with each captured
    // name resolving to `*self.f_i` or `self.f_i` ([Closure-Call]).
    fn lower_closure_body(&mut self, id: u64, params: &[Param], body: &Block) -> R<Type> {
        let info = self.g.closures[&id].clone();
        let self_ty = Type::Ref(Box::new(Type::Closure(id)), Mode::Exclusive);
        // First pass to learn the result type, then the real one.
        let mut ret = Type::Void;
        for pass in 0..2 {
            let probing = pass == 0;
            if probing {
                self.g.probing += 1;
            }
            let saved_uid = self.g.uid;
            let saved_nstr = self.g.nstr;
            let saved_strings = self.g.strings.clone();
            let confined_names = self.g.confined_vecs(body);
            let mut fx = Fx {
                g: &mut *self.g,
                subst: self.subst.clone(),
                scopes: vec![HashMap::new()],
                out: String::new(),
                ret: ret.clone(),
                cleanup: Vec::new(),
                loops: Vec::new(),
                ind: 1,
                self_ref: Some(("p_self".to_string(), id)),
                pinned: pinned_names(body),
                assigned: assigned_names(body),
                rebind_frames: HashMap::new(),
                no_overflow: HashSet::new(),
                proven_next: false,
                confined_names,
                confined: HashSet::new(),
                confined_params: HashMap::new(),
                probes: HashMap::new(),
                kept: Vec::new(),
                dollar: Vec::new(),
                place_write: false,
                vec_index_exprs: HashMap::new(),
                then_exprs: HashMap::new(),
            };
            fx.push_frame();
            fx.bind_param(&Param { ty: self_ty.clone(), name: "self".to_string() })?;
            for p in params {
                fx.bind_param(p)?;
            }
            for (i, (name, fty)) in info.fields.iter().enumerate() {
                let kind = if info.is_move { BindKind::CaptureVal(i) } else { BindKind::CaptureRef(i) };
                fx.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: String::new(), ty: fty.clone(), root: String::new(), kind });
            }
            let v = fx.lower_block_body(body, if probing { None } else { Some(&ret) }, None)?;
            if probing {
                ret = if matches!(v.ty, Type::Never) { Type::Void } else { v.ty.clone() };
                self.g.probing -= 1;
                self.g.uid = saved_uid;
                self.g.nstr = saved_nstr;
                self.g.strings = saved_strings;
                continue;
            }
            if matches!(v.ty, Type::Never) {
                fx.emit("__builtin_unreachable();");
            } else {
                fx.emit_return(v)?;
            }
            while !fx.cleanup.is_empty() {
                fx.end_diverged();
            }
            let out = drop_dead_locations(&resolve_scopes(&fx.out));
            if self.g.probing == 0 {
                let ret_c = self.g.reg_ctype(&ret)?;
                let mut params_c = vec!["cb_ref p_self".to_string()];
                for p in params {
                    let pty = self.sub(&p.ty);
                    let pc = self.g.param_ctype(&pty)?;
                    params_c.push(format!("{} p_{}", pc, san(&p.name)));
                }
                let sig = format!("{} {}({})", ret_c, info.cname, params_c.join(", "));
                writeln!(self.g.protos, "{};", sig).unwrap();
                write!(self.g.fns, "{}\n{{\n{}}}\n\n", sig, out).unwrap();
            }
        }
        Ok(ret)
    }

    // Finishes a call whose C expression is built: the result's type
    // decides how it lands (void, reference, resource via the channel,
    // or a plain value).
    fn finish_call(&mut self, call: String, ret: Type, line: usize) -> R<V> {
        if matches!(ret, Type::Void | Type::Never) {
            self.emit(&format!("{};", call));
            return Ok(if matches!(ret, Type::Never) { never() } else { unit() });
        }
        if matches!(ret, Type::Ref(..)) {
            let r = self.g.fresh("r");
            self.emit(&format!("cb_ref {} = {};", r, call));
            self.emit("cb_recv_ref();");
            return Ok(V { c: r, ty: ret, obj: None });
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&ret)?;
        self.emit(&format!("{} {} = {};", c, t, call));
        let obj = if self.is_res(&ret) {
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, t));
            Some(o)
        } else if self.has_refs(&ret) {
            let tid = self.tid(&ret)?;
            self.emit(&format!("cb_recv_datum(&{}, {}u);", t, tid));
            let loc = self.loc(line);
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
            Some(o)
        } else {
            None
        };
        Ok(V { c: t, ty: ret, obj })
    }

    // Lowers already-typed arguments and puts resources and reference
    // data in flight; `None` when an argument diverged.
    fn lower_args(&mut self, ptypes: &[Type], args: &[Expr]) -> R<Option<Vec<String>>> {
        let mut argv = Vec::new();
        for (pty, a) in ptypes.iter().zip(args.iter()) {
            let v = self.lower_expr(a, Some(pty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(None);
            }
            let t = self.into_temp(v, a.line)?;
            argv.push((pty.clone(), t));
        }
        let mut names = Vec::new();
        for (pty, t) in &argv {
            if self.is_res(pty) {
                self.emit(&format!("cb_send({});", t.obj.as_ref().expect("resource argument has object")));
            } else if self.has_refs(pty) && !matches!(pty, Type::Ref(..)) {
                let tid = self.tid(pty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
            names.push(t.c.clone());
        }
        Ok(Some(names))
    }

    // `[Closure-Call]` on a closure place: an exclusive borrow of it is `self`.
    fn call_closure_place(&mut self, id: u64, p: &P, args: &[Expr], line: usize) -> R<V> {
        let info = self.g.closures[&id].clone();
        let Some(base) = &p.base else { return internal("closure call on a temporary") };
        let loc = self.loc(line);
        let tok = self.g.fresh("tok");
        self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_EXCLUSIVE, {});", tok, base, Self::projs_args(&p.projs), loc));
        let self_ref = format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok);
        let Some(names) = self.lower_args(&info.params, args)? else { return Ok(never()) };
        let call = format!("{}({}{})", info.cname, self_ref, names.iter().map(|n| format!(", {}", n)).collect::<String>());
        self.finish_call(call, info.ret, line)
    }

    // A call through a fn value's handle.
    fn call_fn_value(&mut self, handle: String, fty: &Type, args: &[Expr], line: usize) -> R<V> {
        let Type::Fn(ps, r) = fty else { return internal("call through a non-fn value") };
        let thunk = self.g.thunk(fty)?;
        let ps = ps.clone();
        let ret = (**r).clone();
        let Some(names) = self.lower_args(&ps, args)? else { return Ok(never()) };
        let call = format!("{}({}{})", thunk, handle, names.iter().map(|n| format!(", {}", n)).collect::<String>());
        self.finish_call(call, ret, line)
    }

    // A freshly built aggregate in local `t`: registered as a temporary
    // object when the runtime must know about it.
    fn register_temp(&mut self, t: String, ty: Type, line: usize) -> R<V> {
        let obj = if self.is_res(&ty) || self.has_refs(&ty) {
            let tid = self.tid(&ty)?;
            let loc = self.loc(line);
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
            Some(o)
        } else {
            None
        };
        Ok(V { c: t, ty, obj })
    }

    // `V(lit)` (or nested: `Some(Some(1))`) for a variant V with a payload,
    // not shadowed by a local or a function.
    fn is_pure_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, _) => {
                    (segs.len() > 1 || self.lookup(&segs[0]).is_none())
                        && !self.g.items.fns.contains_key(&segs.join("::"))
                        && self.variant_of(segs).is_some()
                        && (is_bare_literal(&args[0]) || self.is_pure_variant_literal(&args[0]))
                }
                _ => false,
            },
            ExprKind::Paren(inner) => self.is_pure_variant_literal(inner),
            _ => false,
        }
    }

    fn variant_of(&self, segs: &[String]) -> Option<(String, usize)> {
        let vname = segs[segs.len() - 1].clone();
        let mut ename = None;
        if segs.len() >= 2 {
            let prefix = segs[..segs.len() - 1].join("::");
            if self.g.items.enums.contains_key(&prefix) {
                ename = Some(prefix);
            }
        }
        let ename = match ename {
            Some(e) => e,
            None => self.g.items.enum_of_variant.get(&vname)?.clone(),
        };
        let ed = self.g.enum_decl(&ename)?;
        let vi = ed.variants.iter().position(|v| v.name == vname)?;
        Some((ename, vi))
    }

    fn construct_variant(&mut self, ename: &str, vi: usize, targs: &[Type], payload: Option<&Expr>, expected: Option<&Type>, line: usize) -> R<V> {
        let ed = self.g.enum_decl(ename).expect("enum");
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in ed.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(Type::Named(n, args)) = expected {
            if n == ename {
                for (p, t) in ed.type_params.iter().zip(args.iter()) {
                    m.entry(p.clone()).or_insert_with(|| t.clone());
                }
            }
        }
        let pv = match (&ed.variants[vi].payload, payload) {
            (Some(pty), Some(pe)) => {
                let hint = apply(pty, &m);
                let exp = if resolved(&hint, &ed.type_params) { Some(hint) } else { None };
                let v = self.lower_expr(pe, exp.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                unify(pty, &v.ty, &ed.type_params, &mut m);
                Some(self.into_temp(v, pe.line)?)
            }
            (None, None) => None,
            _ => return internal("variant payload arity"),
        };
        let args: Vec<Type> = ed.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if args.len() != ed.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", ename));
        }
        let ty = Type::Named(ename.to_string(), args);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("en");
        self.emit(&format!("{} {};", c, t));
        self.emit(&format!("{}.tag = {}u;", t, vi));
        if let Some(v) = pv {
            let pty = v.ty.clone();
            self.store(&format!("{}.u.v{}", t, vi), &pty, &v)?;
            if self.is_res(&pty) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource payload has object")));
            }
        }
        self.register_temp(t, ty, line)
    }

    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047): the source is
    // borrowed as `&base` would be, and the slice holds that borrow in its
    // reference slot, with a pointer to its first element and its length.
    fn lower_slice_of(&mut self, mode: &Mode, base: &Expr, lo: &Expr, hi: &Expr, line: usize) -> R<V> {
        let pb = self.lower_place(base)?;
        let pb = self.auto_deref(pb)?;
        let loc = self.loc(line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        let (elem, len, data, parent) = match pb.ty.clone() {
            Type::Array(el, n) => ((*el).clone(), format!("{}u", n), format!("&({}).a[0]", pb.c), None),
            Type::Named(n, args) if n == "std::Vec" => (args.first().cloned().unwrap_or(Type::Void), format!("({}).len", pb.c), format!("({}).ptr", pb.c), None),
            Type::Slice(el, _) => {
                let t0 = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_load_ref(&({}).src);", t0, pb.c));
                ((*el).clone(), format!("({}).len", pb.c), format!("({}).data", pb.c), Some(t0))
            }
            _ => return internal("slice of something that is not an array, a Vec or a slice"),
        };
        let lenv = self.g.fresh("len");
        self.emit(&format!("uint64_t {} = {};", lenv, len));
        self.dollar.push(lenv.clone());
        let a = self.lower_expr(lo, Some(&Type::Int(IntTy::Usize)));
        let b = match &a {
            Ok(_) => Some(self.lower_expr(hi, Some(&Type::Int(IntTy::Usize)))),
            Err(_) => None,
        };
        self.dollar.pop();
        let at = self.into_temp(a?, line)?;
        let bt = self.into_temp(b.unwrap()?, line)?;
        self.emit(&format!("if ((uint64_t){} > (uint64_t){} || (uint64_t){} > {}) cb_fault(\"diag.index-out-of-bounds\", {});", at.c, bt.c, bt.c, lenv, loc));
        let tok = self.g.fresh("tok");
        match (&parent, &pb.base) {
            (Some(t0), _) => self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, {}, {});", tok, t0, m, loc)),
            (None, Some(base)) => self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {});", tok, base, Self::projs_args(&pb.projs), m, loc)),
            (None, None) => return internal("slice of an unchecked place"),
        }
        let sty = Type::Slice(Box::new(elem), mode.clone());
        let c = self.g.ctype(&sty)?;
        let t = self.g.fresh("sl");
        self.emit(&format!("{} {};", c, t));
        let rv = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", pb.c, tok), ty: Type::Ref(Box::new(pb.ty.clone()), mode.clone()), obj: None };
        self.store(&format!("{}.src", t), &rv.ty.clone(), &rv)?;
        self.emit(&format!("{}.data = {} + {};", t, data, at.c));
        self.emit(&format!("{}.len = (uint64_t){} - (uint64_t){};", t, bt.c, at.c));
        self.register_temp(t, sty, line)
    }

    fn lower_struct_lit(&mut self, segs: &[String], targs: &[Type], fields: &[(String, Expr)], expected: Option<&Type>, line: usize) -> R<V> {
        let name = segs.join("::");
        let Some(sd) = self.g.struct_decl(&name) else { return internal(&format!("unknown struct `{}`", name)) };
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in sd.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(Type::Named(n, args)) = expected {
            if *n == name {
                for (p, t) in sd.type_params.iter().zip(args.iter()) {
                    m.entry(p.clone()).or_insert_with(|| t.clone());
                }
            }
        }
        let mut vals = Vec::new();
        for (fname, fe) in fields {
            let fd = sd.fields.iter().find(|f| &f.name == fname).expect("field");
            let hint = apply(&fd.ty, &m);
            let exp = if resolved(&hint, &sd.type_params) { Some(hint) } else { None };
            let v = self.lower_expr(fe, exp.as_ref())?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            unify(&fd.ty, &v.ty, &sd.type_params, &mut m);
            let t = self.into_temp(v, fe.line)?;
            vals.push((fname.clone(), t));
        }
        let args: Vec<Type> = sd.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if args.len() != sd.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", name));
        }
        let ty = Type::Named(name, args);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("st");
        self.emit(&format!("{} {};", c, t));
        for (f, v) in vals {
            let fty = v.ty.clone();
            self.store(&format!("{}.{}", t, san(&f)), &fty, &v)?;
            if self.is_res(&fty) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource field has object")));
            }
        }
        self.register_temp(t, ty, line)
    }

    fn lower_unary(&mut self, op: UnOp, inner: &Expr, expected: Option<&Type>, line: usize) -> R<V> {
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                let ty = int_lit_type(suf.as_deref(), expected);
                let neg = (*v as i128).wrapping_neg();
                return Ok(V { c: c_int(ty, neg), ty: Type::Int(ty), obj: None });
            }
        }
        let v = self.lower_expr(inner, expected)?;
        if matches!(v.ty, Type::Never) {
            return Ok(never());
        }
        let x = self.into_temp(v.clone(), line)?.c;
        let loc = self.loc(line);
        Ok(match (op, &v.ty) {
            (UnOp::Neg, Type::Int(t)) => {
                let (min, _) = bounds(*t);
                let c = format!(
                    "({{ {ct} _a = {x}; if (_a == {min}) cb_fault(\"diag.arith-overflow\", {loc}); ({ct})(-_a); }})",
                    ct = c_int_type(*t),
                    x = x,
                    min = c_int(*t, min),
                    loc = loc
                );
                V { c, ty: v.ty.clone(), obj: None }
            }
            (UnOp::Neg, Type::F32 | Type::F64) => V { c: format!("(-{})", x), ty: v.ty.clone(), obj: None },
            (UnOp::Not, Type::Bool) => V { c: format!("((uint8_t)!{})", x), ty: Type::Bool, obj: None },
            (UnOp::BitNot, Type::Int(t)) => V { c: format!("(({})(~{}))", c_int_type(*t), x), ty: v.ty.clone(), obj: None },
            _ => return internal("unary operator on an unexpected type"),
        })
    }

    fn lower_binary(&mut self, op: BinOp, l: &Expr, r: &Expr, expected: Option<&Type>, line: usize) -> R<V> {
        let proven = std::mem::take(&mut self.proven_next);
        if matches!(op, BinOp::And | BinOp::Or) {
            let lv = self.lower_expr(l, Some(&Type::Bool))?;
            if matches!(lv.ty, Type::Never) {
                return Ok(never());
            }
            let res = self.g.fresh("t");
            self.emit(&format!("uint8_t {} = {};", res, lv.c));
            self.emit(&format!("if ({}{}) {{", if op == BinOp::And { "" } else { "!" }, res));
            self.ind += 1;
            let rv = self.lower_expr(r, Some(&Type::Bool))?;
            if !matches!(rv.ty, Type::Never) {
                self.emit(&format!("{} = {};", res, rv.c));
            }
            self.ind -= 1;
            self.emit("}");
            return Ok(V { c: res, ty: Type::Bool, obj: None });
        }
        let arith = matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor);
        let operand_expected = if arith { expected } else { None };
        // `[Read-Resource-Rejected]`: an operand is a value read; decided
        // before either operand runs, so nothing is moved first.
        let lt = self.probe_type(l, operand_expected)?;
        if self.is_res(&lt) {
            let loc = self.loc(line);
            self.emit(&format!("cb_fault(\"diag.read-of-resource\", {});", loc));
            return Ok(never());
        }
        // A literal on the left takes the right operand's type, so the right
        // goes first: a number, or for `==`/`!=` an enum literal whose
        // payload is a literal (`Some(5000000000) == x`). Neither has
        // effects, so the order cannot be observed.
        let left_takes = is_bare_literal(l) || (matches!(op, BinOp::Eq | BinOp::Ne) && self.is_pure_variant_literal(l));
        let (lv, rv) = if left_takes && !is_bare_literal(r) && !self.is_pure_variant_literal(r) {
            let rv = self.lower_expr(r, operand_expected)?;
            if matches!(rv.ty, Type::Never) {
                return Ok(never());
            }
            let rv = self.into_temp(rv, r.line)?;
            let lv = self.lower_expr(l, Some(&rv.ty))?;
            let lv = self.into_temp(lv, l.line)?;
            (lv, rv)
        } else {
            let lv = self.lower_expr(l, operand_expected)?;
            if matches!(lv.ty, Type::Never) {
                return Ok(never());
            }
            let lv = self.into_temp(lv, l.line)?;
            let rv = self.lower_expr(r, Some(&lv.ty))?;
            if matches!(rv.ty, Type::Never) {
                return Ok(never());
            }
            let rv = self.into_temp(rv, r.line)?;
            (lv, rv)
        };
        let (a, b) = (lv.c.clone(), rv.c.clone());
        let loc = self.loc(line);
        let bool_of = |c: String| V { c: format!("((uint8_t)({}))", c), ty: Type::Bool, obj: None };
        Ok(match &lv.ty {
            Type::Int(t) => {
                let ct = c_int_type(*t);
                let (min, _) = bounds(*t);
                let bw = t.bitwidth(64);
                let ut = c_uint_type(*t);
                let same = |c: String| V { c, ty: lv.ty.clone(), obj: None };
                match op {
                    BinOp::Add if proven => same(format!("(({})({} + {}))", ct, a, b)),
                    BinOp::Add | BinOp::Sub | BinOp::Mul => {
                        let builtin = match op {
                            BinOp::Add => "__builtin_add_overflow",
                            BinOp::Sub => "__builtin_sub_overflow",
                            _ => "__builtin_mul_overflow",
                        };
                        same(format!("({{ {ct} _r; if ({builtin}({a}, {b}, &_r)) cb_fault(\"diag.arith-overflow\", {loc}); _r; }})"))
                    }
                    BinOp::Div | BinOp::Rem => {
                        let sym = if op == BinOp::Div { "/" } else { "%" };
                        let ovf = if t.signed() {
                            format!("if (_a == {} && _b == ({})-1) cb_fault(\"diag.div-overflow\", {});", c_int(*t, min), ct, loc)
                        } else {
                            String::new()
                        };
                        same(format!("({{ {ct} _a = {a}, _b = {b}; if (_b == 0) cb_fault(\"diag.div-by-zero\", {loc}); {ovf} ({ct})(_a {sym} _b); }})"))
                    }
                    BinOp::Shl => same(format!(
                        "({{ {ct} _a = {a}; uint64_t _n = (uint64_t)({b}); if (_n >= {bw}u) cb_fault(\"diag.shift-amount-out-of-range\", {loc}); ({ct})((({ut})_a) << _n); }})"
                    )),
                    BinOp::Shr => same(format!(
                        "({{ {ct} _a = {a}; uint64_t _n = (uint64_t)({b}); if (_n >= {bw}u) cb_fault(\"diag.shift-amount-out-of-range\", {loc}); ({ct})(_a >> _n); }})"
                    )),
                    BinOp::BitAnd => same(format!("(({})({} & {}))", ct, a, b)),
                    BinOp::BitOr => same(format!("(({})({} | {}))", ct, a, b)),
                    BinOp::BitXor => same(format!("(({})({} ^ {}))", ct, a, b)),
                    BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                    BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                    BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                    BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                    BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                    BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                    BinOp::And | BinOp::Or => unreachable!(),
                }
            }
            Type::F32 | Type::F64 => {
                let same = |c: String| V { c, ty: lv.ty.clone(), obj: None };
                match op {
                    BinOp::Add => same(format!("({} + {})", a, b)),
                    BinOp::Sub => same(format!("({} - {})", a, b)),
                    BinOp::Mul => same(format!("({} * {})", a, b)),
                    BinOp::Div => same(format!("({} / {})", a, b)),
                    BinOp::Rem => same(format!("fmod({}, {})", a, b)),
                    BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                    BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                    BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                    BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                    BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                    BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                    _ => return internal("bit operator on a float"),
                }
            }
            Type::Bool => match op {
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                _ => return internal("operator on bool"),
            },
            Type::Str => match op {
                BinOp::Eq => bool_of(format!("cb_str_eq({}, {})", a, b)),
                BinOp::Ne => bool_of(format!("!cb_str_eq({}, {})", a, b)),
                _ => return internal("operator on str"),
            },
            Type::Rawptr(_) => match op {
                BinOp::Add => V { c: format!("({} + {})", a, b), ty: lv.ty.clone(), obj: None },
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                _ => return internal("operator on rawptr"),
            },
            // `[Eq-Ref]`: reference identity is the address it targets.
            Type::Ref(..) => match op {
                BinOp::Eq => bool_of(format!("{}.p == {}.p", a, b)),
                BinOp::Ne => bool_of(format!("{}.p != {}.p", a, b)),
                _ => return internal("operator on a reference"),
            },
            // `[Eq-Fn]`: two fn values are equal iff they name one item.
            Type::Fn(..) => match op {
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                _ => return internal("operator on a fn value"),
            },
            // `[Eq-Struct]`: structs, arrays and enums compare by value.
            Type::Named(..) | Type::Array(..) => match op {
                BinOp::Eq | BinOp::Ne => {
                    let e = self.g.eq_expr(&lv.ty, &a, &b)?;
                    bool_of(if op == BinOp::Eq { e } else { format!("!{}", e) })
                }
                _ => return internal("operator on an aggregate"),
            },
            _ => return unsupported("binary operator on an aggregate"),
        })
    }

    // An expression used as a branch: a Block goes through lower_block
    // with the sink; anything else is lowered and stored.
    fn lower_branch(&mut self, e: &Expr, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        match (&e.kind, sink) {
            (ExprKind::Block(b), Some((dst, dty, ov))) | (ExprKind::Unsafe(b), Some((dst, dty, ov))) => {
                self.emit("{");
                self.ind += 1;
                self.push_frame();
                let v = self.lower_block_body(b, expected, Some((dst, dty, ov)))?;
                if !matches!(v.ty, Type::Never) {
                    self.pop_frame();
                } else {
                    self.end_diverged();
                }
                self.ind -= 1;
                self.emit("}");
                Ok(v)
            }
            // An expression body gets its own statement scope, as a block
            // tail does: its temporaries live in the C block the caller
            // opened, whose storage the C compiler may reuse once it
            // closes, so they must end before it does. The result's
            // object, if any, is re-stamped to the enclosing statement.
            (_, Some((dst, dty, ov))) => {
                self.push_stmt();
                let v = self.lower_expr(e, expected)?;
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                    return Ok(v);
                }
                if matches!(v.ty, Type::Void) {
                    self.emit(&format!("(void)({});", v.c));
                    self.pop_stmt();
                    return Ok(unit());
                }
                let obj = self.assign_sink(dst, dty, v, e.line, ov)?;
                if let Some(o) = &obj {
                    self.emit(&format!("cb_result({});", o));
                }
                self.pop_stmt();
                Ok(V { c: dst.to_string(), ty: dty.clone(), obj })
            }
            (_, None) => self.lower_expr(e, expected),
        }
    }

    // The type a branch expression would produce, found by lowering it
    // into a discarded buffer (bodies are small; emission is cheap).
    fn probe_type(&mut self, e: &Expr, expected: Option<&Type>) -> R<Type> {
        let key = (e as *const Expr as usize, format!("{:?}", expected));
        if let Some(t) = self.probes.get(&key) {
            return Ok(t.clone());
        }
        let t = self.probe_type_uncached(e, expected)?;
        self.probes.insert(key, t.clone());
        Ok(t)
    }

    // The type of the place `e`, found as `probe_type` finds a value's:
    // lowered as a place into a discarded buffer.
    fn probe_place_type(&mut self, e: &Expr) -> R<Type> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_uid = self.g.uid;
        let saved_nstr = self.g.nstr;
        let saved_strings = self.g.strings.clone();
        let saved_cleanup = self.cleanup.len();
        let saved_loops = self.loops.clone();
        let saved_scopes = self.scopes.len();
        let saved_write = std::mem::replace(&mut self.place_write, false);
        self.g.probing += 1;
        let p = self.lower_place(e);
        self.g.probing -= 1;
        self.place_write = saved_write;
        self.out = saved_out;
        self.g.uid = saved_uid;
        self.g.nstr = saved_nstr;
        self.g.strings = saved_strings;
        self.cleanup.truncate(saved_cleanup);
        self.loops = saved_loops;
        self.scopes.truncate(saved_scopes);
        Ok(p?.ty)
    }

    fn probe_type_uncached(&mut self, e: &Expr, expected: Option<&Type>) -> R<Type> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_uid = self.g.uid;
        let saved_nstr = self.g.nstr;
        let saved_strings = self.g.strings.clone();
        let saved_cleanup = self.cleanup.len();
        let saved_loops = self.loops.clone();
        let saved_scopes = self.scopes.len();
        self.g.probing += 1;
        let v = self.lower_expr(e, expected);
        self.g.probing -= 1;
        self.out = saved_out;
        self.g.uid = saved_uid;
        self.g.nstr = saved_nstr;
        self.g.strings = saved_strings;
        self.cleanup.truncate(saved_cleanup);
        self.loops = saved_loops;
        self.scopes.truncate(saved_scopes);
        Ok(v?.ty)
    }

    fn lower_if(&mut self, c: &Expr, then_b: &Block, else_e: Option<&Expr>, expected: Option<&Type>, line: usize) -> R<V> {
        let cv = self.lower_expr(c, Some(&Type::Bool))?;
        if matches!(cv.ty, Type::Never) {
            return Ok(never());
        }
        let ct = self.into_temp(cv, line)?;
        let then_expr = self
            .then_exprs
            .entry(then_b as *const Block as usize)
            .or_insert_with(|| std::rc::Rc::new(Expr { kind: ExprKind::Block(then_b.clone()), line }))
            .clone();
        // D-0049: a literal then-branch takes the else branch's type.
        let else_hint = match else_e {
            Some(e) if expected.is_none() && coby::ast::is_literal_block(then_b) && !coby::ast::is_literal_branch(e) => {
                Some(self.probe_type(e, None)?).filter(|t| !matches!(t, Type::Never | Type::Void))
            }
            _ => None,
        };
        let expected = expected.or(else_hint.as_ref());
        let then_ty = self.probe_type(&then_expr, expected)?;
        // The else branch's type matters only when the then branch never
        // finishes. Probing it otherwise lowered every `else if` twice, once
        // to probe and once for real, at every level: a chain of n cost 2^n.
        let result_ty = match else_e {
            None => Type::Void,
            Some(e) if matches!(then_ty, Type::Never) => self.probe_type(e, expected)?,
            Some(_) => then_ty.clone(),
        };
        let (sink, ov) = if matches!(result_ty, Type::Void | Type::Never) {
            (None, None)
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            let ov = self.sink_obj_var(&result_ty);
            (Some(r), ov)
        };
        self.emit(&format!("if ({}) {{", ct.c));
        self.ind += 1;
        let tv = self.lower_branch(&then_expr, expected, sink.as_deref().map(|s| (s, &result_ty, ov.as_deref())))?;
        self.ind -= 1;
        let mut obj = tv.obj.clone();
        if let Some(e) = else_e {
            self.emit("} else {");
            self.ind += 1;
            let exp = expected.cloned().or_else(|| if matches!(then_ty, Type::Never) { None } else { Some(then_ty.clone()) });
            let ev = self.lower_branch(e, exp.as_ref(), sink.as_deref().map(|s| (s, &result_ty, ov.as_deref())))?;
            self.ind -= 1;
            if obj.is_none() {
                obj = ev.obj.clone();
            }
        }
        self.emit("}");
        Ok(match sink {
            Some(r) => V { c: r, ty: result_ty, obj },
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    fn lower_match(&mut self, scrut: &Expr, arms: &[Arm], expected: Option<&Type>, line: usize) -> R<V> {
        // D-0049: a resource binding matched by value stays where it is
        // unless the arm taken moves its payload out; that arm moves it
        // (`cb_take`). The discriminant is read in place (`[Read]`'s checks).
        // Any place scrutinee of a resource enum type is matched in place:
        // an arm can move out of a whole binding only (the checker rejects
        // moving a payload out of a field, an element or `*r`), and an arm
        // that moves nothing leaves any place as it was.
        let mut place_root: Option<String> = None;
        let res_place = match &strip_parens(scrut).kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup(&segs[0]).map_or(false, |b| {
                matches!(&b.ty, Type::Named(n, _) if self.g.items.enums.contains_key(n)) && self.is_res(&b.ty)
            }),
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                let t = self.probe_place_type(scrut)?;
                matches!(&t, Type::Named(n, _) if self.g.items.enums.contains_key(n)) && self.is_res(&t)
            }
            _ => false,
        };
        let sv = if res_place {
            let p = self.lower_place(scrut)?;
            let loc = self.loc(line);
            if let Some(base) = p.base.clone() {
                self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                if p.is_binding && p.projs.is_empty() && p.raw.is_none() {
                    place_root = Some(base);
                }
            }
            V { c: p.c, ty: p.ty, obj: None }
        } else {
            self.lower_expr(scrut, None)?
        };
        if matches!(sv.ty, Type::Never) {
            return Ok(never());
        }
        // `[Match-By-Ref]` (D-0046): a reference scrutinee is matched on its
        // referent, and a binder is a reference of the same mode to the
        // payload, borrowed from the scrutinee's token.
        let mut by_ref: Option<(Mode, String)> = None;
        let sv = match sv.ty.clone() {
            Type::Ref(pointee, mode) if matches!(&*pointee, Type::Named(n, _) if self.g.items.enums.contains_key(n)) => {
                let r = self.into_temp(sv, line)?;
                let loc = self.loc(line);
                self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r.c, loc));
                let ec = self.g.ctype(&pointee)?;
                by_ref = Some((mode, format!("({}).tok", r.c)));
                V { c: format!("(*({} *)({}).p)", ec, r.c), ty: (*pointee).clone(), obj: None }
            }
            _ => sv,
        };
        if matches!(sv.ty, Type::Int(_) | Type::Bool) {
            return self.lower_match_scalar(sv, arms, expected, line);
        }
        let (ename, args) = match &sv.ty {
            Type::Named(n, a) if self.g.items.enums.contains_key(n) => (n.clone(), a.clone()),
            _ => {
                // `[Match]` on a non-enum: the evaluator's dynamic verdict.
                let loc = self.loc(line);
                self.emit(&format!("cb_fault(\"diag.type-mismatch\", {});", loc));
                return Ok(never());
            }
        };
        let ed = self.g.enum_decl(&ename).expect("enum");
        let sub = subst_of(&ed.type_params, &args);
        let s = if by_ref.is_some() || res_place { sv } else { self.into_temp(sv, line)? };
        let scrut_res = by_ref.is_none() && !res_place && self.is_res(&s.ty);
        // The first arm's type fixes the rest.
        let mut arm_expected = expected.cloned();
        let mut result_ty = Type::Never;
        // D-0049: an arm that is only a literal takes its type from the
        // first arm that is not, so that one is probed first.
        let mut order: Vec<&Arm> = arms.iter().collect();
        if expected.is_none() {
            if let Some(k) = arms.iter().position(|a| !coby::ast::is_literal_branch(&a.body)) {
                let a = order.remove(k);
                order.insert(0, a);
            }
        }
        for arm in order {
            // The first arm that finishes fixes the type; probing the rest
            // would lower each nested match twice more at every level.
            if !matches!(result_ty, Type::Never) {
                break;
            }
            let probe_exp = arm_expected.clone();
            let t = self.probe_type_arm(arm, &ed, &sub, probe_exp.as_ref(), by_ref.as_ref().map(|(m, _)| m.clone()))?;
            if !matches!(t, Type::Never | Type::Void) && arm_expected.is_none() {
                arm_expected = Some(t.clone());
            }
            if matches!(result_ty, Type::Never) && !matches!(t, Type::Never) {
                result_ty = t;
            }
        }
        let sink = if matches!(result_ty, Type::Void | Type::Never) {
            None
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            Some(r)
        };
        let objs_var = if sink.is_some() { self.sink_obj_var(&result_ty) } else { None };
        for (i, arm) in arms.iter().enumerate() {
            // D-0056: the variant index at each level of the arm's pattern,
            // tested in turn down the payload slots; the binder takes the
            // innermost payload.
            let (vis, inner_ty) = self.arm_levels(&ed, &sub, arm)?;
            let vi = vis.first().copied();
            let mut slot_path = s.c.clone();
            let mut tests = Vec::new();
            for v in &vis {
                tests.push(format!("{}.tag == {}u", slot_path, v));
                slot_path = format!("{}.u.v{}", slot_path, v);
            }
            // D-0057: a literal the innermost payload must equal.
            if let (Some(l), Some(lt)) = (&arm.lit, inner_ty.as_ref()) {
                let lv = self.lower_expr(l, Some(lt))?;
                tests.push(format!("{} == {}", slot_path, lv.c));
            }
            let head = if tests.is_empty() { "{".to_string() } else { format!("if ({}) {{", tests.join(" && ")) };
            self.emit(&format!("{}{}", if i > 0 { "else " } else { "" }, head));
            self.ind += 1;
            self.push_frame();
            self.scopes.push(HashMap::new());
            let mut moved_out = false;
            if let (Some(_), Some(binder), Some((mode, tok))) = (vi, &arm.binder, &by_ref) {
                if let Some(pty) = inner_ty.clone() {
                    let rty = Type::Ref(Box::new(pty), mode.clone());
                    let rc = self.g.ctype(&rty)?;
                    let cname = format!("l_{}_{}", san(binder), self.g.fresh(""));
                    self.emit(&format!("{} {};", rc, cname));
                    let loc = self.loc(line);
                    let t = self.g.fresh("tok");
                    let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    let projs = vec!["{ CB_PAYLOAD, 0u }"; vis.len()].join(", ");
                    self.emit(&format!("uint64_t {} = cb_borrow({}, (cb_proj[]){{ {} }}, {}, {}, {});", t, tok, projs, vis.len(), m, loc));
                    let fv = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", slot_path, t), ty: rty.clone(), obj: None };
                    self.bind_value(binder, cname, &rty, &fv, line)?;
                }
            } else if let (Some(_), Some(binder)) = (vi, &arm.binder) {
                if let Some(pty) = inner_ty.clone() {
                    let pc = self.g.ctype(&pty)?;
                    let cname = format!("l_{}_{}", san(binder), self.g.fresh(""));
                    self.emit(&format!("{} {};", pc, cname));
                    let slot = slot_path.clone();
                    let fv = if matches!(pty, Type::Ref(..)) { self.read_ref_slot(&slot, &pty) } else { V { c: slot, ty: pty.clone(), obj: None } };
                    if self.is_res(&pty) {
                        // [Relocate-Out] of the payload into the binder; a
                        // binding scrutinee is moved from first (D-0049).
                        let taken = match &place_root {
                            Some(root) => {
                                let loc = self.loc(line);
                                let o = self.g.fresh("o");
                                self.emit(&format!("uint64_t {} = cb_take({}, {});", o, root, loc));
                                Some(o)
                            }
                            None => None,
                        };
                        self.store(&cname, &pty, &fv)?;
                        if let Some(o) = taken {
                            self.emit(&format!("cb_end_moved_out({});", o));
                        }
                        let tid = self.tid(&pty)?;
                        let loc = self.loc(line);
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                        self.note_frame(binder, &root);
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root, kind: BindKind::Local });
                        moved_out = true;
                    } else {
                        self.bind_value(binder, cname, &pty, &fv, line)?;
                    }
                }
            }
            if scrut_res {
                let o = s.obj.as_ref().expect("resource scrutinee has object");
                // D-0049: consumed only by an arm that moved the payload
                // out; otherwise the temporary ends with its statement.
                if moved_out {
                    self.emit(&format!("cb_end_moved_out({});", o));
                }
            }
            let v = self.lower_branch(&arm.body, arm_expected.as_ref(), sink.as_deref().map(|r| (r, &result_ty, objs_var.as_deref())))?;
            self.scopes.pop();
            if !matches!(v.ty, Type::Never) {
                self.pop_frame();
            } else {
                self.end_diverged();
            }
            self.ind -= 1;
            self.emit("}");
        }
        Ok(match sink {
            Some(r) => V { c: r, ty: result_ty, obj: objs_var },
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    // D-0057: `match` on an integer or `bool`: an `if`/`else if` chain of
    // equality tests against the arms' literals, in order; `_` is `else`.
    fn lower_match_scalar(&mut self, sv: V, arms: &[Arm], expected: Option<&Type>, line: usize) -> R<V> {
        let s = self.into_temp(sv, line)?;
        let mut arm_expected = expected.cloned();
        let mut result_ty = Type::Never;
        let mut order: Vec<&Arm> = arms.iter().collect();
        if expected.is_none() {
            if let Some(k) = arms.iter().position(|a| !coby::ast::is_literal_branch(&a.body)) {
                let a = order.remove(k);
                order.insert(0, a);
            }
        }
        for arm in order {
            if !matches!(result_ty, Type::Never) {
                break;
            }
            let probe_exp = arm_expected.clone();
            self.scopes.push(HashMap::new());
            let t = self.probe_type(&arm.body, probe_exp.as_ref());
            self.scopes.pop();
            let t = t?;
            if !matches!(t, Type::Never | Type::Void) && arm_expected.is_none() {
                arm_expected = Some(t.clone());
            }
            if matches!(result_ty, Type::Never) && !matches!(t, Type::Never) {
                result_ty = t;
            }
        }
        let sink = if matches!(result_ty, Type::Void | Type::Never) {
            None
        } else {
            let r = self.g.fresh("r");
            let rc = self.g.reg_ctype(&result_ty)?;
            self.emit(&format!("{} {};", rc, r));
            Some(r)
        };
        let objs_var = if sink.is_some() { self.sink_obj_var(&result_ty) } else { None };
        for (i, arm) in arms.iter().enumerate() {
            let head = match &arm.lit {
                Some(l) => {
                    let lv = self.lower_expr(l, Some(&s.ty))?;
                    format!("if ({} == {}) {{", s.c, lv.c)
                }
                None => "{".to_string(),
            };
            self.emit(&format!("{}{}", if i > 0 { "else " } else { "" }, head));
            self.ind += 1;
            self.push_frame();
            self.scopes.push(HashMap::new());
            let v = self.lower_branch(&arm.body, arm_expected.as_ref(), sink.as_deref().map(|r| (r, &result_ty, objs_var.as_deref())))?;
            self.scopes.pop();
            if !matches!(v.ty, Type::Never) {
                self.pop_frame();
            } else {
                self.end_diverged();
            }
            self.ind -= 1;
            self.emit("}");
        }
        Ok(match sink {
            Some(r) => V { c: r, ty: result_ty, obj: objs_var },
            None => {
                if matches!(result_ty, Type::Never) {
                    never()
                } else {
                    unit()
                }
            }
        })
    }

    // D-0056: arm's variant index at each level of its pattern (empty for
    // `_`), and the type of the innermost payload it reaches.
    fn arm_levels(&self, ed: &EnumDecl, sub: &HashMap<String, Type>, arm: &Arm) -> R<(Vec<usize>, Option<Type>)> {
        let mut vis = Vec::new();
        let mut decl = ed.clone();
        let mut dsub = sub.clone();
        let mut payload: Option<Type> = None;
        for (k, vn) in arm.chain().iter().enumerate() {
            if k > 0 {
                match &payload {
                    Some(Type::Named(n, a)) if self.g.items.enums.contains_key(n) => {
                        let d = self.g.enum_decl(n).expect("enum");
                        dsub = subst_of(&d.type_params, a);
                        decl = (*d).clone();
                    }
                    _ => return internal("nested pattern into a payload that is not an enum"),
                }
            }
            let Some(vi) = decl.variants.iter().position(|v| &v.name == vn) else {
                return internal("unknown variant");
            };
            vis.push(vi);
            payload = decl.variants[vi].payload.as_ref().map(|p| apply(p, &dsub));
        }
        Ok((vis, payload))
    }

    fn probe_type_arm(&mut self, arm: &Arm, ed: &EnumDecl, sub: &HashMap<String, Type>, expected: Option<&Type>, by_ref: Option<Mode>) -> R<Type> {
        self.scopes.push(HashMap::new());
        if let Some(binder) = &arm.binder {
            if let (_, Some(p)) = self.arm_levels(ed, sub, arm)? {
                let pty = match &by_ref {
                    Some(m) => Type::Ref(Box::new(p), m.clone()),
                    None => p,
                };
                self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: "probe".into(), ty: pty, root: "0".into(), kind: BindKind::Local });
            }
        }
        let t = self.probe_type(&arm.body, expected);
        self.scopes.pop();
        t
    }

    fn lower_call(&mut self, callee: &Expr, args: &[Expr], expected: Option<&Type>, line: usize) -> R<V> {
        let (segs, targs) = match &callee.kind {
            ExprKind::Path(segs, targs) => (segs, targs),
            _ => {
                // A callee that is itself an expression: a `fn` value is
                // called through its thunk; a closure temporary gets a path
                // of its own for `[Closure-Call]`'s exclusive `self` borrow,
                // and stays a temporary of this statement.
                let v = self.lower_expr(callee, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let t = self.into_temp(v, line)?;
                return match t.ty.clone() {
                    Type::Fn(..) => {
                        let fty = t.ty.clone();
                        self.call_fn_value(t.c, &fty, args, line)
                    }
                    Type::Closure(id) => {
                        let loc = self.loc(line);
                        let o = match &t.obj {
                            Some(o) => o.clone(),
                            None => {
                                let tid = self.tid(&t.ty)?;
                                let o = self.g.fresh("o");
                                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t.c, tid, loc));
                                o
                            }
                        };
                        let tok = self.g.fresh("tok");
                        self.emit(&format!("uint64_t {} = cb_temp_path({});", tok, o));
                        let p = P { c: t.c, ty: t.ty, base: Some(tok), projs: Vec::new(), is_binding: false, raw: None };
                        self.call_closure_place(id, &p, args, line)
                    }
                    _ => internal("call of a non-callable value"),
                };
            }
        };
        if segs.len() == 1 {
            if let Some(b) = self.lookup(&segs[0]) {
                let p = self.lower_place(callee)?;
                return match &b.ty {
                    Type::Closure(id) => self.call_closure_place(*id, &p, args, line),
                    Type::Fn(..) => {
                        let loc = self.loc(line);
                        if let Some(base) = &p.base {
                            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                        }
                        let h = self.g.fresh("h");
                        self.emit(&format!("void *{} = {};", h, p.c));
                        let fty = p.ty.clone();
                        self.call_fn_value(h, &fty, args, line)
                    }
                    _ => internal("call of a non-callable binding"),
                };
            }
        }
        let name = segs.join("::");
        if let Some((ename, vi)) = self.variant_of(segs) {
            if !self.g.items.fns.contains_key(&name) && args.len() == 1 {
                return self.construct_variant(&ename, vi, targs, Some(&args[0]), expected, line);
            }
        }
        if let Some(f) = self.g.items.fns.get(&name).cloned() {
            if f.name == "drop" && f.assoc_type.is_some() {
                return internal("direct destructor call");
            }
            return self.lower_user_call(&name, &f, targs, args, expected, line, None);
        }
        if self.g.items.externs.contains_key(&name) {
            if name == "std::write" || name == "std::write_err" {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                let f = if name == "std::write" { "cb_write_out" } else { "cb_write_err" };
                return Ok(V { c: format!("{}({}, {})", f, pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::file_read" || name == "std::file_write" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k % 2 == 1 { Some(Type::Int(IntTy::Usize)) } else { None };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                let f = if name == "std::file_read" { "cb_file_read" } else { "cb_file_write" };
                return Ok(V { c: format!("{}((void *)({}), {}, (void *)({}), {})", f, cs[0], cs[1], cs[2], cs[3]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::file_op" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 2 { None } else { Some(Type::Int(IntTy::Usize)) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_file_op({}, {}, (void *)({}), {})", cs[0], cs[1], cs[2], cs[3]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::file_at" {
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 2 { Type::Int(IntTy::U64) } else { Type::Int(IntTy::Usize) };
                    let v = self.lower_expr(a, Some(&hint))?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_file_at({}, {}, {})", cs[0], cs[1], cs[2]), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::arg_bytes" {
                let i = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let it = self.into_temp(i, line)?;
                let p = self.lower_expr(&args[1], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                return Ok(V { c: format!("cb_arg_bytes({}, {}, {})", it.c, pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::read" {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                return Ok(V { c: format!("cb_read_in({}, {})", pt.c, nt.c), ty: Type::Int(IntTy::Isize), obj: None });
            }
            return self.lower_extern_call(&name, args, line);
        }
        self.lower_intrinsic(&name, targs, args, expected, line)
    }

    // `access`: as for `lower_vec_index_pre`.
    // D-0053: the `StringView` operation the static pass found at `e`.
    fn view_op(&self, e: &Expr) -> Option<coby::interp::ViewOp> {
        self.g.items.view_ops.lock().unwrap().get(&(e as *const Expr as usize)).copied()
    }

    // D-0053: a `StringView` operation, carried out by `std`, as `coby`
    // does: the expression's own operands in order, then the call.
    fn lower_view_op(&mut self, e: &Expr, op: coby::interp::ViewOp) -> R<V> {
        use coby::interp::ViewOp;
        let line = e.line;
        let loc = self.loc(line);
        match (op, &e.kind) {
            (ViewOp::SliceString, ExprKind::SliceOf(_, base, lo, hi)) => {
                // `&s` as a borrow forms it (through a reference, its referent).
                let p = self.lower_place(base)?;
                let p = self.auto_deref(p)?;
                let Some(b) = p.base.clone() else { return internal("a view of a String with no access path") };
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_SHARED, {});", tok, b, Self::projs_args(&p.projs), loc));
                let r = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty.clone()), Mode::Shared), obj: None };
                let r = self.into_temp(r, line)?;
                let len = format!("(({}).bytes.len)", p.c);
                let (l, h) = self.lower_view_bounds(len, lo, hi)?;
                self.call_std_values("std::String::view", &[r, l, h], line)
            }
            (ViewOp::SliceView, ExprKind::SliceOf(_, base, lo, hi)) => {
                let v = self.lower_expr(base, None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, line)?;
                let len = format!("(({}).bytes.len)", v.c);
                let (l, h) = self.lower_view_bounds(len, lo, hi)?;
                self.call_std_values("std::StringView::sub", &[v, l, h], line)
            }
            (ViewOp::Eq { view_left, other_is_str, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.lower_expr(a, None)?;
                let va = self.into_temp(va, line)?;
                let vb = self.lower_expr(b, None)?;
                let vb = self.into_temp(vb, line)?;
                let (v, o) = if view_left { (va, vb) } else { (vb, va) };
                let name = if other_is_str { "std::StringView::eq_str" } else { "std::StringView::eq" };
                let r = self.call_std_values(name, &[v, o], line)?;
                Ok(if negate { V { c: format!("((uint8_t)!({}))", r.c), ty: Type::Bool, obj: None } } else { r })
            }
            _ => internal("a StringView operation of an unexpected shape"),
        }
    }

    // A view's bounds, with `$` its length.
    fn lower_view_bounds(&mut self, len: String, lo: &Expr, hi: &Expr) -> R<(V, V)> {
        let lenv = self.g.fresh("len");
        self.emit(&format!("uint64_t {} = {};", lenv, len));
        self.dollar.push(lenv);
        let l = self.lower_expr(lo, Some(&Type::Int(IntTy::Usize))).and_then(|v| self.into_temp(v, lo.line));
        let h = match &l {
            Ok(_) => Some(self.lower_expr(hi, Some(&Type::Int(IntTy::Usize))).and_then(|v| self.into_temp(v, hi.line))),
            Err(_) => None,
        };
        self.dollar.pop();
        Ok((l?, h.expect("lowered")?))
    }

    // A call of a non-generic `std` function with arguments already
    // lowered (as `lower_user_call` ends: in flight, then the call).
    fn call_std_values(&mut self, name: &str, args: &[V], line: usize) -> R<V> {
        let f = self.g.items.fns.get(name).cloned().expect("a std function");
        let cname = self.g.request_variant(name, vec![], 0)?;
        for (p, v) in f.params.iter().zip(args.iter()) {
            if self.is_res(&p.ty) {
                self.emit(&format!("cb_send({});", v.obj.as_ref().expect("resource argument has object")));
            } else if self.has_refs(&p.ty) && !matches!(p.ty, Type::Ref(..)) {
                let tid = self.tid(&p.ty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", v.c, tid));
            }
        }
        let call = format!("{}({})", cname, args.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", "));
        self.finish_call(call, f.ret.clone(), line)
    }

    // D-0049: an argument `ref<τ, exclusive>` (`slice<τ, exclusive>`) for
    // a parameter `ref<τ, shared>` (`slice<τ, shared>`) is passed as
    // `&*e`: a shared borrow through it.
    fn weaken_arg(&mut self, t: V, pty: &Type, line: usize) -> R<V> {
        let loc = self.loc(line);
        match (&t.ty, pty) {
            (Type::Ref(inner, Mode::Exclusive), Type::Ref(_, Mode::Shared)) => {
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow(({}).tok, NULL, 0, CB_SHARED, {});", tok, t.c, loc));
                let r = self.g.fresh("r");
                self.emit(&format!("cb_ref {} = ((cb_ref){{ ({}).p, {} }});", r, t.c, tok));
                Ok(V { c: r, ty: Type::Ref(inner.clone(), Mode::Shared), obj: None })
            }
            (Type::Slice(el, Mode::Exclusive), Type::Slice(_, Mode::Shared)) => {
                let sty = Type::Slice(el.clone(), Mode::Shared);
                let c = self.g.ctype(&sty)?;
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_borrow(cb_load_ref(&({}).src), NULL, 0, CB_SHARED, {});", tok, t.c, loc));
                let n = self.g.fresh("sl");
                self.emit(&format!("{} {};", c, n));
                self.emit(&format!("{}.src = ({}).src; cb_store_ref(&{}.src, {});", n, t.c, n, tok));
                self.emit(&format!("{}.data = ({}).data; {}.len = ({}).len;", n, t.c, n, t.c));
                self.register_temp(n, sty, line)
            }
            _ => Ok(t),
        }
    }

    fn lower_user_call(&mut self, name: &str, f: &FnDecl, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize, access: Option<bool>) -> R<V> {
        if in_prelude(f) {
            self.at(line);
        }
        if let Some(v) = self.lower_confined_call(name, f, args)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_vec_index_pre(name, f, args, line, access)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_vec_len_pre(name, f, args)? {
            return Ok(v);
        }
        // Confined vectors passed to confining parameters
        // (`Gen::confining_table`): the call goes to the variant compiled
        // for them, and each is passed as its address and path, with no
        // borrow formed (`bind_confined`: nothing could fail on it).
        let conf = self.g.confining.get(name).cloned().unwrap_or_default();
        let mut mask = 0u64;
        for (j, a) in args.iter().enumerate().take(64) {
            if conf.get(j) == Some(&true) && self.confined_local(a).is_some() {
                mask |= 1 << j;
            }
        }
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in f.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.sub(t));
        }
        if let Some(exp) = expected {
            unify(&f.ret, exp, &f.type_params, &mut m);
        }
        let mut argv = Vec::new();
        let mut weakened: Vec<String> = Vec::new();
        for (j, (p, a)) in f.params.iter().zip(args.iter()).enumerate() {
            if mask & (1 << j) != 0 {
                let (vec, tok, elem) = self.confined_local(a).expect("confined argument");
                let Type::Ref(_, mode) = &p.ty else { return internal("confining parameter of a non-reference type") };
                let ty = Type::Ref(Box::new(Type::Named("std::Vec".to_string(), vec![elem])), mode.clone());
                unify(&p.ty, &ty, &f.type_params, &mut m);
                argv.push(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", vec, tok), ty, obj: None });
                continue;
            }
            let hint = apply(&p.ty, &m);
            let exp = if resolved(&hint, &f.type_params) { Some(hint) } else { None };
            let is_index = j == 1 && (name == "std::Vec::index_shared" || name == "std::Vec::index_exclusive");
            let v = match argv.first() {
                Some(r0) if is_index => {
                    let vt = match &r0.ty {
                        Type::Ref(t, _) => (**t).clone(),
                        t => t.clone(),
                    };
                    let vc = self.g.ctype(&vt)?;
                    let len = format!("((({} *)({}).p))->len", vc, r0.c);
                    self.lower_index_arg(len, a)?
                }
                _ => self.lower_expr(a, exp.as_ref())?,
            };
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            unify(&p.ty, &v.ty, &f.type_params, &mut m);
            let v_ty_before = v.ty.clone();
            let t = self.into_temp(v, a.line)?;
            let t = self.weaken_arg(t, &p.ty, a.line)?;
            // A slice formed for the call (written `&e[i .. j]` there, or
            // reborrowed shared for it, D-0049) is held only by the callee's
            // parameter, as a reference argument is: it ends at the call's
            // return, as in coby, not with the statement.
            let formed_here = matches!(strip_parens(a).kind, ExprKind::SliceOf(..));
            let reborrowed = matches!((&v_ty_before, &t.ty), (Type::Slice(_, Mode::Exclusive), Type::Slice(_, Mode::Shared)));
            if matches!(t.ty, Type::Slice(..)) && (formed_here || reborrowed) {
                if let Some(o) = &t.obj {
                    weakened.push(o.clone());
                }
            }
            argv.push(t);
        }
        let inst: Vec<Type> = f.type_params.iter().map(|p| m.get(p).cloned()).collect::<Option<_>>().unwrap_or_default();
        if inst.len() != f.type_params.len() {
            return internal(&format!("cannot infer type arguments of `{}`", name));
        }
        if let Some(write) = access {
            if let Some(v) = self.native_elem_access(name, f, &inst, &argv, write, line)? {
                return Ok(v);
            }
        }
        let cname = self.g.request_variant(name, inst.clone(), mask)?;
        let fsub = subst_of(&f.type_params, &inst);
        // Resources and reference data go in flight in argument order.
        for (p, t) in f.params.iter().zip(argv.iter()) {
            let pty = apply(&p.ty, &fsub);
            if self.is_res(&pty) {
                self.emit(&format!("cb_send({});", t.obj.as_ref().expect("resource argument has object")));
            } else if self.has_refs(&pty) && !matches!(pty, Type::Ref(..)) {
                let tid = self.tid(&pty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
        }
        let ret = apply(&f.ret, &fsub);
        let call = format!("{}({})", cname, argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", "));
        let v = self.finish_call(call, ret, line)?;
        // A slice reborrowed for a shared parameter (D-0049) served only
        // the call: it ends now, as a reference argument's path does.
        for o in weakened {
            self.emit(&format!("cb_absorb({});", o));
        }
        Ok(v)
    }

    // `Vec::index_shared(&v, i)` / `Vec::index_exclusive(&mut v, i)` with the
    // borrow written in the call: `cb_borrow_check` makes the borrow's
    // check and the body's read check at once and mints no token (the
    // borrow would be unheld and end with the statement), then the
    // `…_pre` body does the rest. Any other first argument takes the
    // ordinary call.
    //
    // With `access` (`Some(write)`), the call is the operand of a `*`
    // that is read, or assigned a stateless value, at once
    // (`elem_access_place`), and a plain element type makes the result
    // the element's address rather than a reference: see
    // `cb_elem_access`. Any other element type returns the reference.
    // `[Index-Vec]` (D-0047): `e[k]` on a `Vec` is `*Vec::index_shared(&e, k)`,
    // or `*Vec::index_exclusive(&mut e, k)` where the place is written, with
    // `$` the `Vec`'s length (bound where the call's index is lowered). The
    // element is its own object, as `coby` has it: a borrow of it is not a
    // borrow of the whole `Vec`. `None` for anything else.
    fn vec_index_call(&mut self, e: &Expr, write: bool) -> R<Option<std::rc::Rc<Expr>>> {
        let ExprKind::Index(base, idx) = &e.kind else { return Ok(None) };
        let key = (e as *const Expr as usize, write);
        if let Some(x) = self.vec_index_exprs.get(&key) {
            return Ok(Some(x.clone()));
        }
        // The base's type without reading it: a place's as a place
        // (reading a resource field as a value would move it); anything
        // else (a call's result) as the value it is.
        let is_place = {
            let mut x: &Expr = base;
            while let ExprKind::Paren(y) = &x.kind {
                x = y;
            }
            matches!(x.kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_))
        };
        let local_ty = match &base.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => match self.lookup(&segs[0]) {
                Some(b) => Some(b.ty),
                // A confined parameter (`lower_fn`'s mask) has no binding:
                // it is a reference to a `Vec` of the element type recorded.
                None => self.confined_params.get(&segs[0]).map(|(_, _, el)| {
                    Type::Ref(Box::new(Type::Named("std::Vec".to_string(), vec![el.clone()])), Mode::Shared)
                }),
            },
            _ => None,
        };
        let bt = if let Some(t) = local_ty {
            t
        } else if is_place {
            self.probe_place_type(base)?
        } else {
            self.probe_type(base, None)?
        };
        let (through_ref, inner) = match &bt {
            Type::Ref(t, _) => (true, (**t).clone()),
            t => (false, t.clone()),
        };
        if !matches!(&inner, Type::Named(n, _) if n == "std::Vec") {
            return Ok(None);
        }
        let line = e.line;
        let place = if through_ref { Expr { kind: ExprKind::Deref(base.clone()), line } } else { (**base).clone() };
        let (fname, mode) = if write { ("index_exclusive", Mode::Exclusive) } else { ("index_shared", Mode::Shared) };
        let callee = Expr { kind: ExprKind::Path(vec!["std".to_string(), "Vec".to_string(), fname.to_string()], Vec::new()), line };
        let arg0 = Expr { kind: ExprKind::Borrow(mode, Box::new(place)), line };
        let call = Expr { kind: ExprKind::Call(Box::new(callee), vec![arg0, (**idx).clone()]), line };
        let x = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(call)), line });
        self.vec_index_exprs.insert(key, x.clone());
        Ok(Some(x))
    }

    // `$` for the index argument of `Vec::index_*` (only `vec_index_call`
    // writes one there: `$` anywhere else is rejected statically).
    fn lower_index_arg(&mut self, len: String, a: &Expr) -> R<V> {
        self.dollar.push(len);
        let r = self.lower_expr(a, Some(&Type::Int(IntTy::Usize)));
        self.dollar.pop();
        r
    }

    fn lower_vec_index_pre(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize, access: Option<bool>) -> R<Option<V>> {
        if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
            return Ok(None);
        }
        let in_prelude = matches!(f.body.stmts.first(), Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) if e.line <= prelude::line_count());
        let (true, [a0, a1]) = (in_prelude, args) else { return Ok(None) };
        let ExprKind::Borrow(mode, inner) = &a0.kind else { return Ok(None) };
        // The index is evaluated after the borrow's check here, where the
        // ordinary call makes the read check after it; only an index that
        // cannot change the runtime's state keeps the two in agreement.
        if !self.stateless(a1) {
            return Ok(None);
        }
        let p = self.lower_place(inner)?;
        let Some(base) = p.base.clone() else { return Ok(None) };
        let elem = match &p.ty {
            Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => a[0].clone(),
            _ => return Ok(None),
        };
        let loc = self.loc(a0.line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
        let i = self.lower_index_arg(format!("({}).len", p.c), a1)?;
        if matches!(i.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let it = self.into_temp(i, a1.line)?;
        let cname = self.g.request_fn(name, vec![elem.clone()])?;
        let rmode = if name == "std::Vec::index_exclusive" { Mode::Exclusive } else { Mode::Shared };
        let plain = !self.is_res(&elem) && !self.has_refs(&elem) && !matches!(elem, Type::Fn(..) | Type::Closure(_));
        if let (Some(write), true) = (access, plain) {
            // The `…_pre` body inline (its location, its bounds fault, the
            // element's address), then the access's checks in one call.
            let vec_c = self.g.ctype(&p.ty)?;
            let elem_c = self.g.ctype(&elem)?;
            let tid = self.tid(&elem)?;
            let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
            let rm = if rmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            self.at(line);
            self.emit(&format!("{} *{} = ({} *)&({});", vec_c, v, vec_c, p.c));
            self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
            self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
            let loc = self.loc(line);
            self.emit(&format!("cb_elem_access({}, {}u, {}, {}, {});", e, tid, rm, write as u32, loc));
            return Ok(Some(V { c: e, ty: elem, obj: None }));
        }
        let r = self.g.fresh("r");
        self.emit(&format!("cb_ref {} = {}_pre(((cb_ref){{ (void *)&({}), 0 }}), {});", r, cname, p.c, it.c));
        Ok(Some(V { c: r, ty: Type::Ref(Box::new(elem), rmode), obj: None }))
    }

    // `Vec::push(&mut x, e)` and `Vec::len(&x)` for a confined `x`
    // (`bind_confined`): the native bodies without their checks, and no
    // path formed for the borrow. `grow` is handed `x`'s root path.
    fn lower_confined_call(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if (name != "std::Vec::push" && name != "std::Vec::len") || !in_prelude(f) {
            return Ok(None);
        }
        let Some((vec, tok, elem)) = args.first().and_then(|a| self.confined_local(a)) else { return Ok(None) };
        if name == "std::Vec::len" {
            let t = self.g.fresh("t");
            self.emit(&format!("uint64_t {} = ({}).len;", t, vec));
            return Ok(Some(V { c: t, ty: Type::Int(IntTy::Usize), obj: None }));
        }
        let v = self.lower_expr(&args[1], Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, args[1].line)?;
        let cname = self.g.request_fn(name, vec![elem])?;
        self.emit(&format!("{}_nc(((cb_ref){{ (void *)&({}), {} }}), {});", cname, vec, tok, x.c));
        Ok(Some(unit()))
    }

    // `Vec::len(&v)`: as `lower_vec_index_pre`, `cb_borrow_check` makes
    // the borrow's check and the body's read check at once; the length is
    // then read in place.
    fn lower_vec_len_pre(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::len" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0] = args else { return Ok(None) };
        let ExprKind::Borrow(mode, inner) = &a0.kind else { return Ok(None) };
        let p = self.lower_place(inner)?;
        // A place with no path (under a raw pointer) takes the call; the
        // operand of `&` is a place, so lowering it again repeats nothing.
        let Some(base) = p.base.clone() else { return Ok(None) };
        if !matches!(&p.ty, Type::Named(n, a) if n == "std::Vec" && a.len() == 1) {
            return Ok(None);
        }
        let loc = self.loc(a0.line);
        let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
        self.emit(&format!("cb_borrow_check({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
        let t = self.g.fresh("t");
        self.emit(&format!("uint64_t {} = ({}).len;", t, p.c));
        Ok(Some(V { c: t, ty: Type::Int(IntTy::Usize), obj: None }))
    }

    // `Vec::index_*(r, i)` with its arguments evaluated as for the call,
    // for a plain element type and an `access` (`elem_access_place`): the
    // native body inline (`native_vec_index`: its location, its checked
    // read of `len`, its bounds fault), then `cb_elem_access` in place of
    // the element borrow and the result's return. With the arguments
    // already evaluated, the checks keep the call's order whatever the
    // index is.
    fn native_elem_access(&mut self, name: &str, f: &FnDecl, inst: &[Type], argv: &[V], write: bool, line: usize) -> R<Option<V>> {
        let rm = match name {
            "std::Vec::index_shared" => "CB_SHARED",
            "std::Vec::index_exclusive" => "CB_EXCLUSIVE",
            _ => return Ok(None),
        };
        let in_prelude = matches!(f.body.stmts.first(), Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) if e.line <= prelude::line_count());
        let ([elem], [r, i], true) = (inst, argv, in_prelude) else { return Ok(None) };
        if self.is_res(elem) || self.has_refs(elem) || matches!(elem, Type::Fn(..) | Type::Closure(_)) {
            return Ok(None);
        }
        let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let elem_c = self.g.ctype(elem)?;
        let tid = self.tid(elem)?;
        let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
        self.at(line);
        self.emit(&format!("cb_read({}.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);", r.c));
        self.emit(&format!("{} *{} = ({} *){}.p;", vec_c, v, vec_c, r.c));
        self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", i.c, v));
        self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, i.c));
        let loc = self.loc(line);
        self.emit(&format!("cb_elem_access({}, {}u, {}, {}, {});", e, tid, rm, write as u32, loc));
        Ok(Some(V { c: e, ty: elem.clone(), obj: None }))
    }

    // `*Vec::index_*(&v, i)` read, or assigned (`write`), at once, for a
    // plain element type: the element as an unchecked place, its checks
    // made by `cb_elem_access`. `None` for any other operand.
    fn elem_access_place(&mut self, inner: &Expr, write: bool, line: usize) -> R<Option<P>> {
        let ExprKind::Call(callee, args) = &inner.kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &callee.kind else { return Ok(None) };
        if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
            return Ok(None);
        }
        let name = segs.join("::");
        if name != "std::Vec::index_shared" && name != "std::Vec::index_exclusive" {
            return Ok(None);
        }
        let Some(f) = self.g.items.fns.get(&name).cloned() else { return Ok(None) };
        if let (true, [a0, a1]) = (in_prelude(&f), args.as_slice()) {
            if let Some((vec, _, elem)) = self.confined_local(a0) {
                // Confined (`bind_confined`): the body's location and
                // bounds fault, and the element's address; nothing else.
                let i = self.lower_index_arg(format!("({}).len", vec), a1)?;
                if matches!(i.ty, Type::Never) {
                    return Ok(Some(P { c: String::new(), ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None }));
                }
                let it = self.into_temp(i, a1.line)?;
                let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
                let elem_c = self.g.ctype(&elem)?;
                let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
                self.at(line);
                self.emit(&format!("{} *{} = &({});", vec_c, v, vec));
                self.emit(&format!("if ({} >= {}->len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);", it.c, v));
                self.emit(&format!("{} *{} = {}->ptr + (int64_t){};", elem_c, e, v, it.c));
                return Ok(Some(P { c: format!("(*{})", e), ty: elem, base: None, projs: Vec::new(), is_binding: false, raw: None }));
            }
        }
        let v = self.lower_user_call(&name, &f, targs, args, None, line, Some(write))?;
        match &v.ty {
            Type::Ref(pointee, _) => {
                let c = self.g.ctype(pointee)?;
                Ok(Some(P { c: format!("(*({} *){}.p)", c, v.c), ty: (**pointee).clone(), base: Some(format!("{}.tok", v.c)), projs: Vec::new(), is_binding: false, raw: None }))
            }
            // A diverging argument: the caller emits nothing more.
            Type::Never => Ok(Some(P { c: String::new(), ty: Type::Never, base: None, projs: Vec::new(), is_binding: false, raw: None })),
            _ => Ok(Some(P { c: format!("(*{})", v.c), ty: v.ty, base: None, projs: Vec::new(), is_binding: false, raw: None })),
        }
    }

    // An expression whose evaluation cannot change the runtime's state.
    // An integer conversion that cannot fault (`widen`, `narrow_wrapping`,
    // `reinterpret`, a `narrow` into a range that holds the operand's) is
    // as stateless as its operand.
    fn stateless(&mut self, e: &Expr) -> bool {
        fn stateless(e: &Expr, conv: &mut dyn FnMut(&str, &Type, &Expr) -> bool) -> bool {
            match &e.kind {
                ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) | ExprKind::Unit | ExprKind::Path(..) => true,
                ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Field(x, _) => stateless(x, conv),
                ExprKind::Binary(_, l, r) => stateless(l, conv) && stateless(r, conv),
                ExprKind::Call(callee, args) => match (&callee.kind, args.as_slice()) {
                    (ExprKind::Path(segs, targs), [x]) if segs.len() == 1 && targs.len() == 1 => {
                        stateless(x, conv) && conv(&segs[0], &targs[0], x)
                    }
                    _ => false,
                },
                _ => false,
            }
        }
        let mut conv = |name: &str, dst: &Type, x: &Expr| -> bool {
            let intrinsic = self.lookup(name).is_none() && !self.g.items.fns.contains_key(name) && !self.g.items.externs.contains_key(name);
            let dst = self.sub(dst);
            match (intrinsic, name, dst) {
                (true, "widen" | "narrow_wrapping" | "reinterpret", Type::Int(_)) => matches!(self.probe_type(x, None), Ok(Type::Int(_))),
                (true, "narrow", Type::Int(d)) => matches!(self.probe_type(x, None), Ok(Type::Int(s)) if int_range_within(s, d)),
                _ => false,
            }
        };
        stateless(e, &mut conv)
    }

    // `[Guard-Deref]`: a guard is read in place, never moved (it is a
    // resource). Its storage holds the mutex interior's address and, in
    // the slot table, the lock path, so `*g` is `*r` for that reference.
    fn guard_place(&mut self, inner: &Expr, line: usize) -> R<P> {
        let is_place = matches!(inner.kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) | ExprKind::Paren(..));
        let (slot, ty) = if is_place {
            let p = self.lower_place(inner)?;
            if let Some(base) = &p.base {
                let loc = self.loc(line);
                self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
            }
            (p.c, p.ty)
        } else {
            let v = self.lower_expr(inner, None)?;
            let t = self.into_temp(v, line)?;
            (t.c, t.ty)
        };
        let Type::Guard(pointee) = ty else { return internal("guard deref of a non-guard") };
        let c = self.g.ctype(&pointee)?;
        let r = self.g.fresh("r");
        self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", r, slot, slot));
        Ok(P { c: format!("(*({} *){}.p)", c, r), ty: *pointee, base: Some(format!("{}.tok", r)), projs: Vec::new(), is_binding: false, raw: None })
    }

    // `[Extern-Call]` (`spec/20` §3): the arguments are values, evaluated
    // left to right; the result is a value of the declared FfiType (a
    // claim). The C function is the symbol named by the declaration,
    // resolved by the system linker against libc and libm. It is declared
    // under a private C name bound to that symbol with an assembler
    // label, so a CobaltC signature never collides with the C headers'
    // own declaration of the same function.
    fn lower_extern_call(&mut self, name: &str, args: &[Expr], line: usize) -> R<V> {
        let ext = self.g.items.externs[name].clone();
        let cname = match self.g.externs_out.get(&ext.name) {
            Some(c) => c.clone(),
            None => {
                let cname = format!("cbx_{}", san(&ext.name));
                let ret = if matches!(ext.ret, Type::Void) { "void".to_string() } else { self.g.ctype(&ext.ret)? };
                let mut ps = Vec::new();
                for p in &ext.params {
                    ps.push(self.g.ctype(&p.ty)?);
                }
                let params = if ps.is_empty() { "void".to_string() } else { ps.join(", ") };
                writeln!(self.g.protos, "extern {} {}({}) __asm__(\"{}\");", ret, cname, params, ext.name).unwrap();
                self.g.externs_out.insert(ext.name.clone(), cname.clone());
                cname
            }
        };
        let mut argv = Vec::new();
        for (p, a) in ext.params.iter().zip(args.iter()) {
            let v = self.lower_expr(a, Some(&p.ty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            argv.push(self.into_temp(v, line)?.c);
        }
        let call = format!("{}({})", cname, argv.join(", "));
        if matches!(ext.ret, Type::Void) {
            self.emit(&format!("{};", call));
            return Ok(unit());
        }
        let t = self.g.fresh("t");
        let c = self.g.ctype(&ext.ret)?;
        self.emit(&format!("{} {} = {};", c, t, call));
        Ok(V { c: t, ty: ext.ret.clone(), obj: None })
    }

    // `[Spawn]`: the callee as a `fn` value and the arguments go into a
    // heap block that holds them (references as held slots, resources
    // detached from this statement); the new thread runs a trampoline
    // that makes an ordinary call from the block, through the callee's
    // thunk, and leaves the result at the block's start.
    fn lower_spawn(&mut self, args: &[Expr], line: usize) -> R<V> {
        let fv = self.lower_expr(&args[0], None)?;
        if matches!(fv.ty, Type::Never) {
            return Ok(never());
        }
        let fty = match &fv.ty {
            Type::Fn(..) => fv.ty.clone(),
            Type::Closure(id) => {
                let info = self.g.closures[id].clone();
                Type::Fn(info.params.clone(), Box::new(info.ret.clone()))
            }
            _ => return internal("spawn of a non-callable"),
        };
        let fv = if matches!(fv.ty, Type::Closure(_)) { self.box_closure(fv, fty.clone(), line)? } else { fv };
        let Type::Fn(ps, ret) = fty.clone() else { unreachable!() };
        let ret = *ret;
        if matches!(ret, Type::Ref(..)) {
            return unsupported("a thread whose result is a reference");
        }
        let h = self.g.fresh("h");
        self.emit(&format!("void *{} = {};", h, fv.c));
        let mut argv = Vec::new();
        for (pty, a) in ps.iter().zip(args[1..].iter()) {
            let v = self.lower_expr(a, Some(pty))?;
            if matches!(v.ty, Type::Never) {
                return Ok(never());
            }
            let t = self.into_temp(v, a.line)?;
            argv.push(self.weaken_arg(t, pty, a.line)?);
        }

        // The block's layout: the result first, then the callee, then
        // each argument in memory form (and a resource's object id).
        let n = self.g.fresh("");
        let env = format!("cb_env_{}", n);
        let rc = self.g.ctype(&ret)?;
        let mut fields = format!("{} res; void *h;", rc);
        for (i, pty) in ps.iter().enumerate() {
            if matches!(pty, Type::Ref(..)) {
                write!(fields, " void *a{};", i).unwrap();
            } else {
                let c = self.g.ctype(pty)?;
                write!(fields, " {} a{};", c, i).unwrap();
                if self.is_res(pty) {
                    write!(fields, " uint64_t o{};", i).unwrap();
                }
            }
        }
        let e = self.g.fresh("env");
        self.emit(&format!("struct {} *{} = cb_env_alloc(sizeof(struct {}));", env, e, env));
        self.emit(&format!("{}->h = {};", e, h));
        let mut body = String::new();
        let mut call_args = Vec::new();
        for (i, (pty, t)) in ps.iter().zip(argv.iter()).enumerate() {
            if matches!(pty, Type::Ref(..)) {
                self.emit(&format!("{}->a{} = {}.p;", e, i, t.c));
                self.emit(&format!("cb_store_ref(&{}->a{}, {}.tok);", e, i, t.c));
                writeln!(body, "    cb_ref r{} = {{ e->a{}, cb_load_ref(&e->a{}) }};", i, i, i).unwrap();
                call_args.push(format!("r{}", i));
                continue;
            }
            self.emit(&format!("{}->a{} = {};", e, i, t.c));
            if self.is_res(pty) {
                let o = t.obj.as_ref().expect("resource argument has object");
                self.emit(&format!("cb_move_to({}, &{}->a{});", o, e, i));
                self.emit(&format!("cb_hold({});", o));
                self.emit(&format!("{}->o{} = {};", e, i, o));
                writeln!(body, "    cb_send(e->o{});", i).unwrap();
            } else if self.has_refs(pty) {
                let tid = self.tid(pty)?;
                self.emit(&format!("cb_copy_datum(&{}->a{}, &({}), {}u);", e, i, t.c, tid));
                writeln!(body, "    cb_send_datum(&e->a{}, {}u);", i, tid).unwrap();
            }
            call_args.push(format!("e->a{}", i));
        }
        let thunk = self.g.thunk(&fty)?;
        let call = format!("{}(e->h{})", thunk, call_args.iter().map(|a| format!(", {}", a)).collect::<String>());
        let rtid = self.tid(&ret)?;
        let done = if matches!(ret, Type::Void | Type::Never) {
            writeln!(body, "    {};", call).unwrap();
            writeln!(body, "    cb_env_forget(e, sizeof(*e));").unwrap();
            "0".to_string()
        } else {
            writeln!(body, "    e->res = {};", call).unwrap();
            writeln!(body, "    cb_env_forget(e, sizeof(*e));").unwrap();
            if self.is_res(&ret) {
                writeln!(body, "    uint64_t o = cb_recv(&e->res);").unwrap();
                "o".to_string()
            } else {
                if self.has_refs(&ret) {
                    writeln!(body, "    cb_recv_datum(&e->res, {}u);", rtid).unwrap();
                }
                "0".to_string()
            }
        };
        writeln!(body, "    cb_drop_fn(e->h);").unwrap();
        writeln!(body, "    cb_thread_done({});", done).unwrap();
        let tramp = format!("cb_thr_{}", n);
        // A probe (`probe_type`) discards what it lowered; the block and
        // trampoline are written only by the real lowering.
        if self.g.probing == 0 {
            writeln!(self.g.thunks_out, "struct {} {{ {} }};", env, fields).unwrap();
            writeln!(self.g.thunks_out, "static void {}(void *p)\n{{\n    struct {} *e = p;\n{}}}", tramp, env, body).unwrap();
        }

        let hty = Type::Handle(Box::new(ret));
        let htid = self.tid(&hty)?;
        let loc = self.loc(line);
        let hv = self.g.fresh("t");
        self.emit(&format!("uint64_t {} = cb_spawn({}, {}, sizeof(struct {}), {}u);", hv, tramp, e, env, rtid));
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, hv, htid, loc));
        Ok(V { c: hv, ty: hty, obj: Some(o) })
    }

    // A hash-table key's bytes, as an address and a length (`key_hash`,
    // `key_eq`): an integer's or `bool`'s own storage (little-endian, its
    // width), a `str`'s or `String`'s UTF-8 bytes. `a` is a `ref<K, shared>`.
    fn lower_key_bytes(&mut self, a: &Expr, line: usize) -> R<(String, String)> {
        let t = self.probe_type(a, None)?;
        let Type::Ref(inner, _) = &t else {
            return internal("a key that is not a reference");
        };
        if matches!(&**inner, Type::Named(n, _) if n == "std::String") {
            let r = self.lower_expr(a, None)?;
            let r = self.into_temp(r, line)?.c;
            let loc = self.loc(line);
            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
            let sc = self.g.ctype(&Type::Named("std::String".into(), vec![]))?;
            return Ok((format!("(const void *)(({} *)({}).p)->bytes.ptr", sc, r), format!("(({} *)({}).p)->bytes.len", sc, r)));
        }
        let d = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line });
        self.kept.push(d.clone());
        let v = self.lower_expr(&d, None)?;
        let ct = self.g.ctype(&v.ty)?;
        let k = self.g.fresh("k");
        self.emit(&format!("{} {} = {};", ct, k, v.c));
        match &v.ty {
            Type::Str => Ok((format!("(const void *)({}).p", k), format!("({}).n", k))),
            Type::Int(_) | Type::Bool => Ok((format!("(const void *)&{}", k), format!("sizeof({})", k))),
            _ => internal("a key of a type that is not hashable"),
        }
    }

    fn lower_intrinsic(&mut self, name: &str, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize) -> R<V> {
        let targ = |i: usize, fx: &Self| targs.get(i).map(|t| fx.sub(t));
        let loc = self.loc(line);
        match name {
            "sizeof" | "alignof" => {
                let t = targ(0, self).expect("type argument");
                let (s, a) = self.g.size_align(&t)?;
                Ok(V { c: format!("((uint64_t){}u)", if name == "sizeof" { s } else { a }), ty: Type::Int(IntTy::Usize), obj: None })
            }
            // D-0052: computed before the program was compiled.
            "static_assert" => Ok(unit()),
            "min_value" | "max_value" => {
                // D-0051: a float's largest finite value (as a hex float
                // literal, exact), negated for `min_value`.
                match targ(0, self) {
                    Some(Type::F32) => {
                        let c = if name == "max_value" { "(0x1.fffffep+127f)" } else { "(-0x1.fffffep+127f)" };
                        return Ok(V { c: c.to_string(), ty: Type::F32, obj: None });
                    }
                    Some(Type::F64) => {
                        let c = if name == "max_value" { "(0x1.fffffffffffffp+1023)" } else { "(-0x1.fffffffffffffp+1023)" };
                        return Ok(V { c: c.to_string(), ty: Type::F64, obj: None });
                    }
                    _ => {}
                }
                let Some(Type::Int(t)) = targ(0, self) else { return internal("a limit of a non-integer type") };
                let (min, max) = bounds(t);
                Ok(V { c: c_int(t, if name == "min_value" { min } else { max }), ty: Type::Int(t), obj: None })
            }
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let c = match (name, &v.ty, &dst) {
                    ("widen", Type::Int(_), Type::Int(d)) => format!("(({}){})", c_int_type(*d), x),
                    ("narrow", Type::Int(s), Type::Int(d)) => narrow_check(*s, *d, &x, &loc),
                    ("narrow_wrapping", Type::Int(_), Type::Int(d)) => format!("(({}){})", c_int_type(*d), x),
                    ("reinterpret", Type::Int(s), Type::Int(d)) => {
                        let (sb, db) = (s.bitwidth(64), d.bitwidth(64));
                        if db > sb {
                            let mid = if d.signed() { c_sint_of_width(sb) } else { c_uint_of_width(sb) };
                            format!("(({})({})({}){})", c_int_type(*d), mid, c_uint_of_width(sb), x)
                        } else {
                            format!("(({}){})", c_int_type(*d), x)
                        }
                    }
                    // D-0051: `f32` into `f64` exactly; `f64` into `f32`
                    // rounds as IEEE-754 does (C's conversion under Annex F,
                    // which gcc and clang implement on these targets).
                    ("widen" | "to_float", Type::F32 | Type::F64, Type::F64) => format!("((double){})", x),
                    ("widen" | "to_float", Type::F32 | Type::F64, Type::F32) => format!("((float){})", x),
                    // D-0051: a float's bits as an integer of its width, and back.
                    ("reinterpret", Type::F32, Type::Int(d)) => format!("(({})((union {{ float f; uint32_t u; }}){{ .f = {} }}).u)", c_int_type(*d), x),
                    ("reinterpret", Type::F64, Type::Int(d)) => format!("(({})((union {{ double f; uint64_t u; }}){{ .f = {} }}).u)", c_int_type(*d), x),
                    ("reinterpret", Type::Int(_), Type::F32) => format!("(((union {{ uint32_t u; float f; }}){{ .u = (uint32_t){} }}).f)", x),
                    ("reinterpret", Type::Int(_), Type::F64) => format!("(((union {{ uint64_t u; double f; }}){{ .u = (uint64_t){} }}).f)", x),
                    ("to_float", Type::Int(_), Type::F64) => format!("((double){})", x),
                    ("to_float", Type::Int(_), Type::F32) => format!("((float){})", x),
                    ("to_int", Type::F32 | Type::F64, Type::Int(d)) => to_int_check(*d, &x, &loc),
                    _ => return internal("conversion on an unexpected type"),
                };
                Ok(V { c, ty: dst, obj: None })
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" | "checked_add"
            | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem" => {
                let (av, bv) = self.lower_operands(&args[0], &args[1], expected)?;
                let (Some(av), Some(bv)) = (av, bv) else { return Ok(never()) };
                let Type::Int(t) = av.ty else { return internal("integer intrinsic on a non-integer") };
                let a = av.c;
                let b = bv.c;
                let ct = c_int_type(t);
                let ut = c_uint_type(t);
                let (min, max) = bounds(t);
                let (family, opname) = name.split_once('_').unwrap();
                let builtin = match opname {
                    "add" => "__builtin_add_overflow",
                    "sub" => "__builtin_sub_overflow",
                    "mul" => "__builtin_mul_overflow",
                    _ => "",
                };
                match family {
                    "wrapping" => {
                        let sym = match opname {
                            "add" => "+",
                            "sub" => "-",
                            _ => "*",
                        };
                        Ok(V { c: format!("(({})((({}){}) {} (({}){})))", ct, ut, a, sym, ut, b), ty: Type::Int(t), obj: None })
                    }
                    "saturating" => {
                        let toward_max = match opname {
                            "add" => format!("({} > 0)", b),
                            "sub" => format!("({} < 0)", b),
                            _ => format!("(({} < 0) == ({} < 0))", a, b),
                        };
                        let toward_max = if t.signed() { toward_max } else if opname == "sub" { "0".to_string() } else { "1".to_string() };
                        Ok(V {
                            c: format!(
                                "({{ {ct} _r; if ({builtin}({a}, {b}, &_r)) _r = {tm} ? {max} : {min}; _r; }})",
                                ct = ct,
                                builtin = builtin,
                                a = a,
                                b = b,
                                tm = toward_max,
                                max = c_int(t, max),
                                min = c_int(t, min)
                            ),
                            ty: Type::Int(t),
                            obj: None,
                        })
                    }
                    _ => {
                        let opt = Type::Named("std::Option".to_string(), vec![Type::Int(t)]);
                        let oc = self.g.ctype(&opt)?;
                        let fail = match opname {
                            "div" | "rem" => {
                                let sym = if opname == "div" { "/" } else { "%" };
                                let ovf = if t.signed() { format!(" || ({} == {} && {} == ({})-1)", a, c_int(t, min), b, ct) } else { String::new() };
                                format!("if ({b} == 0{ovf}) _f = 1; else _r = ({ct})({a} {sym} {b});")
                            }
                            _ => format!("_f = {}({}, {}, &_r);", builtin, a, b),
                        };
                        let t2 = self.g.fresh("ck");
                        self.emit(&format!("{} {};", oc, t2));
                        self.emit(&format!("{{ {} _r = 0; int _f = 0; {} if (_f) {}.tag = 1u; else {{ {}.tag = 0u; {}.u.v0 = _r; }} }}", ct, fail, t2, t2, t2));
                        Ok(V { c: t2, ty: opt, obj: None })
                    }
                }
            }
            "slice_len" => {
                let p = self.lower_place(&args[0])?;
                let p = self.auto_deref(p)?;
                Ok(V { c: format!("({}).len", p.c), ty: Type::Int(IntTy::Usize), obj: None })
            }
            "str_len" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                Ok(V { c: format!("({}).n", st.c), ty: Type::Int(IntTy::Usize), obj: None })
            }
            "str_byte" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                let i = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let it = self.into_temp(i, line)?;
                self.emit(&format!("cb_index_check((uint64_t){}, {}.n, {});", it.c, st.c, loc));
                Ok(V { c: format!("{}.p[{}]", st.c, it.c), ty: Type::Int(IntTy::U8), obj: None })
            }
            "str_ptr" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                Ok(V { c: format!("((uint8_t *)({}).p)", st.c), ty: Type::Rawptr(Box::new(Type::Int(IntTy::U8))), obj: None })
            }
            "rawptr_of" => {
                // `[Rawptr-Of]`: the address of the place — a borrowed
                // place directly, or a reference value's target.
                if let ExprKind::Borrow(_, inner) = &args[0].kind {
                    let p = self.lower_place(inner)?;
                    return Ok(V { c: format!("(&{})", p.c), ty: Type::Rawptr(Box::new(p.ty)), obj: None });
                }
                let v = self.lower_expr(&args[0], None)?;
                let Type::Ref(pointee, _) = &v.ty else { return internal("rawptr_of on a non-reference") };
                let r = self.into_temp(v.clone(), line)?;
                Ok(V { c: format!("({}.p)", r.c), ty: Type::Rawptr(pointee.clone()), obj: None })
            }
            "allocate" => {
                let s = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let st = self.into_temp(s, line)?;
                let a = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let at = self.into_temp(a, line)?;
                let rty = Type::Named("std::Result".into(), vec![Type::Rawptr(Box::new(Type::Int(IntTy::U8))), Type::Named("std::AllocError".into(), vec![])]);
                let rc = self.g.ctype(&rty)?;
                let t = self.g.fresh("al");
                self.emit(&format!("{} {};", rc, t));
                self.emit(&format!("{{ uint8_t *_p = cb_allocate({}, {}); if (_p) {{ {}.tag = 0u; {}.u.v0 = _p; }} else {}.tag = 1u; }}", st.c, at.c, t, t, t));
                Ok(V { c: t, ty: rty, obj: None })
            }
            "deallocate" => {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let s = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let st = self.into_temp(s, line)?;
                let a = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let at = self.into_temp(a, line)?;
                self.emit(&format!("cb_deallocate({}, {}, {});", pt.c, st.c, at.c));
                Ok(unit())
            }
            "release" => {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                self.emit(&format!("cb_release({}, {});", pt.c, nt.c));
                Ok(unit())
            }
            "copy_raw" => {
                let d = self.lower_expr(&args[0], None)?;
                let dt = self.into_temp(d, line)?;
                let s = self.lower_expr(&args[1], None)?;
                let st = self.into_temp(s, line)?;
                let n = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                self.emit(&format!("cb_copy_raw({}, {}, {});", dt.c, st.c, nt.c));
                Ok(unit())
            }
            "reinterpret_ptr" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({}){})", c, v.c), ty, obj: None })
            }
            "dangling" => {
                let dst = targ(0, self).expect("type argument");
                let ty = Type::Rawptr(Box::new(dst));
                let c = self.g.ctype(&ty)?;
                Ok(V { c: format!("(({})1)", c), ty, obj: None })
            }
            "fault" => {
                let ExprKind::Path(segs, _) = &args[0].kind else { return internal("fault's argument") };
                let id = format!("diag.{}", segs[0].replace('_', "-"));
                self.emit(&format!("cb_fault(\"{}\", {});", id, loc));
                Ok(never())
            }
            "drop" => {
                // `drop(e)` of a temporary: the value is destroyed at once
                // (a plain one simply ends).
                if !matches!(args[0].kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) | ExprKind::Paren(..)) {
                    let v = self.lower_expr(&args[0], None)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let t = self.into_temp(v, line)?;
                    if let Some(o) = &t.obj {
                        self.emit(&format!("cb_consume({}, {});", o, loc));
                    }
                    return Ok(unit());
                }
                let p = self.lower_place(&args[0])?;
                if p.raw.is_some() {
                    // `drop(*p)` of raw storage: the value moves out
                    // (`[Rawptr-Move-Out]`) and is destroyed at once.
                    let v = self.read_place(p, line)?;
                    let t = self.into_temp(v, line)?;
                    if let Some(o) = &t.obj {
                        self.emit(&format!("cb_consume({}, {});", o, loc));
                    }
                    return Ok(unit());
                }
                match (&p.base, p.is_binding, p.projs.is_empty()) {
                    (Some(root), true, true) => self.emit(&format!("cb_destroy({}, {});", root, loc)),
                    (Some(tok), false, true) => self.emit(&format!("cb_destroy_via({}, {});", tok, loc)),
                    (Some(_), _, false) => {
                        // `[Destroy-Projection]`: a field, element, or payload.
                        self.emit(&format!("cb_fault(\"diag.move-out-of-field\", {});", loc));
                        return Ok(never());
                    }
                    (None, _, _) => return internal("drop of a place with no path"),
                }
                Ok(unit())
            }
            "reclaim" => internal("reclaim outside place position"),
            "spawn" => self.lower_spawn(args, line),
            "join" => {
                // `[Join]`: the handle is consumed; the result, claimed
                // from the thread's block, is this statement's temporary.
                let hv = self.lower_expr(&args[0], None)?;
                if matches!(hv.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Handle(rty) = hv.ty.clone() else { return internal("join of a non-handle") };
                let rty = *rty;
                let ht = self.into_temp(hv, line)?;
                let ho = ht.obj.clone().expect("a handle has an object");
                let t = self.g.fresh("t");
                let c = self.g.ctype(&rty)?;
                self.emit(&format!("{} {};", c, t));
                let ro = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_join({}, &{});", ro, ht.c, t));
                // `[Handle-Destructor]` finds the result taken: nothing to discard.
                self.emit(&format!("cb_consume({}, {});", ho, loc));
                if matches!(rty, Type::Void) {
                    return Ok(unit());
                }
                let obj = if self.is_res(&rty) {
                    Some(ro)
                } else if self.has_refs(&rty) {
                    let tid = self.tid(&rty)?;
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
                    Some(o)
                } else {
                    None
                };
                Ok(V { c: t, ty: rty, obj })
            }
            "lock" => {
                // `[Lock]`: a guard owning the lock path, held in its
                // storage as a stored reference (`[Repr-Guard]`).
                let mv = self.lower_expr(&args[0], None)?;
                if matches!(mv.ty, Type::Never) {
                    return Ok(never());
                }
                let inner = match &mv.ty {
                    Type::Ref(p, _) => match &**p {
                        Type::Mutex(i) => (**i).clone(),
                        _ => return internal("lock of a non-mutex"),
                    },
                    _ => return internal("lock of a non-reference"),
                };
                let mt = self.into_temp(mv, line)?;
                let gt = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_lock({}.tok, {}.p, {});", gt, mt.c, mt.c, loc));
                let gty = Type::Guard(Box::new(inner));
                let tid = self.tid(&gty)?;
                let g = self.g.fresh("g");
                self.emit(&format!("void *{} = {}.p;", g, mt.c));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, g, tid, loc));
                self.emit(&format!("cb_store_ref(&{}, {});", g, gt));
                Ok(V { c: g, ty: gty, obj: Some(o) })
            }
            "Mutex::new" => {
                // `[Mutex-New]`: `inner` holds the value; the state cells
                // are zero and never read by a rule.
                let hint = match (targ(0, self), expected) {
                    (Some(t), _) => Some(t),
                    (None, Some(Type::Mutex(i))) => Some((**i).clone()),
                    _ => None,
                };
                let v = self.lower_expr(&args[0], hint.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, line)?;
                let inner = v.ty.clone();
                let mty = Type::Mutex(Box::new(inner.clone()));
                let c = self.g.ctype(&mty)?;
                let t = self.g.fresh("t");
                self.emit(&format!("{} {};", c, t));
                self.emit(&format!("{}.state = 0;", t));
                self.store(&format!("{}.inner", t), &inner, &v)?;
                if self.is_res(&inner) {
                    self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource has object")));
                }
                self.register_temp(t, mty, line)
            }
            "std::print" => {
                // `rule.stdlib.print`: `str` and `ref<String, shared>` go to
                // `std`'s two helpers; numbers and `bool` to the runtime.
                let t = self.probe_type(&args[0], None)?;
                let helper = match &t {
                    Type::Str => Some("std::print_str"),
                    Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n == "std::String") => Some("std::print_string"),
                    _ => None,
                };
                if let Some(h) = helper {
                    let f = self.g.items.fns.get(h).cloned().expect("std's print helper");
                    return self.lower_user_call(h, &f, &[], args, None, line, None);
                }
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let call = match &v.ty {
                    Type::Int(t) => {
                        let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                        format!("{{ cb_u128 _p = {}; cb_print_int((uint64_t)(_p >> 64), (uint64_t)_p, {}u); }}", wide, t.signed() as u32)
                    }
                    Type::F64 => format!("cb_print_f64({});", x),
                    Type::F32 => format!("cb_print_f32({});", x),
                    Type::Bool => format!("cb_print_bool({});", x),
                    _ => return internal("print of an unprintable type"),
                };
                self.emit(&call);
                Ok(unit())
            }
            // `rule.stdlib.text` (spec/21 §2d): `std`'s own helpers for
            // `String::append<T>` and `parse<T>`.
            "append_native" => {
                // `str` and `ref<String, shared>` go to `std`'s two helpers,
                // numbers and `bool` to `append_text` (through `text_write`).
                let t = self.probe_type(&args[1], None)?;
                let (h, targs) = match &t {
                    Type::Str => ("std::String::append_str", vec![]),
                    Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n == "std::String") => ("std::String::append_string", vec![]),
                    Type::Named(n, _) if n == "std::StringView" => ("std::String::append_view", vec![]), // D-0053
                    _ => ("std::String::append_text", vec![t.clone()]),
                };
                let f = self.g.items.fns.get(h).cloned().expect("std's append helper");
                self.lower_user_call(h, &f, &targs, args, None, line, None)
            }
            // `foreach` (D-0042): the expansion for the type of the hidden
            // binding the call names first (coby's src/each.rs).
            n if coby::each::NAMES.contains(&n) => {
                let t0 = self.probe_type(&args[0], None)?;
                match coby::each::expand(n, &t0, args, line) {
                    Ok(x) => {
                        // Kept alive: `probes` is keyed by address.
                        let x = std::rc::Rc::new(x);
                        self.kept.push(x.clone());
                        self.lower_expr(&x, expected)
                    }
                    Err(d) => internal(&format!("foreach over {:?}: {}", t0, d)),
                }
            }
            // `swap_places(a, b)` (D-0048): the bytes of the two places
            // exchange, and so do the reference slots' tokens a value with
            // references keeps beside its bytes.
            "swap_places" => {
                let at = self.probe_type(&args[0], None)?;
                let Type::Ref(inner, _) = &at else { return internal("swap of a non-reference") };
                let ty = (**inner).clone();
                let a = self.lower_expr(&args[0], None)?;
                let a = self.into_temp(a, line)?;
                let b = self.lower_expr(&args[1], None)?;
                let b = self.into_temp(b, line)?;
                let c = self.g.ctype(&ty)?;
                let loc = self.loc(line);
                // `[Swap-Places]` needs both places still there; it has no
                // conflict premise (the same place twice is left alone).
                self.emit(&format!("cb_valid(({}).tok, {});", a.c, loc));
                self.emit(&format!("cb_valid(({}).tok, {});", b.c, loc));
                let (pa, pb) = (format!("(({} *)({}).p)", c, a.c), format!("(({} *)({}).p)", c, b.c));
                let t = self.g.fresh("sw");
                self.emit(&format!("if ({} != {}) {{", pa, pb));
                self.emit(&format!("{} {}; memcpy(&{}, {}, sizeof({}));", c, t, t, pa, c));
                if self.has_refs(&ty) {
                    let tid = self.tid(&ty)?;
                    self.emit(&format!("cb_copy_datum(&{}, {}, {}u);", t, pa, tid));
                    self.emit(&format!("memcpy({}, {}, sizeof({})); cb_copy_datum({}, {}, {}u);", pa, pb, c, pa, pb, tid));
                    self.emit(&format!("memcpy({}, &{}, sizeof({})); cb_copy_datum({}, &{}, {}u);", pb, t, c, pb, t, tid));
                } else {
                    self.emit(&format!("memcpy({}, {}, sizeof({}));", pa, pb, c));
                    self.emit(&format!("memcpy({}, &{}, sizeof({}));", pb, t, c));
                }
                self.emit("}");
                Ok(unit())
            }
            // `rule.stdlib.hashmap` (spec/21 §1a, D-0041): a key's hash and
            // equality over its bytes, read through the key's reference.
            "key_hash" | "key_eq" => {
                let mut keys = Vec::new();
                for a in args {
                    keys.push(self.lower_key_bytes(a, line)?);
                }
                let c = if name == "key_hash" {
                    format!("cb_hash_bytes({}, {})", keys[0].0, keys[0].1)
                } else {
                    format!("cb_bytes_eq({}, {}, {}, {})", keys[0].0, keys[0].1, keys[1].0, keys[1].1)
                };
                let ty = if name == "key_hash" { Type::Int(IntTy::U64) } else { Type::Bool };
                Ok(V { c, ty, obj: None })
            }
            "text_write" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let p = self.lower_expr(&args[1], None)?;
                let p = self.into_temp(p, line)?.c;
                let c = match &v.ty {
                    Type::Int(t) => {
                        let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                        format!("cb_text_int((uint64_t)({} >> 64), (uint64_t)({}), {}u, (void *)({}))", wide, wide, t.signed() as u32, p)
                    }
                    Type::F64 => format!("cb_text_f64({}, (void *)({}))", x, p),
                    Type::F32 => format!("cb_text_f32({}, (void *)({}))", x, p),
                    Type::Bool => format!("cb_text_bool({}, (void *)({}))", x, p),
                    _ => return internal("text of an unprintable type"),
                };
                Ok(V { c, ty: Type::Int(IntTy::Usize), obj: None })
            }
            "parse_check" | "parse_value" => {
                let t = targ(0, self).expect("type argument");
                let p = self.lower_expr(&args[0], None)?;
                let p = self.into_temp(p, line)?.c;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let n = self.into_temp(n, line)?.c;
                let check = name == "parse_check";
                match &t {
                    Type::Int(it) => {
                        let head = format!("cb_parse_int((const void *)({}), {}, {}u, {}u", p, n, it.signed() as u32, it.bitwidth(64));
                        if check {
                            return Ok(V { c: format!("{}, 0)", head), ty: Type::Int(IntTy::Isize), obj: None });
                        }
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {}[2];", o));
                        self.emit(&format!("{}, {});", head, o));
                        let c = format!("(({})((((cb_u128){}[0]) << 64) | {}[1]))", self.g.ctype(&t)?, o, o);
                        Ok(V { c, ty: t.clone(), obj: None })
                    }
                    Type::F64 | Type::F32 => {
                        let head = format!("cb_parse_float((const void *)({}), {}, {}u", p, n, matches!(t, Type::F32) as u32);
                        if check {
                            return Ok(V { c: format!("{}, 0)", head), ty: Type::Int(IntTy::Isize), obj: None });
                        }
                        let o = self.g.fresh("o");
                        self.emit(&format!("double {};", o));
                        self.emit(&format!("{}, &{});", head, o));
                        Ok(V { c: format!("(({}){})", self.g.ctype(&t)?, o), ty: t.clone(), obj: None })
                    }
                    _ => internal("parse of a non-numeric type"),
                }
            }
            // `rule.stdlib.format` (D-0038): the text of `printf` and
            // `String::appendf` (`modres` made them `print`/`append` of
            // `$fmt(…)`), in the runtime's buffer until that takes it.
            "$fmt" => {
                let fv = self.lower_expr(&args[0], None)?;
                let fv = self.into_temp(fv, line)?;
                let mut inits = Vec::new();
                for a in &args[1..] {
                    // A reference to a number, `bool` or `str` is formatted
                    // as the value it refers to (D-0042).
                    let at = self.probe_type(a, None)?;
                    let deref_arg;
                    let a = match &at {
                        Type::Ref(inner, _) if matches!(&**inner, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str) => {
                            deref_arg = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line });
                            self.kept.push(deref_arg.clone());
                            &*deref_arg
                        }
                        _ => a,
                    };
                    let v = self.lower_expr(a, None)?;
                    if matches!(v.ty, Type::Never) {
                        return Ok(never());
                    }
                    let v = self.into_temp(v, a.line)?;
                    let x = v.c.clone();
                    inits.push(match &v.ty {
                        Type::Int(t) => {
                            let wide = if t.signed() { format!("(cb_u128)(cb_i128)({})", x) } else { format!("(cb_u128)({})", x) };
                            format!("{{ {}u, {}u, (uint64_t)({} >> 64), (uint64_t)({}), 0.0, 0, 0 }}", t.signed() as u32, t.bitwidth(64), wide, wide)
                        }
                        Type::F64 => format!("{{ 2u, 64u, 0, 0, {}, 0, 0 }}", x),
                        Type::F32 => format!("{{ 2u, 32u, 0, 0, (double)({}), 0, 0 }}", x),
                        Type::Str => format!("{{ 3u, 0u, 0, 0, 0.0, ({}).p, ({}).n }}", x, x),
                        Type::Bool => format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *)(({}) ? \"true\" : \"false\"), ({}) ? 4u : 5u }}", x, x),
                        // D-0053: a view's bytes, read through its slice's reference.
                        Type::Named(n, _) if n == "std::StringView" => {
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(cb_load_ref(&({}).bytes.src), NULL, 0, {});", x, loc));
                            format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *)({}).bytes.data, ({}).bytes.len }}", x, x)
                        }
                        Type::Ref(..) => {
                            // A `&String`: its bytes, read through the reference.
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", x, loc));
                            let sc = self.g.ctype(&Type::Named("std::String".into(), vec![]))?;
                            format!("{{ 3u, 0u, 0, 0, 0.0, (({} *)({}).p)->bytes.ptr, (({} *)({}).p)->bytes.len }}", sc, x, sc, x)
                        }
                        _ => return internal("a format argument of an unformattable type"),
                    });
                }
                let arr = self.g.fresh("fa");
                let out = self.g.fresh("fs");
                if inits.is_empty() {
                    self.emit(&format!("cb_str {}; cb_format(({}).p, ({}).n, NULL, 0, &{});", out, fv.c, fv.c, out));
                } else {
                    self.emit(&format!("cb_fmt_arg {}[] = {{ {} }};", arr, inits.join(", ")));
                    self.emit(&format!("cb_str {}; cb_format(({}).p, ({}).n, {}, {}, &{});", out, fv.c, fv.c, arr, inits.len(), out));
                }
                Ok(V { c: out, ty: Type::Str, obj: None })
            }
            "std::map_err" => {
                // `map_err(r, f)`: `Ok(v)` passes through, `Err(e)` becomes
                // `Err(f(e))`; the source's contents relocate either way.
                let rv = self.lower_expr(&args[0], None)?;
                if matches!(rv.ty, Type::Never) {
                    return Ok(never());
                }
                let rt = self.into_temp(rv, line)?;
                let (t_ty, e_ty) = match &rt.ty {
                    Type::Named(n, a) if n == "std::Result" && a.len() == 2 => (a[0].clone(), a[1].clone()),
                    _ => return internal("map_err on a non-Result"),
                };
                let fv = self.lower_expr(&args[1], None)?;
                let e2 = match &fv.ty {
                    Type::Fn(_, r) => (**r).clone(),
                    Type::Closure(id) => self.g.closures[id].ret.clone(),
                    _ => return internal("map_err's function"),
                };
                let out_ty = Type::Named("std::Result".into(), vec![t_ty.clone(), e2.clone()]);
                let oc = self.g.ctype(&out_ty)?;
                let o = self.g.fresh("me");
                self.emit(&format!("{} {};", oc, o));
                self.emit(&format!("if ({}.tag == 0u) {{", rt.c));
                self.ind += 1;
                self.emit(&format!("{}.tag = 0u;", o));
                let pv = V { c: format!("{}.u.v0", rt.c), ty: t_ty.clone(), obj: None };
                let pv = if matches!(t_ty, Type::Ref(..)) { self.read_ref_slot(&pv.c, &t_ty) } else { pv };
                self.store(&format!("{}.u.v0", o), &t_ty, &pv)?;
                self.ind -= 1;
                self.emit("} else {");
                self.ind += 1;
                // The Err payload becomes the call's argument.
                let ev = V { c: format!("{}.u.v1", rt.c), ty: e_ty.clone(), obj: None };
                let ev = if matches!(e_ty, Type::Ref(..)) { self.read_ref_slot(&ev.c, &e_ty) } else { ev };
                let ev = if self.is_res(&e_ty) {
                    let t = self.g.fresh("t");
                    let c = self.g.ctype(&e_ty)?;
                    self.emit(&format!("{} {} = {};", c, t, ev.c));
                    let tid = self.tid(&e_ty)?;
                    let oo = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", oo, t, tid, loc));
                    V { c: t, ty: e_ty.clone(), obj: Some(oo) }
                } else {
                    self.into_temp(ev, line)?
                };
                if self.is_res(&e_ty) {
                    self.emit(&format!("cb_send({});", ev.obj.as_ref().unwrap()));
                } else if self.has_refs(&e_ty) && !matches!(e_ty, Type::Ref(..)) {
                    let tid = self.tid(&e_ty)?;
                    self.emit(&format!("cb_send_datum(&{}, {}u);", ev.c, tid));
                }
                let res = match &fv.ty {
                    Type::Fn(..) => {
                        let fh = self.into_temp(fv.clone(), line)?;
                        let thunk = self.g.thunk(&fv.ty)?;
                        self.finish_call(format!("{}({}, {})", thunk, fh.c, ev.c), e2.clone(), line)?
                    }
                    Type::Closure(id) => {
                        let id = *id;
                        let info = self.g.closures[&id].clone();
                        let Some(co) = &fv.obj else { return internal("closure without an object") };
                        let root = self.g.fresh("root");
                        self.emit(&format!("uint64_t {} = cb_bind({});", root, co));
                        let tok = self.g.fresh("tok");
                        self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, CB_EXCLUSIVE, {});", tok, root, loc));
                        let self_ref = format!("((cb_ref){{ (void *)&({}), {} }})", fv.c, tok);
                        self.finish_call(format!("{}({}, {})", info.cname, self_ref, ev.c), e2.clone(), line)?
                    }
                    _ => unreachable!(),
                };
                let res = self.into_temp(res, line)?;
                self.emit(&format!("{}.tag = 1u;", o));
                self.store(&format!("{}.u.v1", o), &e2, &res)?;
                if self.is_res(&e2) {
                    self.emit(&format!("cb_absorb({});", res.obj.as_ref().expect("resource result has object")));
                }
                self.ind -= 1;
                self.emit("}");
                if let Some(ro) = &rt.obj {
                    self.emit(&format!("cb_end_moved_out({});", ro));
                }
                self.register_temp(o, out_ty, line)
            }
            _ => internal(&format!("unknown callee `{}`", name)),
        }
    }

    fn lower_operands(&mut self, l: &Expr, r: &Expr, expected: Option<&Type>) -> R<(Option<V>, Option<V>)> {
        let exp = match expected {
            Some(Type::Named(n, a)) if n == "std::Option" && a.len() == 1 => Some(a[0].clone()),
            Some(t) => Some(t.clone()),
            None => None,
        };
        if is_bare_literal(l) && !is_bare_literal(r) {
            let rv = self.lower_expr(r, exp.as_ref())?;
            if matches!(rv.ty, Type::Never) {
                return Ok((None, None));
            }
            let rv = self.into_temp(rv, r.line)?;
            let lv = self.lower_expr(l, Some(&rv.ty))?;
            let lv = self.into_temp(lv, l.line)?;
            Ok((Some(lv), Some(rv)))
        } else {
            let lv = self.lower_expr(l, exp.as_ref())?;
            if matches!(lv.ty, Type::Never) {
                return Ok((None, None));
            }
            let lv = self.into_temp(lv, l.line)?;
            let rv = self.lower_expr(r, Some(&lv.ty))?;
            if matches!(rv.ty, Type::Never) {
                return Ok((None, None));
            }
            let rv = self.into_temp(rv, r.line)?;
            Ok((Some(lv), Some(rv)))
        }
    }
}

// The C name of a function instance: `f_` and the sanitized path, then
// the mangled type arguments of a generic one.
fn fn_cname(name: &str, targs: &[Type]) -> String {
    let mut c = format!("f_{}", san(name));
    if !targs.is_empty() {
        c.push_str("_L");
        for t in targs {
            c.push_str(&mangle(t));
            c.push('_');
        }
        c.push('R');
    }
    c
}

// A declaration from the prelude's own source (a program may declare
// its own function of the same name), known by its body's first line.
fn in_prelude(f: &FnDecl) -> bool {
    let line = match f.body.stmts.first() {
        Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) | Some(Stmt::Let { init: Some(e), .. }) => Some(e.line),
        Some(_) => None,
        None => f.body.tail.as_ref().map(|e| e.line),
    };
    line.map_or(false, |l| l <= prelude::line_count())
}

fn stmt_line(s: &Stmt) -> Option<usize> {
    match s {
        Stmt::Let { init: Some(e), .. } => Some(e.line),
        Stmt::Let { .. } => None,
        Stmt::Destructure { init, .. } => Some(init.line),
        Stmt::Expr(e) | Stmt::BlockLike(e) => Some(e.line),
    }
}

fn id_of(g: &Gen, t: &Type) -> u32 {
    g.type_ids[t]
}

// Stage 3 (`COBC-PLAN.md` §10, T1): which statement scopes and frames
// a function body needs. Each one was emitted as a marker line: `F`/`S`
// its push, `f`/`s` its pop, `g`/`t` a pop made by a jump's unwind, `e`
// the end of its extent after a jump already left it. Walking the text
// keeps the stack of those open at each line, which is the runtime's
// own stack there (the code is structured). A scope or frame is kept
// only if some runtime call made while it is open could record
// something in it:
//
// - into the innermost statement scope (`top_scope`, which looks past
//   frames): temporaries (`cb_new`, `cb_recv`, `cb_take`) and derived
//   paths (`cb_borrow`, `cb_recv_ref`, and the `…_pre` element access,
//   which stamps its path into its caller's scope);
// - into the innermost frame (`top_frame`): `cb_bind`;
// - nothing: checks, slot bookkeeping and sends (`cb_read`, `cb_write`,
//   `cb_load_ref`, `cb_store_ref`, `cb_copy_datum`, `cb_move_to`,
//   `cb_send…`, `cb_borrow_check`, …) and calls of generated functions,
//   whose own scopes hold what they form — a result they hand back
//   arrives through a `cb_recv…` here;
// - any other runtime call keeps every scope and frame open around it.
//
// One that nothing records into would end nothing at `[Stmt-Exit]` or
// `[Block-Exit]`, so its push and every pop of it are dropped.
// The executing statement's location (`cb_at`) is read only by a fault
// raised without a location of its own (`CB_NOLOC`), and inside the
// runtime (pops, destructors, the checks it faults from). A `cb_at` is
// dropped when, in straight-line code, the next line to set it comes
// before any line that could read it: a call of anything but the pure
// helpers below, or a `cb_fault`/`cb_index_check` without a location of
// its own. Any brace or jump ends the straight line, and keeps it.
fn drop_dead_locations(out: &str) -> String {
    const PURE: &[&str] = &["cb_at", "cb_str_eq", "if", "while", "for", "sizeof", "offsetof", "__builtin_expect", "__builtin_add_overflow", "__builtin_sub_overflow", "__builtin_mul_overflow"];
    fn reads_location(line: &str) -> bool {
        let b = line.as_bytes();
        let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        while i < b.len() {
            if !is_id(b[i]) || (i > 0 && is_id(b[i - 1])) || b[i].is_ascii_digit() {
                i += 1;
                continue;
            }
            let mut e = i;
            while e < b.len() && is_id(b[e]) {
                e += 1;
            }
            let id = &line[i..e];
            let mut k = e;
            while k < b.len() && b[k] == b' ' {
                k += 1;
            }
            if b.get(k) == Some(&b'(') && !PURE.contains(&id) {
                if id != "cb_fault" && id != "cb_index_check" {
                    return true;
                }
                // Its own location, unless that is `CB_NOLOC`: up to the
                // call's closing parenthesis.
                let mut depth = 0;
                let mut j = k;
                while j < b.len() {
                    match b[j] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                if line[k..j.min(b.len())].contains("CB_NOLOC") {
                    return true;
                }
            }
            i = e;
        }
        false
    }
    fn boundary(t: &str) -> bool {
        t.ends_with('{') || t.starts_with('}') || t.starts_with("break") || t.starts_with("continue") || t.starts_with("return") || t.starts_with("__builtin_unreachable") || t.ends_with(':')
    }
    let lines: Vec<&str> = out.lines().collect();
    let mut res = String::with_capacity(out.len());
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("cb_at(") {
            let mut dead = false;
            for next in &lines[i + 1..] {
                let t = next.trim_start();
                if t.starts_with("cb_at(") {
                    dead = true;
                    break;
                }
                if boundary(t) || reads_location(t) {
                    break;
                }
            }
            if dead {
                continue;
            }
        }
        res.push_str(line);
        res.push('\n');
    }
    res
}

fn resolve_scopes(out: &str) -> String {
    enum Eff {
        Scope,
        Frame,
        All,
    }
    const FREE: &[&str] = &[
        "cb_fault", "cb_index_check", "cb_str_eq", "cb_write_out", "cb_write_err", "cb_hash_bytes", "cb_bytes_eq", "cb_read_in", "cb_arg_bytes", "cb_set_args", "cb_text_int", "cb_text_f64", "cb_text_f32", "cb_text_bool", "cb_parse_int", "cb_parse_float", "cb_frame_here", "cb_file_read", "cb_file_write", "cb_file_op", "cb_file_at", "cb_format", "cb_at", "cb_unit", "cb_str", "cb_i128", "cb_u128", "cb_ref", "cb_proj",
        "cb_read", "cb_write", "cb_load_ref", "cb_store_ref", "cb_copy_datum", "cb_move_to", "cb_send", "cb_send_ref", "cb_send_datum", "cb_borrow_check", "cb_elem_access",
    ];
    const SCOPE: &[&str] = &["cb_new", "cb_new_uninit", "cb_recv", "cb_recv_ref", "cb_take", "cb_borrow"];
    fn effects(line: &str, out: &mut Vec<Eff>) {
        let b = line.as_bytes();
        let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        while i < b.len() {
            if !is_id(b[i]) || (i > 0 && is_id(b[i - 1])) {
                i += 1;
                continue;
            }
            let mut e = i;
            while e < b.len() && is_id(b[e]) {
                e += 1;
            }
            let id = &line[i..e];
            if id.starts_with("cb_") {
                if SCOPE.contains(&id) {
                    out.push(Eff::Scope);
                } else if id == "cb_bind" {
                    out.push(Eff::Frame);
                } else if !FREE.contains(&id) {
                    out.push(Eff::All);
                }
            } else if id.ends_with("_pre") && b.get(e) == Some(&b'(') {
                out.push(Eff::Scope);
            }
            i = e;
        }
    }
    fn marker(line: &str) -> Option<(char, u32)> {
        let m = line.trim_start().strip_prefix('\u{1}')?;
        let k = m.chars().next()?;
        Some((k, m[1..].parse().expect("scope marker id")))
    }

    let mut open: Vec<(bool, u32)> = Vec::new();
    let mut used: HashSet<u32> = HashSet::new();
    let mut effs = Vec::new();
    for line in out.lines() {
        match marker(line) {
            Some(('F', id)) => open.push((true, id)),
            Some(('S', id)) => open.push((false, id)),
            Some(('f' | 's' | 'e', id)) => {
                let top = open.pop();
                assert_eq!(top.map(|t| t.1), Some(id), "cobc: scope markers out of order");
            }
            Some(_) => {}
            None => {
                effs.clear();
                effects(line, &mut effs);
                for e in &effs {
                    match e {
                        Eff::Scope => used.extend(open.iter().rev().find(|t| !t.0).map(|t| t.1)),
                        Eff::Frame => used.extend(open.iter().rev().find(|t| t.0).map(|t| t.1)),
                        Eff::All => used.extend(open.iter().map(|t| t.1)),
                    }
                }
            }
        }
    }
    assert!(open.is_empty(), "cobc: scope markers left open");

    let mut res = String::with_capacity(out.len());
    for line in out.lines() {
        match marker(line) {
            Some((k, id)) => {
                let call = match k {
                    'F' => "cb_frame_push();",
                    'S' => "cb_stmt_push();",
                    'f' | 'g' => "cb_frame_pop();",
                    's' | 't' => "cb_stmt_pop();",
                    _ => continue,
                };
                if used.contains(&id) {
                    let ind = &line[..line.len() - line.trim_start().len()];
                    res.push_str(ind);
                    res.push_str(call);
                    res.push('\n');
                }
            }
            None => {
                res.push_str(line);
                res.push('\n');
            }
        }
    }
    res
}

// Every expression in `b`, closures' bodies included.
fn visit_exprs(b: &Block, f: &mut dyn FnMut(&Expr)) {
    fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
        f(e);
        match &e.kind {
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => walk(a, f),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                walk(a, f);
                walk(b, f);
            }
            ExprKind::SliceOf(_, a, lo, hi) => {
                walk(a, f);
                walk(lo, f);
                walk(hi, f);
            }
            ExprKind::Call(c, args) => {
                walk(c, f);
                args.iter().for_each(|a| walk(a, f));
            }
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| walk(a, f)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|a| walk(a, f)),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => visit_exprs(b, f),
            ExprKind::If(c, t, e2) => {
                walk(c, f);
                visit_exprs(t, f);
                if let Some(e2) = e2 {
                    walk(e2, f);
                }
            }
            ExprKind::While(c, b, step) => {
                walk(c, f);
                visit_exprs(b, f);
                if let Some(st) = step {
                    walk(st, f);
                }
            }
            ExprKind::Match(sc, arms) => {
                walk(sc, f);
                arms.iter().for_each(|a| walk(&a.body, f));
            }
            ExprKind::Return(Some(a)) => walk(a, f),
            ExprKind::Closure { body, .. } => visit_exprs(body, f),
            _ => {}
        }
    }
    for st in &b.stmts {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => walk(e, f),
            Stmt::Destructure { init, .. } => walk(init, f),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        walk(t, f);
    }
}

// A loop `while (x < E) body` (or a `for` with that condition and a
// `step`), where `x` is a local whose every access is proven
// (`unchecked`, as `plain` says), and the only write to `x` in `body` and
// `step` is one `x = x + 1` outside any nested loop, and nothing there
// declares another `x`: at that write `x` is unchanged since the test,
// so `x < E <= max`, and `x + 1` cannot overflow. The `+`'s address, for
// `lower_binary` to leave unchecked.
fn loop_increment(c: &Expr, body: &Block, step: Option<&Expr>, plain: impl Fn(&str) -> bool) -> Option<usize> {
    let ExprKind::Binary(BinOp::Lt, a, _) = &c.kind else { return None };
    let ExprKind::Path(segs, _) = &a.kind else { return None };
    let [x] = segs.as_slice() else { return None };
    if !plain(x) {
        return None;
    }
    // Every write to `x`, with whether it is inside a nested loop; and
    // whether anything declares a name `x`.
    struct Scan<'a> {
        x: &'a str,
        writes: Vec<(&'a Expr, bool)>,
        declares: bool,
    }
    impl<'a> Scan<'a> {
        fn expr(&mut self, e: &'a Expr, nested: bool) {
            match &e.kind {
                ExprKind::Assign(l, r) => {
                    if matches!(&l.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == self.x) {
                        self.writes.push((e, nested));
                    }
                    self.expr(l, nested);
                    self.expr(r, nested);
                }
                ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => self.expr(a, nested),
                ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                    self.expr(a, nested);
                    self.expr(b, nested);
                }
                ExprKind::SliceOf(_, a, lo, hi) => {
                    self.expr(a, nested);
                    self.expr(lo, nested);
                    self.expr(hi, nested);
                }
                ExprKind::Call(c, args) => {
                    self.expr(c, nested);
                    args.iter().for_each(|a| self.expr(a, nested));
                }
                ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| self.expr(a, nested)),
                ExprKind::ArrayLit(es) => es.iter().for_each(|a| self.expr(a, nested)),
                ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b, nested),
                ExprKind::If(c, t, f) => {
                    self.expr(c, nested);
                    self.block(t, nested);
                    if let Some(f) = f {
                        self.expr(f, nested);
                    }
                }
                ExprKind::While(c, b, st) => {
                    self.expr(c, true);
                    self.block(b, true);
                    if let Some(st) = st {
                        self.expr(st, true);
                    }
                }
                ExprKind::Match(sc, arms) => {
                    self.expr(sc, nested);
                    for a in arms {
                        if a.binder.as_deref() == Some(self.x) {
                            self.declares = true;
                        }
                        self.expr(&a.body, nested);
                    }
                }
                ExprKind::Return(Some(a)) => self.expr(a, nested),
                ExprKind::Closure { params, body, .. } => {
                    if params.iter().any(|p| p.name == self.x) {
                        self.declares = true;
                    }
                    self.block(body, true);
                }
                _ => {}
            }
        }
        fn block(&mut self, b: &'a Block, nested: bool) {
            for st in &b.stmts {
                match st {
                    Stmt::Let { name, init, .. } => {
                        if name == self.x {
                            self.declares = true;
                        }
                        if let Some(e) = init {
                            self.expr(e, nested);
                        }
                    }
                    Stmt::Destructure { fields, init, .. } => {
                        if fields.iter().any(|f| f == self.x) {
                            self.declares = true;
                        }
                        self.expr(init, nested);
                    }
                    Stmt::Expr(e) | Stmt::BlockLike(e) => self.expr(e, nested),
                }
            }
            if let Some(t) = &b.tail {
                self.expr(t, nested);
            }
        }
    }
    let mut scan = Scan { x, writes: Vec::new(), declares: false };
    scan.block(body, false);
    if let Some(st) = step {
        scan.expr(st, false);
    }
    let ([(w, false)], false) = (scan.writes.as_slice(), scan.declares) else { return None };
    let ExprKind::Assign(_, rhs) = &w.kind else { return None };
    let ExprKind::Binary(BinOp::Add, p, one) = &rhs.kind else { return None };
    let is_x = matches!(&p.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == *x);
    let is_one = matches!(&one.kind, ExprKind::IntLit(1, _));
    (is_x && is_one).then(|| &**rhs as *const Expr as usize)
}

// The names a body assigns to as a whole (`x = …`): a binding of one of
// them may be given a new value after its old one moved away or ended
// (D-0033 (1)), so its frame is recorded (`Fx::note_frame`). By name,
// ignoring scopes, which only records more than needed.
fn assigned_names(b: &Block) -> HashSet<String> {
    let mut out = HashSet::new();
    visit_exprs(b, &mut |e| {
        if let ExprKind::Assign(l, _) = &e.kind {
            if let ExprKind::Path(segs, _) = &l.kind {
                if segs.len() == 1 {
                    out.insert(segs[0].clone());
                }
            }
        }
    });
    out
}

// The names a body borrows (`&x…`, `&mut x…`, including `rawptr_of(&x)`),
// captures (a closure's capture list), or drops (`drop(x…)`): the
// bindings whose checks `rule.control.flow-analysis` may not prove, so
// they keep their runtime objects. By name, ignoring scopes, which only
// errs toward checking. A binding missed here would fail loudly: a
// borrow or drop of a place with no path is an internal error.
fn pinned_names(b: &Block) -> HashSet<String> {
    // The binding whose own storage a place is part of. Through `*` the
    // place is the referent's, not the reference variable's: `&*r`
    // borrows what `r` points to, and leaves `r` itself unborrowed.
    fn root(e: &Expr) -> Option<&str> {
        match &e.kind {
            ExprKind::Path(segs, _) if segs.len() == 1 => Some(&segs[0]),
            ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Paren(b) => root(b),
            _ => None,
        }
    }
    fn expr(e: &Expr, out: &mut HashSet<String>) {
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Break | ExprKind::Continue | ExprKind::Dollar => {}
            ExprKind::Borrow(_, inner) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out);
            }
            ExprKind::SliceOf(_, inner, lo, hi) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out);
                expr(lo, out);
                expr(hi, out);
            }
            ExprKind::Call(c, args) => {
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(n) = args.first().and_then(root) {
                            out.insert(n.to_string());
                        }
                    }
                }
                expr(c, out);
                args.iter().for_each(|a| expr(a, out));
            }
            ExprKind::Closure { captures, body, .. } => {
                out.extend(captures.iter().cloned());
                block(body, out);
            }
            ExprKind::StructLit(_, _, fields) => fields.iter().for_each(|(_, f)| expr(f, out)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|x| expr(x, out)),
            ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => expr(a, out),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                expr(a, out);
                expr(b, out);
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, out),
            ExprKind::If(c, t, f) => {
                expr(c, out);
                block(t, out);
                if let Some(f) = f {
                    expr(f, out);
                }
            }
            ExprKind::While(c, b, step) => {
                expr(c, out);
                block(b, out);
                if let Some(st) = step {
                    expr(st, out);
                }
            }
            ExprKind::Match(s, arms) => {
                expr(s, out);
                arms.iter().for_each(|a| expr(&a.body, out));
            }
            ExprKind::Return(r) => {
                if let Some(r) = r {
                    expr(r, out);
                }
            }
        }
    }
    fn block(b: &Block, out: &mut HashSet<String>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { init, .. } => {
                    if let Some(e) = init {
                        expr(e, out);
                    }
                }
                Stmt::Destructure { init, .. } => expr(init, out),
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, out),
            }
        }
        if let Some(t) = &b.tail {
            expr(t, out);
        }
    }
    let mut out = HashSet::new();
    block(b, &mut out);
    out
}

// `spec/15` §6: a capture is exclusive iff the body writes, exclusively
// borrows, drops, or moves a place rooted at it (the interpreter's
// `closure_body_writes`).
fn body_writes(body: &Block, name: &str) -> bool {
    fn root_is(e: &Expr, name: &str) -> bool {
        match &e.kind {
            ExprKind::Path(segs, _) => segs.len() == 1 && segs[0] == name,
            ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Deref(b) => root_is(b, name),
            _ => false,
        }
    }
    fn scan(e: &Expr, name: &str, found: &mut bool) {
        match &e.kind {
            ExprKind::Assign(l, r) => {
                if root_is(l, name) {
                    *found = true;
                }
                scan(l, name, found);
                scan(r, name, found);
            }
            ExprKind::Borrow(Mode::Exclusive, inner) => {
                if root_is(inner, name) {
                    *found = true;
                }
                scan(inner, name, found);
            }
            ExprKind::SliceOf(m, inner, lo, hi) => {
                if *m == Mode::Exclusive && root_is(inner, name) {
                    *found = true;
                }
                scan(inner, name, found);
                scan(lo, name, found);
                scan(hi, name, found);
            }
            ExprKind::Borrow(_, inner) | ExprKind::Unary(_, inner) | ExprKind::Deref(inner) | ExprKind::Field(inner, _) | ExprKind::Paren(inner) | ExprKind::Propagate(inner) => scan(inner, name, found),
            ExprKind::Call(c, args) => {
                scan(c, name, found);
                for a in args {
                    scan(a, name, found);
                }
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(a0) = args.first() {
                            if root_is(a0, name) {
                                *found = true;
                            }
                        }
                    }
                }
            }
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                scan(a, name, found);
                scan(b, name, found);
            }
            ExprKind::StructLit(_, _, fields) => {
                for (_, f) in fields {
                    scan(f, name, found);
                }
            }
            ExprKind::ArrayLit(es) => {
                for x in es {
                    scan(x, name, found);
                }
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => scan_block(b, name, found),
            ExprKind::If(c, t, e) => {
                scan(c, name, found);
                scan_block(t, name, found);
                if let Some(e) = e {
                    scan(e, name, found);
                }
            }
            ExprKind::While(c, b, step) => {
                scan(c, name, found);
                scan_block(b, name, found);
                if let Some(st) = step {
                    scan(st, name, found);
                }
            }
            ExprKind::Match(s, arms) => {
                scan(s, name, found);
                for a in arms {
                    scan(&a.body, name, found);
                }
            }
            ExprKind::Return(Some(x)) => scan(x, name, found),
            ExprKind::Closure { body, .. } => scan_block(body, name, found),
            _ => {}
        }
    }
    fn scan_block(b: &Block, name: &str, found: &mut bool) {
        for s in &b.stmts {
            match s {
                Stmt::Expr(e) | Stmt::BlockLike(e) => scan(e, name, found),
                Stmt::Let { init: Some(e), .. } => scan(e, name, found),
                Stmt::Destructure { init, .. } => scan(init, name, found),
                _ => {}
            }
        }
        if let Some(t) = &b.tail {
            scan(t, name, found);
        }
    }
    let mut found = false;
    scan_block(body, name, &mut found);
    found
}

// ---- types and names ----

fn subst_of(params: &[String], args: &[Type]) -> HashMap<String, Type> {
    params.iter().cloned().zip(args.iter().cloned()).collect()
}

fn apply(t: &Type, s: &HashMap<String, Type>) -> Type {
    match t {
        Type::Named(n, args) if args.is_empty() && s.contains_key(n) => s[n].clone(),
        Type::Named(n, args) => Type::Named(n.clone(), args.iter().map(|a| apply(a, s)).collect()),
        Type::Ref(i, m) => Type::Ref(Box::new(apply(i, s)), m.clone()),
        Type::Slice(i, m) => Type::Slice(Box::new(apply(i, s)), m.clone()),
        Type::Rawptr(i) => Type::Rawptr(Box::new(apply(i, s))),
        Type::Array(i, n) => Type::Array(Box::new(apply(i, s)), *n),
        Type::Fn(ps, r) => Type::Fn(ps.iter().map(|p| apply(p, s)).collect(), Box::new(apply(r, s))),
        Type::Handle(i) => Type::Handle(Box::new(apply(i, s))),
        Type::Mutex(i) => Type::Mutex(Box::new(apply(i, s))),
        Type::Guard(i) => Type::Guard(Box::new(apply(i, s))),
        other => other.clone(),
    }
}

fn resolved(t: &Type, params: &[String]) -> bool {
    match t {
        Type::Named(n, args) => !(args.is_empty() && params.contains(n)) && args.iter().all(|a| resolved(a, params)),
        Type::Ref(i, _) | Type::Slice(i, _) | Type::Rawptr(i) | Type::Array(i, _) | Type::Handle(i) | Type::Mutex(i) | Type::Guard(i) => resolved(i, params),
        Type::Fn(ps, r) => ps.iter().all(|p| resolved(p, params)) && resolved(r, params),
        _ => true,
    }
}

fn unify(pattern: &Type, actual: &Type, params: &[String], m: &mut HashMap<String, Type>) {
    match (pattern, actual) {
        (Type::Named(n, args), _) if args.is_empty() && params.contains(n) => {
            m.entry(n.clone()).or_insert_with(|| actual.clone());
        }
        (Type::Named(n1, a1), Type::Named(n2, a2)) if n1 == n2 && a1.len() == a2.len() => {
            for (p, a) in a1.iter().zip(a2.iter()) {
                unify(p, a, params, m);
            }
        }
        (Type::Ref(p, _), Type::Ref(a, _)) | (Type::Slice(p, _), Type::Slice(a, _)) | (Type::Rawptr(p), Type::Rawptr(a)) | (Type::Array(p, _), Type::Array(a, _)) | (Type::Handle(p), Type::Handle(a)) | (Type::Mutex(p), Type::Mutex(a)) | (Type::Guard(p), Type::Guard(a)) => unify(p, a, params, m),
        (Type::Fn(ps, r), Type::Fn(as_, ar)) if ps.len() == as_.len() => {
            for (p, a) in ps.iter().zip(as_.iter()) {
                unify(p, a, params, m);
            }
            unify(r, ar, params, m);
        }
        _ => {}
    }
}

fn is_bare_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None)),
        ExprKind::Paren(inner) => is_bare_literal(inner),
        _ => false,
    }
}

fn int_lit_type(suf: Option<&str>, expected: Option<&Type>) -> IntTy {
    if let Some(t) = suf.and_then(IntTy::from_str) {
        t
    } else if let Some(Type::Int(t)) = expected {
        *t
    } else {
        IntTy::I32
    }
}

// A type's `min`/`max` as the `i128` payload `c_int` expects: for the
// 128-bit types that is the bit pattern (`u128::MAX` is all ones).
fn bounds(t: IntTy) -> (i128, i128) {
    match t {
        IntTy::U128 => (0, -1),
        IntTy::I128 => (i128::MIN, i128::MAX),
        _ => int_min_max(t),
    }
}

// Every value of `s` is a value of `d`: a `narrow` from `s` to `d`
// cannot fault.
fn int_range_within(s: IntTy, d: IntTy) -> bool {
    let (sb, db) = (s.bitwidth(64), d.bitwidth(64));
    match (s.signed(), d.signed()) {
        (false, false) | (true, true) => sb <= db,
        (false, true) => sb < db,
        (true, false) => false,
    }
}

fn round_up(x: u64, a: u64) -> u64 {
    if a <= 1 {
        x
    } else {
        (x + a - 1) / a * a
    }
}

pub fn san(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn mangle(t: &Type) -> String {
    match t {
        Type::Int(i) => i.name().to_string(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "bool".into(),
        Type::Str => "str".into(),
        Type::Void => "void".into(),
        Type::Never => "never".into(),
        Type::Ref(i, m) => format!("ref{}_{}", if *m == Mode::Shared { "s" } else { "x" }, mangle(i)),
        Type::Slice(i, m) => format!("slice{}_{}", if *m == Mode::Shared { "s" } else { "x" }, mangle(i)),
        Type::Rawptr(i) => format!("ptr_{}", mangle(i)),
        Type::Array(i, n) => format!("arr{}_{}", n, mangle(i)),
        Type::Fn(ps, r) => format!("fn{}_{}", ps.iter().map(mangle).collect::<Vec<_>>().join("_"), mangle(r)),
        Type::Handle(i) => format!("handle_{}", mangle(i)),
        Type::Mutex(i) => format!("mutex_{}", mangle(i)),
        Type::Guard(i) => format!("guard_{}", mangle(i)),
        Type::Named(n, args) => {
            if args.is_empty() {
                san(n)
            } else {
                format!("{}_L{}_R", san(n), args.iter().map(mangle).collect::<Vec<_>>().join("_"))
            }
        }
        Type::Closure(id) => format!("closure{}", id),
    }
}

fn c_int_type(t: IntTy) -> &'static str {
    match t {
        IntTy::I8 => "int8_t",
        IntTy::I16 => "int16_t",
        IntTy::I32 => "int32_t",
        IntTy::I64 | IntTy::Isize => "int64_t",
        IntTy::I128 => "cb_i128",
        IntTy::U8 => "uint8_t",
        IntTy::U16 => "uint16_t",
        IntTy::U32 => "uint32_t",
        IntTy::U64 | IntTy::Usize => "uint64_t",
        IntTy::U128 => "cb_u128",
    }
}

fn c_uint_type(t: IntTy) -> &'static str {
    c_uint_of_width(t.bitwidth(64))
}

fn c_uint_of_width(bw: u32) -> &'static str {
    match bw {
        8 => "uint8_t",
        16 => "uint16_t",
        32 => "uint32_t",
        64 => "uint64_t",
        _ => "cb_u128",
    }
}

fn c_sint_of_width(bw: u32) -> &'static str {
    match bw {
        8 => "int8_t",
        16 => "int16_t",
        32 => "int32_t",
        64 => "int64_t",
        _ => "cb_i128",
    }
}

fn c_int(t: IntTy, v: i128) -> String {
    let bits = v as u128;
    let hi = (bits >> 64) as u64;
    let lo = bits as u64;
    if hi == 0 && !matches!(t, IntTy::I128 | IntTy::U128) {
        format!("(({}){}ULL)", c_int_type(t), lo)
    } else {
        format!("(({})(((cb_u128){}ULL << 64) | (cb_u128){}ULL))", c_int_type(t), hi, lo)
    }
}

fn c_float(f: f64, ty: &Type) -> String {
    let mut s = format!("{:?}", f);
    if !s.contains('.') && !s.contains('e') && !s.contains("inf") && !s.contains("NaN") {
        s.push_str(".0");
    }
    if matches!(ty, Type::F32) {
        format!("(float){}", s)
    } else {
        s
    }
}

fn narrow_check(s: IntTy, d: IntTy, x: &str, loc: &str) -> String {
    let dt = c_int_type(d);
    let cond = if matches!(d, IntTy::U128) {
        if s.signed() { format!("{} >= 0", x) } else { "1".to_string() }
    } else if matches!(s, IntTy::U128) {
        let (_, max) = bounds(d);
        format!("(cb_u128){} <= (cb_u128){}", x, c_int(IntTy::U128, max))
    } else {
        let (min, max) = bounds(d);
        format!("(cb_i128){x} >= {} && (cb_i128){x} <= {}", c_int(IntTy::I128, min), c_int(IntTy::I128, max), x = x)
    };
    format!("({{ if (!({})) cb_fault(\"diag.narrowing-overflow\", {}); ({}){}; }})", cond, loc, dt, x)
}

fn to_int_check(d: IntTy, x: &str, loc: &str) -> String {
    let dt = c_int_type(d);
    let range = match d {
        IntTy::U128 => "_t >= 0.0 && _t < 340282366920938463463374607431768211456.0".to_string(),
        IntTy::I128 => "_t >= -170141183460469231731687303715884105728.0 && _t < 170141183460469231731687303715884105728.0".to_string(),
        _ => {
            let (min, max) = bounds(d);
            // The bounds as typed constants: a bare `-9223372036854775808`
            // in C is a minus applied to a literal that fits no signed
            // type, which GCC makes 128-bit and Clang makes unsigned
            // (so +2^63), and the lower-bound test then failed under Clang.
            format!(
                "_t >= (double){} && _t <= (double){} && (cb_i128)_t >= {} && (cb_i128)_t <= {}",
                c_int(d, min),
                c_int(d, max),
                c_int(IntTy::I128, min),
                c_int(IntTy::I128, max)
            )
        }
    };
    format!(
        "({{ double _f = (double){}; if (!isfinite(_f)) cb_fault(\"diag.narrowing-overflow\", {loc}); double _t = trunc(_f); if (!({})) cb_fault(\"diag.narrowing-overflow\", {loc}); ({})_t; }})",
        x, range, dt, loc = loc
    )
}

fn c_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn strip_parens(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Paren(inner) => strip_parens(inner),
        _ => e,
    }
}
