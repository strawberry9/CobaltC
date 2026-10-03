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

// `str`, `String` or `StringView`: a level a text literal is matched
// against (D-0127).
fn is_text_ty(t: &Type) -> bool {
    match t {
        Type::Str => true,
        Type::Named(n, _) => n == "std::String" || n == "std::StringView",
        _ => false,
    }
}

// The pointer and length of the text value `c` of text type `ty`.
fn text_parts(c: &str, ty: &Type) -> (String, String) {
    match ty {
        Type::Str => (format!("(const void *)({}).p", c), format!("({}).n", c)),
        Type::Named(n, _) if n == "std::StringView" => (format!("(const void *)({}).bytes.data", c), format!("({}).bytes.len", c)),
        _ => (format!("(const void *)({}).bytes.ptr", c), format!("({}).bytes.len", c)),
    }
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
    // D-0110: a struct or enum key type's hash and order functions.
    khs: HashMap<Type, String>,
    kcs: HashMap<Type, String>,
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
    // `pure_direct`'s answers, by function and parameter position.
    pure_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    // `reader_param`'s answers.
    reader_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    // The last segments of every path named other than as a callee
    // (`used_as_value`), gathered on first need.
    fn_values: std::cell::RefCell<Option<HashSet<String>>>,
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
            khs: HashMap::new(),
            kcs: HashMap::new(),
            externs_out: HashMap::new(),
            probing: 0,
            nscope: 0,
            confining: HashMap::new(),
            variants: HashMap::new(),
            pure_memo: std::cell::RefCell::new(HashMap::new()),
            reader_memo: std::cell::RefCell::new(HashMap::new()),
            fn_values: std::cell::RefCell::new(None),
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
        out.push_str("__thread uintptr_t cb_stack_floor = 0;\n");
        out.push_str("void cb_set_stack_floor(uintptr_t floor) { cb_stack_floor = floor; }\n");
        out.push_str("\nstatic const char *const cb_files[] = {");
        for f in &self.files {
            write!(out, "\"{}\", ", c_escape(f)).unwrap();
        }
        out.push_str("0};\n");
        writeln!(
            out,
            // D-0077: the program runs on a thread with a large stack
            // (reserved, not committed), as `coby` runs it; `main`'s own
            // thread only starts it.
            "static void cb_program(void)\n{{\n    cb_init(cb_files, {}, cb_types, {});\n    cb_frame_push();\n    {}\n    cb_frame_pop();\n    cb_terminate_ok({});\n}}\nint main(int argc, char **argv)\n{{\n    cb_set_args(argc, argv);\n    cb_run_main(cb_program);\n}}",
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
    // D-0089: a value of `t` holds a `fn` value somewhere, whose closure
    // it owns: copied or moved slot by slot (`cb_fn_split`), and ended
    // with it.
    fn holds_fn(&self, t: &Type) -> bool {
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
                    if let Some(b) = sd.bits {
                        return self.size_align(&Type::Int(b));
                    }
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
                            writeln!(self.types, "struct {} {{{} }};", cname, body).unwrap();
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
        // `static`: nothing outside the program's own C calls it by name
        // (the runtime reaches it through pointers, and foreign code is
        // only called, `rule.trust.extern-code`), so the C compiler may
        // inline it wherever it is called, and drop it where it is not.
        // Not `main`: inlined into `cb_program`, which only C's `main`
        // calls, GCC would compile the whole program as code run once,
        // for size (a `div` for a `%` by a constant, say).
        let storage = if cname == "f_main" { "" } else { "static " };
        let sig = format!("{}{} {}({})", storage, ret_c, cname, if params_c.is_empty() { "void".to_string() } else { params_c.join(", ") });
        writeln!(self.protos, "{};", sig).unwrap();

        if mask == 0 {
            if let Some(body) = self.native_vec_index(name, &f, targs, &cname)? {
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, body).unwrap();
                return Ok(());
            }
        }

        let pinned = pinned_names(&f.body, &|segs: &[String], j: usize| self.pure_direct_ref_arg(segs, j));
        let confined_names = self.confined_vecs(&f.body, &f.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
        let mut fx = Fx { g: self, subst, scopes: vec![HashMap::new()], out: String::new(), ret: ret.clone(), cleanup: Vec::new(), loops: Vec::new(), ind: 1, self_ref: None, assigned: assigned_names(&f.body), rebind_frames: HashMap::new(), holder_roots: HashMap::new(), no_overflow: HashSet::new(), proven_next: false, pinned, confined_names, confined: HashSet::new(), confined_params: HashMap::new(), probes: HashMap::new(), kept: Vec::new(), dollar: Vec::new(), place_write: false,
            fused_scrut: None, map_acc: None, map_acc_used: false, elem_mode: None, temp_root: false, frame_line: None, vec_index_exprs: HashMap::new(), propagate_scrutinee: false, elem_alias: HashMap::new(), alias_kept: Vec::new(), stmt_match: false, branch_scope: false, objectless: false, inert_c: HashSet::new(), direct_params: HashSet::new(), direct_refs: HashSet::new(), pure_slices: HashSet::new(), unstored: HashMap::new(), unstored_params: HashSet::new(), held_locals: HashSet::new(), held_c: HashSet::new(), unminted_binders: HashMap::new() };
        fx.push_frame();
        fx.direct_params = fx.g.direct_ref_params(&f, &fx.subst)?;
        fx.unstored_params = fx.g.unstored_ref_params(&f, &fx.subst, &fx.direct_params)?;
        fx.held_locals = held_locals(&f.body);
        fx.pure_slices = f.params.iter().enumerate().filter(|(i, p)| matches!(p.ty, Type::Slice(..)) && fx.g.pure_direct(name, *i)).map(|(_, p)| p.name.clone()).collect();
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
        let body = elide_implied_checks(&drop_dead_locations(&resolve_scopes(&fx.out)));
        // D-0077: a call too deep for the stack faults here, at the call.
        // A body that calls nothing (`calls_nothing`) cannot go deeper:
        // its caller's check, and the floor's margin, cover its frame.
        let check = if calls_nothing(&body) { "" } else { "    cb_stack_check();\n" };
        write!(self.fns, "{}\n{{\n{}{}}}\n\n", sig, check, body).unwrap();
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
                        let sig = format!("static {} *{}_rawk_acc(cb_ref p_m, {} p_k, {} p_fresh)", vc, cname, kc, vc);
                        writeln!(self.protos, "{};", sig).unwrap();
                        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, acc).unwrap();
                    }
                }
                if name == "std::HashMap::entry" {
                    if let [k, v] = targs {
                        let kc = self.ctype(k)?;
                        let vc = self.ctype(v)?;
                        let acc = Self::entry_acc_body(&b, self.type_id(v)?);
                        let sig = format!("static {} *{}_acc(cb_ref p_m, {} p_k, {} p_fresh)", vc, cname, kc, vc);
                        writeln!(self.protos, "{};", sig).unwrap();
                        write!(self.fns, "{}\n{{\n{}}}\n\n", sig, acc).unwrap();
                    }
                }
                return Ok(Some(b));
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
            let sig = format!("static inline {} {}_raw({})", sc, cname, param);
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
            writeln!(b, "    cb_read(p_b.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_NOLOC, 0);").unwrap();
            writeln!(b, "    void *a = (void *)(({} *)p_b.p)->ptr;", bc).unwrap();
            writeln!(b, "    return (cb_ref){{ a, cb_elem_borrow(a, {}u, {}) }};", tid, m).unwrap();
            return Ok(Some(b));
        }
        let mode = match name {
            "std::Vec::index_shared" => "CB_SHARED",
            "std::Vec::index_exclusive" => "CB_EXCLUSIVE",
            "std::Vec::drop" | "std::Vec::push" | "std::Vec::pop" | "std::Vec::len" | "std::Vec::clear" | "std::Vec::sort" | "std::Vec::grow" | "std::Vec::reserve" => "",
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
        if name == "std::Vec::pop" {
            let b = self.native_vec_pop(elem, &vec_c, true)?;
            if b.is_some() {
                // For a confined vector (`Fx::bind_confined`): no checks.
                let nc = self.native_vec_pop(elem, &vec_c, false)?.expect("the same element type");
                let opt_c = self.ctype(&Type::Named("std::Option".to_string(), vec![elem.clone()]))?;
                let sig = format!("static inline {} {}_nc(cb_ref p_v)", opt_c, cname);
                writeln!(self.protos, "{};", sig).unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, nc).unwrap();
            }
            return Ok(b);
        }
        if name == "std::Vec::push" {
            let b = self.native_vec_push(elem, &vec_c)?;
            // `Vec<String>::push` of a fresh `String` (`lower_push_fresh`):
            // the `_raw` variant, which neither receives nor moves an object.
            if let (Some(body), true) = (&b, matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty())) {
                let raw: String = body.lines().filter(|l| !l.contains("cb_recv(&p_x)") && !l.contains("cb_raw_move_in(o,")).map(|l| format!("{}\n", l)).collect();
                let elem_c = self.reg_ctype(elem)?;
                let sig = format!("static void {}_raw(cb_ref p_v, {} p_x)", cname, elem_c);
                writeln!(self.protos, "{};", sig).unwrap();
                write!(self.fns, "{}\n{{\n{}}}\n\n", sig, raw).unwrap();
            }
            if b.is_some() && self.plain_data(elem) {
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
    fn native_vec_drop_strings(&mut self, elem: &Type, vec_c: &str) -> R<String> {
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
        writeln!(b, "            if (e->bytes.len > 0) cb_vec_drop_plain(e->bytes.ptr, e->bytes.len, 1u);").unwrap();
        // A `String`'s bytes, and a buffer of `String`s, never hold a
        // reference (`cb_deallocate_plain`).
        writeln!(b, "            if (e->bytes.cap > 0) cb_deallocate_plain((uint8_t *)e->bytes.ptr, e->bytes.cap);").unwrap();
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
    fn native_vec_drop(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        if matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
            return self.native_vec_drop_strings(elem, vec_c).map(Some);
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
    fn native_vec_clear(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
        let (size, _) = self.size_align(elem)?;
        if size == 0 || !self.plain_data(elem) {
            return Ok(None);
        }
        let mut b = String::new();
        writeln!(b, "    cb_read(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 1u }} }}, 1, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *v = ({} *)p_v.p;", vec_c, vec_c).unwrap();
        writeln!(b, "    if (v->len > 0) {{").unwrap();
        writeln!(b, "        cb_vec_drop_plain(v->ptr, v->len, {}u);", size).unwrap();
        writeln!(b, "        cb_release((uint8_t *)v->ptr, v->len * {}u);", size).unwrap();
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
    fn native_vec_sort(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
    fn native_vec_sort_keys(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
    fn native_string_copy(&mut self, clone: bool) -> R<String> {
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

    // `HashMap::find` with `String` keys (`spec/21` §4, D-0041): the probe
    // loop over `m.slots` comparing keys with `key_eq`. Natively: the
    // body's checked reads of `m.slots` (once: the same path, and nothing
    // in the loop writes) and of `k` (`key_hash`'s, then `key_eq`'s, which
    // cannot fail after it), `len - 1`'s overflow, each slot's element
    // access, and for each candidate key the read of `m.keys` (on the
    // first), its bounds fault, and the element access `key_eq`'s read of
    // it makes (`cb_elem_access`: checked where an object lies over the
    // key, and otherwise unobservable, as for a plain element, D-0023).
    fn native_map_find_string(&mut self, v: &Type) -> R<String> {
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
        writeln!(b, "        cb_elem_access(se, {}u, CB_SHARED, 0, CB_NOLOC, 0);", uty).unwrap();
        writeln!(b, "        uint64_t e = *se;").unwrap();
        writeln!(b, "        if (e == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (!keys_read) {{ cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0); keys_read = 1; }}").unwrap();
        writeln!(b, "        if (e >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e;", sc).unwrap();
        writeln!(b, "        cb_elem_access(ke, {}u, CB_SHARED, 0, CB_NOLOC, 0);", sty).unwrap();
        writeln!(b, "        if (cb_bytes_eq((const void *)ke->bytes.ptr, ke->bytes.len, (const void *)k->bytes.ptr, k->bytes.len)) break;").unwrap();
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
    fn map_probe(&mut self, k: &Type, kp: &str, read_k: Option<&str>) -> R<String> {
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
    fn native_queue_peek(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
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
    fn native_queue_op(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
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

    fn native_map_op(&mut self, name: &str, targs: &[Type]) -> R<Option<String>> {
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
                writeln!(b, "    cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
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
                // The field checks through `m` cannot fail where the first
                // passed, and no object lies over a cell of `slots`.
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
                    // `Some(Vec::replace(&mut m.values, e, v))`.
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
                    writeln!(b, "    if (e >= m->values.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
                    writeln!(b, "    {} *ve = m->values.ptr + e;", vc).unwrap();
                    writeln!(b, "    return (cb_ref){{ (void *)ve, cb_elem_borrow(ve, {}u, CB_EXCLUSIVE) }};", vty).unwrap();
                }
                Ok(Some(b))
            }
        }
    }

    // A native `entry` body returning the value's address instead of a
    // reference to it: the element access its caller's read and write
    // through that reference would make (`cb_elem_access`, exclusive, a
    // write: the read after it cannot fail on an initialized element).
    fn entry_acc_body(b: &str, vty: u32) -> String {
        let ret = format!("    return (cb_ref){{ (void *)ve, cb_elem_borrow(ve, {}u, CB_EXCLUSIVE) }};", vty);
        assert!(b.contains(&ret), "entry's return");
        b.replace(&ret, &format!("    cb_elem_access(ve, {}u, CB_EXCLUSIVE, 1, CB_NOLOC, 0);\n    return ve;", vty))
    }

    // `HashMap::find` with an integer or `bool` key: as for `String` keys
    // (`native_map_find_string`), `key_hash`'s read of `*k` and its hash
    // of the value's bytes (`cb_hash_bytes`, as `lower_key_bytes` makes
    // it), and for each candidate key its element access and `key_eq`'s
    // comparison, which for these types is equality of the values.
    fn native_map_find_scalar(&mut self, k: &Type, v: &Type) -> R<String> {
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
        writeln!(b, "        cb_elem_access(se, {}u, CB_SHARED, 0, CB_NOLOC, 0);", uty).unwrap();
        writeln!(b, "        uint64_t e = *se;").unwrap();
        writeln!(b, "        if (e == UINT64_MAX) break;").unwrap();
        writeln!(b, "        if (!keys_read) {{ cb_read(p_m.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0); keys_read = 1; }}").unwrap();
        writeln!(b, "        if (e >= m->keys.len) cb_fault(\"diag.index-out-of-bounds\", CB_NOLOC, 0);").unwrap();
        writeln!(b, "        {} *ke = m->keys.ptr + e;", kc).unwrap();
        writeln!(b, "        cb_elem_access(ke, {}u, CB_SHARED, 0, CB_NOLOC, 0);", kty).unwrap();
        writeln!(b, "        if (*ke == kv) break;").unwrap();
        writeln!(b, "        i = (i + 1) & mask;").unwrap();
        writeln!(b, "    }}").unwrap();
        writeln!(b, "    return i;").unwrap();
        Ok(b)
    }

    fn native_append_string(&mut self) -> R<String> {
        let vec_t = Type::Named("std::Vec".to_string(), vec![Type::Int(IntTy::U8)]);
        let grow = self.request_fn("std::Vec::grow", vec![Type::Int(IntTy::U8)])?;
        let sc = self.ctype(&Type::Named("std::String".to_string(), vec![]))?;
        let _ = self.ctype(&vec_t)?;
        let mut b = String::new();
        writeln!(b, "    cb_read(p_t.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 1u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    {} *t = ({} *)p_t.p;", sc, sc).unwrap();
        writeln!(b, "    {} *s = ({} *)p_s.p;", sc, sc).unwrap();
        writeln!(b, "    uint64_t n = t->bytes.len;").unwrap();
        // `Vec::reserve(&mut s.bytes, n)` (D-0129), as the body calls it:
        // the borrow formed for the call, then the call.
        let reserve = self.request_fn("std::Vec::reserve", vec![Type::Int(IntTy::U8)])?;
        writeln!(b, "    {{ uint64_t rt = cb_borrow_unstamped(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&s->bytes, rt }}), n); cb_call_done(rt); }}", reserve).unwrap();
        writeln!(b, "    if (n == 0) return;").unwrap();
        writeln!(b, "    cb_read(p_t.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }}, {{ CB_FIELD, 0u }} }}, 2, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    cb_borrow_check(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    for (uint64_t i = 0; i < n; i++) {{").unwrap();
        writeln!(b, "        uint8_t x = t->bytes.ptr[i];").unwrap();
        writeln!(b, "        if (s->bytes.len == s->bytes.cap) {{ uint64_t gt = cb_borrow_unstamped(p_s.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, CB_EXCLUSIVE, CB_NOLOC, 0); {}(((cb_ref){{ (void *)&s->bytes, gt }})); cb_call_done(gt); }}", grow).unwrap();
        writeln!(b, "        s->bytes.ptr[s->bytes.len] = x;").unwrap();
        writeln!(b, "        s->bytes.len += 1;").unwrap();
        writeln!(b, "    }}").unwrap();
        Ok(b)
    }

    // `Vec::grow`: the body's first checked read (of `v.cap`), its
    // arithmetic and faults, `allocate`, `copy_raw` of the elements,
    // `deallocate` of the old cells, then its two checked writes, in its
    // order. Its other reads go through the same path with nothing in
    // between that touches `v`, and cannot fail where the first passed.
    fn native_vec_grow(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->ptr = ({} *)p;", self.ctype(elem)?).unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->cap = nc;").unwrap();
        Ok(Some(b))
    }

    // `Vec::reserve` (D-0129): the body's first checked read (of `v.len`),
    // its checked `len + n`, then nothing when the room is there, or the
    // cells moved as `native_vec_grow` moves them, to the body's capacity,
    // with its two checked writes. Its other reads go through the same path
    // with nothing between that touches `v`.
    fn native_vec_reserve(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 0u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->ptr = ({} *)p;", self.ctype(elem)?).unwrap();
        writeln!(b, "    cb_write(p_v.tok, (cb_proj[]){{ {{ CB_FIELD, 2u }} }}, 1, 0u, CB_NOLOC, 0);").unwrap();
        writeln!(b, "    v->cap = nc;").unwrap();
        Ok(Some(b))
    }

    // `Vec::pop` for a plain element type: the body's checked read of
    // `v.len`, `None` when it is 0, the checked write of `len - 1` (which
    // cannot overflow there), then the element's `[Rawptr-Read]` and the
    // `[Release]` of its cells, as the prelude's body does. Its other
    // reads of `v` go through the same path with nothing in between but
    // that write, and cannot fail where the first passed. `checked`
    // false: the confined form (`_nc`), without the two checks.
    fn native_vec_pop(&mut self, elem: &Type, vec_c: &str, checked: bool) -> R<Option<String>> {
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
        writeln!(b, "    cb_release((uint8_t *)(v->ptr + n), {}u);", size).unwrap();
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
    fn native_vec_push(&mut self, elem: &Type, vec_c: &str) -> R<Option<String>> {
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
    fn confined_vecs(&self, body: &Block, params: &[String]) -> HashSet<String> {
        let (declared, bad) = self.scan_vec_uses(body, params, &self.confining, true);
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
                let pnames: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                let (declared, bad) = self.scan_vec_uses(&f.body, &pnames, &t, false);
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
    fn scan_vec_uses(&self, body: &Block, params: &[String], table: &HashMap<String, Vec<bool>>, fn_body: bool) -> (HashSet<String>, HashSet<String>) {
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
            // Every name the function binds (`bound_names`): a call by a
            // single name is to the function of that name only if no
            // binding of it (a closure, a `fn` value) could be called.
            bound: HashSet<String>,
            params: HashSet<String>,
        }
        const PUSH: &str = "std::Vec::push";
        const POP: &str = "std::Vec::pop";
        const RESERVE: &str = "std::Vec::reserve";
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
                ExprKind::ArrayRepeat(a, _) => walk(a, f),
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
                    "std::sqrt", "std::floor", "std::ceil", "std::round", "std::trunc", "std::ln", "std::exp", "std::log2", "std::log10", "std::sin",
                    "std::cos", "std::tan", "std::atan2", "std::powf",
                    "saturating_add", "saturating_sub", "saturating_mul", "checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem",
                    // `printf`'s, `sprintf`'s, `assert`'s … formatted
                    // arguments (`modres` makes the call `$fmt(…)`), each
                    // lowered as a value; one written `&s` is still a place,
                    // through the borrow (`Ctx::Place` below).
                    "$fmt",
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
                            if (name == PUSH || name == LEN || name == POP || name == RESERVE) && args.first().and_then(vec_arg).is_some() {
                                args[1..].iter().for_each(|a| self.expr(a, Ctx::Value));
                                return;
                            }
                            // A name a binding could stand for: a call of
                            // whatever it holds, so no rule below applies.
                            let shadowed = !name.contains("::") && self.bound.contains(&name);
                            // `&x[i]` / `&mut x[i]` for a pure direct
                            // parameter (`Gen::pure_direct`), `x` named by no
                            // other argument: the callee keeps no path, and
                            // the element's address is all it is given
                            // (`elem_access_place`'s confined case); only the
                            // index is a use.
                            let elem_arg = |j: usize, a: &Expr| -> Option<String> {
                                let ExprKind::Borrow(_, place) = &a.kind else { return None };
                                let (x, _) = index_expr_on_local(place)?;
                                let f = self.g.items.fns.get(&name)?;
                                let ok = matches!(f.params.get(j).map(|p| &p.ty), Some(Type::Ref(..)))
                                    && self.g.pure_direct(&name, j)
                                    && !args.iter().enumerate().any(|(k, b)| k != j && mentions(b, x));
                                ok.then(|| x.to_string())
                            };
                            if !shadowed && args.iter().enumerate().any(|(j, a)| elem_arg(j, a).is_some()) {
                                self.expr(callee, Ctx::Place);
                                for (j, a) in args.iter().enumerate() {
                                    match (elem_arg(j, a), &a.kind) {
                                        (Some(_), ExprKind::Borrow(_, place)) => {
                                            let (_, i) = index_expr_on_local(place).expect("an element argument");
                                            self.expr(i, Ctx::Value);
                                        }
                                        _ => self.expr(a, self.args_ctx(&name)),
                                    }
                                }
                                return;
                            }
                            if let Some(conf) = self.table.get(&name).filter(|_| !shadowed) {
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
                    // `x[i].f…` read as a value: the element read, then
                    // its fields; no path is formed.
                    ExprKind::Field(..) if ctx == Ctx::Value && field_chain_root(e).map_or(false, |r| index_expr_on_local(r).is_some()) => {
                        let (_, i) = index_expr_on_local(field_chain_root(e).unwrap()).unwrap();
                        self.expr(i, Ctx::Value);
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
                    ExprKind::ArrayRepeat(a, _) => self.expr(a, Ctx::Value),
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
                        // A `foreach (x in &v)` over a local whose body
                        // reads `x` only as `*x` (`Fx::foreach_alias`): the
                        // loop's holder `$c` borrows `v` shared, and its uses
                        // become bounds-checked element reads of `v`, so the
                        // loop leaves `v` confined. Any other use of `v` in
                        // the body is scanned as usual; a push to `v` there
                        // is already rejected statically (the holder lives).
                        Stmt::Let { name, init: Some(e), .. } if foreach_reads_only(name, e, b) => {
                            self.declared.insert(name.clone());
                        }
                        // `foreach (x in p)`, `p` a parameter (a reference:
                        // the only kind `confining_table` considers), whose
                        // body reads `x` only as `*x` or `x.f…`: the loop
                        // over the borrow `p` is (`each.rs`), its holder `p`
                        // itself, which `foreach_alias` reads elements
                        // through. `p` is not copied anywhere else.
                        Stmt::Let { name, init: Some(e), .. } if foreach_param_reads_only(name, e, b, &self.params) => {
                            self.declared.insert(name.clone());
                        }
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
                    // So may a struct built as the result (`String { .bytes
                    // = v }`): its bare fields move in; the rest are scanned.
                    Some(t) if fn_body && matches!(&t.kind, ExprKind::StructLit(..)) => {
                        let ExprKind::StructLit(_, _, fs) = &t.kind else { unreachable!() };
                        for (_, fe) in fs {
                            if !matches!(&fe.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty()) {
                                self.expr(fe, Ctx::Value);
                            }
                        }
                    }
                    Some(t) => self.expr(t, Ctx::Value),
                    None => {}
                }
            }
        }
        let bound: HashSet<String> = params.iter().cloned().chain(bound_names(body)).collect();
        let mut sc = Scan { g: self, table, declared: HashSet::new(), bad: HashSet::new(), bound, params: params.iter().cloned().collect() };
        sc.block(body, fn_body);
        (sc.declared, sc.bad)
    }

    // The *direct* reference parameters of `f`: a `ref<T, _>` with `T`
    // plain data, the function's only parameter that is or holds a
    // reference, which the body uses only as `r.f…` or `*r`, read or
    // assigned whole (`direct_uses`). Its accesses need no check: the
    // caller formed and checked the borrow, and while it lives no other
    // path can reach `*r` -- one would clash with `r` and fault at its
    // own access, the only place it could; `r` is the sole reference the
    // call was given, so no argument aliases it (whose clash would
    // otherwise be found at the first access meeting both, `spec/08`);
    // and nothing in the body forms a path from it, hands it on or keeps
    // it. `T` holds no reference or resource, so no slot or authority is
    // involved. Such a parameter gets no runtime binding at all.
    fn direct_ref_params(&self, f: &FnDecl, subst: &HashMap<String, Type>) -> R<HashSet<String>> {
        self.direct_refs_of(&f.params, &f.body, subst)
    }

    // `direct_ref_params` for a parameter list and body (a function's, or
    // a closure's that captures nothing). Besides a sole reference
    // parameter, any number of *shared* references to plain data qualify
    // together: two shared paths never clash, and an exclusive one to the
    // same storage could not have been formed while they live.
    fn direct_refs_of(&self, params: &[Param], body: &Block, subst: &HashMap<String, Type>) -> R<HashSet<String>> {
        let mut out = HashSet::new();
        let tys: Vec<Type> = params.iter().map(|p| apply(&p.ty, subst)).collect();
        let refish: Vec<&Type> = tys.iter().filter(|t| self.has_refs(t) || self.holds_fn(t)).collect();
        let all_shared_plain = refish.iter().all(|t| matches!(t, Type::Ref(inner, Mode::Shared) | Type::Slice(inner, Mode::Shared) if self.plain_data(inner)));
        if refish.len() != 1 && !(refish.len() > 1 && all_shared_plain) {
            return Ok(out);
        }
        for (p, t) in params.iter().zip(&tys) {
            let (Type::Ref(inner, _) | Type::Slice(inner, _)) = t else { continue };
            // A reference to a `Vec` of plain elements qualifies when it is
            // only ever handed on whole: every access is then the callee's,
            // checked through the path the call was given.
            let vec_of_plain = matches!(t, Type::Ref(..)) && matches!(&**inner, Type::Named(n, a) if n == "std::Vec" && a.len() == 1 && self.plain_data(&a[0]));
            if vec_of_plain && only_handed_on(body, &p.name) {
            } else if matches!(t, Type::Ref(..)) && matches!(&**inner, Type::Named(n, _) if self.struct_decl(n).is_some()) && !self.has_refs(inner) && !self.holds_fn(inner) && box_rec_uses(body, &p.name) {
                // A struct reached through `Box`es (`stress/perf/tree-design.md`):
                // used only as `r.f` read, `r.f = e`, and `match (&r.f)` whose
                // binder goes only to `Box::get[_mut](b)` handed on. Direct, and
                // the lowering keeps the value checks a resource field needs.
                out.insert(p.name.clone());
                continue;
            } else if self.size_align(inner)?.0 == 0 || !self.plain_data(inner) {
                continue;
            }
            // A slice is used only through its elements and `slice_len`:
            // never handed on, so its stored path is never needed.
            if matches!(t, Type::Slice(..)) {
                if direct_uses(body, &p.name, &|_, _| false) {
                    out.insert(p.name.clone());
                }
                continue;
            }
            // `r` handed whole to a named function whose result is plain
            // data: the callee's path ends with the call, and it can give
            // nothing derived from `r` back. Not `spawn` (the thread keeps
            // it) and not a closure value.
            let callee_ok = |segs: &[String], _: usize| {
                let key = segs.join("::");
                let key = if self.items.fns.contains_key(&key) { key } else { format!("std::{}", key) };
                key != "spawn" && key != "std::spawn" && self.items.fns.get(&key).map_or(false, |g| matches!(g.ret, Type::Void) || self.plain_data(&g.ret))
            };
            if direct_uses(body, &p.name, &callee_ok) {
                out.insert(p.name.clone());
            }
        }
        Ok(out)
    }

    // Whether parameter `pos` of the non-generic function `key` is a
    // direct reference (`direct_ref_params`) handed on, if at all, only to
    // such parameters: then no use of the reference a call passes reads
    // its path, and the call may pass a bare address after the borrow's
    // check (`Fx::lower_user_call`). Recursion through a parameter answers
    // no (it would need a fixed point; no is safe).
    // A function's sole reference(-holding) parameter, of reference type,
    // that the body never assigns, borrows as a binding (`&r`) or
    // captures: no slot need hold its token -- with no other reference
    // into the function, nothing could clash with the path while the call
    // runs, and an unheld path no check counts (D-0018). Every access
    // through it stays checked, through the token it came with.
    // Several reference parameters qualify when every one is shared: two
    // shared paths never clash, and no exclusive path can be formed from
    // a shared one, so with no other reference into the function nothing
    // could clash with them either (`HashMap::get(m, k)` and the functions
    // it hands both on to).
    fn unstored_ref_params(&self, f: &FnDecl, subst: &HashMap<String, Type>, direct: &HashSet<String>) -> R<HashSet<String>> {
        let mut out = HashSet::new();
        let tys: Vec<Type> = f.params.iter().map(|p| apply(&p.ty, subst)).collect();
        let holding: Vec<&Type> = tys.iter().filter(|t| self.has_refs(t) || self.holds_fn(t)).collect();
        let all_shared = holding.iter().all(|t| matches!(t, Type::Ref(_, Mode::Shared)));
        if holding.is_empty() || (holding.len() > 1 && !all_shared) {
            return Ok(out);
        }
        let assigned = assigned_names(&f.body);
        for (p, t) in f.params.iter().zip(&tys) {
            if !matches!(t, Type::Ref(..)) || direct.contains(&p.name) || assigned.contains(&p.name) {
                continue;
            }
            let name = p.name.as_str();
            let is_r = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
            let mut ok = true;
            visit_exprs(&f.body, &mut |e| match &e.kind {
                ExprKind::Borrow(_, a) if is_r(a) => ok = false,
                ExprKind::Closure { body, captures, .. } if captures.iter().any(|c| c == name) || block_names(body, name) => ok = false,
                _ => {}
            });
            let mut shadow = false;
            visit_stmts(&f.body, &mut |st| {
                if let Stmt::Let { name: n, .. } = st {
                    if n == name {
                        shadow = true;
                    }
                }
            });
            if ok && !shadow {
                out.insert(p.name.clone());
            }
        }
        Ok(out)
    }

    // Whether a function item is named anywhere but as a call's callee
    // (by its last segment: a coarse, safe answer).
    fn used_as_value(&self, key: &str) -> bool {
        if self.fn_values.borrow().is_none() {
            let mut names = HashSet::new();
            for f in self.items.fns.values() {
                let mut callees: HashSet<*const Expr> = HashSet::new();
                visit_exprs(&f.body, &mut |e| {
                    if let ExprKind::Call(c, _) = &e.kind {
                        if matches!(c.kind, ExprKind::Path(..)) {
                            callees.insert(&**c as *const Expr);
                        }
                    }
                });
                visit_exprs(&f.body, &mut |e| {
                    if let ExprKind::Path(segs, _) = &e.kind {
                        if !callees.contains(&(e as *const Expr)) {
                            if let Some(l) = segs.last() {
                                names.insert(l.clone());
                            }
                        }
                    }
                });
            }
            *self.fn_values.borrow_mut() = Some(names);
        }
        let last = key.rsplit("::").next().unwrap_or(key);
        self.fn_values.borrow().as_ref().unwrap().contains(last)
    }

    // Whether parameter `pos` of `key` is a *reader*: a shared reference
    // the body only reads through (`p.f…`, a value) and hands on, whole or
    // as `&p.f…`, to other readers (`reader_uses`), and the function's
    // result holds no reference. Nothing derived from the reference is then
    // kept, returned or stored, and nothing is written through it: a caller
    // that has proven every check on the referent for the call's duration
    // may pass it with token 0 (`cb_read`'s note in cbrt), and every check
    // through it is then the no-op the proof makes it. Recursion answers no.
    fn reader_param(&self, key: &str, pos: usize) -> bool {
        let visiting = std::cell::RefCell::new(Vec::new());
        self.reader_param_in(key, pos, &visiting)
    }
    fn reader_param_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        let id = (key.to_string(), pos);
        if let Some(&b) = self.reader_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        let Some(p) = f.params.get(pos) else { return false };
        if !matches!(p.ty, Type::Ref(_, Mode::Shared)) || !resolved(&f.ret, &f.type_params) || self.has_refs(&f.ret) || self.holds_fn(&f.ret) {
            self.reader_memo.borrow_mut().insert(id, false);
            return false;
        }
        visiting.borrow_mut().push(id.clone());
        let ok = reader_uses(&f.body, &p.name, true, &|segs: &[String], at: usize| {
            let k = segs.join("::");
            let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
            self.items.fns.get(&k).and_then(|g| g.params.get(at)).map_or(false, |q| matches!(q.ty, Type::Ref(_, Mode::Shared))) && self.reader_param_in(&k, at, visiting)
        });
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.reader_memo.borrow_mut().insert(id, ok);
        }
        ok
    }

    fn pure_direct(&self, key: &str, pos: usize) -> bool {
        let visiting = std::cell::RefCell::new(Vec::new());
        self.pure_direct_in(key, pos, &visiting)
    }

    // A call `segs(…)` whose argument `pos` is a pure direct reference
    // parameter (`pure_direct`), named so that no local can be what it
    // calls (two segments or more: `Type::f`, `m::f`). `&x` passed there
    // mints no path on `x` (`lower_user_call`), so it does not pin `x`
    // (`pinned_names`).
    fn pure_direct_ref_arg(&self, segs: &[String], pos: usize) -> bool {
        if segs.len() < 2 {
            return false;
        }
        let key = segs.join("::");
        let Some(f) = self.items.fns.get(&key) else { return false };
        matches!(f.params.get(pos).map(|p| &p.ty), Some(Type::Ref(..))) && self.pure_direct(&key, pos)
    }
    fn pure_direct_in(&self, key: &str, pos: usize, visiting: &std::cell::RefCell<Vec<(String, usize)>>) -> bool {
        let id = (key.to_string(), pos);
        if let Some(&b) = self.pure_memo.borrow().get(&id) {
            return b;
        }
        if visiting.borrow().contains(&id) {
            return false;
        }
        let Some(f) = self.items.fns.get(key) else { return false };
        // A generic function qualifies when no parameter's type names a
        // type parameter: then which parameters are direct is the same in
        // every instance. A slice parameter, which the caller keeps
        // instead of sending, also needs every call to be one the
        // compiler sees: the function never used as a `fn` value.
        if pos >= f.params.len() || !f.params.iter().all(|p| resolved(&p.ty, &f.type_params)) {
            return false;
        }
        match &f.params[pos].ty {
            Type::Ref(..) => {}
            Type::Slice(..) if !self.used_as_value(key) => {}
            _ => return false,
        }
        let name = f.params[pos].name.clone();
        if !self.direct_ref_params(f, &HashMap::new()).map_or(false, |d| d.contains(&name)) {
            self.pure_memo.borrow_mut().insert(id, false);
            return false;
        }
        // A struct reached through `Box`es (`box_rec_uses`): never handed
        // on, so pure.
        if box_rec_uses(&f.body, &name) {
            self.pure_memo.borrow_mut().insert(id, true);
            return true;
        }
        visiting.borrow_mut().push(id.clone());
        let my_mode = match &f.params[pos].ty {
            Type::Ref(_, m) | Type::Slice(_, m) => Some(m.clone()),
            _ => None,
        };
        let ok = direct_uses(&f.body, &name, &|segs: &[String], at: usize| {
            let k = segs.join("::");
            let k = if self.items.fns.contains_key(&k) { k } else { format!("std::{}", k) };
            // Handed on in the same mode only: an exclusive reference
            // passed where a shared one is wanted is reborrowed from its
            // token, and a pure parameter's token may be none at all.
            let same_mode = self.items.fns.get(&k).and_then(|g| g.params.get(at)).map_or(false, |q| match &q.ty {
                Type::Ref(_, m) | Type::Slice(_, m) => Some(m.clone()) == my_mode,
                _ => false,
            });
            same_mode && self.pure_direct_in(&k, at, visiting)
        });
        visiting.borrow_mut().pop();
        if visiting.borrow().is_empty() || ok {
            self.pure_memo.borrow_mut().insert(id, ok);
        }
        ok
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
        writeln!(self.thunks_out, "static uint8_t {}(const {} *a, const {} *b)\n{{\n{}}}", name, ct, ct, body).unwrap();
        self.eqs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // D-0110: the fields (or an enum's tag and payloads) of a struct or
    // enum key type, with their C access and type, for `key_hash_fn` and
    // `key_cmp_fn`: `(None, …)` for a struct's fields, `(Some(i), …)` for
    // variant i's payload.
    fn key_parts(&mut self, t: &Type) -> R<(bool, Vec<(Option<usize>, String, Type)>)> {
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
    fn key_hash_part(&mut self, t: &Type, e: &str) -> R<String> {
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
    fn key_cmp_part(&mut self, t: &Type, a: &str, b: &str) -> R<String> {
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
    fn key_hash_fn(&mut self, t: &Type) -> R<String> {
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
        writeln!(self.thunks_out, "static uint64_t {}(uint64_t h, const {} *k)\n{{\n{}}}", name, ct, body).unwrap();
        self.khs.insert(t.clone(), name.clone());
        Ok(name)
    }

    // D-0110: `cb_kc_…`, the key types' order on a struct or enum key:
    // field by field in declaration order; an enum by variant, then
    // payload.
    fn key_cmp_fn(&mut self, t: &Type) -> R<String> {
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
        writeln!(self.thunks_out, "static int {}(const {} *a, const {} *b)\n{{\n{}}}", name, ct, ct, body).unwrap();
        self.kcs.insert(t.clone(), name.clone());
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
            "static {rc} {name}({params})\n{{\n    if (!h) cb_fn_moved();\n    const cb_fnbox *b = (const cb_fnbox *)h;\n    if (b->closure == 2u) {{\n        cb_ref self = {{ b->data, 0 }};\n        {ret}(({rc} (*)({closure_sig}))b->code)({closure_args});\n    }}\n    else if (b->closure) {{\n        cb_ref self = {{ b->data, cb_call_self(b->root) }};\n        {keep}(({rc} (*)({closure_sig}))b->code)({closure_args});\n        cb_call_done(self.tok);\n        {give}\n    }}\n    {ret2}(({rc} (*)({item_sig}))b->code)({item_args});\n}}",
            rc = rc,
            name = name,
            params = params.join(", "),
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
    // A `match`'s fused scrutinee, lowered as its place (`lower_match`).
    fused_scrut: Option<V>,
    // `*HashMap::entry(…) op= e` (`lower_block_body`'s compound pair):
    // asks the native `entry` call for its `_acc` variant (the value's
    // address after one element access); `map_acc_used` says it did.
    map_acc: Option<usize>,
    map_acc_used: bool,
    // The mode `native_elem_access` checks the element in, when it is not
    // the call's own (`&mut v[i]` formed through `index_shared`'s place).
    elem_mode: Option<&'static str>,
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
    // D-0111: an unchecked holder's root, by its C name (its object is
    // ended at its last use, `end_ref_bindings`).
    holder_roots: HashMap<String, String>,
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
    // D-0047: the C expressions `$` stands for, innermost `[…]` last.
    dollar: Vec<String>,
    // `v[i]` on a `Vec` (D-0047 `[Index-Vec]`): the place is written (an
    // assignment's target, `&mut`'s operand), so it is `index_exclusive`.
    place_write: bool,
    // D-0103: the next temporary `lower_place` reaches is the root of a
    // borrow that outlives the expression: make it a checked object.
    temp_root: bool,
    // The line the next block's frame reports a destruction fault at
    // (`push_frame_at`), set by the construct that owns the block.
    frame_line: Option<usize>,
    // `v[i]` on a `Vec` as the `*Vec::index_*(&v, i)` it stands for, kept
    // (address-stable for `probe_type`'s cache) per expression and mode.
    vec_index_exprs: HashMap<(usize, bool), std::rc::Rc<Expr>>,
    // D-0060: the next `lower_match` is a `?`'s: a temporary scrutinee
    // that owns nothing ends as soon as the match has taken it apart.
    propagate_scrutinee: bool,
    // A `foreach` element over a shared `Vec` of plain elements that the
    // body only reads (`*x`): `x` stands for the checked element read
    // `(*$c)[$i]` instead of a reference bound each step (`foreach_alias`).
    elem_alias: HashMap<String, std::rc::Rc<Expr>>,
    // Direct reference parameters (`Gen::direct_ref_params`): by name, and
    // the C names of their bindings, whose places are reached unchecked.
    direct_params: HashSet<String>,
    direct_refs: HashSet<String>,
    // Pure direct slice parameters (`Gen::pure_direct`): not received --
    // every caller keeps the slice's path for the call instead.
    pure_slices: HashSet<String>,
    // Unstored reference parameters (`Gen::unstored_ref_params`): the C
    // name of the binding's address, and the incoming token used for it.
    unstored: HashMap<String, String>,
    unstored_params: HashSet<String>,
    // Locals of reference type whose path the frame can hold with no
    // binding object (`held_locals`), and the C names of those bound so.
    held_locals: HashSet<String>,
    held_c: HashSet<String>,
    // Arm binders with no path minted (their one use is `Box::get(b)` /
    // `Box::get_mut(b)`, made inline): C name -> (base path, projection).
    unminted_binders: HashMap<String, (String, Vec<String>)>,
    // Every alias made, kept to the function's end: the lowering caches
    // synthesized expressions by address (`vec_index_exprs`), and a freed
    // alias's address reused by a later one would hit a stale entry.
    alias_kept: Vec<std::rc::Rc<Expr>>,
    // A `match` that is a whole statement, its value unused (`lower_stmt`):
    // taken by that `match` (`lower_match`), whose arms then get a statement
    // scope each (`branch_scope`, taken by `lower_branch`). The arm is the
    // last thing the statement evaluates, so its temporaries ending with
    // the arm is their ending with the statement; an arm that forms none
    // pays for no scope (`resolve_scopes` drops it).
    stmt_match: bool,
    branch_scope: bool,
    // The literal about to be built (`lower_struct_lit`, `construct_variant`,
    // `lower_box_new`, which take it) goes straight into the aggregate or
    // `Box` being built around it and owns nothing a fault could leave
    // undestroyed (`inert_expr`): it gets no object. `inert_c`: the C
    // temporaries holding such values, which `into_temp` and the absorbing
    // literal accept without one.
    objectless: bool,
    inert_c: HashSet<String>,
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

    fn holds_fn(&self, t: &Type) -> bool {
        self.g.holds_fn(t)
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    // D-0100: a format argument holding the name of the variant `tag`
    // selects, as text.
    fn variant_name_text(&mut self, tag: &str, names: &[String]) -> String {
        let nm = self.g.fresh("vn");
        let nl = self.g.fresh("vl");
        let mut sw = format!("const char *{} = \"\"; uint64_t {} = 0; switch ({}) {{", nm, nl, tag);
        for (i, n) in names.iter().enumerate() {
            sw.push_str(&format!(" case {}u: {} = \"{}\"; {} = {}u; break;", i, nm, n, nl, n.len()));
        }
        sw.push_str(" default: break; }");
        self.emit(&sw);
        format!("{{ 3u, 0u, 0, 0, 0.0, (const uint8_t *){}, {} }}", nm, nl)
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
        self.push_frame_at(None);
    }

    // A frame whose end reports `line` for a destruction fault (the
    // block expression's, or its `if`'s): the marker carries the
    // location to `resolve_scopes`, which emits `cb_frame_push_at`.
    fn push_frame_at(&mut self, line: Option<usize>) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        match line {
            Some(l) => {
                let loc = self.loc(l);
                self.emit(&format!("\u{1}F{}\u{2}{}", id, loc));
            }
            None => self.scope_marker('F', id),
        }
        self.cleanup.push(Cleanup::Frame(id));
    }

    fn pop_frame(&mut self) {
        let Some(Cleanup::Frame(id)) = self.cleanup.pop() else { panic!("cobc: frame pop closes a statement scope") };
        self.scope_marker('f', id);
    }

    fn push_stmt(&mut self) {
        self.push_stmt_at(None);
    }

    // A statement scope whose end reports `line` for a destruction fault
    // (a temporary that ends with the statement while a kept reference
    // holds it, D-0103/D-0135): `resolve_scopes` emits `cb_stmt_push_at`.
    fn push_stmt_at(&mut self, line: Option<usize>) {
        self.g.nscope += 1;
        let id = self.g.nscope;
        match line {
            Some(l) => {
                let loc = self.loc(l);
                self.emit(&format!("\u{1}S{}\u{2}{}", id, loc));
            }
            None => self.scope_marker('S', id),
        }
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
        if v.obj.is_none() && self.inert_c.contains(&v.c) {
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty: v.ty, obj: None });
        }
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
        !self.is_res(ty) && !self.has_refs(ty) && !self.holds_fn(ty) && !matches!(ty, Type::Closure(_)) && !self.pinned.contains(name)
    }

    // The same proof for a local that holds references (a reference, or
    // plain data with one inside): a reference is never consumed, and
    // nothing else reaches the variable's own storage. Its runtime
    // object stays — the slots it holds are what make its references
    // held — but accesses to the variable itself are not checked.
    fn unchecked_holder(&self, name: &str, ty: &Type) -> bool {
        !self.is_res(ty) && self.has_refs(ty) && !self.holds_fn(ty) && !matches!(ty, Type::Closure(_)) && !self.pinned.contains(name)
    }

    fn bind_value(&mut self, name: &str, cname: String, ty: &Type, v: &V, line: usize) -> R<()> {
        // A new binding of the name: whatever `confined_params` recorded for
        // an earlier one (a `foreach` holder registered by `foreach_alias`,
        // perhaps in a probe pass, under that pass's C name) is not this one.
        self.confined_params.remove(name);
        if matches!(ty, Type::Ref(..)) && self.held_locals.contains(name) && !self.unchecked(name, ty) {
            // A reference local never assigned, borrowed or captured: the
            // frame holds its path (`cb_frame_hold`) as its binding object
            // would, and its uses go through that path's token.
            let r = self.into_temp(v.clone(), line)?;
            let tk = self.g.fresh("tk");
            self.emit(&format!("{} = ({}).p;", cname, r.c));
            self.emit(&format!("uint64_t {} = ({}).tok;", tk, r.c));
            self.emit(&format!("cb_frame_hold({});", tk));
            self.unstored.insert(cname.clone(), tk);
            self.held_c.insert(cname.clone());
            self.scopes.last_mut().unwrap().insert(name.to_string(), Binding { c: cname, ty: ty.clone(), root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
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
        let root = if self.unchecked_holder(name, ty) {
            self.holder_roots.insert(cname.clone(), root);
            String::new()
        } else {
            root
        };
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
        if self.pure_slices.contains(&p.name) {
            // Not received: the caller holds the slice's path for the call
            // (`Gen::pure_direct`); only its elements and length are used.
            self.direct_refs.insert(pc.clone());
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: pc, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.direct_params.contains(&p.name) && matches!(ty, Type::Slice(..)) {
            // A direct slice (`Gen::direct_refs_of`) is received and bound
            // as any slice -- every caller, a `fn` value's thunk too, puts
            // it in flight -- and its path held for the call; only its
            // element accesses go unchecked.
            self.direct_refs.insert(pc.clone());
        } else if self.direct_params.contains(&p.name) {
            // A direct reference (`Gen::direct_ref_params`): the address
            // alone; no object, no stored path.
            let l = format!("l_{}", pc);
            self.emit(&format!("void *{} = {}.p;", l, pc));
            self.direct_refs.insert(l.clone());
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: l, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
        if self.unstored_params.contains(&p.name) && matches!(ty, Type::Ref(..)) {
            // Its accesses go through the token it came with; the binding
            // itself is never written, borrowed or captured.
            let l = format!("l_{}", pc);
            self.emit(&format!("void *{} = {}.p;", l, pc));
            self.unstored.insert(l.clone(), format!("{}.tok", pc));
            self.scopes.last_mut().unwrap().insert(p.name.clone(), Binding { c: l, ty, root: String::new(), kind: BindKind::Local });
            return Ok(());
        }
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
        let mut aliases: Vec<String> = Vec::new();
        let mut skip_next = false;
        for (si, s) in b.stmts.iter().enumerate() {
            if std::mem::take(&mut skip_next) {
                continue;
            }
            if let Some(l) = stmt_line(s) {
                self.at(l);
            }
            // `p op= e` with a call in `p` (D-0035) is the block
            // `{ auto t = &mut p; *t = *t op e; }`. With `e` a literal or a
            // local, nothing runs between the borrow and the write but the
            // read through `t`, so `t` need not be a binding that holds the
            // path (D-0018): the two statements are lowered as one, `t`'s
            // uses through the borrow's token, which that one statement
            // keeps until its end.
            if let (Stmt::Let { ty: None, name, init: Some(e), line }, Some(next)) = (s, b.stmts.get(si + 1)) {
                if name.starts_with("__compound") && matches!(&e.kind, ExprKind::Borrow(Mode::Exclusive, _)) && compound_write_inert(next, name) {
                    self.push_stmt();
                    // `*HashMap::entry(…) op= e`: the native `entry`'s
                    // `_acc` variant gives the value's address, the
                    // element's access made; the read and write through it
                    // then need no check (`direct_refs`).
                    let acc_call = match &e.kind {
                        ExprKind::Borrow(_, inner) => match &strip_parens(inner).kind {
                            ExprKind::Deref(c) => match &strip_parens(c).kind {
                                ExprKind::Call(callee, cargs) if matches!(&callee.kind, ExprKind::Path(segs, _) if segs.join("::") == "std::HashMap::entry") => Some((&**c, cargs.as_ptr() as usize)),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some((c, target)) = acc_call {
                        self.map_acc = Some(target);
                        self.map_acc_used = false;
                        let v = self.lower_expr(c, None);
                        self.map_acc = None;
                        let v = v?;
                        if matches!(v.ty, Type::Never) {
                            self.end_diverged();
                            result = never();
                            break;
                        }
                        if std::mem::take(&mut self.map_acc_used) {
                            let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                            self.emit(&format!("void *{} = (void *){};", l, v.c));
                            self.direct_refs.insert(l.clone());
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: v.ty.clone(), root: String::new(), kind: BindKind::Local });
                        } else {
                            // An ordinary call: its reference, as the borrow
                            // of `*call` would be (the same path).
                            let r = self.into_temp(v, *line)?;
                            let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                            self.emit(&format!("void *{} = ({}).p;", l, r.c));
                            self.unstored.insert(l.clone(), format!("({}).tok", r.c));
                            self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: r.ty.clone(), root: String::new(), kind: BindKind::Local });
                        }
                        let diverged = self.lower_stmt(next)?;
                        if diverged {
                            self.end_diverged();
                            result = never();
                            break;
                        }
                        self.pop_stmt();
                        skip_next = true;
                        continue;
                    }
                    let v = self.lower_expr(e, None)?;
                    if matches!(v.ty, Type::Never) {
                        self.end_diverged();
                        result = never();
                        break;
                    }
                    let r = self.into_temp(v, *line)?;
                    let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                    self.emit(&format!("void *{} = ({}).p;", l, r.c));
                    self.unstored.insert(l.clone(), format!("({}).tok", r.c));
                    self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l, ty: r.ty.clone(), root: String::new(), kind: BindKind::Local });
                    let diverged = self.lower_stmt(next)?;
                    if diverged {
                        self.end_diverged();
                        result = never();
                        break;
                    }
                    self.pop_stmt();
                    skip_next = true;
                    continue;
                }
            }
            if let Some((name, alias)) = self.foreach_alias(s, b.stmts.get(si + 1))? {
                self.alias_kept.push(alias.clone());
                self.elem_alias.insert(name.clone(), alias);
                aliases.push(name);
                continue;
            }
            self.push_stmt_at(stmt_line(s));
            let diverged = self.lower_stmt(s)?;
            if diverged {
                self.end_diverged();
                result = never();
                break;
            }
            self.pop_stmt();
            self.end_ref_bindings(s);
        }
        if !matches!(result.ty, Type::Never) {
            if let Some(t) = &b.tail {
                self.at(t.line);
                self.push_stmt_at(Some(t.line));
                // A tail `match` is its statement's last evaluation as a
                // statement `match` is (`stmt_match`).
                self.stmt_match = matches!(t.kind, ExprKind::Match(..));
                let v = self.lower_expr(t, expected);
                self.stmt_match = false;
                let v = v?;
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
        for a in aliases {
            self.elem_alias.remove(&a);
        }
        self.scopes.pop();
        Ok(result)
    }

    // D-0111 `[Ref-Binding-Last-Use]`: after `s`, each local reference,
    // slice or `StringView` binding of this block the checker ended there
    // ends as at the block's exit (its object, and the path it holds).
    fn end_ref_bindings(&mut self, s: &Stmt) {
        if !self.g.items.any_early_ends.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let names = self.g.items.early_ends.lock().unwrap().get(&(s as *const Stmt as usize)).cloned();
        for n in names.unwrap_or_default() {
            let Some(b) = self.scopes.last().and_then(|sc| sc.get(&n)).cloned() else { continue };
            let is_ref = coby::typecheck::ends_at_last_use(&self.g.items, &b.ty);
            if self.held_c.contains(&b.c) {
                // A held local (`cb_frame_hold`): its frame lets go now.
                if let Some(tk) = self.unstored.get(&b.c).cloned() {
                    self.emit(&format!("cb_frame_unhold({});", tk));
                }
                continue;
            }
            let root = if b.root.is_empty() { self.holder_roots.get(&b.c).cloned().unwrap_or_default() } else { b.root.clone() };
            if !is_ref || root.is_empty() || !matches!(b.kind, BindKind::Local) {
                continue;
            }
            self.emit(&format!("cb_end_binding({});", root));
        }
    }

    // Whether `e` is `r.f…` for a direct reference parameter `r`.
    fn direct_place(&self, e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) => self.direct_place(a),
            ExprKind::Path(segs, _) if segs.len() == 1 => self.lookup(&segs[0]).map_or(false, |b| self.direct_refs.contains(&b.c) && matches!(b.ty, Type::Ref(..))),
            _ => false,
        }
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
        let frame_line = self.frame_line.take();
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
                self.push_frame_at(frame_line);
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
        self.push_frame_at(frame_line);
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
            // `foreach (x in p)` over a confined parameter `p`
            // (`foreach_param_reads_only`): its hidden holders `$e = p` and
            // `$d = $each_drain($e, n)` (the reference itself, `each.rs`)
            // are `p` under other names, reaching the vector as `p` does.
            Stmt::Let { ty: None, name, init: Some(e), .. } if name.starts_with('$') && self.confined_holder(e).is_some() => {
                let (lv, path, elem) = self.confined_holder(e).unwrap();
                // Bound as a direct reference is (`bind_param`): the
                // address alone, its accesses unchecked.
                let vty = Type::Named("std::Vec".to_string(), vec![elem.clone()]);
                let vec_c = self.g.ctype(&vty)?;
                let l = format!("l_{}_{}", san(name), self.g.fresh(""));
                self.emit(&format!("void *{} = (void *)&({});", l, lv));
                self.direct_refs.insert(l.clone());
                self.scopes.last_mut().unwrap().insert(name.clone(), Binding { c: l.clone(), ty: Type::Ref(Box::new(vty), Mode::Shared), root: String::new(), kind: BindKind::Local });
                self.confined_params.insert(name.clone(), (format!("(*({} *){})", vec_c, l), path, elem));
                Ok(false)
            }
            Stmt::Let { ty, name, init, .. } => {
                let declared = ty.as_ref().map(|t| self.sub(t));
                let cname = format!("l_{}_{}", san(name), self.g.fresh(""));
                match init {
                    Some(e) => {
                        let v = self.lower_expr(e, declared.as_ref())?;
                        // A closure-typed binding the body borrows (`&g`,
                        // passed where `ref<fn(…) : R, m>` is expected) is
                        // kept as a callable box: a reference to it is then
                        // a reference to a `fn` value, as the callee reads
                        // it. Its value, copies and calls are the box's.
                        let v = match (&declared, &v.ty) {
                            (None, Type::Closure(id)) if self.pinned.contains(name) => {
                                let info = self.g.closures[id].clone();
                                let dst = Type::Fn(info.params.clone(), Box::new(info.ret.clone()));
                                self.box_closure(v, dst, e.line)?
                            }
                            _ => v,
                        };
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
            Stmt::Destructure { struct_name, fields, names, rest, init } => {
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
                let decl_names: Vec<String> = sd.fields.iter().map(|f| f.name.clone()).collect();
                for (fname, binder) in coby::ast::destructure_pairs(names, fields, *rest, &decl_names) {
                    let field = &fname;
                    let fd = sd.fields.iter().find(|f| &f.name == field).expect("field");
                    let fty = apply(&fd.ty, &sub);
                    let cname = format!("l_{}_{}", san(&binder), self.g.fresh(""));
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
                        self.note_frame(&binder, &root);
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: fty, root, kind: BindKind::Local });
                    } else {
                        self.bind_value(&binder, cname, &fty, &fv, init.line)?;
                    }
                }
                if let Some(o) = &whole.obj {
                    self.emit(&format!("cb_end_moved_out({});", o));
                }
                Ok(false)
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => {
                self.stmt_match = matches!(e.kind, ExprKind::Match(..));
                let v = self.lower_expr(e, None);
                self.stmt_match = false;
                let v = v?;
                if matches!(v.ty, Type::Never) {
                    return Ok(true);
                }
                if !matches!(v.ty, Type::Void) {
                    if v.obj.is_none() && self.holds_fn(&v.ty) {
                        // D-0089: a discarded value's closures end with it.
                        let t = self.into_temp(v, e.line)?;
                        let tid = self.tid(&t.ty)?;
                        self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
                    } else {
                        self.emit(&format!("(void)({});", v.c));
                    }
                }
                Ok(false)
            }
        }
    }

    // ---- places ----

    fn lower_place(&mut self, e: &Expr) -> R<P> {
        // `x.f…` over a `foreach` element alias, read in place.
        if let Some(r) = self.alias_field_chain(e) {
            return self.lower_place(&r);
        }
        // `*Vec::index_*(&x, i)` (`x[i]`, `vec_index_call`) for a confined
        // `x`: the element's address after the bounds check, as
        // `lower_expr` reaches it (`elem_access_place`'s confined case).
        if let ExprKind::Deref(inner) = &e.kind {
            if let ExprKind::Call(c, a) = &inner.kind {
                let index = matches!(&c.kind, ExprKind::Path(segs, _) if matches!(segs.join("::").as_str(), "std::Vec::index_shared" | "std::Vec::index_exclusive"));
                if index && a.first().and_then(|a0| self.confined_local(a0)).is_some() {
                    if let Some(p) = self.elem_access_place(inner, self.place_write, e.line)? {
                        return Ok(p);
                    }
                }
            }
        }
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
                // D-0064: `g.f` through a guard is `(*g).f`.
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
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
                // `vec_index_call` probes the base's type; the D-0103 flag is
                // for the lowering proper.
                let temp_root = std::mem::take(&mut self.temp_root);
                let vx = self.vec_index_call(e, self.place_write)?;
                self.temp_root = temp_root;
                if let Some(x) = vx {
                    // The `Vec`'s borrow in the rewritten call is part of the
                    // same temporary (D-0103).
                    if temp_root {
                        let call = match &x.kind {
                            ExprKind::Deref(c) => &**c,
                            _ => &*x,
                        };
                        if let ExprKind::Call(_, xa) = &call.kind {
                            if let Some(a0 @ Expr { kind: ExprKind::Borrow(..), .. }) = xa.first() {
                                self.g.items.temp_borrows.lock().unwrap().insert(a0 as *const Expr as usize);
                            }
                        }
                        self.temp_root = false;
                    }
                    self.place_write = false;
                    return self.lower_place(&x);
                }
                let pb = self.lower_place(base)?;
                let pb = self.auto_deref(pb)?;
                let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
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
                    Type::Slice(elem, _) if pb.is_binding && self.direct_refs.contains(&pb.c) => {
                        // A direct slice (`Gen::direct_refs_of`): bounds only.
                        let lenv = self.g.fresh("len");
                        self.emit(&format!("uint64_t {} = ({}).len;", lenv, pb.c));
                        self.dollar.push(lenv.clone());
                        let i = self.lower_expr(idx, Some(&Type::Int(IntTy::Usize)));
                        self.dollar.pop();
                        let it = self.into_temp(i?, e.line)?;
                        let loc = self.loc(e.line);
                        self.emit(&format!("cb_index_check((uint64_t){}, {}, {});", it.c, lenv, loc));
                        return Ok(P { c: format!("({}).data[{}]", pb.c, it.c), ty: (*elem).clone(), base: None, projs: Vec::new(), is_binding: false, raw: None });
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
                if let ExprKind::Path(segs, targs) = &inner.kind {
                    if segs.len() == 1 && targs.is_empty() {
                        if let Some(b) = self.lookup(&segs[0]).filter(|b| self.direct_refs.contains(&b.c)) {
                            let p = P { c: b.c, ty: b.ty, base: None, projs: Vec::new(), is_binding: true, raw: None };
                            return self.auto_deref(p);
                        }
                    }
                }
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
                // A temporary used as a place (`make().f`): unchecked,
                // unless a borrow of part of it outlives the expression
                // (D-0103): then it is an object of this statement, and the
                // place is checked from its root path.
                let temp_root = std::mem::take(&mut self.temp_root);
                let v = self.lower_expr(e, None)?;
                let t = self.into_temp(v, e.line)?;
                if temp_root && !matches!(t.ty, Type::Ref(..)) {
                    let loc = self.loc(e.line);
                    let o = match &t.obj {
                        Some(o) => o.clone(),
                        None => {
                            let tid = self.tid(&t.ty)?;
                            let o = self.g.fresh("o");
                            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t.c, tid, loc));
                            o
                        }
                    };
                    let root = self.g.fresh("root");
                    self.emit(&format!("uint64_t {} = cb_temp_path({});", root, o));
                    return Ok(P { c: t.c, ty: t.ty, base: Some(root), projs: Vec::new(), is_binding: false, raw: None });
                }
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
            Type::Ref(pointee, _) if p.is_binding && self.direct_refs.contains(&p.c) => {
                let c = self.g.ctype(pointee)?;
                Ok(P { c: format!("(*({} *){})", c, p.c), ty: (**pointee).clone(), base: None, projs: Vec::new(), is_binding: false, raw: None })
            }
            Type::Ref(pointee, _) => {
                let tk = self.g.fresh("tk");
                let load = self.slot_tok(&p.c);
                self.emit(&format!("uint64_t {} = {};", tk, load));
                let c = self.g.ctype(pointee)?;
                let inner = P { c: format!("(*({} *){})", c, p.c), ty: (**pointee).clone(), base: Some(tk), projs: Vec::new(), is_binding: false, raw: None };
                self.auto_deref(inner)
            }
            _ => Ok(p),
        }
    }

    // A reference stored in memory, read back as a register value.
    fn read_ref_slot(&self, slot: &str, ty: &Type) -> V {
        V { c: format!("((cb_ref){{ {}, {} }})", slot, self.slot_tok(slot)), ty: ty.clone(), obj: None }
    }

    // The token a reference slot holds: loaded, or, for an unstored
    // parameter, the one it came with.
    fn slot_tok(&self, slot: &str) -> String {
        match self.unstored.get(slot) {
            Some(t) => t.clone(),
            None => format!("cb_load_ref(&{})", slot),
        }
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
                // A resource moved out of a part of a live object, or out
                // through a reference whose referent its owner still
                // reaches (`[Authority-Transfer-Aliased]`): the checks
                // above found the place valid, and the move itself is the
                // fault, as `coby` reports it.
                let d = if p.projs.is_empty() { "diag.move-while-aliased" } else { "diag.move-out-of-field" };
                self.emit(&format!("cb_fault(\"{}\", {});", d, loc));
                let o = self.g.fresh("o");
                self.emit(&format!("uint64_t {} = 0;", o));
                return Ok(V { c: p.c, ty: p.ty, obj: Some(o) });
            }
            let o = self.g.fresh("o");
            self.emit(&format!("uint64_t {} = cb_take({}, {});", o, p.base.as_deref().unwrap(), loc));
            return Ok(V { c: p.c, ty: p.ty, obj: Some(o) });
        }
        if matches!(p.ty, Type::Ref(..)) {
            return Ok(self.read_ref_slot(&p.c, &p.ty));
        }
        if matches!(p.ty, Type::Fn(..)) {
            // `[Read]` of a fn value (D-0089): its closure is copied, or
            // moves out and empties the slot when it owns a resource.
            let t = self.g.fresh("t");
            self.emit(&format!("void *{} = cb_fn_read((void **)&({}), {});", t, p.c, loc));
            return Ok(V { c: t, ty: p.ty, obj: None });
        }
        if self.holds_fn(&p.ty) {
            // A value holding fn values, read by value: the same, slot by
            // slot, after the copy of its bytes.
            let t = self.g.fresh("t");
            let c = self.g.ctype(&p.ty)?;
            let tid = self.tid(&p.ty)?;
            self.emit(&format!("{} {} = {};", c, t, p.c));
            self.emit(&format!("cb_fn_split(&{}, &({}), {}u, {});", t, p.c, tid, loc));
            return Ok(V { c: t, ty: p.ty, obj: None });
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
        if info.fields.is_empty() {
            // Its code never reaches `self` (`lower_closure_body`): a call
            // through the box forms no borrow of the object (`CbFnBox`).
            self.emit(&format!("((cb_fnbox *){})->closure = 2u;", h));
        }
        Ok(V { c: h, ty: dst, obj: None })
    }

    fn lower_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> R<V> {
        match &e.kind {
            ExprKind::IntLit(v, None) if matches!(expected, Some(Type::F32 | Type::F64)) && self.g.items.number_literals.lock().unwrap().contains(&(e as *const Expr as usize)) => {
                // D-0090: `T x = 0` with `T` a floating-point type.
                let ty = expected.cloned().unwrap();
                Ok(V { c: c_float(*v as f64, &ty), ty, obj: None })
            }
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
                // A direct reference parameter handed on whole: the
                // reference the call was given, its path unchanged.
                if let Some(b) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.lookup(n)).filter(|b| self.direct_refs.contains(&b.c) && matches!(b.ty, Type::Ref(..))) {
                    let pc = b.c.strip_prefix("l_").unwrap_or(&b.c).to_string();
                    return Ok(V { c: format!("((cb_ref){{ {}, ({}).tok }})", b.c, pc), ty: b.ty, obj: None });
                }
                if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
                    let p = self.lower_place(e)?;
                    return self.read_place(p, e.line);
                }
                // A `foreach` element aliased for a reader loop
                // (`foreach_alias`), used whole: the element's address with
                // token 0, every check on it proven for the loop's step.
                if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
                    let p = self.lower_place(&alias)?;
                    if matches!(p.ty, Type::Never) {
                        return Ok(never());
                    }
                    return Ok(V { c: format!("((cb_ref){{ (void *)&({}), 0 }})", p.c), ty: Type::Ref(Box::new(p.ty), Mode::Shared), obj: None });
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
                        // D-0093 rejects the no-evidence binding up front;
                        // what still reaches here is an argument position
                        // whose call this lowering cannot solve — a limit,
                        // not an internal error.
                        return unsupported(&format!("cannot infer type arguments of `{}` as a value here", name));
                    };
                    let cname = self.g.request_fn(&name, inst.clone())?;
                    let fsub = subst_of(&f.type_params, &inst);
                    let fty = Type::Fn(f.params.iter().map(|p| self.sub(&apply(&p.ty, &fsub))).collect(), Box::new(self.sub(&apply(&f.ret, &fsub))));
                    return Ok(V { c: format!("cb_fn_item((void *){})", cname), ty: fty, obj: None });
                }
                internal(&format!("unresolved path `{}`", name))
            }
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                // D-0103: a field moved out of a temporary.
                if matches!(e.kind, ExprKind::Field(..)) && self.g.items.any_temp_moves.load(std::sync::atomic::Ordering::Relaxed) {
                    let r = self.g.items.temp_moves.lock().unwrap().get(&(e as *const Expr as usize)).cloned();
                    if let Some(r) = r {
                        return self.lower_expr(&r, expected);
                    }
                }
                if let ExprKind::Deref(inner) = &e.kind {
                    if let ExprKind::Path(segs, _) = &inner.kind {
                        if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
                            return self.lower_expr(&alias, expected);
                        }
                    }
                }
                if let Some(r) = self.alias_field_chain(e) {
                    return self.lower_expr(&r, expected);
                }
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
                // D-0118: a bit field is read out of the whole backing integer.
                if let ExprKind::Field(base, f) = &e.kind {
                    if let Some((w, off, backing, fty)) = self.bitfield_of(base, f)? {
                        let pb = self.lower_place(base)?;
                        let pb = self.auto_deref(pb)?;
                        let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                        let v = self.read_place(pb, e.line)?;
                        let v = self.into_temp(v, e.line)?;
                        let fc = self.g.ctype(&fty)?;
                        let c = format!("(({})((({}).bits >> {}u) & {}))", fc, v.c, off, c_bits_mask(backing, w));
                        return Ok(V { c, ty: fty, obj: None });
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
            // `[v; N]` (D-0072): v once, copied into every element (its type
            // is plain: no resource, reference or function value).
            ExprKind::ArrayRepeat(x, n) => {
                let elem_ty = match expected {
                    Some(Type::Array(inner, _)) => Some((**inner).clone()),
                    _ => None,
                };
                let v = self.lower_expr(x, elem_ty.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let elem = elem_ty.unwrap_or_else(|| v.ty.clone());
                let ec = self.g.ctype(&elem)?;
                let one = self.g.fresh("rep");
                self.emit(&format!("{} {} = {};", ec, one, v.c));
                let ty = Type::Array(Box::new(elem), *n);
                let c = self.g.ctype(&ty)?;
                let t = self.g.fresh("arr");
                let k = self.g.fresh("k");
                self.emit(&format!("{} {};", c, t));
                self.emit(&format!("for (uint64_t {k} = 0; {k} < {n}ULL; {k}++) {t}.a[{k}] = {one};"));
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
                None => {
                    // D-0103: a temporary sliced as a call's argument.
                    self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                    let r = self.lower_slice_of(mode, base, lo, hi, e.line);
                    self.temp_root = false;
                    r
                }
            },
            // D-0073: a temporary borrowed as a call's argument: the value
            // is an object of this statement, borrowed from its root path.
            ExprKind::Borrow(mode, inner)
                if coby::typecheck::is_temp_value(inner)
                    || (matches!(inner.kind, ExprKind::Path(..) | ExprKind::Paren(_)) && self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize))) =>
            {
                // D-0084: a borrowed literal takes the referent type expected.
                let pointee = match expected {
                    Some(Type::Ref(t, _)) => Some((**t).clone()),
                    _ => None,
                };
                let v = self.lower_expr(inner, pointee.as_ref())?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let v = self.into_temp(v, e.line)?;
                let loc = self.loc(e.line);
                let o = match &v.obj {
                    Some(o) => o.clone(),
                    None => {
                        let tid = self.tid(&v.ty)?;
                        let o = self.g.fresh("o");
                        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, v.c, tid, loc));
                        o
                    }
                };
                let root = self.g.fresh("root");
                self.emit(&format!("uint64_t {} = cb_temp_path({});", root, o));
                let tok = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("uint64_t {} = cb_borrow({}, NULL, 0, {}, {});", tok, root, m, loc));
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", v.c, tok), ty: Type::Ref(Box::new(v.ty), mode.clone()), obj: None })
            }
            ExprKind::Borrow(mode, inner) => {
                self.place_write = *mode == Mode::Exclusive;
                // D-0103: part of a temporary, as a call's argument: its
                // root becomes an object of this statement.
                self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                let p = self.lower_place(inner);
                self.temp_root = false;
                self.place_write = false;
                let p = p?;
                let Some(base) = &p.base else { return internal("borrow of a temporary") };
                // `&v[i]` / `&mut v[i]` on a `Vec`: the index call, in the
                // borrow's own mode (`vec_index_call` chose it from
                // `place_write`), has just formed the element's path; a
                // borrow of it in that mode, with nothing in between,
                // cannot fail and would be a second path no check could
                // tell from the first. That path is the result.
                // The same for `&*f(…)` / `&mut *f(…)`, `f` returning a
                // reference of the borrow's mode: the reference the call
                // just returned is that path (`*HashMap::entry(…) += 1`).
                if let ExprKind::Deref(x) = &strip_parens(inner).kind {
                    if matches!(&strip_parens(x).kind, ExprKind::Call(..)) && p.projs.is_empty() && p.raw.is_none() && base.ends_with(".tok") && !p.is_binding {
                        if let Ok(Type::Ref(_, rm)) = self.probe_type(x, None) {
                            if rm == *mode {
                                return Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, base), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                            }
                        }
                    }
                }
                if matches!(&strip_parens(inner).kind, ExprKind::Index(..)) && p.projs.is_empty() && p.raw.is_none() && base.ends_with(".tok") && !p.is_binding {
                    if let Some(Type::Named(n, _)) = self.probe_place_type(match &strip_parens(inner).kind { ExprKind::Index(b, _) => b, _ => unreachable!() }).ok().map(|t| match t { Type::Ref(x, _) => *x, t => t }) {
                        if n == "std::Vec" {
                            return Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, base), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None });
                        }
                    }
                }
                let loc = self.loc(e.line);
                let tok = self.g.fresh("tok");
                let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {});", tok, base, Self::projs_args(&p.projs), m, loc));
                Ok(V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty), mode.clone()), obj: None })
            }
            ExprKind::Call(callee, args) => self.lower_call(callee, args, expected, e.line),
            ExprKind::Assign(place, val) => {
                // `*x = e` for a `foreach (x in &mut v)` element aliased to
                // `(*$c)[$i]` (`foreach_alias`): the write is to that element.
                if let ExprKind::Deref(inner) = &strip_parens(place).kind {
                    if let ExprKind::Path(segs, _) = &inner.kind {
                        if let Some(alias) = segs.first().filter(|_| segs.len() == 1).and_then(|n| self.elem_alias.get(n)).cloned() {
                            let w = std::rc::Rc::new(Expr { kind: ExprKind::Assign(Box::new((*alias).clone()), val.clone()), line: e.line });
                            self.alias_kept.push(w.clone());
                            return self.lower_expr(&w, expected);
                        }
                    }
                }
                // D-0118: a write into a bit field: the whole backing integer
                // is the place written; the value must fit the width.
                if let ExprKind::Field(base, f) = &strip_parens(place).kind {
                    if let Some((w, off, backing, fty)) = self.bitfield_of(base, f)? {
                        self.place_write = true;
                        let pb = self.lower_place(base);
                        self.place_write = false;
                        let pb = self.auto_deref(pb?)?;
                        let pb = if matches!(pb.ty, Type::Guard(_)) { self.guard_through(pb, e.line)? } else { pb };
                        let v = self.lower_expr(val, Some(&fty))?;
                        if matches!(v.ty, Type::Never) {
                            return Ok(never());
                        }
                        let v = self.into_temp(v, val.line)?;
                        let loc = self.loc(e.line);
                        if let Some(b) = &pb.base {
                            self.emit(&format!("cb_write({}, {}, 0u, {});", b, Self::projs_args(&pb.projs), loc));
                        }
                        let bc = c_int_type(backing);
                        let mask = c_bits_mask(backing, w);
                        self.emit(&format!("if (({})({}) > {}) cb_fault(\"diag.narrowing-overflow\", {});", bc, v.c, mask, loc));
                        self.emit(&format!("({}).bits = (({}).bits & ~({} << {}u)) | ((({})({})) << {}u);", pb.c, pb.c, mask, off, bc, v.c, off));
                        return Ok(unit());
                    }
                }
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
                // A value built in place that owns nothing observable
                // (`inert_value`): only the target's checks come between it
                // and the store, and if one faults, destroying the value
                // would run nothing; it needs no object. Not into raw
                // memory, where the move also ends what lay there.
                self.objectless = p.raw.is_none() && self.is_res(&p.ty) && self.inert_value(val)?;
                let v = self.lower_expr(val, Some(&p.ty));
                self.objectless = false;
                let v = v?;
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
                } else if self.is_res(&p.ty) && p.raw.is_none() && self.direct_place(place) {
                    // A resource field through a direct parameter
                    // (`box_rec_uses`): no path to check, but the value's
                    // check stays -- overwriting a live resource faults
                    // (`[Write-Resource-Overwrite-Rejected]`), as `cb_write`
                    // would for the target's value.
                    let target = if coby::typecheck::always_owns(&self.g.items, &p.ty) {
                        "1".to_string()
                    } else {
                        let tid = self.tid(&p.ty)?;
                        format!("cb_owns(&({}), {}u)", p.c, tid)
                    };
                    self.emit(&format!("if ({}) cb_fault(\"diag.overwrite-of-live-resource\", {});", target, loc));
                }
                self.store(&p.c, &p.ty, &v)?;
                if self.is_res(&p.ty) && !self.inert_c.contains(&v.c) {
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
                self.propagate_scrutinee = true;
                self.lower_match(inner, &arms, expected, l)
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => {
                self.frame_line = Some(e.line);
                self.lower_block(b, expected)
            }
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
            ExprKind::Closure { is_move, captures, params, ret, body } => self.lower_closure(*is_move, captures, params, ret.as_ref(), body, e.line),
        }
    }

    // `[Closure-Form-Borrow]`/`[Closure-Form-Move]`: the capture struct,
    // then the body as a C function taking `self`.
    fn lower_closure(&mut self, is_move: bool, captures: &[String], params: &[Param], declared: Option<&Type>, body: &Block, line: usize) -> R<V> {
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
        let ret = self.lower_closure_body(id, params, declared, body)?;
        self.g.closures.get_mut(&id).unwrap().ret = ret;
        Ok(V { c: t, ty, obj: Some(o) })
    }

    // The body as `ret cl_N(cb_ref p_self, params…)`, with each captured
    // name resolving to `*self.f_i` or `self.f_i` ([Closure-Call]).
    fn lower_closure_body(&mut self, id: u64, params: &[Param], declared: Option<&Type>, body: &Block) -> R<Type> {
        let info = self.g.closures[&id].clone();
        let self_ty = Type::Ref(Box::new(Type::Closure(id)), Mode::Exclusive);
        // First pass to learn the result type, then the real one; a
        // written result type (D-0081) needs no first pass.
        let mut ret = declared.map(|t| self.sub(t)).unwrap_or(Type::Void);
        for pass in (if declared.is_some() { 1 } else { 0 })..2 {
            let probing = pass == 0;
            if probing {
                self.g.probing += 1;
            }
            let saved_uid = self.g.uid;
            let saved_nstr = self.g.nstr;
            let saved_strings = self.g.strings.clone();
            // A closure's own parameters and its captures are its bindings.
            let bound: Vec<String> = params.iter().map(|p| p.name.clone()).chain(info.fields.iter().map(|(n, _)| n.clone())).collect();
            let confined_names = self.g.confined_vecs(body, &bound);
            let pinned = pinned_names(body, &|segs: &[String], j: usize| self.g.pure_direct_ref_arg(segs, j));
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
                pinned,
                assigned: assigned_names(body),
                rebind_frames: HashMap::new(),
                holder_roots: HashMap::new(),
                no_overflow: HashSet::new(),
                proven_next: false,
                confined_names,
                confined: HashSet::new(),
                confined_params: HashMap::new(),
                probes: HashMap::new(),
                kept: Vec::new(),
                dollar: Vec::new(),
                place_write: false,
            elem_mode: None,
            fused_scrut: None,
            map_acc: None,
            map_acc_used: false,
            temp_root: false,
                frame_line: None,
                vec_index_exprs: HashMap::new(),
                propagate_scrutinee: false,
                elem_alias: HashMap::new(),
                direct_params: HashSet::new(),
                direct_refs: HashSet::new(),
                pure_slices: HashSet::new(),
                unstored: HashMap::new(),
                unstored_params: HashSet::new(),
                held_locals: HashSet::new(),
                held_c: HashSet::new(),
                unminted_binders: HashMap::new(),
                alias_kept: Vec::new(),
                stmt_match: false,
                branch_scope: false,
                objectless: false,
                inert_c: HashSet::new(),
               
            };
            fx.push_frame();
            // A closure that captures nothing never reaches `self`: it is
            // bound as a bare address (`bind_param`'s direct form), and its
            // own parameters may be direct as a function's are.
            if info.fields.is_empty() {
                fx.direct_params = fx.g.direct_refs_of(params, body, &fx.subst)?;
                fx.direct_params.insert("self".to_string());
            }
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
            let out = elide_implied_checks(&drop_dead_locations(&resolve_scopes(&fx.out)));
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
                write!(self.g.fns, "{}\n{{\n    cb_stack_check();\n{}}}\n\n", sig, out).unwrap();
            }
        }
        Ok(ret)
    }

    // `Vec::push(&mut v, s)` for a `Vec<String>`, `s` being
    // `String::from_str(…)` or `String::clone(…)` (so `sprintf`): as
    // `lower_map_fresh_key` passes a fresh key, the new `String` goes to
    // the native push (`native_vec_push`) with no object -- received,
    // then moved into the buffer, where a reference-free object ends at
    // once, with nothing able to see it. `v` a local is passed with its
    // root path after the borrow's check (the native push only checks
    // through it).
    // `Box::new(v)` for a `T` with no reference and no `fn` value in it:
    // `std`'s body inline (`spec/21` §0: its bodies are normative for what
    // they do, not how). The cells `Box::cells<T>()` allocates, the alloc
    // fault, the value stored, and `Box { .ptr, .full = true }`. The
    // body's `*bp = v` moves `v` into raw memory, where a reference-free
    // object ends at once (`cb_raw_move_in`); here `v`'s object, if it has
    // one, ends the same way, and a value built in place never gets one.
    // The `Box` gets its object as any fresh resource value does.
    fn lower_box_new(&mut self, name: &str, f: &FnDecl, targs: &[Type], args: &[Expr], expected: Option<&Type>, line: usize) -> R<Option<V>> {
        if name != "std::Box::new" || !in_prelude(f) || args.len() != 1 {
            return Ok(None);
        }
        let objectless = std::mem::take(&mut self.objectless);
        // `T` as the call would infer it: written, from the `Box<T>`
        // expected (a literal argument takes it, D-0079), or the argument's.
        let t = match (targs.first(), expected) {
            (Some(t), _) => self.sub(t),
            (None, Some(Type::Named(n, a))) if n == "std::Box" && a.len() == 1 => a[0].clone(),
            _ => self.probe_type(&args[0], None)?,
        };
        if self.has_refs(&t) || self.holds_fn(&t) || matches!(t, Type::Closure(_) | Type::Never) {
            return Ok(None);
        }
        self.objectless = self.inert_expr(&args[0])?;
        let v = self.lower_expr(&args[0], Some(&t));
        self.objectless = false;
        let v = v?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let (size, align) = self.g.size_align(&t)?;
        let tc = self.g.ctype(&t)?;
        let bt = Type::Named("std::Box".to_string(), vec![t.clone()]);
        let bc = self.g.ctype(&bt)?;
        self.at(line);
        let p = self.g.fresh("bx");
        self.emit(&format!("uint8_t *{} = cb_allocate({}u, {}u);", p, size.max(1), align));
        self.emit(&format!("if (!{}) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);", p));
        if size > 0 {
            self.emit(&format!("*({} *){} = {};", tc, p, v.c));
        }
        if let Some(o) = &v.obj {
            self.emit(&format!("cb_raw_move_in({}, {});", o, p));
        }
        let b = self.g.fresh("b");
        self.emit(&format!("{} {};", bc, b));
        self.emit(&format!("{}.ptr = ({} *){};", b, tc, p));
        self.emit(&format!("{}.full = 1;", b));
        // As a variant's payload (`inert_box_new`) it is absorbed next, with
        // nothing evaluated between: no object.
        if objectless {
            self.inert_c.insert(b.clone());
            return Ok(Some(V { c: b, ty: bt, obj: None }));
        }
        // Its object, established over it as a struct literal's is.
        let tid = self.tid(&bt)?;
        let loc = self.loc(line);
        let o = self.g.fresh("o");
        self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, b, tid, loc));
        Ok(Some(V { c: b, ty: bt, obj: Some(o) }))
    }

    // A literal that owns nothing: a variant without a payload (`None`), or
    // a struct literal of a struct with no destructor, not declared
    // `resource`, each field of which is plain data (no reference, no `fn`
    // value) or itself inert. Built in place, it needs no object: there is
    // nothing to destroy if a fault ends the statement first, and nothing
    // can reach it before the aggregate around it absorbs it.
    fn inert_expr(&mut self, e: &Expr) -> R<bool> {
        match &strip_parens(e).kind {
            ExprKind::Path(segs, _) => {
                if segs.len() == 1 && self.lookup(&segs[0]).is_some() {
                    return Ok(false);
                }
                Ok(match self.variant_of(segs) {
                    Some((ename, vi)) => self.g.enum_decl(&ename).map_or(false, |ed| ed.variants[vi].payload.is_none()),
                    None => false,
                })
            }
            ExprKind::StructLit(segs, _, fields) => {
                let name = segs.join("::");
                let Some(sd) = self.g.struct_decl(&name) else { return Ok(false) };
                if sd.resource || sd.bits.is_some() || self.g.items.fns.contains_key(&format!("{}::drop", name)) {
                    return Ok(false);
                }
                // By each field's declared type (a generic one is not
                // judged: not inert).
                for (fname, fe) in fields {
                    let Some(fd) = sd.fields.iter().find(|f| &f.name == fname) else { return Ok(false) };
                    if !resolved(&fd.ty, &sd.type_params) || !sd.type_params.is_empty() {
                        return Ok(false);
                    }
                    let ft = fd.ty.clone();
                    let ok = if self.is_res(&ft) { self.inert_expr(fe)? } else { self.g.plain_data(&ft) };
                    if !ok {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    // An inert literal (`inert_expr`), or a variant whose payload is one or
    // is a `Box` of one (`inert_box_new`): destroying it runs no destructor.
    fn inert_value(&mut self, e: &Expr) -> R<bool> {
        if self.inert_expr(e)? {
            return Ok(true);
        }
        let ExprKind::Call(c, args) = &strip_parens(e).kind else { return Ok(false) };
        let ExprKind::Path(segs, _) = &c.kind else { return Ok(false) };
        if args.len() != 1 || (segs.len() == 1 && self.lookup(&segs[0]).is_some()) || self.g.items.fns.contains_key(&segs.join("::")) {
            return Ok(false);
        }
        let Some((ename, vi)) = self.variant_of(segs) else { return Ok(false) };
        if !self.g.enum_decl(&ename).map_or(false, |ed| ed.variants[vi].payload.is_some()) {
            return Ok(false);
        }
        Ok(self.inert_expr(&args[0])? || self.inert_box_new(&args[0])?)
    }

    // `Box::new(x)` that `lower_box_new` builds natively, `x` inert: as a
    // variant's payload, a `Box` that needs no object (`objectless`).
    fn inert_box_new(&mut self, e: &Expr) -> R<bool> {
        let ExprKind::Call(c, args) = &strip_parens(e).kind else { return Ok(false) };
        let ExprKind::Path(segs, _) = &c.kind else { return Ok(false) };
        if segs.join("::") != "std::Box::new" || args.len() != 1 || !self.g.items.fns.get("std::Box::new").map_or(false, |f| in_prelude(f)) {
            return Ok(false);
        }
        let Ok(t) = self.probe_type(&args[0], None) else { return Ok(false) };
        if self.has_refs(&t) || self.holds_fn(&t) || matches!(t, Type::Closure(_) | Type::Never) {
            return Ok(false);
        }
        self.inert_expr(&args[0])
    }

    fn lower_push_fresh(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if name != "std::Vec::push" || !in_prelude(f) || args.len() != 2 {
            return Ok(None);
        }
        let Ok(Type::Ref(vt, Mode::Exclusive)) = self.probe_type(&args[0], None) else { return Ok(None) };
        let Type::Named(vn, ea) = &*vt else { return Ok(None) };
        let [elem] = ea.as_slice() else { return Ok(None) };
        if vn != "std::Vec" || !matches!(elem, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, _) = &callee.kind else { return Ok(None) };
        let prod = segs.join("::");
        if !matches!(prod.as_str(), "std::String::from_str" | "std::String::clone") || cargs.len() != 1 {
            return Ok(None);
        }
        if !self.g.items.fns.get(&prod).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        // A confined vector has its own path (`lower_confined_call`).
        if self.confined_local(&args[0]).is_some() {
            return Ok(None);
        }
        let pass = match &args[0].kind {
            ExprKind::Borrow(Mode::Exclusive, place) => path_name(place).and_then(|n| self.lookup(n)).filter(|b| {
                matches!(b.kind, BindKind::Local) && !b.root.is_empty() && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && !matches!(b.ty, Type::Ref(..))
            }),
            _ => None,
        };
        let vv = if let Some(b) = pass {
            let loc = self.loc(args[0].line);
            self.emit(&format!("cb_borrow_unminted({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
            V { c: format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root), ty: Type::Ref(Box::new(b.ty.clone()), Mode::Exclusive), obj: None }
        } else {
            let vv = self.lower_expr(&args[0], None)?;
            if matches!(vv.ty, Type::Never) {
                return Ok(Some(never()));
            }
            self.into_temp(vv, line)?
        };
        let a = self.lower_expr(&cargs[0], if prod == "std::String::from_str" { Some(&Type::Str) } else { None })?;
        if matches!(a.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let a = self.into_temp(a, line)?;
        let pname = self.g.request_fn(&prod, Vec::new())?;
        let sc = self.g.ctype(elem)?;
        let kt = self.g.fresh("k");
        self.emit(&format!("{} {} = {}_raw({});", sc, kt, pname, a.c));
        let pushn = self.g.request_fn(name, vec![elem.clone()])?;
        self.emit(&format!("{}_raw({}, {});", pushn, vv.c, kt));
        Ok(Some(unit()))
    }

    // Whether argument `j` of a call to `name` goes to a native map body
    // (`native_map_op`) that only checks through its path: the map, and
    // for `get`, `get_mut` and `contains` the key.
    fn native_map_passthrough(&mut self, name: &str, f: &FnDecl, args: &[Expr], j: usize) -> bool {
        let ops = ["std::HashMap::get", "std::HashMap::get_mut", "std::HashMap::contains", "std::HashMap::entry", "std::HashMap::insert"];
        if !ops.contains(&name) || !in_prelude(f) || !(j == 0 || (j == 1 && !matches!(name, "std::HashMap::entry" | "std::HashMap::insert"))) {
            return false;
        }
        let Some(a0) = args.first() else { return false };
        let Ok(Type::Ref(mt, _)) = self.probe_type(a0, None) else { return false };
        let Type::Named(hn, kv) = &*mt else { return false };
        let [k, v] = kv.as_slice() else { return false };
        let text = matches!(k, Type::Named(n, a) if n == "std::String" && a.is_empty());
        if hn != "std::HashMap" || !(text || matches!(k, Type::Int(_) | Type::Bool)) {
            return false;
        }
        let Ok((vs, _)) = self.g.size_align(v) else { return false };
        if vs == 0 || self.has_refs(v) || self.holds_fn(v) {
            return false;
        }
        !matches!(name, "std::HashMap::entry" | "std::HashMap::insert") || self.g.plain_data(v)
    }

    // `HashMap::entry(m, k, x)` / `insert(m, k, x)`, natively for a
    // `String` key and a plain value (`native_map_op`), where `k` is
    // `String::from_str(s)` or `String::clone(r)` (`sprintf` is the
    // former) and `x` a literal or a local: the new `String` goes from the
    // copy to the map body with no object. Its object would be established,
    // sent, received, moved, sent and received again, then destroyed or
    // moved into the keys' cells (where an object ends at once, being
    // reference-free) -- with nothing in between that could see it (`x`
    // forms no path; a fault ends the program). So the `_raw` copy and the
    // `_rawk` body pass it as a value.
    fn lower_map_fresh_key(&mut self, name: &str, f: &FnDecl, args: &[Expr], line: usize) -> R<Option<V>> {
        if !matches!(name, "std::HashMap::entry" | "std::HashMap::insert") || !in_prelude(f) || args.len() != 3 {
            return Ok(None);
        }
        let Ok(Type::Ref(mt, Mode::Exclusive)) = self.probe_type(&args[0], None) else { return Ok(None) };
        let Type::Named(hn, kv) = &*mt else { return Ok(None) };
        let [k, v] = kv.as_slice() else { return Ok(None) };
        if hn != "std::HashMap" || !matches!(k, Type::Named(n, a) if n == "std::String" && a.is_empty()) {
            return Ok(None);
        }
        if self.g.size_align(v)?.0 == 0 || !self.g.plain_data(v) || self.has_refs(v) || self.holds_fn(v) {
            return Ok(None);
        }
        let ExprKind::Call(callee, cargs) = &strip_parens(&args[1]).kind else { return Ok(None) };
        let ExprKind::Path(segs, _) = &callee.kind else { return Ok(None) };
        let prod = segs.join("::");
        if !matches!(prod.as_str(), "std::String::from_str" | "std::String::clone") || cargs.len() != 1 {
            return Ok(None);
        }
        if !self.g.items.fns.get(&prod).map_or(false, |pf| in_prelude(pf)) {
            return Ok(None);
        }
        let inert = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_)) || path_name(e).map_or(false, |n| !n.starts_with("__"));
        if !inert(&args[2]) {
            return Ok(None);
        }
        let pass = match &args[0].kind {
            ExprKind::Borrow(Mode::Exclusive, place) => path_name(place).and_then(|n| self.lookup(n)).filter(|b| {
                matches!(b.kind, BindKind::Local) && !b.root.is_empty() && !self.direct_refs.contains(&b.c) && !self.unstored.contains_key(&b.c) && !matches!(b.ty, Type::Ref(..))
            }),
            _ => None,
        };
        let mv = if let Some(b) = pass {
            // As `native_map_passthrough` does for an ordinary call.
            let loc = self.loc(args[0].line);
            self.emit(&format!("cb_borrow_unminted({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
            V { c: format!("((cb_ref){{ (void *)&({}), {} }})", b.c, b.root), ty: Type::Ref(Box::new(b.ty.clone()), Mode::Exclusive), obj: None }
        } else {
            let mv = self.lower_expr(&args[0], None)?;
            if matches!(mv.ty, Type::Never) {
                return Ok(Some(never()));
            }
            self.into_temp(mv, line)?
        };
        let a = self.lower_expr(&cargs[0], if prod == "std::String::from_str" { Some(&Type::Str) } else { None })?;
        if matches!(a.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let a = self.into_temp(a, line)?;
        let pname = self.g.request_fn(&prod, Vec::new())?;
        let sc = self.g.ctype(k)?;
        let kt = self.g.fresh("k");
        self.emit(&format!("{} {} = {}_raw({});", sc, kt, pname, a.c));
        let x = self.lower_expr(&args[2], Some(v))?;
        let x = self.into_temp(x, line)?;
        let ename = self.g.request_fn(name, vec![k.clone(), v.clone()])?;
        let ret = if name == "std::HashMap::entry" { Type::Ref(Box::new(v.clone()), Mode::Exclusive) } else { Type::Named("std::Option".to_string(), vec![v.clone()]) };
        if self.map_acc == Some(args.as_ptr() as usize) && name == "std::HashMap::entry" {
            self.map_acc = None;
            let vc = self.g.ctype(v)?;
            let pv = self.g.fresh("pv");
            self.emit(&format!("{} *{} = {}_rawk_acc({}, {}, {});", vc, pv, ename, mv.c, kt, x.c));
            self.map_acc_used = true;
            return Ok(Some(V { c: pv, ty: ret, obj: None }));
        }
        let call = format!("{}_rawk({}, {}, {})", ename, mv.c, kt, x.c);
        self.finish_call(call, ret, line).map(Some)
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
            } else {
                // `m::V`: the enum of module `m` with that variant.
                ename = self.g.items.module_variant_enum(&prefix, &vname);
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
        let objectless = std::mem::take(&mut self.objectless);
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
                // The payload is the last thing evaluated before the
                // variant is: an inert literal, or a `Box` of one, needs no
                // object of its own (`objectless`).
                self.objectless = self.inert_expr(pe)? || self.inert_box_new(pe)?;
                let v = self.lower_expr(pe, exp.as_ref());
                self.objectless = false;
                let v = v?;
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
            if self.is_res(&pty) && !self.inert_c.contains(&v.c) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource payload has object")));
            }
        }
        if objectless && self.is_res(&ty) && !self.has_refs(&ty) {
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty, obj: None });
        }
        self.register_temp(t, ty, line)
    }

    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047): the source is
    // borrowed as `&base` would be, and the slice holds that borrow in its
    // reference slot, with a pointer to its first element and its length.
    fn lower_slice_of(&mut self, mode: &Mode, base: &Expr, lo: &Expr, hi: &Expr, line: usize) -> R<V> {
        self.lower_slice_of_as(mode, base, lo, hi, line, true)
    }

    // `minted` false: for a pure direct slice parameter (`Gen::pure_direct`),
    // the borrow's checks only, and a bare `{ data, len }`: no path, no
    // tracked slot, no temporary object.
    fn lower_slice_of_as(&mut self, mode: &Mode, base: &Expr, lo: &Expr, hi: &Expr, line: usize, minted: bool) -> R<V> {
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
        if !minted {
            match (&parent, &pb.base) {
                (Some(t0), _) => self.emit(&format!("cb_borrow_range_unminted({}, NULL, 0, (uint64_t){}, (uint64_t){}, {}, {});", t0, at.c, bt.c, m, loc)),
                (None, Some(base)) => self.emit(&format!("cb_borrow_range_unminted({}, {}, (uint64_t){}, (uint64_t){}, {}, {});", base, Self::projs_args(&pb.projs), at.c, bt.c, m, loc)),
                (None, None) => return internal("slice of an unchecked place"),
            }
            let sty = Type::Slice(Box::new(elem), mode.clone());
            let c = self.g.ctype(&sty)?;
            let t = self.g.fresh("sl");
            self.emit(&format!("{} {};", c, t));
            self.emit(&format!("{}.src = (void *)&({});", t, pb.c));
            self.emit(&format!("{}.data = {} + {};", t, data, at.c));
            self.emit(&format!("{}.len = (uint64_t){} - (uint64_t){};", t, bt.c, at.c));
            return Ok(V { c: t, ty: sty, obj: None });
        }
        // D-0070: the slice borrows its range (made absolute by the runtime
        // when the source is itself a slice), not the whole source.
        let tok = self.g.fresh("tok");
        match (&parent, &pb.base) {
            (Some(t0), _) => self.emit(&format!("uint64_t {} = cb_borrow_range({}, NULL, 0, (uint64_t){}, (uint64_t){}, {}, {});", tok, t0, at.c, bt.c, m, loc)),
            (None, Some(base)) => self.emit(&format!("uint64_t {} = cb_borrow_range({}, {}, (uint64_t){}, (uint64_t){}, {}, {});", tok, base, Self::projs_args(&pb.projs), at.c, bt.c, m, loc)),
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
        let objectless = std::mem::take(&mut self.objectless);
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
            // An inert field owns nothing: a fault in a later field leaves
            // nothing of it to destroy (`inert_expr`).
            self.objectless = self.inert_expr(fe)?;
            let v = self.lower_expr(fe, exp.as_ref());
            self.objectless = false;
            let v = v?;
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
        let ty = Type::Named(name.clone(), args);
        let c = self.g.ctype(&ty)?;
        let t = self.g.fresh("st");
        self.emit(&format!("{} {};", c, t));
        if let Some(backing) = sd.bits {
            // D-0118: the fields packed, each checked against its width.
            let loc = self.loc(line);
            let bc = c_int_type(backing);
            self.emit(&format!("{}.bits = 0;", t));
            for (f, v) in vals {
                let (w, off, _, _) = bitfield_layout(&sd, &f).expect("bit field");
                let mask = c_bits_mask(backing, w);
                self.emit(&format!("if (({})({}) > {}) cb_fault(\"diag.narrowing-overflow\", {});", bc, v.c, mask, loc));
                self.emit(&format!("{}.bits |= (({})({})) << {}u;", t, bc, v.c, off));
            }
            return self.register_temp(t, ty, line);
        }
        for (f, v) in vals {
            let fty = v.ty.clone();
            self.store(&format!("{}.{}", t, san(&f)), &fty, &v)?;
            if self.is_res(&fty) && !self.inert_c.contains(&v.c) {
                self.emit(&format!("cb_absorb({});", v.obj.as_ref().expect("resource field has object")));
            }
        }
        if objectless && self.is_res(&ty) && !self.has_refs(&ty) {
            self.inert_c.insert(t.clone());
            return Ok(V { c: t, ty, obj: None });
        }
        self.register_temp(t, ty, line)
    }

    fn lower_unary(&mut self, op: UnOp, inner: &Expr, expected: Option<&Type>, line: usize) -> R<V> {
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                if suf.is_none() && matches!(expected, Some(Type::F32 | Type::F64)) && self.g.items.number_literals.lock().unwrap().contains(&(inner as *const Expr as usize)) {
                    let ty = expected.cloned().unwrap();
                    return Ok(V { c: c_float(-(*v as f64), &ty), ty, obj: None });
                }
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
            // The right operand is skipped by `goto`, not wrapped in a C
            // block: a temporary it creates lives to the end of the whole
            // statement (D-0084), so its C storage must too — a block
            // ended it early, and `cb_stmt_pop`'s destruction read a dead
            // stack slot (stack-use-after-scope, found by the sanitizer
            // sweep on conf.hashmap-clear). Skipped declarations are
            // never read: every temporary is written before use, on the
            // one path that reaches it.
            let res = self.g.fresh("t");
            let skip = self.g.fresh("sc");
            self.emit(&format!("uint8_t {} = {};", res, lv.c));
            self.emit(&format!("if ({}{}) goto {};", if op == BinOp::And { "!" } else { "" }, res, skip));
            let rv = self.lower_expr(r, Some(&Type::Bool))?;
            if !matches!(rv.ty, Type::Never) {
                self.emit(&format!("{} = {};", res, rv.c));
            }
            self.emit(&format!("{}:;", skip));
            return Ok(V { c: res, ty: Type::Bool, obj: None });
        }
        let arith = matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor);
        let operand_expected = if arith { expected } else { None };
        // `[Read-Resource-Rejected]`: an operand is a value read; decided
        // before either operand runs, so nothing is moved first.
        // An operator's or a literal's result is never a resource (the
        // checker rejects one), and probing it would lower a long operator
        // chain once per level (a 40 000-term sum was quadratic).
        let plain = matches!(strip_parens(l).kind, ExprKind::Binary(..) | ExprKind::Unary(..) | ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_));
        let lt = if plain { Type::Void } else { self.probe_type(l, operand_expected)? };
        if self.is_res(&lt) {
            let loc = self.loc(line);
            self.emit(&format!("cb_fault(\"diag.read-of-resource\", {});", loc));
            return Ok(never());
        }
        // A literal on the left takes the right operand's type, so the right
        // goes first: a number, or for `==`/`!=` an enum literal whose
        // payload is a literal (`Some(5000000000) == x`). Neither has
        // effects, so the order cannot be observed.
        // A literal expression takes it too (D-0037: `((1 << 40) - 1) & k`).
        // Not for a shift in a context that gives a type: its right side is
        // the amount, and the literal on its left takes the context's type
        // (`x | (1 << n)` with `x : u64` shifts a `u64`, D-0037).
        let left_takes = (coby::ast::is_literal_expr(l) && !(matches!(op, BinOp::Shl | BinOp::Shr) && operand_expected.is_some())) || (matches!(op, BinOp::Eq | BinOp::Ne) && self.is_pure_variant_literal(l));
        let (lv, rv) = if left_takes && !coby::ast::is_literal_expr(r) && !self.is_pure_variant_literal(r) {
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
            // D-0108: `false < true`.
            Type::Bool => match op {
                BinOp::Eq => bool_of(format!("{} == {}", a, b)),
                BinOp::Ne => bool_of(format!("{} != {}", a, b)),
                BinOp::Lt => bool_of(format!("{} < {}", a, b)),
                BinOp::Le => bool_of(format!("{} <= {}", a, b)),
                BinOp::Gt => bool_of(format!("{} > {}", a, b)),
                BinOp::Ge => bool_of(format!("{} >= {}", a, b)),
                _ => return internal("operator on bool"),
            },
            // D-0108: `str` in byte order.
            Type::Str => match op {
                BinOp::Eq => bool_of(format!("cb_str_eq({}, {})", a, b)),
                BinOp::Ne => bool_of(format!("!cb_str_eq({}, {})", a, b)),
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    let sym = match op { BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                    bool_of(format!("cb_bytes_cmp(({a}).p, ({a}).n, ({b}).p, ({b}).n) {sym} 0"))
                }
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
            // `[Eq-Unit]`: `() == ()` is true; both operands still run.
            Type::Void => match op {
                BinOp::Eq => bool_of(format!("((void)({}), (void)({}), 1)", a, b)),
                BinOp::Ne => bool_of(format!("((void)({}), (void)({}), 0)", a, b)),
                _ => return internal("operator on unit"),
            },
            _ => return unsupported("binary operator on an aggregate"),
        })
    }

    // An expression used as a branch: a Block goes through lower_block
    // with the sink; anything else is lowered and stored.
    // A block as a branch: its own C block, frame and scope. An `if`'s
    // then-branch is lowered here in place: the checker's side tables
    // (view operations, closure types, temporary borrows) are keyed by the
    // address of each expression, so a copy of the block would miss them.
    fn lower_branch_block(&mut self, b: &Block, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        let Some((dst, dty, ov)) = sink else { return self.lower_block(b, expected) };
        let frame_line = self.frame_line.take();
        self.emit("{");
        self.ind += 1;
        self.push_frame_at(frame_line);
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

    fn lower_branch(&mut self, e: &Expr, expected: Option<&Type>, sink: Option<(&str, &Type, Option<&str>)>) -> R<V> {
        let own_scope = std::mem::take(&mut self.branch_scope);
        match (&e.kind, sink) {
            (ExprKind::Block(b), Some(sink)) | (ExprKind::Unsafe(b), Some(sink)) => {
                self.frame_line = Some(e.line);
                self.lower_branch_block(b, expected, Some(sink))
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
            // An arm of a `match` that is a whole statement (`stmt_match`):
            // its own scope, which `resolve_scopes` drops if it is unused.
            (_, None) if own_scope => {
                self.push_stmt_at(Some(e.line));
                let v = self.lower_expr(e, expected)?;
                if matches!(v.ty, Type::Never) {
                    self.end_diverged();
                } else {
                    self.pop_stmt();
                }
                Ok(v)
            }
            (_, None) => self.lower_expr(e, expected),
        }
    }

    // The type a branch expression would produce, found by lowering it
    // into a discarded buffer (bodies are small; emission is cheap).
    // D-0118: `base.f` with `base` a bitstruct (directly or through a
    // reference or guard): the field's width, bit offset, backing and type.
    fn bitfield_of(&mut self, base: &Expr, f: &str) -> R<Option<(u32, u32, IntTy, Type)>> {
        let bt = self.probe_type(base, None)?;
        let bt = match bt {
            Type::Ref(t, _) | Type::Guard(t) => *t,
            t => t,
        };
        let Type::Named(n, _) = &bt else { return Ok(None) };
        let Some(sd) = self.g.struct_decl(n) else { return Ok(None) };
        if sd.bits.is_none() {
            return Ok(None);
        }
        Ok(bitfield_layout(&sd, f))
    }

    fn probe_type(&mut self, e: &Expr, expected: Option<&Type>) -> R<Type> {
        let key = (e as *const Expr as usize, format!("{:?}", expected));
        if let Some(t) = self.probes.get(&key) {
            return Ok(t.clone());
        }
        let t = self.probe_with(|fx| fx.lower_expr(e, expected))?;
        self.probes.insert(key, t.clone());
        Ok(t)
    }

    // `probe_type` of a block (an `if`'s then-branch), cached apart from
    // expressions' probes, whose keys are expression addresses.
    fn probe_block_type(&mut self, b: &Block, expected: Option<&Type>) -> R<Type> {
        let key = (b as *const Block as usize, format!("block {:?}", expected));
        if let Some(t) = self.probes.get(&key) {
            return Ok(t.clone());
        }
        let t = self.probe_with(|fx| fx.lower_block(b, expected))?;
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

    fn probe_with(&mut self, lower: impl FnOnce(&mut Self) -> R<V>) -> R<Type> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_uid = self.g.uid;
        let saved_nstr = self.g.nstr;
        let saved_strings = self.g.strings.clone();
        let saved_cleanup = self.cleanup.len();
        let saved_loops = self.loops.clone();
        let saved_scopes = self.scopes.len();
        self.g.probing += 1;
        let v = lower(self);
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
        // D-0049: a literal then-branch takes the else branch's type.
        let else_hint = match else_e {
            Some(e) if expected.is_none() && coby::ast::is_literal_block(then_b) && !coby::ast::is_literal_branch(e) => {
                Some(self.probe_type(e, None)?).filter(|t| !matches!(t, Type::Never | Type::Void))
            }
            _ => None,
        };
        let expected = expected.or(else_hint.as_ref());
        let then_ty = self.probe_block_type(then_b, expected)?;
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
        self.frame_line = Some(line);
        let tv = self.lower_branch_block(then_b, expected, sink.as_deref().map(|s| (s, &result_ty, ov.as_deref())))?;
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
        let consume_temp = std::mem::replace(&mut self.propagate_scrutinee, false);
        let arm_scopes = std::mem::take(&mut self.stmt_match);
        // D-0058: a first arm that matches everything, on a scrutinee that
        // is not an enum (nor an integer or `bool`): no discriminant; the
        // arm binds the whole value, or (`_`) a place is left alone.
        if arms.first().map_or(false, |a| a.variant.is_none() && a.lit.is_none()) {
            let st = if matches!(strip_parens(scrut).kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_)) {
                self.probe_place_type(scrut)?
            } else {
                self.probe_type(scrut, None)?
            };
            let flat = match &st {
                Type::Named(n, _) => !self.g.items.enums.contains_key(n),
                Type::Ref(inner, _) => !matches!(&**inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)),
                Type::Int(_) | Type::Bool => false,
                _ => true,
            };
            if flat {
                let is_place = matches!(strip_parens(scrut).kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_));
                if arms[0].binder.is_none() && is_place {
                    let v = unit();
                    return self.lower_match_scalar(v, &arms[..1], expected, line, None, arm_scopes);
                }
                let sv = self.lower_expr(scrut, None)?;
                if matches!(sv.ty, Type::Never) {
                    return Ok(never());
                }
                return self.lower_match_scalar(sv, &arms[..1], expected, line, None, arm_scopes);
            }
        }
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
                // A borrow capture's place is the captured value.
                let t = match (&b.kind, &b.ty) {
                    (BindKind::CaptureRef(_), Type::Ref(t, _)) => (**t).clone(),
                    (_, t) => t.clone(),
                };
                (matches!(&t, Type::Named(n, _) if self.g.items.enums.contains_key(n)) || is_text_ty(&t)) && self.is_res(&t)
            }),
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) => {
                let t = self.probe_place_type(scrut)?;
                (matches!(&t, Type::Named(n, _) if self.g.items.enums.contains_key(n)) || is_text_ty(&t)) && self.is_res(&t)
            }
            _ => false,
        };
        // `match (&mut p)` / `match (&p)` on an enum place with a checked
        // base: the borrow's check (`cb_borrow_unminted`) and the tag's read
        // through the base, with no path minted for the scrutinee; a
        // binder's path is borrowed from the base, through the place's
        // projection and the payload. The scrutinee's own path would be
        // unheld all its life, and no check counts an unheld path (D-0018),
        // as an ancestor or otherwise.
        let mut fused: Option<(Mode, String, Vec<String>)> = None;
        if let ExprKind::Borrow(bm, place) = &strip_parens(scrut).kind {
            let pt = self.probe_place_type(place)?;
            if matches!(&pt, Type::Named(n, _) if self.g.items.enums.contains_key(n)) {
                let p = self.lower_place(place)?;
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                if let (Some(base), None) = (p.base.clone(), &p.raw) {
                    let loc = self.loc(line);
                    let m = if *bm == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&p.projs), m, loc));
                    self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                    fused = Some((bm.clone(), base, p.projs.clone()));
                    // Lowered once: the place stands for the scrutinee below.
                    self.fused_scrut = Some(V { c: p.c, ty: p.ty, obj: None });
                } else if p.raw.is_none() && self.direct_place(place) {
                    // A field of a direct parameter (`box_rec_uses`): no
                    // check can fail (the parameter is the only way in), so
                    // none is made; the binder's base is "" (direct).
                    fused = Some((bm.clone(), String::new(), p.projs.clone()));
                    self.fused_scrut = Some(V { c: p.c, ty: p.ty, obj: None });
                } else {
                    return internal("a fused match scrutinee was lowered without a base");
                }
            }
        }
        let sv = if let Some(v) = self.fused_scrut.take() {
            v
        } else if res_place {
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
        let mut by_ref_prefix: Vec<String> = Vec::new();
        if let Some((m, base, projs)) = fused.take() {
            by_ref = Some((m, base));
            by_ref_prefix = projs;
        }
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
        if matches!(sv.ty, Type::Int(_) | Type::Bool) || is_text_ty(&sv.ty) {
            return self.lower_match_scalar(sv, arms, expected, line, place_root, arm_scopes);
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
            let whole_ty = Type::Named(ename.clone(), args.clone());
            let t = self.probe_type_arm(arm, &ed, &sub, &whole_ty, probe_exp.as_ref(), by_ref.as_ref().map(|(m, _)| m.clone()))?;
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
            let (vis, inner_ty, crosses) = self.arm_levels_crossing(&ed, &sub, arm)?;
            // D-0058: a binder alone binds the whole scrutinee.
            let inner_ty = if vis.is_empty() && arm.lit.is_none() && arm.binder.is_some() { Some(s.ty.clone()) } else { inner_ty };
            let mut slot_path = s.c.clone();
            let mut tests = Vec::new();
            // D-0109: the last reference the pattern looked through (its
            // slot), and the levels below it.
            let mut cross_slot: Option<String> = None;
            let mut below = vis.len();
            let loc = self.loc(line);
            let through = |me: &mut Self, k: usize, slot_path: &mut String, cross_slot: &mut Option<String>, below: &mut usize| -> R<()> {
                if let Some((_, _, pointee)) = crosses.iter().find(|c| c.0 == k) {
                    let ec = me.g.ctype(pointee)?;
                    let raw = slot_path.clone();
                    *slot_path = format!("(*({} *)({{ cb_read(cb_load_ref(&{}), NULL, 0, {}); {}; }}))", ec, raw, loc, raw);
                    *cross_slot = Some(raw);
                    *below = vis.len() - k;
                }
                Ok(())
            };
            for (k, v) in vis.iter().enumerate() {
                through(self, k, &mut slot_path, &mut cross_slot, &mut below)?;
                tests.push(format!("{}.tag == {}u", slot_path, v));
                slot_path = format!("{}.u.v{}", slot_path, v);
            }
            if arm.lit.is_some() {
                through(self, vis.len(), &mut slot_path, &mut cross_slot, &mut below)?;
            }
            // D-0057: a literal the innermost payload must equal; D-0127: a
            // text literal, its bytes.
            if let (Some(l), Some(lt)) = (&arm.lit, inner_ty.as_ref()) {
                if is_text_ty(lt) {
                    let lv = self.lower_expr(l, Some(&Type::Str))?;
                    let (p, n) = text_parts(&slot_path, lt);
                    tests.push(format!("cb_bytes_eq({}, {}, (const void *)({}).p, ({}).n)", p, n, lv.c, lv.c));
                } else {
                    let lv = self.lower_expr(l, Some(lt))?;
                    tests.push(format!("{} == {}", slot_path, lv.c));
                }
            }
            let head = if tests.is_empty() { "{".to_string() } else { format!("if ({}) {{", tests.join(" && ")) };
            self.emit(&format!("{}{}", if i > 0 { "else " } else { "" }, head));
            self.ind += 1;
            self.push_frame();
            self.scopes.push(HashMap::new());
            let mut moved_out = false;
            // D-0109: through a reference the pattern crossed, the binder
            // borrows from that reference (in the weaker mode).
            let ref_bind: Option<(Mode, String, usize)> = match (&cross_slot, &by_ref, crossing_mode(&crosses)) {
                (Some(cs), b, Some(cm)) => Some((if matches!(b, Some((Mode::Shared, _))) { Mode::Shared } else { cm }, self.slot_tok(cs), below)),
                (_, Some((m, tok)), _) => Some((m.clone(), tok.clone(), vis.len())),
                _ => None,
            };
            if let (Some(_), Some(binder), Some((mode, tok, nproj))) = (inner_ty.as_ref(), &arm.binder, &ref_bind) {
                if let Some(pty) = inner_ty.clone() {
                    let rty = Type::Ref(Box::new(pty), mode.clone());
                    let rc = self.g.ctype(&rty)?;
                    let cname = format!("l_{}_{}", san(binder), self.g.fresh(""));
                    self.emit(&format!("{} {};", rc, cname));
                    let loc = self.loc(line);
                    let t = self.g.fresh("tok");
                    let m = if *mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    // Through a fused scrutinee, the place's projection first.
                    let prefix: &[String] = if cross_slot.is_none() { &by_ref_prefix } else { &[] };
                    let mut all: Vec<String> = prefix.to_vec();
                    all.extend(std::iter::repeat("{ CB_PAYLOAD, 0u }".to_string()).take(*nproj));
                    let projs = if all.is_empty() { "NULL".to_string() } else { format!("(cb_proj[]){{ {} }}", all.join(", ")) };
                    // `Some(b) : … Box::get_mut(b) …`, `b` used only there,
                    // first: `b`'s path would serve only that call's read of
                    // the box's pointer. The borrow is checked
                    // (`cb_borrow_unminted`) and none minted; the call is
                    // made inline through the base (`unminted_binders`).
                    let box_use = binder_used_once_first(&arm.body, binder) && binder_box_call(&arm.body, binder).map_or(false, |g| g == "std::Box::get" || (g == "std::Box::get_mut" && *mode == Mode::Exclusive));
                    if !box_use && tok.is_empty() {
                        return internal("a binder through a direct place used other than by `Box::get`");
                    }
                    if box_use {
                        if !tok.is_empty() {
                            self.emit(&format!("cb_borrow_unminted({}, {}, {}, {}, {});", tok, projs, all.len(), m, loc));
                        }
                        self.emit(&format!("{} = (void *)&({});", cname, slot_path));
                        self.unminted_binders.insert(cname.clone(), (tok.clone(), all.clone()));
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: rty.clone(), root: String::new(), kind: BindKind::Local });
                        // (`t` unused.)
                        let _ = &t;
                    } else {
                    self.emit(&format!("uint64_t {} = cb_borrow({}, {}, {}, {}, {});", t, tok, projs, all.len(), m, loc));
                    if binder_used_once_first(&arm.body, binder) {
                        // Used once, by the first thing the arm evaluates:
                        // nothing runs between the binding and its use, so
                        // its path need not be held by a binding object
                        // (D-0018: no check counts an unheld path); the
                        // use goes through the borrow's token (`unstored`).
                        self.emit(&format!("{} = (void *)&({});", cname, slot_path));
                        self.unstored.insert(cname.clone(), t.clone());
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: rty.clone(), root: String::new(), kind: BindKind::Local });
                    } else {
                        let fv = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", slot_path, t), ty: rty.clone(), obj: None };
                        self.bind_value(binder, cname, &rty, &fv, line)?;
                    }
                    }
                }
            } else if let (Some(_), Some(binder)) = (inner_ty.as_ref(), &arm.binder) {
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
                    } else if matches!(pty, Type::Ref(..)) && binder_used_once_first(&arm.body, binder) {
                        // The reference stays in the scrutinee's slot, held
                        // there until the statement ends; a binder used once,
                        // by the first thing the arm evaluates, needs no
                        // binding of its own (as `ref_bind`'s, D-0018).
                        let tk = self.g.fresh("tk");
                        self.emit(&format!("uint64_t {} = cb_load_ref(&{});", tk, slot_path));
                        self.emit(&format!("{} = {};", cname, slot_path));
                        self.unstored.insert(cname.clone(), tk);
                        self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: cname, ty: pty, root: String::new(), kind: BindKind::Local });
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
            self.branch_scope = arm_scopes && sink.is_none();
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
        // D-0060: `?` consumes a temporary `Result`/`Option` that owns
        // nothing: it ends here, not with the statement, so a reference it
        // held no longer counts as a path.
        if consume_temp && by_ref.is_none() && !res_place && !scrut_res {
            if let Some(o) = &s.obj {
                self.emit(&format!("cb_end_moved_out({});", o));
            }
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

    // A `foreach` over `&v`, `v` a `Vec` of plain, reference-free
    // elements, whose element name `x` the body uses only as `*x` read as
    // a value: `x`'s `let` is not lowered, and each `*x` is the checked
    // read `(*$c)[$i]`. Unobservable: the loop's `$c` already holds `v`
    // shared, so no write to an element can happen while `x` would have
    // lived; `x`'s own shared path, and the element object it would have
    // established, are seen by no check (the argument of
    // `cb_elem_access`, D-0023). One element object and path less per
    // step.
    // `x.f…` read as a value, `x` a `foreach` element aliased to
    // `(*$c)[$i]` (`foreach_alias`): the same fields of that element
    // (`(*x).f…`, the element being plain data). Kept alive with the body,
    // as `probe_type`'s cache needs.
    fn alias_field_chain(&mut self, e: &Expr) -> Option<std::rc::Rc<Expr>> {
        fn rebuild(e: &Expr, alias: &HashMap<String, std::rc::Rc<Expr>>) -> Option<Expr> {
            match &e.kind {
                ExprKind::Field(a, f) => Some(Expr { kind: ExprKind::Field(Box::new(rebuild(a, alias)?), f.clone()), line: e.line }),
                ExprKind::Paren(a) => rebuild(a, alias),
                ExprKind::Path(segs, _) if segs.len() == 1 => alias.get(&segs[0]).map(|a| (**a).clone()),
                // `(*x).f…`: `*x` is the element itself.
                ExprKind::Deref(a) => match &strip_parens(a).kind {
                    ExprKind::Path(segs, _) if segs.len() == 1 => alias.get(&segs[0]).map(|a| (**a).clone()),
                    _ => None,
                },
                _ => None,
            }
        }
        if self.elem_alias.is_empty() || !matches!(e.kind, ExprKind::Field(..)) {
            return None;
        }
        let r = std::rc::Rc::new(rebuild(e, &self.elem_alias)?);
        self.alias_kept.push(r.clone());
        Some(r)
    }

    // `p` or `$each_drain(p, n)`, `p` a confined parameter or a holder
    // standing for one: its `confined_params` entry.
    fn confined_holder(&self, e: &Expr) -> Option<(String, String, Type)> {
        let name = |e: &Expr| match &e.kind {
            ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() => Some(segs[0].clone()),
            _ => None,
        };
        let n = match &e.kind {
            ExprKind::Call(c, a) if name(c).as_deref() == Some("$each_drain") => name(a.first()?)?,
            _ => name(e)?,
        };
        // A confined parameter has no binding; a holder for one has the
        // direct binding `lower_stmt` gave it. Any other binding of the
        // name is not one.
        let entry = self.confined_params.get(&n)?;
        match self.lookup(&n) {
            Some(b) if !self.direct_refs.contains(&b.c) => None,
            _ => Some(entry.clone()),
        }
    }

    fn foreach_alias(&mut self, s: &Stmt, next: Option<&Stmt>) -> R<Option<(String, std::rc::Rc<Expr>)>> {
        let Stmt::Let { ty: None, name, init: Some(init), .. } = s else { return Ok(None) };
        let ExprKind::Call(callee, args) = &init.kind else { return Ok(None) };
        // `$each_at` over `&c`; `$each_take` over a shared reference or slice
        // written without `&` (which `each.rs` loops over as that borrow).
        let mutable = matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "$each_at_mut");
        if !(mutable || matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && (p[0] == "$each_at" || p[0] == "$each_take"))) || args.len() != 3 {
            return Ok(None);
        }
        let ExprKind::Path(hsegs, _) = &args[0].kind else { return Ok(None) };
        if hsegs.len() != 1 {
            return Ok(None);
        }
        let Some(Stmt::BlockLike(bx)) = next else { return Ok(None) };
        let ExprKind::Block(body) = &bx.kind else { return Ok(None) };
        // The element type, and whether the holder is a slice (indexed as
        // it is) rather than a reference to a `Vec` or an array (indexed
        // through `*`).
        // A holder standing for a confined parameter (`confined_holder`):
        // a shared `Vec`, its elements reached as the parameter's are.
        let (elem, slice) = match self.lookup(&hsegs[0]).map(|b| b.ty) {
            // `&mut v` (`$each_at_mut`): a `Vec` only, its elements read
            // and assigned whole through `*x` (`block_only_derefs`).
            Some(Type::Ref(inner, Mode::Exclusive)) if mutable => match *inner {
                Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => (a[0].clone(), false),
                _ => return Ok(None),
            },
            Some(Type::Ref(inner, Mode::Shared)) if !mutable => match *inner {
                Type::Named(n, a) if n == "std::Vec" && a.len() == 1 => (a[0].clone(), false),
                Type::Array(t, _) => ((*t).clone(), false),
                _ => return Ok(None),
            },
            Some(Type::Slice(t, Mode::Shared)) => ((*t).clone(), true),
            _ => return Ok(None),
        };
        let (size, _) = self.g.size_align(&elem)?;
        // A shared loop over elements that own something but hold no
        // reference (`String`s), the body handing `x` only to reader
        // parameters (`Gen::reader_param`) or reading plain fields: `x` is
        // the element with token 0 (`lower_expr`'s alias case). The holder
        // `$c` holds the `Vec` shared from the first step to the last, so
        // no exclusive path to an element can exist meanwhile and every
        // shared read of one is proven; a reader keeps nothing of it.
        let reader = !mutable && !slice && size > 0 && !self.g.plain_data(&elem) && !self.has_refs(&elem) && !self.holds_fn(&elem) && {
            let bound = bound_names(body);
            let g = &*self.g;
            reader_uses(body, name, false, &|segs: &[String], at: usize| {
                if segs.len() == 1 && bound.contains(&segs[0]) {
                    return false;
                }
                let k = segs.join("::");
                g.items.fns.get(&k).and_then(|f| f.params.get(at)).map_or(false, |q| matches!(q.ty, Type::Ref(_, Mode::Shared))) && g.reader_param(&k, at)
            })
        };
        if size == 0 || !(reader || (self.g.plain_data(&elem) && block_only_derefs(body, name, mutable))) {
            return Ok(None);
        }
        let line = init.line;
        // The loop's holder `$c` of a `Vec` read this way is confined for
        // the loop (`confined_params`): it holds the `Vec` shared from
        // before the first step to after the last, so no path can write an
        // element, change the length or end the `Vec` meanwhile -- a write
        // through any other path clashes with `$c` and faults there, never
        // here. Each step's `(*$c)[$i]` and the condition's `$each_len($c)`
        // then need only the bounds check (`confined_local`): no access
        // check per element. `$c`'s own borrow is formed as before.
        if !slice {
            if let Some(Binding { c: hc, ty: Type::Ref(inner, _), .. }) = self.lookup(&hsegs[0]) {
                if matches!(&*inner, Type::Named(n, _) if n == "std::Vec") {
                    let vec_c = self.g.ctype(&inner)?;
                    self.confined_params.insert(hsegs[0].clone(), (format!("(*({} *){})", vec_c, hc), "0".to_string(), elem.clone()));
                }
            }
        }
        let base = if slice { args[0].clone() } else { Expr { kind: ExprKind::Deref(Box::new(args[0].clone())), line } };
        let alias = Expr { kind: ExprKind::Index(Box::new(base), Box::new(args[1].clone())), line };
        Ok(Some((name.clone(), std::rc::Rc::new(alias))))
    }

    // D-0057: `match` on an integer or `bool`: an `if`/`else if` chain of
    // equality tests against the arms' literals, in order; `_` is `else`.
    // `place_root`: the binding a `String` place scrutinee is (D-0127),
    // for a binder arm to move it out of.
    fn lower_match_scalar(&mut self, sv: V, arms: &[Arm], expected: Option<&Type>, line: usize, place_root: Option<String>, arm_scopes: bool) -> R<V> {
        // D-0127: a `String` place is compared where it is (a bitwise
        // copy of a resource would need an object to move).
        let in_place = is_text_ty(&sv.ty) && self.is_res(&sv.ty) && sv.obj.is_none();
        let s = if matches!(sv.ty, Type::Void) || in_place { sv } else { self.into_temp(sv, line)? };
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
            if let (Some(b), None) = (&arm.binder, &arm.lit) {
                self.scopes.last_mut().unwrap().insert(b.clone(), Binding { c: "probe".into(), ty: s.ty.clone(), root: "0".into(), kind: BindKind::Local });
            }
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
                // D-0127: a text literal, equal bytes.
                Some(l) if is_text_ty(&s.ty) => {
                    let lv = self.lower_expr(l, Some(&Type::Str))?;
                    let (p, n) = text_parts(&s.c, &s.ty);
                    format!("if (cb_bytes_eq({}, {}, (const void *)({}).p, ({}).n)) {{", p, n, lv.c, lv.c)
                }
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
            if let (Some(b), None) = (&arm.binder, &arm.lit) {
                // D-0058: the whole value (moved, for a resource).
                let bty = s.ty.clone();
                let cname = format!("l_{}_{}", san(b), self.g.fresh(""));
                let c = self.g.ctype(&bty)?;
                self.emit(&format!("{} {};", c, cname));
                if in_place {
                    // D-0127: a `String` binding matched against text
                    // literals, moved out whole into the binder, as the
                    // enum path moves a whole resource scrutinee.
                    let Some(root) = place_root.clone() else { return internal("a `String` scrutinee moved out of a place that is not a binding") };
                    let loc = self.loc(line);
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_take({}, {});", o, root, loc));
                    self.store(&cname, &bty, &s)?;
                    self.emit(&format!("cb_end_moved_out({});", o));
                    let tid = self.tid(&bty)?;
                    let o = self.g.fresh("o");
                    self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, cname, tid, loc));
                    let root = self.g.fresh("root");
                    self.emit(&format!("uint64_t {} = cb_bind({});", root, o));
                    self.note_frame(b, &root);
                    self.scopes.last_mut().unwrap().insert(b.clone(), Binding { c: cname, ty: bty, root, kind: BindKind::Local });
                } else {
                    self.bind_value(b, cname, &bty, &s.clone(), line)?;
                }
            }
            self.branch_scope = arm_scopes && sink.is_none();
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
    // `_`), and the type of the innermost payload it reaches; and where
    // the pattern looks through a reference (D-0109): each crossing's
    // level (`vis.len()` for a literal's), mode and referent type. The
    // payload is then the referent's.
    fn arm_levels_crossing(&self, ed: &EnumDecl, sub: &HashMap<String, Type>, arm: &Arm) -> R<(Vec<usize>, Option<Type>, Vec<(usize, Mode, Type)>)> {
        let mut vis = Vec::new();
        let mut crosses = Vec::new();
        let mut decl = ed.clone();
        let mut dsub = sub.clone();
        let mut payload: Option<Type> = None;
        for (k, vn) in arm.chain().iter().enumerate() {
            if k > 0 {
                if let Some(Type::Ref(inner, m)) = payload.clone() {
                    if matches!(&*inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)) {
                        crosses.push((k, m, (*inner).clone()));
                        payload = Some(*inner);
                    }
                }
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
        if arm.lit.is_some() && !vis.is_empty() {
            if let Some(Type::Ref(inner, m)) = payload.clone() {
                // D-0127: text through a reference too.
                if matches!(&*inner, Type::Int(_) | Type::Bool) || is_text_ty(&inner) {
                    crosses.push((vis.len(), m, (*inner).clone()));
                    payload = Some(*inner);
                }
            }
        }
        Ok((vis, payload, crosses))
    }

    fn probe_type_arm(&mut self, arm: &Arm, ed: &EnumDecl, sub: &HashMap<String, Type>, whole_ty: &Type, expected: Option<&Type>, by_ref: Option<Mode>) -> R<Type> {
        self.scopes.push(HashMap::new());
        if let Some(binder) = &arm.binder {
            let whole = if arm.variant.is_none() && arm.lit.is_none() { Some(whole_ty.clone()) } else { None };
            if let Some(w) = whole {
                let pty = match &by_ref {
                    Some(m) => Type::Ref(Box::new(w), m.clone()),
                    None => w,
                };
                self.scopes.last_mut().unwrap().insert(binder.clone(), Binding { c: "probe".into(), ty: pty, root: "0".into(), kind: BindKind::Local });
            } else if let (_, Some(p), crosses) = self.arm_levels_crossing(ed, sub, arm)? {
                let pty = match (by_ref.clone(), crossing_mode(&crosses)) {
                    (Some(m), None) | (None, Some(m)) => Type::Ref(Box::new(p), m),
                    (Some(a), Some(b)) => Type::Ref(Box::new(p), if a == Mode::Shared { Mode::Shared } else { b }),
                    (None, None) => p,
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
                // A place holding a `fn` value (`(t.run)()`, `(*f)(x)`,
                // `(fs[i])(x)`): called where it is, as a binding holding
                // one is. Read as a value, a box whose closure owns a
                // resource would be a read of a resource.
                let bare = strip_parens(callee);
                if matches!(bare.kind, ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_)) {
                    if let Ok(Type::Fn(..)) = self.probe_place_type(bare) {
                        let p = self.lower_place(bare)?;
                        let loc = self.loc(line);
                        if let Some(base) = &p.base {
                            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
                        }
                        let h = self.g.fresh("h");
                        self.emit(&format!("void *{} = {};", h, p.c));
                        let fty = p.ty.clone();
                        return self.call_fn_value(h, &fty, args, line);
                    }
                }
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
            if self.lookup(&segs[0]).is_some() {
                // The place's type, not the binding's: a name a closure
                // captured by borrow is `*self.f_i`, whose place is the
                // `fn` value or closure the reference reaches.
                let p = self.lower_place(callee)?;
                return match &p.ty.clone() {
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
            if name == "std::stdout_write" || name == "std::stderr_write" {
                let p = self.lower_expr(&args[0], None)?;
                let pt = self.into_temp(p, line)?;
                let n = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let nt = self.into_temp(n, line)?;
                let f = if name == "std::stdout_write" { "cb_write_out" } else { "cb_write_err" };
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
            if name == "std::fs_op" || name == "std::fs_query" {
                // D-0061: (op, a, an, b-or-buf, bn-or-cap).
                let mut cs = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    let hint = if k == 1 || k == 3 { None } else { Some(Type::Int(IntTy::Usize)) };
                    let v = self.lower_expr(a, hint.as_ref())?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                let f = if name == "std::fs_op" { "cb_fs_op" } else { "cb_fs_query" };
                return Ok(V { c: format!("{}({}, (void *)({}), {}, (void *)({}), {})", f, cs[0], cs[1], cs[2], cs[3], cs[4]), ty: Type::Int(IntTy::Isize), obj: None });
            }
            if name == "std::event_op" {
                // D-0063: (op, id, seen), a count or an id back.
                let mut cs = Vec::new();
                for a in args {
                    let v = self.lower_expr(a, Some(&Type::Int(IntTy::Usize)))?;
                    cs.push(self.into_temp(v, line)?.c);
                }
                return Ok(V { c: format!("cb_event_op({}, {}, {})", cs[0], cs[1], cs[2]), ty: Type::Int(IntTy::Usize), obj: None });
            }
            if name == "std::clock_read" {
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::Usize)))?;
                let t = self.into_temp(v, line)?;
                return Ok(V { c: format!("cb_clock_read({})", t.c), ty: Type::Int(IntTy::I64), obj: None });
            }
            if name == "std::sleep_ns" {
                let v = self.lower_expr(&args[0], Some(&Type::Int(IntTy::U64)))?;
                let t = self.into_temp(v, line)?;
                return Ok(V { c: format!("cb_sleep_ns({})", t.c), ty: Type::Int(IntTy::I64), obj: None });
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
            if name == "std::stdin_read" {
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
                // D-0135: a temporary `String` (or part of one), as an
                // argument, is an object of this statement (D-0103's root).
                self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(e as *const Expr as usize));
                let p = self.lower_place(base);
                self.temp_root = false;
                let p = p?;
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
            (ViewOp::Less { swap, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.lower_expr(a, None)?;
                let va = self.into_temp(va, line)?;
                let vb = self.lower_expr(b, None)?;
                let vb = self.into_temp(vb, line)?;
                let (x, y) = if swap { (vb, va) } else { (va, vb) };
                let r = self.call_std_values("std::StringView::less", &[x, y], line)?;
                Ok(if negate { V { c: format!("((uint8_t)!({}))", r.c), ty: Type::Bool, obj: None } } else { r })
            }
            (ViewOp::TextCmp, ExprKind::Binary(op, a, b)) => {
                // Another instantiation of a generic body: not text.
                let lt = if is_place_syntax(a) { self.probe_place_type(a)? } else { self.probe_type(a, None)? };
                if !matches!(&lt, Type::Named(n, _) if n == "std::String") {
                    return self.lower_binary(*op, a, b, None, line);
                }
                let x = self.lower_text_operand(a, line)?;
                let y = self.lower_text_operand(b, line)?;
                let sym = match op { BinOp::Eq => "==", BinOp::Ne => "!=", BinOp::Lt => "<", BinOp::Le => "<=", BinOp::Gt => ">", _ => ">=" };
                Ok(V { c: format!("((uint8_t)(cb_bytes_cmp({}, {}) {} 0))", x, y, sym), ty: Type::Bool, obj: None })
            }
            _ => internal("a StringView operation of an unexpected shape"),
        }
    }

    // D-0108 `[Cmp-Text]`: a `String` comparison operand as the C text
    // `ptr, len` of its bytes. A place is borrowed shared, as `&e` would
    // be; a temporary lives to the statement's end.
    fn lower_text_operand(&mut self, e: &Expr, line: usize) -> R<String> {
        let c = if is_place_syntax(e) {
            let loc = self.loc(line);
            let p = self.lower_place(e)?;
            let p = self.auto_deref(p)?;
            let Some(b) = p.base.clone() else { return internal("a compared String with no access path") };
            let tok = self.g.fresh("tok");
            self.emit(&format!("uint64_t {} = cb_borrow({}, {}, CB_SHARED, {});", tok, b, Self::projs_args(&p.projs), loc));
            let r = V { c: format!("((cb_ref){{ (void *)&({}), {} }})", p.c, tok), ty: Type::Ref(Box::new(p.ty.clone()), Mode::Shared), obj: None };
            let r = self.into_temp(r, line)?;
            format!("(*(({} *)({}).p))", self.g.ctype(&p.ty)?, r.c)
        } else {
            let v = self.lower_expr(e, None)?;
            self.into_temp(v, line)?.c
        };
        Ok(format!("(const void *)({}).bytes.ptr, ({}).bytes.len", c, c))
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
        if let Some(v) = self.lower_map_fresh_key(name, f, args, line)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_push_fresh(name, f, args, line)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_box_new(name, f, targs, args, expected, line)? {
            return Ok(v);
        }
        // `Box::get(b)` / `Box::get_mut(b)` on an unminted arm binder: the
        // native body (`native_vec_index`'s Box case) inline, its read of
        // the box's pointer through the binder's base and projection.
        if (name == "std::Box::get" || name == "std::Box::get_mut") && in_prelude(f) && args.len() == 1 {
            if let Some(bind) = path_name(&args[0]).and_then(|n| self.lookup(n)) {
                if let Some((base, projs)) = self.unminted_binders.get(&bind.c).cloned() {
                    let Type::Ref(bt, _) = &bind.ty else { return internal("a box binder") };
                    let Type::Named(_, ba) = &**bt else { return internal("a box binder's type") };
                    let [elem] = ba.as_slice() else { return internal("a box's type") };
                    let bc = self.g.ctype(bt)?;
                    let tid = self.tid(elem)?;
                    let mode = if name == "std::Box::get" { Mode::Shared } else { Mode::Exclusive };
                    let md = if mode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                    let mut all = projs.clone();
                    all.push("{ CB_FIELD, 0u }".to_string());
                    let a = self.g.fresh("a");
                    let r = self.g.fresh("r");
                    if !base.is_empty() {
                        self.emit(&format!("cb_read({}, (cb_proj[]){{ {} }}, {}, CB_NOLOC, 0);", base, all.join(", "), all.len()));
                    }
                    self.emit(&format!("void *{} = (void *)(({} *){})->ptr;", a, bc, bind.c));
                    self.emit(&format!("cb_ref {} = (cb_ref){{ {}, cb_elem_borrow({}, {}u, {}) }};", r, a, a, tid, md));
                    self.emit("cb_recv_ref();");
                    return Ok(V { c: r, ty: Type::Ref(Box::new(elem.clone()), mode), obj: None });
                }
            }
        }
        if let Some(v) = self.lower_confined_call(name, f, args)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_push_checked_once(name, f, args)? {
            return Ok(v);
        }
        if let Some(v) = self.lower_sort_by_literal(name, f, args)? {
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
        // D-0079: an unsuffixed literal whose parameter type is not yet
        // known is lowered after the other arguments (it has no effects),
        // at the type they and the expected type fixed.
        let mut deferred: Vec<usize> = Vec::new();
        let mut order: Vec<usize> = (0..f.params.len().min(args.len())).collect();
        let mut k = 0;
        while k < order.len() {
            let j = order[k];
            k += 1;
            let (p, a) = (&f.params[j], &args[j]);
            if !deferred.contains(&j) && is_bare_num_literal(a) && !resolved(&apply(&p.ty, &m), &f.type_params) && mask & (1 << j) == 0 {
                deferred.push(j);
                order.push(j);
                argv.push(V { c: String::new(), ty: Type::Void, obj: None });
                continue;
            }
            // A payload-less variant (`None`) fixes no type parameter
            // either: it waits for the other arguments, deferred literals
            // included (`Option::unwrap_or(None, 7)`), having no effects.
            // So does a generic fn item named as a value, which needs the
            // type its parameter gets (`twice(id, 10)`).
            let bare_variant = matches!(&a.kind, ExprKind::Path(segs, t) if t.is_empty() && self.lookup(&segs[0]).is_none()
                && (self.variant_of(segs).is_some() || self.g.items.fns.get(&segs.join("::")).is_some_and(|g| !g.type_params.is_empty())));
            if bare_variant && !resolved(&apply(&p.ty, &m), &f.type_params) && mask & (1 << j) == 0 && deferred.iter().filter(|&&d| d == j).count() < 2 {
                let first = !deferred.contains(&j);
                deferred.push(j);
                order.push(j);
                if first {
                    argv.push(V { c: String::new(), ty: Type::Void, obj: None });
                }
                continue;
            }
            if mask & (1 << j) != 0 {
                let (vec, tok, elem) = self.confined_local(a).expect("confined argument");
                let Type::Ref(_, mode) = &p.ty else { return internal("confining parameter of a non-reference type") };
                let ty = Type::Ref(Box::new(Type::Named("std::Vec".to_string(), vec![elem])), mode.clone());
                unify(&p.ty, &ty, &f.type_params, &mut m);
                put_arg(&mut argv, j, V { c: format!("((cb_ref){{ (void *)&({}), {} }})", vec, tok), ty, obj: None });
                continue;
            }
            let mut hint = apply(&p.ty, &m);
            // A type parameter this argument cannot fix by itself
            // (`f(Vec::new(), g)`): the later arguments' types fix it
            // first, as coby's inference does. They are probed, not
            // lowered, so the evaluation order is unchanged.
            if !resolved(&hint, &f.type_params) && !is_bare_num_literal(a) {
                for l in (j + 1)..f.params.len().min(args.len()) {
                    if !probe_safe(&args[l]) || mask & (1 << l) != 0 {
                        continue;
                    }
                    let lh = apply(&f.params[l].ty, &m);
                    let lexp = if resolved(&lh, &f.type_params) { Some(lh) } else { None };
                    let Ok(lt) = self.probe_with(|fx| fx.lower_expr(&args[l], lexp.as_ref())) else { continue };
                    let lt = match (&f.params[l].ty, &lt) {
                        (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                            Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                            None => lt,
                        },
                        _ => lt,
                    };
                    unify(&f.params[l].ty, &lt, &f.type_params, &mut m);
                }
                hint = apply(&p.ty, &m);
            }
            let exp = if resolved(&hint, &f.type_params) { Some(hint) } else { None };
            // `&e[lo .. hi]` formed for a pure direct slice parameter: the
            // borrow's checks only (`lower_slice_of_as`).
            if let ExprKind::SliceOf(smode, sbase, slo, shi) = &strip_parens(a).kind {
                let sa = strip_parens(a);
                let temp = self.g.items.temp_borrows.lock().unwrap().contains(&(sa as *const Expr as usize));
                if matches!(&p.ty, Type::Slice(_, pm) if pm == smode) && self.view_op(sa).is_none() && !temp && self.g.pure_direct(name, j) {
                    let v = self.lower_slice_of_as(smode, sbase, slo, shi, sa.line, false)?;
                    put_arg(&mut argv, j, v);
                    continue;
                }
            }
            // A slice place for a pure direct slice parameter: its read's
            // check, then its bytes; the path its slot holds stays where it
            // is, held for the call (`Gen::pure_direct`).
            if matches!(p.ty, Type::Slice(..)) && is_place_expr(a) && root_bound(a, &|n| self.lookup(n).is_some()) && self.g.pure_direct(name, j) {
                let pl = self.lower_place(a)?;
                if pl.raw.is_none() && matches!(pl.ty, Type::Slice(..)) {
                    if let Some(base) = &pl.base {
                        let loc = self.loc(a.line);
                        self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&pl.projs), loc));
                    }
                    let t = self.g.fresh("t");
                    let c = self.g.ctype(&pl.ty)?;
                    self.emit(&format!("{} {} = {};", c, t, pl.c));
                    put_arg(&mut argv, j, V { c: t, ty: pl.ty.clone(), obj: None });
                    continue;
                }
            }
            // A native map operation's map (or key) reference
            // (`native_map_op`): its body uses the path only for checks
            // through it, keeps none of it and returns nothing derived from
            // it. `&x` / `&mut x` of a local is checked as the borrow would
            // be (`cb_borrow_unminted`) and passed with `x`'s own root
            // path; no path is minted for the call (an unheld path no check
            // counts, D-0018, ending with the call).
            if let ExprKind::Borrow(bmode, place) = &a.kind {
                if self.native_map_passthrough(name, f, args, j) {
                    if let Some(bind) = path_name(place).and_then(|n| self.lookup(n)) {
                        if matches!(bind.kind, BindKind::Local) && !bind.root.is_empty() && !self.direct_refs.contains(&bind.c) && !self.unstored.contains_key(&bind.c) && !matches!(bind.ty, Type::Ref(..)) {
                            let loc = self.loc(a.line);
                            let md = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                            self.emit(&format!("cb_borrow_unminted({}, NULL, 0, {}, {});", bind.root, md, loc));
                            let ty = Type::Ref(Box::new(bind.ty.clone()), bmode.clone());
                            unify(&p.ty, &ty, &f.type_params, &mut m);
                            put_arg(&mut argv, j, V { c: format!("((cb_ref){{ (void *)&({}), {} }})", bind.c, bind.root), ty, obj: None });
                            continue;
                        }
                    }
                }
            }
            // `Box::get[_mut](b)`, `b` an unminted binder (`unminted_binders`),
            // for a direct parameter: the box's pointer read (checked
            // through the binder's base, if it has one), and the contents'
            // element access in the borrow's mode (`cb_elem_access`: checked
            // where an object lies over them -- a reference kept elsewhere
            // clashes there, where `cb_elem_borrow` would have faulted -- and
            // otherwise nothing to see), then the address alone: the callee
            // reads no path (tree-design.md).
            if let ExprKind::Call(gc, gargs) = &strip_parens(a).kind {
                let gname = match &gc.kind { ExprKind::Path(segs, _) => segs.join("::"), _ => String::new() };
                if (gname == "std::Box::get" || gname == "std::Box::get_mut") && gargs.len() == 1 && matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) {
                    if let Some(bind) = path_name(&gargs[0]).and_then(|n| self.lookup(n)) {
                        if let Some((base, bprojs)) = self.unminted_binders.get(&bind.c).cloned() {
                            if let Type::Ref(bt, _) = &bind.ty {
                                if let Type::Named(_, ba) = &**bt {
                                    if let [elem] = ba.as_slice() {
                                        let bc = self.g.ctype(bt)?;
                                        let tid = self.tid(elem)?;
                                        let md = if gname == "std::Box::get" { "CB_SHARED" } else { "CB_EXCLUSIVE" };
                                        if !base.is_empty() {
                                            let mut all = bprojs.clone();
                                            all.push("{ CB_FIELD, 0u }".to_string());
                                            self.emit(&format!("cb_read({}, (cb_proj[]){{ {} }}, {}, CB_NOLOC, 0);", base, all.join(", "), all.len()));
                                        }
                                        let ad = self.g.fresh("ad");
                                        let loc = self.loc(a.line);
                                        // The call's location, as `lower_user_call` sets it for a
                                        // `std` function: where its element borrow would fault.
                                        self.at(a.line);
                                        self.emit(&format!("void *{} = (void *)(({} *){})->ptr;", ad, bc, bind.c));
                                        self.emit(&format!("cb_elem_access({}, {}u, {}, 0, {});", ad, tid, md, loc));
                                        let mode = if gname == "std::Box::get" { Mode::Shared } else { Mode::Exclusive };
                                        let ty = Type::Ref(Box::new(elem.clone()), mode);
                                        unify(&p.ty, &ty, &f.type_params, &mut m);
                                        put_arg(&mut argv, j, V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None });
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // `&v[i]`/`&mut v[i]`, a plain element, for a pure direct
            // parameter: `v`'s checked read and the bounds fault as the
            // index call makes them, then the element checked in the
            // borrow's mode (`cb_elem_access`: where an object lies over
            // it; elsewhere the borrow would establish an ephemeral object
            // whose one path no check counts, D-0018, D-0023), and its
            // address. No element path is minted or retired.
            if let ExprKind::Borrow(bmode, place) = &a.kind {
                // `v[i]` as the index call it stands for.
                let rewritten = match &strip_parens(place).kind {
                    ExprKind::Index(..) if matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) => self.vec_index_call(strip_parens(place), false)?,
                    _ => None,
                };
                let place_x: &Expr = match &rewritten {
                    Some(x) => &**x,
                    None => &**place,
                };
                if let ExprKind::Deref(inner) = &strip_parens(place_x).kind {
                    let plain_elem = matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) && {
                        let et = self.probe_type(place_x, None)?;
                        !self.is_res(&et) && !self.has_refs(&et) && !self.holds_fn(&et) && !matches!(et, Type::Closure(_))
                    };
                    if plain_elem {
                        let m = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                        let saved = self.elem_mode.replace(m);
                        let r = self.elem_access_place(inner, *bmode == Mode::Exclusive, a.line);
                        self.elem_mode = saved;
                        if let Some(pl) = r? {
                            if matches!(pl.ty, Type::Never) {
                                return Ok(never());
                            }
                            if pl.base.is_none() && pl.raw.is_none() {
                                let ad = self.g.fresh("ad");
                                self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                                let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                                put_arg(&mut argv, j, V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None });
                                continue;
                            }
                            // The index call ran in full (not the prelude's
                            // own, say): its reference is the place's base.
                            if let Some(base) = pl.base.clone() {
                                let loc = self.loc(a.line);
                                self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&pl.projs), m, loc));
                                let ad = self.g.fresh("ad");
                                self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                                let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                                put_arg(&mut argv, j, V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None });
                                continue;
                            }
                            return internal("an element place with neither base nor plain access");
                        }
                    }
                }
            }
            // `&p`/`&mut p` for a pure direct parameter (`Gen::pure_direct`):
            // the borrow's check, and its address with no path minted --
            // nothing reads it, and an unheld path no check counts (D-0018).
            // A place rooted in a binding only: a temporary (`&mk()`) lowered
            // here and then left to the general path below was evaluated
            // twice, its effects and its destruction with it.
            if let ExprKind::Borrow(bmode, place) = &a.kind {
                if matches!(p.ty, Type::Ref(..)) && self.g.pure_direct(name, j) && is_place_expr(place) && root_bound(place, &|n| self.lookup(n).is_some()) {
                    let pl = self.lower_place(place)?;
                    // A local `pinned_names` left unpinned for this very
                    // borrow: no object, every check on it proven
                    // (`unchecked`); its address is the argument.
                    if pl.base.is_none() && pl.raw.is_none() && pl.is_binding {
                        let ad = self.g.fresh("ad");
                        self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                        let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                        put_arg(&mut argv, j, V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None });
                        continue;
                    }
                    if let (Some(base), None) = (pl.base.clone(), &pl.raw) {
                        let loc = self.loc(a.line);
                        let m = if *bmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
                        self.emit(&format!("cb_borrow_unminted({}, {}, {}, {});", base, Self::projs_args(&pl.projs), m, loc));
                        // The address taken now, as the borrow would be.
                        let ad = self.g.fresh("ad");
                        self.emit(&format!("void *{} = (void *)&({});", ad, pl.c));
                        let ty = Type::Ref(Box::new(pl.ty.clone()), bmode.clone());
                        put_arg(&mut argv, j, V { c: format!("((cb_ref){{ {}, 0 }})", ad), ty, obj: None });
                        continue;
                    }
                }
            }
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
            // A closure fixes a `fn(…) : R` parameter's type parameters by
            // its signature (D-0073).
            let unify_ty = match (&p.ty, &v.ty) {
                (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                    Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                    None => v.ty.clone(),
                },
                _ => v.ty.clone(),
            };
            unify(&p.ty, &unify_ty, &f.type_params, &mut m);
            // Then it moves into a callable box of that type, as it would
            // had the type been known before (`[Callable-Closure]`).
            let v = match (&p.ty, &v.ty) {
                (Type::Fn(..), Type::Closure(_)) => {
                    let dst = apply(&p.ty, &m);
                    self.box_closure(v, dst, a.line)?
                }
                _ => v,
            };
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
            put_arg(&mut argv, j, t);
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
        // Resources and reference data go in flight in argument order; a
        // slice for a pure direct parameter stays (`Gen::pure_direct`).
        for (j, (p, t)) in f.params.iter().zip(argv.iter()).enumerate() {
            let pty = apply(&p.ty, &fsub);
            if self.is_res(&pty) {
                self.emit(&format!("cb_send({});", t.obj.as_ref().expect("resource argument has object")));
            } else if matches!(pty, Type::Slice(..)) && self.g.pure_direct(name, j) {
            } else if self.has_refs(&pty) && !matches!(pty, Type::Ref(..)) {
                let tid = self.tid(&pty)?;
                self.emit(&format!("cb_send_datum(&{}, {}u);", t.c, tid));
            }
        }
        let ret = apply(&f.ret, &fsub);
        if self.map_acc == Some(args.as_ptr() as usize) && name == "std::HashMap::entry" && self.native_map_passthrough(name, f, args, 0) {
            self.map_acc = None;
            // The `_acc` variant: the value's address, its element access
            // made (`lower_block_body`'s compound pair).
            let Type::Ref(vt, _) = &ret else { return internal("entry's result") };
            let vc = self.g.ctype(vt)?;
            let pv = self.g.fresh("pv");
            self.emit(&format!("{} *{} = {}_acc({});", vc, pv, cname, argv.iter().map(|v| v.c.clone()).collect::<Vec<_>>().join(", ")));
            self.map_acc_used = true;
            return Ok(V { c: pv, ty: ret, obj: None });
        }
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
                // A name a closure captured by borrow is `*self.f_i`: its
                // place is the captured value, not the reference.
                Some(b) => Some(match (&b.kind, b.ty) {
                    (BindKind::CaptureRef(_), Type::Ref(t, _)) => *t,
                    (_, t) => t,
                }),
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
        // D-0103: a `Vec` that is part of a temporary argument.
        self.temp_root = self.g.items.temp_borrows.lock().unwrap().contains(&(a0 as *const Expr as usize));
        let p = self.lower_place(inner);
        self.temp_root = false;
        let p = p?;
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
        let plain = !self.is_res(&elem) && !self.has_refs(&elem) && !self.holds_fn(&elem) && !matches!(elem, Type::Closure(_));
        if let (Some(write), true) = (access, plain) {
            // The `…_pre` body inline (its location, its bounds fault, the
            // element's address), then the access's checks in one call.
            let vec_c = self.g.ctype(&p.ty)?;
            let elem_c = self.g.ctype(&elem)?;
            let tid = self.tid(&elem)?;
            let (v, e) = (self.g.fresh("v"), self.g.fresh("e"));
            let rm = if rmode == Mode::Exclusive { "CB_EXCLUSIVE" } else { "CB_SHARED" };
            let rm = self.elem_mode.unwrap_or(rm);
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
        if (name != "std::Vec::push" && name != "std::Vec::len" && name != "std::Vec::pop" && name != "std::Vec::reserve") || !in_prelude(f) {
            return Ok(None);
        }
        let Some((vec, tok, elem)) = args.first().and_then(|a| self.confined_local(a)) else { return Ok(None) };
        if name == "std::Vec::reserve" {
            // The body, handed the root path as `grow` is: its checks
            // through it cannot fail (`bind_confined`).
            let v = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
            if matches!(v.ty, Type::Never) {
                return Ok(Some(never()));
            }
            let n = self.into_temp(v, args[1].line)?;
            let cname = self.g.request_fn(name, vec![elem])?;
            self.emit(&format!("{}(((cb_ref){{ (void *)&({}), {} }}), {});", cname, vec, tok, n.c));
            return Ok(Some(unit()));
        }
        if name == "std::Vec::pop" {
            let cname = self.g.request_fn(name, vec![elem.clone()])?;
            let opt = Type::Named("std::Option".to_string(), vec![elem]);
            let opt_c = self.g.ctype(&opt)?;
            let t = self.g.fresh("t");
            self.emit(&format!("{} {} = {}_nc(((cb_ref){{ (void *)&({}), {} }}));", opt_c, t, cname, vec, tok));
            return Ok(Some(V { c: t, ty: opt, obj: None }));
        }
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

    // `Vec::push(&mut x, e)` for a checked local `x` and a plain element
    // type, with `e` an expression that can neither move nor end `x` (no
    // call but a conversion, no assignment): `cb_borrow_check` makes the
    // borrow's check and the body's first read check at once, as for
    // `Vec::len`; the body's other checks are through that path with
    // nothing in between that could change their outcome, and the path,
    // unheld, would end with the statement unobserved (D-0018). Then the
    // native body without its checks (`_nc`), handed `x`'s root path, as
    // a confined `x`'s push is.
    fn lower_push_checked_once(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::push" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0, a1] = args else { return Ok(None) };
        let ExprKind::Borrow(Mode::Exclusive, inner) = &a0.kind else { return Ok(None) };
        let ExprKind::Path(segs, targs) = &inner.kind else { return self.lower_push_place_once(f, inner, a0, a1) };
        if segs.len() != 1 || !targs.is_empty() {
            return Ok(None);
        }
        let Some(b) = self.lookup(&segs[0]) else { return Ok(None) };
        if !matches!(b.kind, BindKind::Local) || b.root.is_empty() || self.direct_refs.contains(&b.c) {
            return Ok(None);
        }
        let Type::Named(n, a) = &b.ty else { return Ok(None) };
        if n != "std::Vec" || a.len() != 1 {
            return Ok(None);
        }
        let elem = a[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) || !self.inert(a1) {
            return Ok(None);
        }
        let loc = self.loc(a0.line);
        self.emit(&format!("cb_borrow_check({}, NULL, 0, CB_EXCLUSIVE, {});", b.root, loc));
        let v = self.lower_expr(a1, Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, a1.line)?;
        let cname = self.g.request_fn(name, vec![elem])?;
        self.emit(&format!("{}_nc(((cb_ref){{ (void *)&({}), {} }}), {});", cname, b.c, b.root, x.c));
        Ok(Some(unit()))
    }

    // `Vec::sort_by(&mut x, [](ref<T, shared> a, ref<T, shared> b) : bool
    // { … })` for a plain `T`, the closure capturing nothing and using
    // `a` and `b` only as `*a`/`*b` or projection bases -- never lent,
    // stored or handed on (`direct_uses` with no callee). Each comparison's
    // element borrows are then unobservable: unheld, ended with their
    // statement, and against a vector the call holds exclusively. The
    // prelude's merge sort runs natively, in the same order of
    // comparisons (the closure may print or fault), calling the closure's
    // code with bare addresses; then the elements' cells are released and
    // the vector written, as `apply_order` and native `sort` do. The
    // closure value itself, never observed, is not formed.
    fn lower_sort_by_literal(&mut self, name: &str, f: &FnDecl, args: &[Expr]) -> R<Option<V>> {
        if name != "std::Vec::sort_by" || !in_prelude(f) {
            return Ok(None);
        }
        let [a0, a1] = args else { return Ok(None) };
        let ExprKind::Borrow(Mode::Exclusive, inner) = &a0.kind else { return Ok(None) };
        let ExprKind::Closure { is_move, captures, params, ret, body } = &a1.kind else { return Ok(None) };
        if !captures.is_empty() || params.len() != 2 {
            return Ok(None);
        }
        let Type::Named(n, targs) = self.probe_type(inner, None)? else { return Ok(None) };
        if n != "std::Vec" || targs.len() != 1 {
            return Ok(None);
        }
        let elem = targs[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) {
            return Ok(None);
        }
        for p in params {
            if !matches!(self.sub(&p.ty), Type::Ref(t, Mode::Shared) if *t == elem) || !direct_uses(body, &p.name, &|_, _| false) {
                return Ok(None);
            }
        }
        if self.g.direct_refs_of(params, body, &self.subst)?.len() != 2 {
            return Ok(None);
        }
        let p = self.lower_place(inner)?;
        let Some(base) = p.base.clone() else { return Ok(None) };
        let loc = self.loc(a0.line);
        self.emit(&format!("cb_borrow_check({}, {}, CB_EXCLUSIVE, {});", base, Self::projs_args(&p.projs), loc));
        // The closure's code alone (`lower_closure` without its value).
        self.g.uid += 1;
        let id = self.g.uid as u64;
        let ptypes: Vec<Type> = params.iter().map(|p| self.sub(&p.ty)).collect();
        let cl = format!("cl_{}", id);
        self.g.closures.insert(id, ClosureInfo { fields: Vec::new(), is_move: *is_move, params: ptypes, ret: Type::Void, cname: cl.clone() });
        let rt = self.lower_closure_body(id, params, ret.as_ref(), body)?;
        self.g.closures.get_mut(&id).unwrap().ret = rt.clone();
        if rt != Type::Bool {
            return internal("sort_by comparator not bool");
        }
        let vec_c = self.g.ctype(&Type::Named("std::Vec".to_string(), vec![elem.clone()]))?;
        let elem_c = self.g.reg_ctype(&elem)?;
        let size = self.g.size_align(&elem)?.0;
        let helper = format!("sort_by_{}", id);
        let mut h = String::new();
        writeln!(h, "static void {}({} *v)\n{{", helper, vec_c).unwrap();
        writeln!(h, "    uint64_t n = v->len;").unwrap();
        writeln!(h, "    if (n < 2) return;").unwrap();
        writeln!(h, "    uint64_t *a = (uint64_t *)malloc(n * 2 * sizeof(uint64_t));").unwrap();
        writeln!(h, "    {} *out = ({} *)malloc(n * sizeof({}));", elem_c, elem_c, elem_c).unwrap();
        writeln!(h, "    if (!a || !out) cb_fault(\"diag.alloc-failure\", CB_NOLOC, 0);").unwrap();
        writeln!(h, "    uint64_t *b = a + n;").unwrap();
        writeln!(h, "    for (uint64_t i = 0; i < n; i++) {{ a[i] = i; b[i] = 0; }}").unwrap();
        writeln!(h, "    uint64_t width = 1;").unwrap();
        writeln!(h, "    while (width < n) {{").unwrap();
        writeln!(h, "        uint64_t lo = 0;").unwrap();
        writeln!(h, "        while (lo < n) {{").unwrap();
        writeln!(h, "            uint64_t mid = n - lo > width ? lo + width : n;").unwrap();
        writeln!(h, "            uint64_t hi = n - mid > width ? mid + width : n;").unwrap();
        writeln!(h, "            uint64_t l = lo, r = mid, k = lo;").unwrap();
        writeln!(h, "            while (k < hi) {{").unwrap();
        writeln!(h, "                int take_left = l < mid && (r >= hi || !{}(((cb_ref){{ 0, 0 }}), ((cb_ref){{ (void *)&v->ptr[a[r]], 0 }}), ((cb_ref){{ (void *)&v->ptr[a[l]], 0 }})));", cl).unwrap();
        writeln!(h, "                if (take_left) b[k] = a[l++]; else b[k] = a[r++];").unwrap();
        writeln!(h, "                k++;").unwrap();
        writeln!(h, "            }}").unwrap();
        writeln!(h, "            lo = hi;").unwrap();
        writeln!(h, "        }}").unwrap();
        writeln!(h, "        uint64_t *t = a; a = b; b = t;").unwrap();
        writeln!(h, "        width = width > n ? n : width * 2;").unwrap();
        writeln!(h, "    }}").unwrap();
        writeln!(h, "    for (uint64_t i = 0; i < n; i++) out[i] = v->ptr[a[i]];").unwrap();
        writeln!(h, "    memcpy(v->ptr, out, n * sizeof({}));", elem_c).unwrap();
        writeln!(h, "    free(a < b ? a : b);").unwrap();
        writeln!(h, "    free(out);").unwrap();
        writeln!(h, "    cb_vec_drop_plain((uint8_t *)v->ptr, n, {}u);", size).unwrap();
        writeln!(h, "    cb_release((uint8_t *)v->ptr, n * {}u);", size).unwrap();
        writeln!(h, "}}\n").unwrap();
        if self.g.probing == 0 {
            writeln!(self.g.protos, "static void {}({} *v);", helper, vec_c).unwrap();
            self.g.fns.push_str(&h);
        }
        self.emit(&format!("{}(&({}));", helper, p.c));
        Ok(Some(unit()))
    }

    // `lower_push_checked_once` for `&mut p` with `p` a place other than a
    // bare local (`&mut s.bytes`): the same single check; the body inline.
    // `grow`, when needed, is handed a path formed for it then -- the
    // borrow the call would have formed, which nothing since could have
    // made fail -- unstamped, and ended at once.
    fn lower_push_place_once(&mut self, f: &FnDecl, place: &Expr, a0: &Expr, a1: &Expr) -> R<Option<V>> {
        if !in_prelude(f) || !self.inert(a1) || !root_bound(place, &|n| self.lookup(n).is_some()) {
            return Ok(None);
        }
        let pt = self.probe_type(place, None)?;
        let Type::Named(n, a) = &pt else { return Ok(None) };
        if n != "std::Vec" || a.len() != 1 {
            return Ok(None);
        }
        let elem = a[0].clone();
        if self.g.size_align(&elem)?.0 == 0 || !self.g.plain_data(&elem) {
            return Ok(None);
        }
        let pl = self.lower_place(place)?;
        let (Some(base), None) = (pl.base.clone(), &pl.raw) else { return Ok(None) };
        let loc = self.loc(a0.line);
        let projs = Self::projs_args(&pl.projs);
        self.emit(&format!("cb_borrow_check({}, {}, CB_EXCLUSIVE, {});", base, projs, loc));
        let v = self.lower_expr(a1, Some(&elem))?;
        if matches!(v.ty, Type::Never) {
            return Ok(Some(never()));
        }
        let x = self.into_temp(v, a1.line)?;
        let vec_c = self.g.ctype(&pt)?;
        let grow = self.g.request_fn("std::Vec::grow", vec![elem])?;
        let vv = self.g.fresh("vv");
        self.emit(&format!("{} *{} = &({});", vec_c, vv, pl.c));
        self.emit(&format!(
            "if ({vv}->len == {vv}->cap) {{ uint64_t gt = cb_borrow_unstamped({base}, {projs}, CB_EXCLUSIVE, {loc}); {grow}(((cb_ref){{ (void *){vv}, gt }})); cb_call_done(gt); }}",
            vv = vv, base = base, projs = projs, loc = loc, grow = grow
        ));
        self.emit(&format!("{vv}->ptr[{vv}->len] = {x}; {vv}->len += 1;", vv = vv, x = x.c));
        Ok(Some(unit()))
    }

    // An expression whose evaluation moves, ends or rebinds nothing: the
    // places it reads, literals, operators, and the conversions.
    fn inert(&self, e: &Expr) -> bool {
        const CONVERSIONS: &[&str] = &["str_byte", "str_len", "widen", "narrow", "narrow_wrapping", "reinterpret", "to_float", "to_int", "wrapping_add", "wrapping_sub", "wrapping_mul", "saturating_add", "saturating_sub", "saturating_mul"];
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Dollar => true,
            // A local's read, or a constant's; not a fn item or a variant
            // (a value built, but kept simple).
            ExprKind::Path(segs, targs) => segs.len() == 1 && targs.is_empty() && self.lookup(&segs[0]).map_or(false, |b| self.g.plain_data(&b.ty)),
            ExprKind::Unary(_, a) | ExprKind::Paren(a) | ExprKind::Deref(a) | ExprKind::Field(a, _) => self.inert(a),
            ExprKind::Binary(_, a, c) | ExprKind::Index(a, c) => self.inert(a) && self.inert(c),
            ExprKind::Call(c, args) => {
                matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && CONVERSIONS.contains(&segs[0].as_str()) && !self.g.items.fns.contains_key(&segs[0]))
                    && args.iter().all(|a| self.inert(a))
            }
            _ => false,
        }
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
        let rm = self.elem_mode.unwrap_or(rm);
        let in_prelude = matches!(f.body.stmts.first(), Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) if e.line <= prelude::line_count());
        let ([elem], [r, i], true) = (inst, argv, in_prelude) else { return Ok(None) };
        if self.is_res(elem) || self.has_refs(elem) || self.holds_fn(elem) || matches!(elem, Type::Closure(_)) {
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
    // `[Guard-Deref]` of a guard already lowered as a place: the guard is
    // read (it must be valid), and its lock path is the place's base.
    fn guard_through(&mut self, p: P, line: usize) -> R<P> {
        if let Some(base) = &p.base {
            let loc = self.loc(line);
            self.emit(&format!("cb_read({}, {}, {});", base, Self::projs_args(&p.projs), loc));
        }
        let Type::Guard(pointee) = p.ty else { return internal("guard deref of a non-guard") };
        let c = self.g.ctype(&pointee)?;
        let r = self.g.fresh("r");
        self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", r, p.c, p.c));
        Ok(P { c: format!("(*({} *){}.p)", c, r), ty: *pointee, base: Some(format!("{}.tok", r)), projs: Vec::new(), is_binding: false, raw: None })
    }

    fn guard_place(&mut self, inner: &Expr, line: usize) -> R<P> {
        let is_place = matches!(inner.kind, ExprKind::Path(..) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) | ExprKind::Paren(..));
        let (slot, ty) = if is_place {
            let p = self.lower_place(inner)?;
            return self.guard_through(p, line);
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
        // A generic function item spawned without type arguments
        // (`spawn(run_chunk, v, f)`) is instantiated from the arguments'
        // types, probed without lowering them, as coby infers it.
        let mut callee_expected: Option<Type> = None;
        if let ExprKind::Path(segs, targs) = &args[0].kind {
            let local = segs.len() == 1 && self.lookup(&segs[0]).is_some();
            if let Some(f) = self.g.items.fns.get(&segs.join("::")).cloned().filter(|f| !local && targs.is_empty() && !f.type_params.is_empty()) {
                let mut m: HashMap<String, Type> = HashMap::new();
                for (p, a) in f.params.iter().zip(args[1..].iter()) {
                    if !probe_safe(a) {
                        continue;
                    }
                    let hint = apply(&p.ty, &m);
                    let exp = if resolved(&hint, &f.type_params) { Some(hint) } else { None };
                    let Ok(t) = self.probe_with(|fx| fx.lower_expr(a, exp.as_ref())) else { continue };
                    let t = match (&p.ty, &t) {
                        (Type::Fn(..), Type::Closure(id)) => match self.g.closures.get(id) {
                            Some(info) => Type::Fn(info.params.clone(), Box::new(info.ret.clone())),
                            None => t,
                        },
                        _ => t,
                    };
                    unify(&p.ty, &t, &f.type_params, &mut m);
                }
                if f.type_params.iter().all(|tp| m.contains_key(tp)) {
                    callee_expected = Some(Type::Fn(f.params.iter().map(|p| apply(&p.ty, &m)).collect(), Box::new(apply(&f.ret, &m))));
                }
            }
        }
        let fv = self.lower_expr(&args[0], callee_expected.as_ref())?;
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
        } else if matches!(ret, Type::Ref(..)) {
            // D-0094: a bare reference result travels in its stored form —
            // the pointer in `res`, the token as a slot record at `&res`
            // (as a reference argument travels the other way). The call's
            // own return is received here as any ref-returning call's is;
            // the slot's hold then keeps the token alive past this
            // thread's end, and `cb_join`'s rekeying carries it to the
            // joiner's storage.
            // The result slot is stored before the arguments' slots are
            // forgotten (`res` is the block's first field, so the forget
            // starts after it): when the result token IS an argument's
            // token (a function returning its own reference parameter),
            // a gap in holding would retire it — this thread has no
            // scope to keep an unheld path alive.
            writeln!(body, "    cb_ref rr = {};", call).unwrap();
            writeln!(body, "    cb_recv_ref();").unwrap();
            writeln!(body, "    e->res = rr.p;").unwrap();
            writeln!(body, "    cb_store_ref(&e->res, rr.tok);").unwrap();
            writeln!(body, "    cb_env_forget((uint8_t *)e + sizeof(void *), sizeof(*e) - sizeof(void *));").unwrap();
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
    // D-0110: a struct or enum key, as a pointer to it read through its
    // reference (checked once).
    fn lower_key_ptr(&mut self, a: &Expr, kt: &Type, line: usize) -> R<String> {
        let r = self.lower_expr(a, None)?;
        let r = self.into_temp(r, line)?.c;
        let loc = self.loc(line);
        self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
        let ct = self.g.ctype(kt)?;
        Ok(format!("((const {} *)({}).p)", ct, r))
    }

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
            // D-0091: C's functions of the same names (`<math.h>`, Annex F).
            // D-0136: `std::math`'s, realized natively.
            "std::sqrt" | "std::floor" | "std::ceil" | "std::round" | "std::trunc" | "std::ln" | "std::exp" | "std::log2" | "std::log10" | "std::sin"
            | "std::cos" | "std::tan" => {
                let name = &name["std::".len()..];
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                // D-0101: `ln` is C's `log`.
                let base = if name == "ln" { "log" } else { name };
                let f = if v.ty == Type::F32 { format!("{}f", base) } else { base.to_string() };
                return Ok(V { c: format!("{}({})", f, x), ty: v.ty, obj: None });
            }
            // D-0101: C's `atan2` and `pow` (`powf` is CobaltC's name for the
            // float power; `pow` takes a `u32` exponent).
            "std::atan2" | "std::powf" => {
                let name = &name["std::".len()..];
                let a = self.lower_expr(&args[0], None)?;
                let a = self.into_temp(a, line)?;
                let b = self.lower_expr(&args[1], None)?;
                if matches!(a.ty, Type::Never) || matches!(b.ty, Type::Never) {
                    return Ok(never());
                }
                let b = self.into_temp(b, line)?;
                let base = if name == "atan2" { "atan2" } else { "pow" };
                let f = if a.ty == Type::F32 { format!("{}f", base) } else { base.to_string() };
                return Ok(V { c: format!("{}({}, {})", f, a.c, b.c), ty: a.ty, obj: None });
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
            // D-0122: `narrow` answering `None`.
            "checked_narrow" => {
                let dst = targ(0, self).expect("type argument");
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let x = self.into_temp(v.clone(), line)?.c;
                let (Type::Int(s), Type::Int(d)) = (&v.ty, &dst) else { return internal("checked_narrow on a non-integer") };
                let cond = narrow_cond(*s, *d, &x);
                let opt = Type::Named("std::Option".to_string(), vec![dst.clone()]);
                let oc = self.g.ctype(&opt)?;
                let t2 = self.g.fresh("cn");
                self.emit(&format!("{} {};", oc, t2));
                self.emit(&format!("if ({}) {{ {}.tag = 0u; {}.u.v0 = ({}){}; }} else {}.tag = 1u;", cond, t2, t2, c_int_type(*d), x, t2));
                Ok(V { c: t2, ty: opt, obj: None })
            }
            // D-0123: bit counts on the unsigned image; rotations by `k mod width`.
            "count_ones" | "leading_zeros" | "trailing_zeros" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Int(t) = v.ty else { return internal("bit count on a non-integer") };
                let x = self.into_temp(v.clone(), line)?.c;
                let bw = t.bitwidth(64);
                let c = if bw == 128 {
                    let f = match name { "count_ones" => "cb_popcount128", "leading_zeros" => "cb_clz128", _ => "cb_ctz128" };
                    format!("{}((cb_u128){})", f, x)
                } else {
                    let ux = format!("(uint64_t)({}){}", c_uint_of_width(bw), x);
                    match name {
                        "count_ones" => format!("cb_popcount64({})", ux),
                        "leading_zeros" => format!("cb_clz({}, {}u)", ux, bw),
                        _ => format!("cb_ctz({}, {}u)", ux, bw),
                    }
                };
                Ok(V { c, ty: Type::Int(IntTy::U32), obj: None })
            }
            "rotate_left" | "rotate_right" => {
                let v = self.lower_expr(&args[0], None)?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Int(t) = v.ty else { return internal("rotation of a non-integer") };
                let x = self.into_temp(v.clone(), line)?.c;
                let k = self.lower_expr(&args[1], Some(&Type::Int(IntTy::U32)))?;
                let k = self.into_temp(k, line)?.c;
                let bw = t.bitwidth(64);
                let c = if bw == 128 {
                    let f = if name == "rotate_left" { "cb_rotl128" } else { "cb_rotr128" };
                    format!("(({})({}((cb_u128){}, {})))", c_int_type(t), f, x, k)
                } else {
                    let f = if name == "rotate_left" { "cb_rotl" } else { "cb_rotr" };
                    format!("(({})({})({}((uint64_t)({}){}, {}, {}u)))", c_int_type(t), c_uint_of_width(bw), f, c_uint_of_width(bw), x, k, bw)
                };
                Ok(V { c, ty: Type::Int(t), obj: None })
            }
            // D-0121: through a `volatile` lvalue, so the C compiler keeps
            // every access as written.
            "read_volatile" | "write_volatile" => {
                let p = self.lower_expr(&args[0], None)?;
                if matches!(p.ty, Type::Never) {
                    return Ok(never());
                }
                let Type::Rawptr(pointee) = p.ty.clone() else { return internal("volatile access through a non-pointer") };
                let pc = self.into_temp(p, line)?.c;
                let ct = self.g.ctype(&pointee)?;
                if name == "read_volatile" {
                    return Ok(V { c: format!("(*(volatile {} *)({}))", ct, pc), ty: (*pointee).clone(), obj: None });
                }
                let v = self.lower_expr(&args[1], Some(&pointee))?;
                if matches!(v.ty, Type::Never) {
                    return Ok(never());
                }
                let vc = self.into_temp(v, line)?.c;
                self.emit(&format!("*(volatile {} *)({}) = {};", ct, pc, vc));
                Ok(unit())
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
            // D-0134 (`StringView::of`): the text's bytes as a shared slice.
            // A `str`'s bytes are static data that never ends; the slice
            // borrows the object `[Reclaim]` establishes over them (one per
            // address, as for any reclaimed cells).
            "str_slice" => {
                let s = self.lower_expr(&args[0], None)?;
                let st = self.into_temp(s, line)?;
                let u8t = Type::Int(IntTy::U8);
                let tid = self.tid(&u8t)?;
                let tok = self.g.fresh("tok");
                self.emit(&format!("uint64_t {} = cb_reclaim((uint8_t *)({}).p, {}u);", tok, st.c, tid));
                let sty = Type::Slice(Box::new(u8t.clone()), Mode::Shared);
                let c = self.g.ctype(&sty)?;
                let t = self.g.fresh("sl");
                self.emit(&format!("{} {};", c, t));
                let rv = V { c: format!("((cb_ref){{ (void *)({}).p, {} }})", st.c, tok), ty: Type::Ref(Box::new(u8t), Mode::Shared), obj: None };
                self.store(&format!("{}.src", t), &rv.ty.clone(), &rv)?;
                self.emit(&format!("{}.data = (uint8_t *)({}).p;", t, st.c));
                self.emit(&format!("{}.len = ({}).n;", t, st.c));
                self.register_temp(t, sty, line)
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
                // D-0083: an optional `str` message travels with the fault.
                if let Some(a1) = args.get(1) {
                    let v = self.lower_expr(a1, Some(&Type::Str))?;
                    let t = self.into_temp(v, line)?;
                    self.emit(&format!("cb_fault_msg(\"{}\", {}.p, {}.n, {});", id, t.c, t.c, loc));
                    return Ok(never());
                }
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
                    } else if self.holds_fn(&t.ty) {
                        let tid = self.tid(&t.ty)?;
                        self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
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
                    } else if self.holds_fn(&t.ty) {
                        let tid = self.tid(&t.ty)?;
                        self.emit(&format!("cb_drop_value(&{}, {}u);", t.c, tid));
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
                if matches!(rty, Type::Ref(..)) {
                    // D-0094: the stored form (pointer + rekeyed slot at
                    // `t`) back to register form. The send/recv pair —
                    // both on this thread — detaches the token from the
                    // ended thread's scopes and stamps it into this
                    // statement's, so it outlives the transient slot,
                    // which is forgotten with the C local it sat in.
                    let rr = self.g.fresh("r");
                    self.emit(&format!("cb_ref {} = {{ {}, cb_load_ref(&{}) }};", rr, t, t));
                    self.emit(&format!("cb_send_ref({}.tok);", rr));
                    self.emit("cb_recv_ref();");
                    self.emit(&format!("cb_env_forget((uint8_t *)&{}, sizeof({}));", t, t));
                    return Ok(V { c: rr, ty: rty, obj: None });
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
            // D-0116: `clone(&x)`: a plain `T` is `*r`; a resource `T` is a
            // call of its `T::clone`, declared or derived, at `T`'s arguments.
            "clone" => {
                // `&p` probed as `p`: a direct slice's element has no path
                // to borrow (`Gen::direct_refs_of`).
                let ty = match &args[0].kind {
                    ExprKind::Borrow(Mode::Shared, place) => self.probe_type(place, None)?,
                    _ => {
                        let at = self.probe_type(&args[0], None)?;
                        let Type::Ref(inner, _) = &at else { return internal("clone of a non-reference") };
                        (**inner).clone()
                    }
                };
                if !self.g.is_resource(&ty) {
                    // `clone(&p)` of a place `p`: a read of `p` -- the same
                    // check a shared borrow's formation and its read make,
                    // with no path formed (`[T-Clone]` of a plain value is
                    // its read).
                    if let ExprKind::Borrow(Mode::Shared, place) = &args[0].kind {
                        return self.lower_expr(place, expected);
                    }
                    let d = Expr { kind: ExprKind::Deref(Box::new(args[0].clone())), line };
                    return self.lower_expr(&d, expected);
                }
                let Type::Named(n, targs) = &ty else { return internal("clone of a resource that is not a named type") };
                let key = format!("{}::clone", n);
                let Some(f) = self.g.items.fns.get(&key).cloned() else { return internal("clone of a type with no clone") };
                return self.lower_user_call(&key, &f, targs, args, expected, line, None);
            }
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
                    // Each place's reference slots move with its bytes: a
                    // destination's old ones are forgotten before the new
                    // ones are copied in, and the temporary's after; a slot
                    // left behind (`None` swapped over `Some(r)`) kept `r`'s
                    // path counted as held.
                    let tid = self.tid(&ty)?;
                    self.emit(&format!("cb_copy_datum(&{}, {}, {}u);", t, pa, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *){}, sizeof({}));", pa, c));
                    self.emit(&format!("memcpy({}, {}, sizeof({})); cb_copy_datum({}, {}, {}u);", pa, pb, c, pa, pb, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *){}, sizeof({}));", pb, c));
                    self.emit(&format!("memcpy({}, &{}, sizeof({})); cb_copy_datum({}, &{}, {}u);", pb, t, c, pb, t, tid));
                    self.emit(&format!("cb_env_forget((uint8_t *)&{}, sizeof({}));", t, c));
                } else {
                    self.emit(&format!("memcpy({}, {}, sizeof({}));", pa, pb, c));
                    self.emit(&format!("memcpy({}, &{}, sizeof({}));", pb, t, c));
                }
                self.emit("}");
                Ok(unit())
            }
            // `rule.stdlib.hashmap` (spec/21 §1a, D-0041): a key's hash and
            // equality over its bytes, read through the key's reference.
            // D-0062: the key types' order.
            // `key_less_at(v, i, j)`: `key_less` of elements `i` and `j` of the
            // `Vec` `v` refers to, read in place. `v` is borrowed shared for
            // the call, so no other path can write them; one access check
            // on `v`, none per element.
            "key_less_at" => {
                let vt = self.probe_type(&args[0], None)?;
                let Type::Ref(inner, _) = &vt else { return internal("key_less_at of a non-reference") };
                let Type::Named(_, targs0) = &**inner else { return internal("key_less_at of a non-Vec") };
                let elem = targs0.first().cloned().unwrap_or(Type::Void);
                let vc = self.g.ctype(inner)?;
                let r = self.lower_expr(&args[0], None)?;
                let r = self.into_temp(r, line)?.c;
                let i = self.lower_expr(&args[1], Some(&Type::Int(IntTy::Usize)))?;
                let i = self.into_temp(i, line)?.c;
                let j = self.lower_expr(&args[2], Some(&Type::Int(IntTy::Usize)))?;
                let j = self.into_temp(j, line)?.c;
                self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", r, loc));
                let len = format!("(({} *)({}).p)->len", vc, r);
                self.emit(&format!("cb_index_check((uint64_t){}, {}, {}); cb_index_check((uint64_t){}, {}, {});", i, len, loc, j, len, loc));
                let at = |k: &str| format!("(({} *)({}).p)->ptr[{}]", vc, r, k);
                // A non-key element: the call is on a path `std` never takes
                // for that type (`PriorityQueue::before` for a queue made by
                // `new_by`, D-0137), since the front end checks every call
                // that reaches the key order (`[Sort-Not-Key]`,
                // `[Priority-Queue-Not-Key]`). It lowers to the evaluator's
                // dynamic verdict, as `[Match]` on a non-enum does.
                let key = match &elem {
                    Type::Named(n, _) if n == "std::String" => Some(format!("cb_bytes_less((const void *){}.bytes.ptr, {}.bytes.len, (const void *){}.bytes.ptr, {}.bytes.len)", at(&i), at(&i), at(&j), at(&j))),
                    // D-0110: a struct or enum key.
                    Type::Named(..) => self.g.key_cmp_fn(&elem).ok().map(|f| format!("((uint8_t)({}(&{}, &{}) < 0))", f, at(&i), at(&j))),
                    Type::Str => Some(format!("cb_bytes_less((const void *){}.p, {}.n, (const void *){}.p, {}.n)", at(&i), at(&i), at(&j), at(&j))),
                    Type::Int(_) | Type::Bool => Some(format!("((uint8_t)({} < {}))", at(&i), at(&j))),
                    _ => None,
                };
                let c = match key {
                    Some(c) => c,
                    None => {
                        self.emit(&format!("cb_fault(\"diag.type-mismatch\", {});", loc));
                        "((uint8_t)0)".to_string()
                    }
                };
                return Ok(V { c, ty: Type::Bool, obj: None });
            }
            "key_less" => {
                let t = self.probe_type(&args[0], None)?;
                if let Some(kt) = composite_key(&t) {
                    let f = self.g.key_cmp_fn(&kt)?;
                    let a = self.lower_key_ptr(&args[0], &kt, line)?;
                    let b = self.lower_key_ptr(&args[1], &kt, line)?;
                    return Ok(V { c: format!("((uint8_t)({}({}, {}) < 0))", f, a, b), ty: Type::Bool, obj: None });
                }
                let text = matches!(&t, Type::Ref(inner, _) if matches!(&**inner, Type::Str) || matches!(&**inner, Type::Named(n, _) if n == "std::String"));
                if text {
                    let a = self.lower_key_bytes(&args[0], line)?;
                    let b = self.lower_key_bytes(&args[1], line)?;
                    return Ok(V { c: format!("cb_bytes_less({}, {}, {}, {})", a.0, a.1, b.0, b.1), ty: Type::Bool, obj: None });
                }
                let mut vs = Vec::new();
                for a in args {
                    let d = std::rc::Rc::new(Expr { kind: ExprKind::Deref(Box::new(a.clone())), line: a.line });
                    self.kept.push(d.clone());
                    let v = self.lower_expr(&d, None)?;
                    vs.push(self.into_temp(v, line)?.c);
                }
                Ok(V { c: format!("((uint8_t)({} < {}))", vs[0], vs[1]), ty: Type::Bool, obj: None })
            }
            "key_hash" | "key_eq" => {
                let t = self.probe_type(&args[0], None)?;
                if let Some(kt) = composite_key(&t) {
                    let mut ps = Vec::new();
                    for a in args {
                        ps.push(self.lower_key_ptr(a, &kt, line)?);
                    }
                    return Ok(if name == "key_hash" {
                        let f = self.g.key_hash_fn(&kt)?;
                        V { c: format!("{}(14695981039346656037ull, {})", f, ps[0]), ty: Type::Int(IntTy::U64), obj: None }
                    } else {
                        let f = self.g.key_cmp_fn(&kt)?;
                        V { c: format!("((uint8_t)({}({}, {}) == 0))", f, ps[0], ps[1]), ty: Type::Bool, obj: None }
                    });
                }
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
            "$assert_fail" => {
                // D-0065: the fault, with `$fmt`'s text as its message.
                if args.is_empty() {
                    self.emit(&format!("cb_fault(\"diag.assert-failed\", {});", loc));
                    return Ok(never());
                }
                let v = self.lower_expr(&args[0], Some(&Type::Str))?;
                let t = self.into_temp(v, line)?;
                self.emit(&format!("cb_fault_msg(\"diag.assert-failed\", {}.p, {}.n, {});", t.c, t.c, loc));
                Ok(never())
            }
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
                        // D-0100: an enum prints as its variant's name.
                        Type::Named(n, _) if self.g.items.enums.contains_key(n) => {
                            let names: Vec<String> = self.g.items.enums[n].variants.iter().map(|v| v.name.clone()).collect();
                            self.variant_name_text(&format!("({}).tag", x), &names)
                        }
                        Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if self.g.items.enums.contains_key(n)) => {
                            let Type::Named(n, _) = &**inner else { unreachable!() };
                            let names: Vec<String> = self.g.items.enums[n].variants.iter().map(|v| v.name.clone()).collect();
                            let loc = self.loc(a.line);
                            self.emit(&format!("cb_read(({}).tok, NULL, 0, {});", x, loc));
                            let et = self.g.ctype(inner)?;
                            self.variant_name_text(&format!("(({} *)({}).p)->tag", et, x), &names)
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
            "std::Result::map_err" => {
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
// Checks implied by an earlier one (a pass over a function's C). Within a
// run of lines with no other runtime call and no join (a label, a `}`,
// an `else`), the runtime's state cannot change, and a check is a pure
// function of it, so:
// - after an exclusive check through `base` with projection `P`
//   (`cb_borrow_unminted`/`cb_borrow_check` with `CB_EXCLUSIVE`), a read
//   or exclusive check through `base` under `P` cannot fail on a clash;
// - after a read or shared check under `P`, a read under `P` cannot;
// - after any check that tests initialization (`cb_read`,
//   `cb_borrow_check`) through `base`, none can fail on it (it is the
//   object's, and only a write changes it).
// Such a later `cb_read` / `cb_borrow_unminted` / `cb_borrow_check` is
// dropped. A projection with a range (a slice), and any base not written
// identically, are left alone.
fn elide_implied_checks(out: &str) -> String {
    // (base, projection, exclusive) checked, and bases whose object's
    // initialization was checked.
    let mut seen: Vec<(String, Vec<String>, bool)> = Vec::new();
    let mut inited: HashSet<String> = HashSet::new();
    // `cb_stmt_push`/`cb_stmt_push_at` open a scope: no path's state changes.
    const PURE: &[&str] = &["cb_read", "cb_borrow_unminted", "cb_borrow_check", "cb_at", "cb_fault", "cb_index_check", "cb_hash_bytes", "cb_bytes_eq", "cb_bytes_lt", "cb_load_ref", "cb_stmt_push", "cb_stmt_push_at"];
    // `uint64_t tkN = <token>;` copies: a check through either names one path.
    let mut alias: HashMap<String, String> = HashMap::new();
    fn split_args(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let (mut depth, mut cur) = (0i32, String::new());
        for c in s.chars() {
            match c {
                '(' | '{' | '[' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' | '}' | ']' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur.trim().to_string());
        }
        out
    }
    // The projection's steps, or None if it has a range.
    fn projs(a: &str) -> Option<Vec<String>> {
        if a == "NULL" {
            return Some(Vec::new());
        }
        let inner = a.strip_prefix("(cb_proj[]){")?.strip_suffix('}')?;
        let mut v = Vec::new();
        for st in inner.split("},") {
            let st = st.trim().trim_start_matches('{').trim_end_matches('}').trim();
            if st.is_empty() {
                continue;
            }
            if !(st.starts_with("CB_FIELD") || st.starts_with("CB_PAYLOAD")) {
                return None;
            }
            v.push(st.split_whitespace().collect::<Vec<_>>().join(""));
        }
        Some(v)
    }
    let calls = |line: &str| -> Vec<String> {
        let b = line.as_bytes();
        let mut v = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i].is_ascii_alphabetic() || b[i] == b'_' {
                let st = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if i < b.len() && b[i] == b'(' && (st == 0 || !(b[st - 1].is_ascii_alphanumeric() || b[st - 1] == b'_')) {
                    v.push(line[st..i].to_string());
                }
            } else {
                i += 1;
            }
        }
        v
    };
    let mut res = String::with_capacity(out.len());
    for line in out.lines() {
        let t = line.trim_start();
        // A join, or control arriving from elsewhere: forget everything.
        if t.starts_with('}') || t.starts_with("else") || (t.ends_with(':') && !t.contains(' ')) || t.starts_with("case ") || t.starts_with("default") {
            seen.clear();
            inited.clear();
            res.push_str(line);
            res.push('\n');
            continue;
        }
        if let Some(rest) = t.strip_prefix("uint64_t ") {
            if let Some((name, val)) = rest.strip_suffix(';').and_then(|r| r.split_once(" = ")) {
                let simple = |v: &str| v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '(' || c == ')');
                if !name.contains(' ') && simple(val) && !val.contains("()") && !val.chars().next().map_or(true, |c| c.is_ascii_digit()) {
                    let canon = alias.get(val).cloned().unwrap_or_else(|| val.to_string());
                    alias.insert(name.to_string(), canon);
                }
            }
        }
        let cs = calls(t);
        let kind = ["cb_read(", "cb_borrow_unminted(", "cb_borrow_check("].iter().find(|k| t.starts_with(**k)).map(|k| &k[..k.len() - 1]);
        if let (Some(k), true) = (kind, t.ends_with(");")) {
            let args = split_args(&t[k.len() + 1..t.len() - 2]);
            let base0 = args.first().cloned().unwrap_or_default();
            let base = alias.get(&base0).cloned().unwrap_or(base0);
            let pr = args.get(1).and_then(|a| projs(a));
            let excl = match k {
                "cb_read" => false,
                _ => args.get(3).map_or(false, |m| m == "CB_EXCLUSIVE"),
            };
            let tests_init = k != "cb_borrow_unminted";
            if let Some(p) = pr {
                let covered = seen.iter().any(|(b, q, ex)| *b == base && p.starts_with(q) && (*ex || !excl));
                let init_ok = !tests_init || inited.contains(&base);
                if covered && init_ok {
                    continue;
                }
                seen.push((base.clone(), p, excl));
                if tests_init {
                    inited.insert(base);
                }
            }
            res.push_str(line);
            res.push('\n');
            continue;
        }
        // Any other call may change the runtime's state.
        if cs.iter().any(|c| !PURE.contains(&c.as_str()) && !c.starts_with("__builtin") && !matches!(c.as_str(), "if" | "while" | "for" | "switch" | "sizeof" | "return")) {
            seen.clear();
            inited.clear();
        }
        res.push_str(line);
        res.push('\n');
    }
    res
}

// Whether generated C calls nothing that could call back into the
// program: only C keywords, builtins, and the runtime's location
// bookkeeping (`cb_at`) and faults (`cb_fault`, which does not return).
// A destructor the runtime runs, a `fn` value or closure, and any other
// runtime call count as calls.
fn calls_nothing(body: &str) -> bool {
    const LEAF: &[&str] = &["cb_at", "cb_fault", "if", "while", "for", "switch", "sizeof", "offsetof", "return"];
    let b = body.as_bytes();
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
        let id = &body[i..e];
        let mut k = e;
        while k < b.len() && b[k] == b' ' {
            k += 1;
        }
        if b.get(k) == Some(&b'(') && !LEAF.contains(&id) && !id.starts_with("__builtin") {
            return false;
        }
        i = e;
    }
    true
}

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
        "cb_fault", "cb_fault_msg", "cb_index_check", "cb_str_eq", "cb_write_out", "cb_write_err", "cb_hash_bytes", "cb_bytes_eq", "cb_read_in", "cb_arg_bytes", "cb_set_args", "cb_text_int", "cb_text_f64", "cb_text_f32", "cb_text_bool", "cb_parse_int", "cb_parse_float", "cb_frame_here", "cb_file_read", "cb_file_write", "cb_file_op", "cb_file_at", "cb_fs_op", "cb_fs_query", "cb_clock_read", "cb_event_op", "cb_bytes_less", "cb_format", "cb_at", "cb_unit", "cb_str", "cb_i128", "cb_u128", "cb_ref", "cb_proj", "cb_fmt_arg",
        // Pure inline helpers (`cbrt.h`), and a query that records nothing.
        "cb_bytes_lt", "cb_bytes_cmp", "cb_popcount64", "cb_popcount128", "cb_clz", "cb_clz128", "cb_ctz", "cb_ctz128", "cb_rotl", "cb_rotl128", "cb_rotr", "cb_rotr128", "cb_fnv_more", "cb_live_at",
        "cb_read", "cb_write", "cb_borrow_unminted", "cb_borrow_range_unminted", "cb_borrow_unstamped", "cb_call_done", "cb_load_ref", "cb_store_ref", "cb_copy_datum", "cb_move_to", "cb_send", "cb_send_ref", "cb_send_datum", "cb_borrow_check", "cb_elem_access",
        // A question about the value at an address (`owns`), recording nothing.
        "cb_owns",
        // Raw memory from the allocator (`lower_box_new`): recorded for
        // `deallocate`, in no scope.
        "cb_allocate",
    ];
    // `cb_absorb` ends a temporary made in the statement (a struct or
    // variant literal's part): it detaches that object from its own
    // record, whatever scopes are open, so it needs only that statement.
    // `cb_elem_borrow` mints a path and puts it in flight; the caller's
    // `cb_recv_ref` stamps it into the statement.
    // `cb_raw_move_in` (`lower_box_new`) ends or detaches a value made in
    // the statement, as `cb_absorb` does.
    const SCOPE: &[&str] = &["cb_new", "cb_new_uninit", "cb_recv", "cb_recv_ref", "cb_take", "cb_borrow", "cb_borrow_range", "cb_absorb", "cb_elem_borrow", "cb_raw_move_in"];
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
            // A string literal's array (`cb_s0`, `cb_s1`, …) is data.
            let literal = id.len() > 4 && id.starts_with("cb_s") && id[4..].bytes().all(|c| c.is_ascii_digit());
            if id.starts_with("cb_") && !literal {
                // A fn value's call thunk (`Gen::thunk`) records nothing in
                // the caller's scopes (`cb_call_self`), as a direct call.
                if id.starts_with("cb_call_") {
                } else if SCOPE.contains(&id) {
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
        let id = m[1..].split('\u{2}').next().unwrap_or("");
        Some((k, id.parse().expect("scope marker id")))
    }
    // A frame marker's location, when it carries one.
    fn marker_loc(line: &str) -> Option<&str> {
        line.trim_start().strip_prefix('\u{1}')?.split_once('\u{2}').map(|(_, l)| l)
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
                let at = marker_loc(line).map(|l| format!("cb_{}_push_at({});", if k == 'F' { "frame" } else { "stmt" }, l));
                let call = match k {
                    'F' => at.as_deref().unwrap_or("cb_frame_push();"),
                    'S' => at.as_deref().unwrap_or("cb_stmt_push();"),
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

// Whether an arm's body uses `name` exactly once, and that use is the
// first thing it evaluates: `f(name, …)` or `f(g(name, …), …)`, `f` and
// `g` named (functions, or local closures).
fn binder_used_once_first(body: &Expr, name: &str) -> bool {
    let is_name = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
    // The first thing evaluated: a block's first statement (or its value),
    // and in `x = x op e` (what `x op= e` is, D-0035), for a local `x`
    // other than the binder, `e`: reading or writing a local touches
    // nothing the binder's borrow reaches.
    let mut first = strip_parens(body);
    if let ExprKind::Block(b) = &first.kind {
        first = match (b.stmts.first(), &b.tail) {
            (Some(Stmt::Expr(e)) | Some(Stmt::BlockLike(e)), _) => strip_parens(e),
            (None, Some(t)) => strip_parens(t),
            _ => return false,
        };
    }
    let local = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] != name && t.is_empty());
    if let ExprKind::Assign(l, r) = &first.kind {
        if !local(l) {
            return false;
        }
        first = match &strip_parens(r).kind {
            ExprKind::Binary(_, x, e) if local(x) && path_name(x) == path_name(l) => strip_parens(e),
            _ => strip_parens(r),
        };
    }
    // A read through the binder (`*name`), first: no call to look into.
    if matches!(&first.kind, ExprKind::Deref(x) if is_name(x)) {
        let mut n = 0;
        let b = Block { stmts: Vec::new(), tail: Some(Box::new(body.clone())) };
        visit_exprs(&b, &mut |e| {
            if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
                n += 1;
            }
        });
        return n == 1;
    }
    let ExprKind::Call(callee, args) = &first.kind else { return false };
    // A named callee (a function, or a local closure, whose evaluation
    // only reads that local).
    if !matches!(&callee.kind, ExprKind::Path(..)) {
        return false;
    }
    // The other arguments are evaluated while the binder's value is
    // pending (D-0107): only literals and locals, which form no path and
    // so could not have met the binder's held path.
    let inert = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_) | ExprKind::StrLit(_) | ExprKind::Unit) || local(e);
    let Some(a0) = args.first() else { return false };
    if !args[1..].iter().all(|x| inert(x)) {
        return false;
    }
    let first = is_name(a0)
        || matches!(&strip_parens(a0).kind, ExprKind::Call(g, ga) if matches!(&g.kind, ExprKind::Path(..)) && ga.first().map_or(false, |x| is_name(x)) && ga[1..].iter().all(|x| inert(x)));
    if !first {
        return false;
    }
    let mut n = 0;
    let b = Block { stmts: Vec::new(), tail: Some(Box::new(body.clone())) };
    visit_exprs(&b, &mut |e| {
        if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
            n += 1;
        }
    });
    n == 1
}

