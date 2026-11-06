// D-0257: the C header of a library's entry points (`cobc --lib`): each
// ffi-struct they use as C declares it, with its D-0250 layout asserted,
// the library's own API, and each entry point's prototype with its
// `_try` form. The driver adds the file's guard and the link line.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn header_text(&mut self, entries: &[(String, std::sync::Arc<FnDecl>)]) -> R<String> {
        let mut h = Header { defs: String::new(), done: HashSet::new(), fwd: HashSet::new() };
        let mut protos = String::new();
        for (_, f) in entries {
            let mut params = Vec::new();
            for p in &f.params {
                let (base, dims) = self.hspell(&p.ty, &mut h)?;
                params.push(format!("{}{}{}{}", base, if base.ends_with('*') { "" } else { " " }, hsan(&p.name), dims));
            }
            let plist = if params.is_empty() { "void".to_string() } else { params.join(", ") };
            if f.ret == Type::Void {
                writeln!(protos, "void {}({});", f.name, plist).unwrap();
                writeln!(protos, "int {}_try({});", f.name, plist).unwrap();
            } else {
                let (rc, _) = self.hspell(&f.ret, &mut h)?;
                writeln!(protos, "{} {}({});", rc, f.name, plist).unwrap();
                // The result parameter's name must not be one of the entry's own.
                let mut rn = "result".to_string();
                while f.params.iter().any(|p| hsan(&p.name) == rn) {
                    rn.push('_');
                }
                let mut t = params.clone();
                t.push(format!("{} *{}", rc, rn));
                writeln!(protos, "int {}_try({});", f.name, t.join(", ")).unwrap();
            }
        }
        let mut out = String::new();
        out.push_str("#include <stdint.h>\n#include <stddef.h>\n\n");
        if !h.defs.is_empty() {
            out.push_str("/* The structs that cross by value, laid out as CobaltC lays them out (D-0250). */\n");
            out.push_str(&h.defs);
            out.push('\n');
        }
        out.push_str("/* The library: started by its first call, or here; its buffered output written out; the last fault a _try call contained (its report into buf, at most n - 1 bytes; the report's length, or 0 for none). */\n");
        out.push_str("void cobalt_init(void);\nvoid cobalt_shutdown(void);\nint cobalt_last_fault(char *buf, size_t n);\n\n");
        out.push_str("/* The entry points. A fault inside name(...) reports and ends the process; name_try(...) contains it: 0 and the result through its last argument, or 1 with the result untouched. */\n");
        out.push_str(&protos);
        Ok(out)
    }

    // A type's C spelling for the header: the base type and, for an array,
    // the dimensions that follow the name (`uint8_t f[2][4]`). A struct is
    // defined on first sight, after the structs its fields need.
    fn hspell(&mut self, t: &Type, h: &mut Header) -> R<(String, String)> {
        Ok(match t {
            Type::Int(it) => (c_int_type(*it).to_string(), String::new()),
            Type::F32 => ("float".to_string(), String::new()),
            Type::F64 => ("double".to_string(), String::new()),
            Type::Bool => ("uint8_t".to_string(), String::new()),
            Type::Void => ("void".to_string(), String::new()),
            Type::Rawptr(inner) => match &**inner {
                Type::Void => ("void *".to_string(), String::new()),
                Type::Named(..) => {
                    let n = hname(inner);
                    if !h.done.contains(&n) && h.fwd.insert(n.clone()) {
                        writeln!(h.defs, "struct {};", n).unwrap();
                    }
                    (format!("struct {} *", n), String::new())
                }
                other => {
                    let (b, d) = self.hspell(other, h)?;
                    if !d.is_empty() {
                        return unsupported("a raw pointer to an array in a C-callable signature");
                    }
                    (format!("{} *", b), String::new())
                }
            },
            Type::Array(e, n) => {
                let (b, d) = self.hspell(e, h)?;
                (b, format!("[{}]{}", n, d))
            }
            Type::Named(name, args) => {
                let hn = hname(t);
                if !h.done.contains(&hn) {
                    let Some(decl) = self.items.structs.get(name).cloned() else { return unsupported("a C-callable signature's type is no struct") };
                    let subst: HashMap<String, Type> = decl.type_params.iter().cloned().zip(args.iter().cloned()).collect();
                    let mut body = String::new();
                    for fd in &decl.fields {
                        let ft = hsubst(&fd.ty, &subst);
                        let (b, d) = self.hspell(&ft, h)?;
                        writeln!(body, "    {}{}{}{};", b, if b.ends_with('*') { "" } else { " " }, hsan(&fd.name), d).unwrap();
                    }
                    let (size, align) = self.size_align(t)?;
                    let attr = decl.align.map(|a| format!(" __attribute__((aligned({})))", a)).unwrap_or_default();
                    writeln!(h.defs, "struct {} {{\n{}}}{};", hn, body, attr).unwrap();
                    writeln!(h.defs, "_Static_assert(sizeof(struct {0}) == {1} && _Alignof(struct {0}) == {2}, \"layout of {0}\");", hn, size, align).unwrap();
                    h.done.insert(hn.clone());
                }
                (format!("struct {}", hn), String::new())
            }
            _ => return unsupported("a type in a C-callable signature"),
        })
    }
}

struct Header {
    defs: String,
    done: HashSet<String>,
    fwd: HashSet<String>,
}

// A struct's name in the header: its qualified name with `::` as `__`
// (`Point`, `geo__Point`), and its type arguments as cobc mangles them.
fn hname(t: &Type) -> String {
    mangle(t)
}

// A field or parameter name C accepts.
fn hsan(s: &str) -> String {
    san(s)
}

fn hsubst(t: &Type, subst: &HashMap<String, Type>) -> Type {
    match t {
        Type::Named(n, a) if a.is_empty() => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
        Type::Named(n, a) => Type::Named(n.clone(), a.iter().map(|x| hsubst(x, subst)).collect()),
        Type::Array(e, n) => Type::Array(Box::new(hsubst(e, subst)), *n),
        Type::Rawptr(e) => Type::Rawptr(Box::new(hsubst(e, subst))),
        Type::Simd(e, n) => Type::Simd(Box::new(hsubst(e, subst)), *n),
        other => other.clone(),
    }
}