// `*t = *t op e;` for the hidden `t`, with `e` a literal or a local
// (`lower_block_body`'s compound pair).
fn compound_write_inert(s: &Stmt, t: &str) -> bool {
    let deref_t = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Deref(x) if path_name(x) == Some(t));
    let Stmt::Expr(e) = s else { return false };
    let ExprKind::Assign(l, r) = &e.kind else { return false };
    let ExprKind::Binary(_, a, rhs) = &strip_parens(r).kind else { return false };
    deref_t(l)
        && deref_t(a)
        && (matches!(&strip_parens(rhs).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_))
            || path_name(rhs).map_or(false, |n| n != t && !n.starts_with("__")))
}

// The locals of a function body that `bind_value` may hold by frame
// (`cb_frame_hold`) when of reference type: never assigned (`r = …`),
// never borrowed as a binding (`&r`), and mentioned in no closure (which
// could capture it).
fn held_locals(b: &Block) -> HashSet<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    visit_stmts(b, &mut |st| {
        if let Stmt::Let { name, .. } = st {
            *count.entry(name.clone()).or_default() += 1;
        }
    });
    let assigned = assigned_names(b);
    let mut excluded: HashSet<String> = HashSet::new();
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Borrow(_, a) => {
            if let Some(n) = path_name(a) {
                excluded.insert(n.to_string());
            }
        }
        ExprKind::Closure { body, captures, .. } => {
            excluded.extend(captures.iter().cloned());
            visit_exprs(body, &mut |x| {
                if let ExprKind::Path(s, _) = &x.kind {
                    if s.len() == 1 {
                        excluded.insert(s[0].clone());
                    }
                }
            });
        }
        _ => {}
    });
    // The conditions are by name, so they cover every declaration of it.
    count.into_iter().filter(|(n, _)| !assigned.contains(n) && !excluded.contains(n) && !n.starts_with("__")).map(|(n, _)| n).collect()
}

// The function an arm body's first call passes the binder to: `g` in
// `g(name, …)` as the outer call's first argument or as the outer call.
fn binder_box_call(body: &Expr, name: &str) -> Option<String> {
    let is_name = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(s, t) if s.len() == 1 && s[0] == name && t.is_empty());
    let mut first = strip_parens(body);
    if let ExprKind::Block(b) = &first.kind {
        first = match (b.stmts.first(), &b.tail) {
            (Some(Stmt::Expr(e)) | Some(Stmt::BlockLike(e)), _) => strip_parens(e),
            (None, Some(t)) => strip_parens(t),
            _ => return None,
        };
    }
    if let ExprKind::Assign(_, r) = &first.kind {
        first = match &strip_parens(r).kind {
            ExprKind::Binary(_, _, e) => strip_parens(e),
            _ => strip_parens(r),
        };
    }
    let ExprKind::Call(callee, args) = &first.kind else { return None };
    let ExprKind::Path(segs, _) = &callee.kind else { return None };
    if args.first().map_or(false, |a| is_name(a)) {
        return Some(segs.join("::"));
    }
    match args.first().map(|a| &strip_parens(a).kind) {
        Some(ExprKind::Call(g, ga)) if ga.first().map_or(false, |a| is_name(a)) => match &g.kind {
            ExprKind::Path(gs, _) => Some(gs.join("::")),
            _ => None,
        },
        _ => None,
    }
}

// A one-segment path's name.
fn path_name(e: &Expr) -> Option<&str> {
    match &strip_parens(e).kind {
        ExprKind::Path(s, _) if s.len() == 1 => Some(s[0].as_str()),
        _ => None,
    }
}

// Every expression in `e`, closures' bodies included.
fn visit_expr(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => visit_expr(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
            visit_expr(a, f);
            visit_expr(b, f);
        }
        ExprKind::SliceOf(_, a, lo, hi) => {
            visit_expr(a, f);
            visit_expr(lo, f);
            visit_expr(hi, f);
        }
        ExprKind::Call(c, args) => {
            visit_expr(c, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, a)| visit_expr(a, f)),
        ExprKind::ArrayLit(es) => es.iter().for_each(|a| visit_expr(a, f)),
        ExprKind::ArrayRepeat(a, _) => visit_expr(a, f),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => visit_exprs(b, f),
        ExprKind::If(c, t, e2) => {
            visit_expr(c, f);
            visit_exprs(t, f);
            if let Some(e2) = e2 {
                visit_expr(e2, f);
            }
        }
        ExprKind::While(c, b, step) => {
            visit_expr(c, f);
            visit_exprs(b, f);
            if let Some(st) = step {
                visit_expr(st, f);
            }
        }
        ExprKind::Match(sc, arms) => {
            visit_expr(sc, f);
            arms.iter().for_each(|a| visit_expr(&a.body, f));
        }
        ExprKind::Return(Some(a)) => visit_expr(a, f),
        ExprKind::Closure { body, .. } => visit_exprs(body, f),
        _ => {}
    }
}

// Every name a block binds, at any depth: `let`s, destructured fields,
// match binders, closures' parameters (a `foreach` or `if`-pattern binds
// through these). By name, as `pinned_names` works.
fn bound_names(b: &Block) -> HashSet<String> {
    fn stmts(b: &Block, out: &mut HashSet<String>) {
        for st in &b.stmts {
            match st {
                Stmt::Let { name, .. } => {
                    out.insert(name.clone());
                }
                Stmt::Destructure { fields, .. } => out.extend(fields.iter().cloned()),
                _ => {}
            }
        }
    }
    let mut out = HashSet::new();
    stmts(b, &mut out);
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Block(b) | ExprKind::Unsafe(b) | ExprKind::While(_, b, _) => stmts(b, &mut out),
        ExprKind::If(_, t, _) => stmts(t, &mut out),
        ExprKind::Match(_, arms) => out.extend(arms.iter().filter_map(|a| a.binder.clone())),
        ExprKind::Closure { params, body, .. } => {
            out.extend(params.iter().map(|p| p.name.clone()));
            stmts(body, &mut out);
        }
        _ => {}
    });
    out
}

// `r` (a reference) used only to read through and to hand on to reader
// parameters (`Gen::reader_param`): `r.f…`/`r[i]…`/`*r` read as values;
// `r` itself, or (`field_borrow`) `&r.f…`, as an argument a `c_ok`
// parameter takes. Never otherwise borrowed, sliced, assigned through,
// matched on, captured, rebound or used bare.
fn reader_uses(b: &Block, r: &str, field_borrow: bool, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // A projection chain (fields, elements, `*`) down to `r`.
    fn rooted(e: &Expr, r: &str) -> bool {
        match &e.kind {
            ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) => is_r(a, r) || rooted(a, r),
            ExprKind::Paren(a) => rooted(a, r),
            _ => false,
        }
    }
    // The index expressions inside such a chain.
    fn chain_ok(e: &Expr, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        match &e.kind {
            ExprKind::Field(a, _) | ExprKind::Deref(a) | ExprKind::Paren(a) => is_r(a, r) || chain_ok(a, r, fb, c),
            ExprKind::Index(a, i) => (is_r(a, r) || chain_ok(a, r, fb, c)) && ok(i, r, fb, c),
            _ => false,
        }
    }
    fn handed(a: &Expr, r: &str, fb: bool) -> bool {
        is_r(a, r) || (fb && matches!(&a.kind, ExprKind::Borrow(Mode::Shared, x) if rooted(x, r) && !matches!(strip_parens(x).kind, ExprKind::Deref(_) | ExprKind::Index(..))))
    }
    fn ok(e: &Expr, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        let o = |x: &Expr| ok(x, r, fb, c);
        match &e.kind {
            ExprKind::Path(..) => !is_r(e, r),
            ExprKind::Call(callee, args) => {
                let segs = match &callee.kind {
                    ExprKind::Path(s, _) if !is_r(callee, r) => Some(s),
                    _ => None,
                };
                o(callee) && args.iter().enumerate().all(|(i, a)| if handed(a, r, fb) { segs.map_or(false, |s| c(s, i)) } else { o(a) })
            }
            ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(_) if rooted(e, r) => chain_ok(e, r, fb, c),
            ExprKind::Borrow(_, a) | ExprKind::SliceOf(_, a, _, _) if is_r(a, r) || rooted(a, r) => false,
            ExprKind::Assign(l, _) if is_r(l, r) || rooted(l, r) => false,
            ExprKind::Match(sc, _) if is_r(sc, r) || rooted(sc, r) || matches!(&sc.kind, ExprKind::Borrow(_, x) if is_r(x, r) || rooted(x, r)) => false,
            ExprKind::Closure { .. } => !mentions(e, r),
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => o(a),
            ExprKind::Binary(_, a, x) | ExprKind::Index(a, x) | ExprKind::Assign(a, x) => o(a) && o(x),
            ExprKind::SliceOf(_, a, lo, hi) => o(a) && o(lo) && o(hi),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| o(a)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| o(a)),
            ExprKind::Block(bl) | ExprKind::Unsafe(bl) => block(bl, r, fb, c),
            ExprKind::If(cd, t, f) => o(cd) && block(t, r, fb, c) && f.as_ref().map_or(true, |f| o(f)),
            ExprKind::While(cd, bl, st) => o(cd) && block(bl, r, fb, c) && st.as_ref().map_or(true, |s| o(s)),
            ExprKind::Match(sc, arms) => o(sc) && arms.iter().all(|a| a.binder.as_deref() != Some(r) && o(&a.body)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| o(v)),
            _ => true,
        }
    }
    fn block(b: &Block, r: &str, fb: bool, c: &dyn Fn(&[String], usize) -> bool) -> bool {
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r, fb, c)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && !is_r(init, r) && ok(init, r, fb, c),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r, fb, c),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r, fb, c))
    }
    block(b, r, field_borrow, c_ok)
}

// Whether `e` names `x` anywhere (a closure's capture included).
fn mentions(e: &Expr, x: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |e| match &e.kind {
        ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == x => found = true,
        ExprKind::Closure { captures, .. } if captures.iter().any(|c| c == x) => found = true,
        _ => {}
    });
    found
}

// Every expression in `b`, closures' bodies included.
fn visit_exprs(b: &Block, f: &mut dyn FnMut(&Expr)) {
    for st in &b.stmts {
        match st {
            Stmt::Let { init: Some(e), .. } | Stmt::Expr(e) | Stmt::BlockLike(e) => visit_expr(e, f),
            Stmt::Destructure { init, .. } => visit_expr(init, f),
            Stmt::Let { init: None, .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        visit_expr(t, f);
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
                ExprKind::ArrayRepeat(a, _) => self.expr(a, nested),
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
// An argument whose type can be read by probing it before its turn: a
// name, or a field, element or borrow of one. A literal would take its
// default type there, and a closure would be formed twice.
fn probe_safe(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(..) => true,
        ExprKind::Paren(x) | ExprKind::Field(x, _) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) => probe_safe(x),
        ExprKind::Index(x, i) => probe_safe(x) && matches!(i.kind, ExprKind::Path(..)),
        _ => false,
    }
}

fn pinned_names(b: &Block, direct: &dyn Fn(&[String], usize) -> bool) -> HashSet<String> {
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
    fn expr(e: &Expr, out: &mut HashSet<String>, d: &dyn Fn(&[String], usize) -> bool) {
        match &e.kind {
            ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit | ExprKind::Path(..) | ExprKind::Break | ExprKind::Continue | ExprKind::Dollar => {}
            ExprKind::Borrow(_, inner) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out, d);
            }
            ExprKind::SliceOf(_, inner, lo, hi) => {
                if let Some(n) = root(inner) {
                    out.insert(n.to_string());
                }
                expr(inner, out, d);
                expr(lo, out, d);
                expr(hi, out, d);
            }
            ExprKind::Call(c, args) => {
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(n) = args.first().and_then(root) {
                            out.insert(n.to_string());
                        }
                    }
                }
                expr(c, out, d);
                for (j, a) in args.iter().enumerate() {
                    // `&x`/`&mut x` passed to a pure direct parameter
                    // (`Gen::pure_direct_ref_arg`), no other argument of the
                    // call naming `x`: no path is minted on `x`, the callee
                    // keeps none, and nothing else reaches `x` while the
                    // call's arguments are evaluated, so it pins nothing.
                    if let (ExprKind::Path(segs, _), ExprKind::Borrow(_, inner)) = (&c.kind, &a.kind) {
                        if let ExprKind::Path(xs, _) = &inner.kind {
                            if let [x] = xs.as_slice() {
                                if d(segs, j) && !args.iter().enumerate().any(|(k, o)| k != j && mentions(o, x)) {
                                    continue;
                                }
                            }
                        }
                    }
                    expr(a, out, d);
                }
            }
            ExprKind::Closure { captures, body, .. } => {
                out.extend(captures.iter().cloned());
                block(body, out, d);
            }
            ExprKind::StructLit(_, _, fields) => fields.iter().for_each(|(_, f)| expr(f, out, d)),
            ExprKind::ArrayLit(es) => es.iter().for_each(|x| expr(x, out, d)),
            ExprKind::ArrayRepeat(x, _) => expr(x, out, d),
            ExprKind::Unary(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => expr(a, out, d),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                expr(a, out, d);
                expr(b, out, d);
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, out, d),
            ExprKind::If(c, t, f) => {
                expr(c, out, d);
                block(t, out, d);
                if let Some(f) = f {
                    expr(f, out, d);
                }
            }
            ExprKind::While(c, b, step) => {
                expr(c, out, d);
                block(b, out, d);
                if let Some(st) = step {
                    expr(st, out, d);
                }
            }
            ExprKind::Match(s, arms) => {
                expr(s, out, d);
                arms.iter().for_each(|a| expr(&a.body, out, d));
            }
            ExprKind::Return(r) => {
                if let Some(r) = r {
                    expr(r, out, d);
                }
            }
        }
    }
    fn block(b: &Block, out: &mut HashSet<String>, d: &dyn Fn(&[String], usize) -> bool) {
        for s in &b.stmts {
            match s {
                Stmt::Let { init, .. } => {
                    if let Some(e) = init {
                        expr(e, out, d);
                    }
                }
                Stmt::Destructure { init, .. } => expr(init, out, d),
                Stmt::Expr(e) | Stmt::BlockLike(e) => expr(e, out, d),
            }
        }
        if let Some(t) = &b.tail {
            expr(t, out, d);
        }
    }
    let mut out = HashSet::new();
    block(b, &mut out, direct);
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
            ExprKind::ArrayRepeat(x, _) => scan(x, name, found),
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
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None)),
        ExprKind::Paren(inner) => is_bare_literal(inner),
        _ => false,
    }
}

fn int_lit_type(suf: Option<&str>, expected: Option<&Type>) -> IntTy {
    IntTy::of_literal(suf, expected).0
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

// D-0118: a bit field's C mask and the C expression of a `u128` constant.
fn c_bits_mask(backing: IntTy, w: u32) -> String {
    let m: u128 = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    if backing == IntTy::U128 {
        format!("(((cb_u128){}ull << 64) | (cb_u128){}ull)", (m >> 64) as u64, m as u64)
    } else {
        format!("(({}){}ull)", c_int_type(backing), m as u64)
    }
}

// D-0118: `(width, bit offset, backing type, field type)` of field `f` of
// a bitstruct declaration, or `None` when `sd` is an ordinary struct.
fn bitfield_layout(sd: &StructDecl, f: &str) -> Option<(u32, u32, IntTy, Type)> {
    let backing = sd.bits?;
    let mut off = 0u32;
    for fd in &sd.fields {
        let w = fd.width?;
        if fd.name == f {
            return Some((w, off, backing, fd.ty.clone()));
        }
        off += w;
    }
    None
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

// Whether `x : s` fits `d` (`[Narrow-Overflow]`'s premise), as a C condition.
fn narrow_cond(s: IntTy, d: IntTy, x: &str) -> String {
    if matches!(d, IntTy::U128) {
        if s.signed() { format!("{} >= 0", x) } else { "1".to_string() }
    } else if matches!(s, IntTy::U128) {
        let (_, max) = bounds(d);
        format!("(cb_u128){} <= (cb_u128){}", x, c_int(IntTy::U128, max))
    } else {
        let (min, max) = bounds(d);
        format!("(cb_i128){x} >= {} && (cb_i128){x} <= {}", c_int(IntTy::I128, min), c_int(IntTy::I128, max), x = x)
    }
}

fn narrow_check(s: IntTy, d: IntTy, x: &str, loc: &str) -> String {
    let dt = c_int_type(d);
    let cond = narrow_cond(s, d, x);
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

// Whether every use of the name `x` in `b` is `*x` read as a value
// (`Fx::foreach_alias`): not borrowed, assigned, passed as the reference,
// matched in place, captured, or declared again.
// `r` occurs in `b` only as the root of `r.f…` (fields, and indexes of
// array fields) or as `*r`, read or written whole, never under `&` or a
// slice, never handed on, matched on, captured, assigned or redeclared
// (`Gen::direct_ref_params`).
fn direct_uses(b: &Block, r: &str, callee_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&e.kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // Whether `r` is named anywhere in `e`, closures' bodies included.
    fn mentions(e: &Expr, r: &str) -> bool {
        let m = |x: &Expr| mentions(x, r);
        match &e.kind {
            ExprKind::Path(p, _) => p.len() == 1 && p[0] == r,
            ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) | ExprKind::ArrayRepeat(a, _) => m(a),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => m(a) || m(b),
            ExprKind::SliceOf(_, a, lo, hi) => m(a) || m(lo) || m(hi),
            ExprKind::Call(c, args) => m(c) || args.iter().any(m),
            ExprKind::StructLit(_, _, fs) => fs.iter().any(|(_, a)| m(a)),
            ExprKind::ArrayLit(es) => es.iter().any(m),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block_mentions(b, r),
            ExprKind::If(c, t, f) => m(c) || block_mentions(t, r) || f.as_ref().map_or(false, |f| m(f)),
            ExprKind::While(c, b, st) => m(c) || block_mentions(b, r) || st.as_ref().map_or(false, |x| m(x)),
            ExprKind::Match(sc, arms) => m(sc) || arms.iter().any(|a| m(&a.body)),
            ExprKind::Return(v) => v.as_ref().map_or(false, |v| m(v)),
            ExprKind::Closure { body, .. } => block_mentions(body, r),
            _ => false,
        }
    }
    fn block_mentions(b: &Block, r: &str) -> bool {
        b.stmts.iter().any(|s| match s {
            Stmt::Let { name, init, .. } => name == r || init.as_ref().map_or(false, |e| mentions(e, r)),
            Stmt::Destructure { fields, init, .. } => fields.iter().any(|f| f == r) || mentions(init, r),
            Stmt::Expr(e) | Stmt::BlockLike(e) => mentions(e, r),
        }) || b.tail.as_ref().map_or(false, |e| mentions(e, r))
    }
    // `proj`: `e` is the base of a field or index projection, or the
    // operand of `*`; `lent`: `e` is under `&` or a slice.
    fn ok(e: &Expr, r: &str, proj: bool, lent: bool, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
        let ok = |e: &Expr, r: &str, proj: bool, lent: bool| ok(e, r, proj, lent, c_ok);
        let block_ok = |b: &Block, r: &str| block_ok(b, r, c_ok);
        match &e.kind {
            // `clone(&p)`: for the plain data a direct parameter reaches,
            // the read of `p` it lowers to.
            ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Path(segs, t) if segs.len() == 1 && segs[0] == "clone" && t.is_empty()) && args.len() == 1 && matches!(&args[0].kind, ExprKind::Borrow(Mode::Shared, _)) => {
                let ExprKind::Borrow(_, place) = &args[0].kind else { unreachable!() };
                ok(place, r, false, false)
            }
            // `slice_len(r)`: the length the slice carries.
            ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Path(segs, _) if segs.len() == 1 && segs[0] == "slice_len") && args.len() == 1 && is_r(&args[0], r) => true,
            // `f(…, r, …)`: handed on whole (`Gen::direct_ref_params`).
            ExprKind::Call(c, args)
                if matches!(&c.kind, ExprKind::Path(segs, _) if args.iter().any(|a| is_r(a, r)) && args.iter().enumerate().all(|(i, a)| !is_r(a, r) || c_ok(segs, i))) =>
            {
                args.iter().all(|a| is_r(a, r) || ok(a, r, false, false))
            }
            ExprKind::Path(..) => !is_r(e, r) || (proj && !lent),
            ExprKind::Field(a, _) => ok(a, r, true, lent),
            ExprKind::Index(a, i) => ok(a, r, true, lent) && ok(i, r, false, false),
            ExprKind::Deref(a) => ok(a, r, true, lent),
            ExprKind::Borrow(_, a) => ok(a, r, false, true),
            ExprKind::SliceOf(_, a, lo, hi) => ok(a, r, false, true) && ok(lo, r, false, false) && ok(hi, r, false, false),
            ExprKind::Paren(a) => ok(a, r, proj, lent),
            ExprKind::Unary(_, a) | ExprKind::Propagate(a) => ok(a, r, false, lent),
            ExprKind::Binary(_, a, c) => ok(a, r, false, false) && ok(c, r, false, false),
            ExprKind::Assign(l, v) => ok(l, r, false, false) && ok(v, r, false, false),
            ExprKind::Call(c, args) => ok(c, r, false, false) && args.iter().all(|a| ok(a, r, false, false)),
            ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| ok(a, r, false, false)),
            ExprKind::ArrayLit(es) => es.iter().all(|a| ok(a, r, false, false)),
            ExprKind::ArrayRepeat(a, _) => ok(a, r, false, false),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block_ok(b, r),
            ExprKind::If(c, t, f) => ok(c, r, false, false) && block_ok(t, r) && f.as_ref().map_or(true, |f| ok(f, r, false, false)),
            ExprKind::While(c, b, st) => ok(c, r, false, false) && block_ok(b, r) && st.as_ref().map_or(true, |s| ok(s, r, false, false)),
            ExprKind::Match(sc, arms) => !mentions(sc, r) && arms.iter().all(|a| a.binder.as_deref() != Some(r) && ok(&a.body, r, false, false)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| ok(v, r, false, false)),
            ExprKind::Closure { .. } => !mentions(e, r),
            _ => !mentions(e, r),
        }
    }
    fn block_ok(b: &Block, r: &str, c_ok: &dyn Fn(&[String], usize) -> bool) -> bool {
        let ok = |e: &Expr, r: &str, proj: bool, lent: bool| ok(e, r, proj, lent, c_ok);
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r, false, false)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && ok(init, r, false, false),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r, false, false),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r, false, false))
    }
    block_ok(b, r, callee_ok)
}

// `r` is used in `b` only as (`Gen::direct_refs_of`, tree-design.md):
// - `r.f…` read as a value (fields of fields too),
// - `r.f… = e` with `e` not naming `r`,
// - the scrutinee `&r.f` / `&mut r.f` of a `match` (an `if`-pattern is
//   one), each of whose arms either binds nothing or uses its binder `b`
//   once, first, as `Box::get(b)` / `Box::get_mut(b)` handed as the first
//   argument of a named call whose other arguments are literals or
//   locals; the arms' bodies otherwise follow these same rules.
// Nothing else names `r`: it is never handed on, borrowed otherwise,
// dereferenced whole, captured, assigned or redeclared.
fn box_rec_uses(b: &Block, r: &str) -> bool {
    fn is_r(e: &Expr, r: &str) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty())
    }
    // `r.f…`.
    fn field_of_r(e: &Expr, r: &str) -> bool {
        match &strip_parens(e).kind {
            ExprKind::Field(a, _) => is_r(a, r) || field_of_r(a, r),
            _ => false,
        }
    }
    fn names(e: &Expr, r: &str) -> bool {
        let mut n = false;
        let blk = Block { stmts: Vec::new(), tail: Some(Box::new(e.clone())) };
        visit_exprs(&blk, &mut |x| {
            if is_r(x, r) {
                n = true;
            }
        });
        n
    }
    fn inert(e: &Expr) -> bool {
        matches!(&strip_parens(e).kind, ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::BoolLit(_)) || path_name(e).map_or(false, |n| !n.starts_with("__"))
    }
    fn arm_ok(a: &Arm, r: &str) -> bool {
        match &a.binder {
            None => ok(&a.body, r),
            Some(bn) => {
                // `f(Box::get[_mut](b), inert…)`, possibly as `x = x op …`
                // (`binder_box_call`), `b` used once, first.
                let g = binder_box_call(&a.body, bn);
                if !(binder_used_once_first(&a.body, bn) && matches!(g.as_deref(), Some("std::Box::get" | "std::Box::get_mut"))) {
                    return false;
                }
                // The outer call: `r` appears nowhere in the arm.
                !names(&a.body, r)
            }
        }
    }
    fn ok(e: &Expr, r: &str) -> bool {
        let e = strip_parens(e);
        match &e.kind {
            _ if !names(e, r) => true,
            ExprKind::Field(..) if field_of_r(e, r) => true,
            ExprKind::Assign(l, v) if field_of_r(l, r) => !names(v, r) || ok(v, r) && !names(v, r),
            ExprKind::Match(sc, arms) => {
                let scrut_ok = match &strip_parens(sc).kind {
                    ExprKind::Borrow(_, pl) => field_of_r(pl, r),
                    _ => !names(sc, r),
                };
                scrut_ok && arms.iter().all(|a| arm_ok(a, r))
            }
            ExprKind::Binary(_, a, c) => ok(a, r) && ok(c, r),
            ExprKind::Unary(_, a) => ok(a, r),
            ExprKind::Assign(l, v) => !names(l, r) && ok(v, r),
            ExprKind::Block(b) | ExprKind::Unsafe(b) => block(b, r),
            ExprKind::If(c, t, f) => ok(c, r) && block(t, r) && f.as_ref().map_or(true, |f| ok(f, r)),
            ExprKind::While(c, b, st) => ok(c, r) && block(b, r) && st.as_ref().map_or(true, |s| ok(s, r)),
            ExprKind::Return(v) => v.as_ref().map_or(true, |v| ok(v, r)),
            _ => false,
        }
    }
    fn block(b: &Block, r: &str) -> bool {
        b.stmts.iter().all(|s| match s {
            Stmt::Let { name, init, .. } => name != r && init.as_ref().map_or(true, |e| ok(e, r)),
            Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == r) && !names(init, r),
            Stmt::Expr(e) | Stmt::BlockLike(e) => ok(e, r),
        }) && b.tail.as_ref().map_or(true, |e| ok(e, r))
    }
    let _ = inert;
    block(b, r)
}

// `let $cN = &v;` opening a `foreach (x in &v)`'s expansion (`parse_foreach`)
// whose element binding `x` the body uses only as `*x` read as a value:
// the shape `Fx::foreach_alias` lowers to bounds-checked reads of `v`.
fn foreach_reads_only(name: &str, init: &Expr, b: &Block) -> bool {
    if !name.starts_with("$c") {
        return false;
    }
    let ExprKind::Borrow(mode, inner) = &init.kind else { return false };
    let mutable = *mode == Mode::Exclusive;
    if !matches!(&inner.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() && !segs[0].starts_with('$')) {
        return false;
    }
    let Some(tail) = &b.tail else { return false };
    let ExprKind::While(_, inner_b, _) = &tail.kind else { return false };
    let n = inner_b.stmts.len();
    if n < 2 {
        return false;
    }
    let (Stmt::Let { ty: None, name: x, init: Some(at), .. }, Stmt::BlockLike(body)) = (&inner_b.stmts[n - 2], &inner_b.stmts[n - 1]) else { return false };
    let ExprKind::Call(callee, args) = &at.kind else { return false };
    let holder_is = |e: &Expr| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == name);
    let at_fn = if mutable { "$each_at_mut" } else { "$each_at" };
    if !matches!(&callee.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == at_fn) || args.len() != 3 || !holder_is(&args[0]) {
        return false;
    }
    let ExprKind::Block(body) = &body.kind else { return false };
    block_only_derefs(body, x, mutable)
}

// `foreach (x in p)` written without `&`, `p` one of `params`:
// `{ auto $e = p; auto $d = $each_drain($e, 1); for (…) { auto x = $each_take($d, $i, 1); body } }`
// with `body` reading `x` only as `*x` or `x.f…` (`block_only_derefs`).
fn foreach_param_reads_only(name: &str, init: &Expr, b: &Block, params: &HashSet<String>) -> bool {
    if !name.starts_with("$e") {
        return false;
    }
    if !matches!(&init.kind, ExprKind::Path(segs, targs) if segs.len() == 1 && targs.is_empty() && params.contains(&segs[0])) {
        return false;
    }
    let is = |e: &Expr, n: &str| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == n);
    let holder = b.stmts.iter().find_map(|st| match st {
        Stmt::Let { name: d, init: Some(di), .. } if d.starts_with("$d") => match &di.kind {
            ExprKind::Call(c, a) if is(c, "$each_drain") && a.first().map_or(false, |a0| is(a0, name)) => Some(d.clone()),
            _ => None,
        },
        _ => None,
    });
    let Some(holder) = holder else { return false };
    let Some(tail) = &b.tail else { return false };
    let ExprKind::While(_, inner_b, _) = &tail.kind else { return false };
    let [Stmt::Let { ty: None, name: x, init: Some(at), .. }, Stmt::BlockLike(body)] = inner_b.stmts.as_slice() else { return false };
    let ExprKind::Call(callee, args) = &at.kind else { return false };
    if !is(callee, "$each_take") || args.len() != 3 || !is(&args[0], &holder) {
        return false;
    }
    let ExprKind::Block(body) = &body.kind else { return false };
    block_only_derefs(body, x, false)
}

// `x` used only as `*x` read as a value; with `w`, also the whole place written,
// `*x = e` (which `*x op= e` is): `x` used only as `*x`, read or assigned.
fn block_only_derefs(b: &Block, x: &str, w: bool) -> bool {
    b.stmts.iter().all(|s| match s {
        Stmt::Let { name, init, .. } => name != x && init.as_ref().map_or(true, |e| only_reads_w(e, x, false, w)),
        Stmt::Destructure { fields, init, .. } => !fields.iter().any(|f| f == x) && only_reads_w(init, x, true, w),
        Stmt::Expr(e) | Stmt::BlockLike(e) => only_reads_w(e, x, false, w),
    }) && b.tail.as_ref().map_or(true, |e| only_reads_w(e, x, false, w))
}

fn only_reads_w(e: &Expr, x: &str, place: bool, w: bool) -> bool {
    let only_reads = |e: &Expr, x: &str, place: bool| only_reads_w(e, x, place, w);
    let block_only_reads = |b: &Block, x: &str| block_only_derefs(b, x, w);
    let is_x = |e: &Expr| matches!(&e.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == x);
    match &e.kind {
        ExprKind::Path(..) => !is_x(e),
        ExprKind::Deref(inner) if is_x(inner) => !place,
        ExprKind::Deref(a) | ExprKind::Paren(a) | ExprKind::Unary(_, a) | ExprKind::Propagate(a) => only_reads(a, x, place),
        // `x.f…` read as a value: `(*x).f…`, a field of the element read
        // (`Fx::alias_field_chain`).
        ExprKind::Field(..) if !place && field_chain_root(e).map_or(false, is_x) => true,
        ExprKind::Field(a, _) => only_reads(a, x, place),
        ExprKind::Borrow(_, a) => only_reads(a, x, true),
        ExprKind::Assign(l, r) if w && matches!(&strip_parens(l).kind, ExprKind::Deref(t) if is_x(t)) => only_reads(r, x, false),
        ExprKind::Assign(l, r) => only_reads(l, x, true) && only_reads(r, x, false),
        ExprKind::Index(a, i) => only_reads(a, x, true) && only_reads(i, x, false),
        ExprKind::SliceOf(_, a, lo, hi) => only_reads(a, x, true) && only_reads(lo, x, false) && only_reads(hi, x, false),
        ExprKind::Binary(_, a, b) => only_reads(a, x, false) && only_reads(b, x, false),
        ExprKind::Call(c, args) => {
            // `drop(e)` takes its operand in place position; every other
            // call takes values.
            let in_place = matches!(&c.kind, ExprKind::Path(p, _) if p.len() == 1 && p[0] == "drop");
            only_reads(c, x, false) && args.iter().all(|a| only_reads(a, x, in_place))
        }
        ExprKind::StructLit(_, _, fs) => fs.iter().all(|(_, a)| only_reads(a, x, false)),
        ExprKind::ArrayLit(es) => es.iter().all(|a| only_reads(a, x, false)),
        ExprKind::ArrayRepeat(a, _) => only_reads(a, x, false),
        ExprKind::Block(b) | ExprKind::Unsafe(b) => block_only_reads(b, x),
        ExprKind::If(c, t, f) => only_reads(c, x, false) && block_only_reads(t, x) && f.as_ref().map_or(true, |f| only_reads(f, x, false)),
        ExprKind::While(c, b, st) => only_reads(c, x, false) && block_only_reads(b, x) && st.as_ref().map_or(true, |s| only_reads(s, x, false)),
        ExprKind::Match(sc, arms) => only_reads(sc, x, true) && arms.iter().all(|a| a.binder.as_deref() != Some(x) && only_reads(&a.body, x, false)),
        ExprKind::Return(v) => v.as_ref().map_or(true, |v| only_reads(v, x, false)),
        ExprKind::Closure { .. } => false,
        _ => true,
    }
}


// The root of a chain of field accesses (`a.b.c` -> `a`), through parentheses.
fn field_chain_root(e: &Expr) -> Option<&Expr> {
    let mut a = e;
    while let ExprKind::Field(b, _) | ExprKind::Paren(b) = &a.kind {
        a = b;
    }
    if std::ptr::eq(a, e) {
        None
    } else {
        Some(a)
    }
}

// Argument `j` of a call being lowered: appended in order, or put in the
// slot a deferred literal held (D-0079).
fn put_arg(argv: &mut Vec<V>, j: usize, v: V) {
    if j < argv.len() {
        argv[j] = v;
    } else {
        argv.push(v);
    }
}

// An unsuffixed numeric literal, possibly negated or parenthesized.
fn is_bare_num_literal(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => is_bare_num_literal(inner),
        ExprKind::Paren(inner) => is_bare_num_literal(inner),
        _ => false,
    }
}

// D-0110: the referent of a key reference that is a struct or enum.
fn composite_key(t: &Type) -> Option<Type> {
    match t {
        Type::Ref(inner, _) if matches!(&**inner, Type::Named(n, _) if n != "std::String") => Some((**inner).clone()),
        _ => None,
    }
}

// D-0109: the mode of the references a pattern looks through: shared
// if any is.
fn crossing_mode(crosses: &[(usize, Mode, Type)]) -> Option<Mode> {
    if crosses.is_empty() {
        None
    } else if crosses.iter().any(|c| c.1 == Mode::Shared) {
        Some(Mode::Shared)
    } else {
        Some(Mode::Exclusive)
    }
}

// D-0108: an expression `lower_place` reaches without a temporary: a
// binding, `*r`, or a field or element of one.
fn is_place_syntax(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(segs, _) => segs.len() == 1,
        ExprKind::Deref(_) => true,
        ExprKind::Field(b, _) | ExprKind::Index(b, _) | ExprKind::Paren(b) => is_place_syntax(b),
        _ => false,
    }
}

// A place written as such: a binding, a field, an element, a dereference.
fn is_place_expr(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Path(segs, t) => segs.len() == 1 && t.is_empty(),
        ExprKind::Field(a, _) | ExprKind::Paren(a) => is_place_expr(a),
        ExprKind::Index(a, _) => is_place_expr(a),
        ExprKind::Deref(_) => true,
        _ => false,
    }
}

// Whether the place's root name is a binding (not a constant or an item).
fn root_bound(e: &Expr, bound: &dyn Fn(&str) -> bool) -> bool {
    match &e.kind {
        ExprKind::Path(segs, _) => segs.len() == 1 && bound(&segs[0]),
        ExprKind::Field(a, _) | ExprKind::Paren(a) | ExprKind::Index(a, _) => root_bound(a, bound),
        ExprKind::Deref(_) => true,
        _ => false,
    }
}

// `r` never the base of a projection or the operand of `*`: used, if at
// all, whole (`Gen::direct_refs_of`'s `Vec` case; `direct_uses` decides
// where a whole use may go).
fn only_handed_on(b: &Block, r: &str) -> bool {
    let is_r = |e: &Expr| matches!(&strip_parens(e).kind, ExprKind::Path(p, t) if p.len() == 1 && p[0] == r && t.is_empty());
    let mut ok = true;
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Field(a, _) | ExprKind::Index(a, _) | ExprKind::Deref(a) if is_r(a) => ok = false,
        ExprKind::SliceOf(_, a, _, _) if is_r(a) => ok = false,
        _ => {}
    });
    ok
}

// Whether `name` is named anywhere in `b` (closures' bodies included).
fn block_names(b: &Block, name: &str) -> bool {
    let mut found = false;
    visit_exprs(b, &mut |e| {
        if matches!(&e.kind, ExprKind::Path(s, _) if s.len() == 1 && s[0] == name) {
            found = true;
        }
    });
    found
}

// Every statement in `b`, nested blocks' and closures' included.
fn visit_stmts(b: &Block, f: &mut dyn FnMut(&Stmt)) {
    fn walk_b(b: &Block, f: &mut dyn FnMut(&Stmt)) {
        for st in &b.stmts {
            f(st);
        }
    }
    walk_b(b, f);
    let mut blocks: Vec<*const Block> = Vec::new();
    visit_exprs(b, &mut |e| match &e.kind {
        ExprKind::Block(x) | ExprKind::Unsafe(x) => blocks.push(x as *const Block),
        ExprKind::Closure { body, .. } => blocks.push(&**body as *const Block),
        ExprKind::If(_, x, _) | ExprKind::While(_, x, _) => blocks.push(x as *const Block),
        _ => {}
    });
    for x in blocks {
        walk_b(unsafe { &*x }, f);
    }
}
