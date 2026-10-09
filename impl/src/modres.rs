// Module visibility and name resolution (spec/17-modules.md), enforced as
// a static, purely-lexical pass -- `rule.module.visibility` says exactly
// that ("Purely lexical; always disposition: rejected"), so unlike most of
// this interpreter's checks, there is deliberately no dynamic fallback
// here: if this pass doesn't catch a violation, nothing else in the
// pipeline ever will.
//
// Design: run once, right after parsing and before `build_items`/
// typecheck/eval ever see the program. Walks the whole AST, in module-
// nesting order, and:
//   1. builds a module tree recording, per item (fn/extern/struct/enum/
//      module), its declaring module, its `export` flag, and its fully
//      qualified name (`spec/17` §1: "M::name" from the root);
//   2. re-walks every function body, struct field type, enum variant
//      payload type, extern signature, and `import` target, resolving
//      each item reference against `rule.module.resolve`'s actual
//      resolution order (`[Resolve-Unqualified]`/`[Resolve-Qualified]`)
//      relative to the module it lexically occurs in, and rewrites it in
//      place to the resolved fully-qualified key.
//
// Conservative by design: a reference this pass cannot confidently
// resolve is left untouched rather than force-rejected, so the existing
// (pre-this-pass) `diag.unbound-name` checks in `typecheck.rs`/`interp.rs`
// remain the authority for "does this name exist at all" -- this pass
// only ever *adds* two checks nothing else in the pipeline could ever
// perform: `diag.name-not-visible` and `diag.ambiguous-name`. A name it
// successfully resolves is rewritten to its qualified key, which is safe
// unconditionally (that key names the item that reference denotes).
//
// A qualified path is accepted in *type* position too (`geom::Point p`,
// `ref<geom::Point, shared>`): `parse_type` collects the segments into
// one `Type::Named` name and `resolve_type` resolves it with
// `resolve_qualified`.
//
// Two different enums that are both visible from the same reference
// site and share a variant name are detected as ambiguous *by this pass*
// (`diag.ambiguous-name`, matching `[Resolve-Ambiguous]`'s clause-4 case)
// for a *bare* variant reference; a qualified `E::V` reference names its
// enum by the path prefix in both the evaluator and the static pass
// (`variant_enum`), so the flat `enum_of_variant` table (which keeps
// this pass) remains a single flat table keyed by bare variant name --
// pre-existing, not introduced here. Qualified `Enum::Variant` references
// have their `Enum` segment fully resolved and visibility-checked by this
// pass; the trailing `Variant` segment still dispatches through that same
// flat table.

use crate::ast::*;
use crate::fxhash::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Fn,
    Extern,
    Struct,
    Enum,
    Module,
}

struct ModuleNode {
    parent: Option<String>,
    // simple (unqualified, possibly "Type::name"-compound) name -> (qualified key, export, kind)
    items: HashMap<String, (String, bool, Kind)>,
    imports: Vec<(Vec<String>, usize)>, // segments, declaring line
    // The `export import p;` among them (D-0136): what they bring in is
    // entered in `items` as exported aliases of the items themselves.
    reexports: Vec<(Vec<String>, usize)>,
}

// The same `"diag.xxx@N"` tagging `typecheck.rs`/`interp.rs` use; a
// no-op on an already-tagged string so the innermost site wins.
fn tag(d: String, line: usize) -> String {
    if d.contains('@') {
        d
    } else {
        format!("{d}@{line}")
    }
}

pub struct Resolver {
    modules: HashMap<String, ModuleNode>, // "" = root
    // qualified item key -> (export, kind, declaring module key)
    all_items: HashMap<String, (bool, Kind, String)>,
    // enum qualified key -> its variants' bare names
    enum_variants: HashMap<String, HashSet<String>>,
    // module key -> what each of its `import p;` resolved to, filled once
    // every import has been validated (`rule.module.use`, CHG-0033).
    // Empty while `build` validates the imports themselves, so resolving
    // an import never consults another module import's contents.
    import_targets: HashMap<String, Vec<(Vec<String>, String, Kind)>>,
    // Qualified keys of the `const` items (D-0036): a path resolving to
    // one is rewritten to a call of it.
    consts: HashSet<String>,
}

// A diagnostic with a message naming what it is about (D-0072).
pub fn named(id: &str, msg: String) -> String {
    format!("{}{}{}", id, crate::diagnostics::DETAIL_SEP, msg)
}

// An intrinsic's name (`spec/21` §0); a `$`-name is internal, never an item.
pub fn is_intrinsic_name(name: &str) -> bool {
    !name.contains("::") && !name.starts_with('$') && crate::typecheck::INTRINSIC_NAMES.contains(&name)
}

pub fn qualify(path: &[String], simple: &str) -> String {
    // D-0136: an item of one of `std`'s submodules is keyed by its short
    // path, `std::Vec` rather than `std::collections::Vec`, the path by
    // which `std` re-exports it. One key per item, and every name the
    // implementations write for a `std` item, and every message, stays
    // as it was before `std` was divided; a submodule's own submodules
    // (D-0187, `std::crypto::digest`) key their items the same way. The
    // exception is `std::extensions` (D-0187): its items are an
    // implementation's, never re-exported by `std`, and keep their full
    // path, so they can never share a key with an item of `std`.
    if path.len() > 1 && path[0] == "std" && path[1] != "extensions" {
        return format!("std::{}", simple);
    }
    if path.is_empty() {
        // D-0055: a root item named as an intrinsic is keyed apart from the
        // intrinsic, whose name stays free for the code that reaches it (a
        // root key is otherwise the bare name).
        if is_intrinsic_name(simple) {
            return format!("{}$", simple);
        }
        simple.to_string()
    } else {
        format!("{}::{}", path.join("::"), simple)
    }
}

// A module's key: its full path (`std::crypto::digest`), which is how
// the resolver's module table names it; a module is not an item a
// message names, so `qualify`'s short `std` keys do not apply.
pub fn module_key(path: &[String], name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", path.join("::"), name)
    }
}

impl Resolver {
    fn build(program: &Program) -> Result<Resolver, String> {
        let mut modules = HashMap::default();
        modules.insert(
            String::new(),
            ModuleNode { parent: None, items: HashMap::default(), imports: Vec::new(), reexports: Vec::new() },
        );
        let mut all_items = HashMap::default();
        let mut enum_variants = HashMap::default();
        let mut consts = HashSet::default();

        fn walk(
            items: &[Item],
            path: &mut Vec<String>,
            modules: &mut HashMap<String, ModuleNode>,
            all_items: &mut HashMap<String, (bool, Kind, String)>,
            enum_variants: &mut HashMap<String, HashSet<String>>,
            assoc: &mut Vec<(String, String, usize)>,
            consts: &mut HashSet<String>,
        ) {
            let here = path.join("::");
            for it in items {
                match it {
                    Item::Fn(f) => {
                        let simple = match &f.assoc_type {
                            Some(t) => {
                                assoc.push((here.clone(), t.clone(), f.line));
                                format!("{}::{}", t, f.name)
                            }
                            None => f.name.clone(),
                        };
                        let qk = qualify(path, &simple);
                        if f.is_const {
                            consts.insert(qk.clone());
                        }
                        modules.get_mut(&here).unwrap().items.insert(simple, (qk.clone(), f.export, Kind::Fn));
                        all_items.insert(qk, (f.export, Kind::Fn, here.clone()));
                    }
                    Item::Extern(e) => {
                        let qk = qualify(path, &e.name);
                        modules.get_mut(&here).unwrap().items.insert(e.name.clone(), (qk.clone(), e.export, Kind::Extern));
                        all_items.insert(qk, (e.export, Kind::Extern, here.clone()));
                    }
                    Item::Struct(s) => {
                        let qk = qualify(path, &s.name);
                        modules.get_mut(&here).unwrap().items.insert(s.name.clone(), (qk.clone(), s.export, Kind::Struct));
                        all_items.insert(qk, (s.export, Kind::Struct, here.clone()));
                    }
                    Item::Enum(e) => {
                        let qk = qualify(path, &e.name);
                        modules.get_mut(&here).unwrap().items.insert(e.name.clone(), (qk.clone(), e.export, Kind::Enum));
                        all_items.insert(qk.clone(), (e.export, Kind::Enum, here.clone()));
                        let vs: HashSet<String> = e.variants.iter().map(|v| v.name.clone()).collect();
                        enum_variants.insert(qk, vs);
                    }
                    Item::Module(export, name, inner, _) => {
                        let qk = module_key(path, name);
                        modules.get_mut(&here).unwrap().items.insert(name.clone(), (qk.clone(), *export, Kind::Module));
                        all_items.insert(qk.clone(), (*export, Kind::Module, here.clone()));
                        modules.insert(qk.clone(), ModuleNode { parent: Some(here.clone()), items: HashMap::default(), imports: Vec::new(), reexports: Vec::new() });
                        path.push(name.clone());
                        walk(inner, path, modules, all_items, enum_variants, assoc, consts);
                        path.pop();
                    }
                    Item::Import(export, raw, line) => {
                        let segs: Vec<String> = raw.split("::").map(|s| s.to_string()).collect();
                        let node = modules.get_mut(&here).unwrap();
                        if *export {
                            node.reexports.push((segs.clone(), *line));
                        }
                        node.imports.push((segs, *line));
                    }
                    Item::ExternCode(..) => {}
                }
            }
        }
        let mut assoc = Vec::new();
        walk(&program.items, &mut Vec::new(), &mut modules, &mut all_items, &mut enum_variants, &mut assoc, &mut consts);

        // `std`'s exported functions that both implementations realize
        // natively (`spec/21` §0's latitude) have no parsed declaration:
        // each is entered here, in the submodule that declares it in the
        // specification, where a declaration would be.
        if modules.contains_key("std") {
            let mut native = |module: &str, simple: &str, export: bool| {
                let module = if modules.contains_key(module) { module } else { "std" };
                let key = format!("std::{}", simple);
                modules.get_mut(module).unwrap().items.insert(simple.to_string(), (key.clone(), export, Kind::Fn));
                all_items.insert(key, (export, Kind::Fn, module.to_string()));
            };
            native("std::core", "Result::map_err", true);
            // `std::print<T>` (`rule.stdlib.print`), private to `std` since
            // D-0039: programs print with `printf`, which it is built on.
            native("std", "print", false);
            // `printf`, `sprintf`, `eprintf` and `String::appendf`
            // (`rule.stdlib.format`, D-0038): each call is rewritten below
            // to `$fmt`, `std`'s formatter.
            native("std::io", "printf", true);
            native("std::text", "sprintf", true);
            native("std::io", "eprintf", true);
            native("std::text", "String::appendf", true);
            // `File::printf` (D-0059): formatted text to a `File`.
            native("std::io", "File::printf", true);
            // `TcpStream::printf` (D-0144): formatted text to a connection.
            native("std::net", "TcpStream::printf", true);
            // `assert` (D-0065): rewritten below to a test and `$assert_fail`.
            native("std::core", "assert", true);
            // `exit` (D-0150): ends the program; no CobaltC body can.
            native("std::process", "exit", true);
            // The floating-point functions (`rule.arith.float-fns`, D-0136):
            // `std::math`'s, each typed for `f32` and `f64` as an intrinsic is.
            for f in crate::typecheck::FLOAT_FNS.iter().chain(crate::typecheck::FLOAT2_FNS) {
                native("std::math", f.trim_start_matches("std::"), true);
            }
        }

        // `[Assoc-Fn-Foreign-Type]` (CHG-0033): `fn T::name` in module `M`
        // adds `M::T::name` only where `T` is a struct or enum declared in
        // `M` itself (`spec/17` §1); anywhere else it would be a function
        // merely spelled like a method of some other module's type.
        for (m, t, line) in &assoc {
            let declared_here = matches!(modules[m].items.get(t), Some((_, _, Kind::Struct | Kind::Enum)));
            if !declared_here {
                return Err(tag("diag.foreign-associated-fn".to_string(), *line));
            }
        }

        let mut r = Resolver { modules, all_items, enum_variants, import_targets: HashMap::default(), consts };
        r.reexport()?;

        // Validate every `import` target once, up front: `rule.module.use`
        // requires the target itself resolve (as a qualified path, with
        // visibility checked on its own last hop) from the module that
        // declares the `import`.
        let mut targets: HashMap<String, Vec<(Vec<String>, String, Kind)>> = HashMap::default();
        for (mpath, node) in &r.modules {
            for (imp, line) in &node.imports {
                match r.resolve_qualified(imp, mpath).map_err(|d| tag(d, *line))? {
                    Some((qk, kind)) => targets.entry(mpath.clone()).or_default().push((imp.clone(), qk, kind)),
                    None => return Err(tag(named("diag.unbound-name", format!("`import {}` names nothing", imp.join("::"))), *line)),
                }
            }
        }
        r.import_targets = targets;
        Ok(r)
    }

    /// `rule.module.use`'s re-export (D-0136): each `export import p;` in
    /// `M` enters what `p` brings in -- each exported item of the module
    /// `p` names, or the one item `p` names -- in `M`'s items under its
    /// own name, exported, as an alias of the item itself. Repeated until
    /// nothing changes, so re-exports chain, and a cycle ends: an alias
    /// already present for the same item adds nothing. A name `M` already
    /// has for a different item is `[Item-Duplicate]`.
    fn reexport(&mut self) -> Result<(), String> {
        let mut order: Vec<String> = self.modules.iter().filter(|(_, n)| !n.reexports.is_empty()).map(|(k, _)| k.clone()).collect();
        order.sort();
        loop {
            let mut changed = false;
            // A path that names nothing yet may name an alias a later
            // re-export adds; it names nothing only once none is added.
            let mut unresolved: Option<(Vec<String>, usize)> = None;
            for m in &order {
                for (p, line) in self.modules[m].reexports.clone() {
                    let brought: Vec<(String, (String, bool, Kind))> = match self.resolve_qualified(&p, m).map_err(|d| tag(d, line))? {
                        Some((qk, Kind::Module)) => self.modules[&qk].items.iter().filter(|(_, (_, exp, _))| *exp).map(|(n, v)| (n.clone(), v.clone())).collect(),
                        Some((qk, kind)) => vec![(p.last().unwrap().clone(), (qk, true, kind))],
                        None => {
                            unresolved.get_or_insert((p, line));
                            continue;
                        }
                    };
                    for (name, (qk, _, kind)) in brought {
                        let node = self.modules.get_mut(m).unwrap();
                        match node.items.get(&name) {
                            Some((have, _, _)) if *have == qk => {}
                            Some((have, _, _)) => {
                                let shown = |k: &str| k.trim_end_matches('$').to_string();
                                return Err(tag(
                                    named("diag.duplicate-item", format!("`export import {}` brings in `{}`, but `{}` already names `{}` here", p.join("::"), shown(&qk), name, shown(have))),
                                    line,
                                ));
                            }
                            None => {
                                node.items.insert(name, (qk, true, kind));
                                changed = true;
                            }
                        }
                    }
                }
            }
            if !changed {
                return match unresolved {
                    Some((p, line)) => Err(tag(named("diag.unbound-name", format!("`export import {}` names nothing", p.join("::"))), line)),
                    None => Ok(()),
                };
            }
        }
    }

    fn visible(&self, decl_module: &str, export: bool, from_module: &str) -> bool {
        if export {
            return true;
        }
        // `std`'s submodules share one privacy (`spec/21` §0, D-0136): a
        // private item of any of them is visible in all of `std`, as the
        // fields of its types are (`typecheck`'s `declaring_module`). No
        // program is inside `std`, so none can tell.
        let decl_module = if decl_module.starts_with("std::") { "std" } else { decl_module };
        // "M is M' or nested inside M'" (spec/17 §2): from_module is
        // decl_module itself, or decl_module is an ancestor of from_module.
        let mut cur = Some(from_module.to_string());
        while let Some(m) = cur {
            if m == decl_module {
                return true;
            }
            cur = self.modules.get(&m).and_then(|n| n.parent.clone());
        }
        false
    }

    /// `[Resolve-Unqualified]`. No `visible(...)` premise -- the rule cites
    /// none for any of its four clauses; only `[Resolve-Qualified]`'s final
    /// hop is visibility-checked. Returns `Ok(None)` when nothing yields an
    /// item (the caller decides what that means -- for a bare identifier
    /// that might still be a local binding, "not found" is not itself an
    /// error here).
    fn resolve_unqualified(&self, name: &str, from_module: &str) -> Result<Option<(String, Kind)>, String> {
        // clause (1): from_module's own items.
        if let Some((qk, _exp, kind)) = self.modules[from_module].items.get(name) {
            return Ok(Some((qk.clone(), *kind)));
        }
        // clause (2): an `import p;` in from_module whose last segment is `name`.
        // Once `build` has resolved every import, their targets are read
        // back rather than re-resolved. While it is still resolving them,
        // an import whose whole path is this one name (`import std;`) is
        // not consulted: that would be resolving the import through itself.
        let mut import_hits: Vec<(String, Kind)> = Vec::new();
        if let Some(ts) = self.import_targets.get(from_module) {
            for (imp, qk, kind) in ts {
                if imp.last().map(|s| s.as_str()) == Some(name) && !import_hits.iter().any(|(k, _)| k == qk) {
                    import_hits.push((qk.clone(), *kind));
                }
            }
        } else if self.import_targets.is_empty() {
            for (imp, _) in &self.modules[from_module].imports {
                if imp.last().map(|s| s.as_str()) == Some(name) && imp.len() > 1 {
                    if let Some(hit) = self.resolve_qualified(imp, from_module)? {
                        if !import_hits.contains(&hit) {
                            import_hits.push(hit);
                        }
                    }
                }
            }
        }
        if import_hits.len() > 1 {
            let names: Vec<String> = import_hits.iter().map(|(k, _)| format!("`{}`", k)).collect();
            return Err(named("diag.ambiguous-name", format!("`{}` names more than one imported item: {}; write the one you mean in full", name, names.join(" and "))));
        }
        if let Some(hit) = import_hits.into_iter().next() {
            return Ok(Some(hit));
        }
        // clause (2b), CHG-0033: an exported item `name` of a module that an
        // `import p;` in from_module names. Only where the name is used is
        // a clash between two imported modules an ambiguity, so a module
        // gaining an export never breaks a program that does not use it.
        let mut module_hits: Vec<(String, Kind)> = Vec::new();
        for (m, _) in self.imported(from_module, Kind::Module) {
            // A module's submodules are not brought in (D-0136): they are
            // reached by path or by an import of their own, so a library's
            // submodule never hides a module of the importing program.
            if let Some((qk, true, kind)) = self.modules.get(&m).and_then(|n| n.items.get(name)).filter(|(_, _, k)| *k != Kind::Module) {
                if !module_hits.iter().any(|(k, _)| k == qk) {
                    module_hits.push((qk.clone(), *kind));
                }
            }
        }
        if module_hits.len() > 1 {
            let names: Vec<String> = module_hits.iter().map(|(k, _)| format!("`{}`", k)).collect();
            return Err(named("diag.ambiguous-name", format!("`{}` is exported by more than one imported module: {}; qualify it", name, names.join(" and "))));
        }
        if let Some(hit) = module_hits.into_iter().next() {
            return Ok(Some(hit));
        }
        // clause (3): nearest enclosing module of from_module, repeating outward to the root.
        // D-0055: never for an intrinsic's name -- an item hides an intrinsic only in its own
        // module and where an import names it, so a program's `fn sizeof` does not reach into
        // `std`'s code, or into a nested (inline or file-backed) module's.
        let mut cur = if is_intrinsic_name(name) { None } else { self.modules[from_module].parent.clone() };
        while let Some(m) = cur {
            if let Some((qk, _exp, kind)) = self.modules[&m].items.get(name) {
                return Ok(Some((qk.clone(), *kind)));
            }
            cur = self.modules[&m].parent.clone();
        }
        // clauses (4) and (4b) (CHG-0041): `E::name` for exactly one enum E
        // declared in M, else in the nearest enclosing module that declares
        // one (as clause 3 finds items); only when there is none, for
        // exactly one enum brought in by one of M's imports. Imports are
        // read from the targets `build` resolved once, never re-resolved
        // here, which is what keeps a lookup from resolving an import
        // through itself.
        let mut own: Vec<String> = Vec::new();
        let mut cur = Some(from_module.to_string());
        while let Some(m) = cur {
            for (simple, (qk, _exp, kind)) in &self.modules[&m].items {
                if *kind == Kind::Enum && !simple.contains("::") {
                    if self.enum_variants.get(qk).map_or(false, |vs| vs.contains(name)) && !own.contains(qk) {
                        own.push(qk.clone());
                    }
                }
            }
            if !own.is_empty() {
                break;
            }
            cur = self.modules[&m].parent.clone();
        }
        let candidates = if !own.is_empty() {
            own
        } else {
            // CHG-0033: enums an import brings into from_module -- the enum
            // an `import p;` names, or an exported enum of a module it names.
            let mut imported: Vec<String> = Vec::new();
            for (qk, _) in self.imported(from_module, Kind::Enum) {
                if self.enum_variants.get(&qk).map_or(false, |vs| vs.contains(name)) && !imported.contains(&qk) {
                    imported.push(qk);
                }
            }
            for (m, _) in self.imported(from_module, Kind::Module) {
                if let Some(node) = self.modules.get(&m) {
                    for (simple, (qk, exp, kind)) in &node.items {
                        if *kind == Kind::Enum && *exp && !simple.contains("::") {
                            if self.enum_variants.get(qk).map_or(false, |vs| vs.contains(name)) && !imported.contains(qk) {
                                imported.push(qk.clone());
                            }
                        }
                    }
                }
            }
            imported
        };
        if candidates.len() > 1 {
            let names: Vec<String> = candidates.iter().map(|k| format!("`{}::{}`", k, name)).collect();
            return Err(named("diag.ambiguous-name", format!("`{}` is a variant of more than one enum in scope: {}; qualify it", name, names.join(" and "))));
        }
        if let [qk] = candidates.as_slice() {
            // Execution dispatches a bare variant through the flat
            // `enum_of_variant` table (see module doc comment), which
            // holds one enum per name; a name more than one enum of the
            // program has is written `E::name`, which dispatches on `E`.
            let shared = self.enum_variants.iter().filter(|(_, vs)| vs.contains(name)).count() > 1;
            let resolved = if shared { format!("{}::{}", qk, name) } else { name.to_string() };
            return Ok(Some((resolved, Kind::Enum)));
        }
        Ok(None)
    }

    /// The resolved targets of from_module's imports of one kind.
    fn imported(&self, from_module: &str, kind: Kind) -> Vec<(String, Kind)> {
        self.import_targets.get(from_module).map_or(Vec::new(), |v| v.iter().filter(|(_, _, k)| *k == kind).map(|(_, q, k)| (q.clone(), *k)).collect())
    }

    /// `[Resolve-Qualified]` for `q1::...::qk::name` (or, when `segs.len() ==
    /// 1`, delegates to `[Resolve-Unqualified]` -- a single segment is not
    /// a qualified path at all). Visibility (`visible(qk, name, M)`) is
    /// checked *only* on the final hop, exactly as the rule states.
    fn resolve_qualified(&self, segs: &[String], from_module: &str) -> Result<Option<(String, Kind)>, String> {
        if segs.len() == 1 {
            return self.resolve_unqualified(&segs[0], from_module);
        }
        // Backward-compatible fast path: an associated function's own
        // registered simple key is already the compound "Type::name" form
        // (`spec/17` §1), so a plain two-segment `Type::name` call is first
        // tried as a single compound name against ordinary unqualified
        // resolution -- this is not a special case of the rule, it *is*
        // clause (1)/(2)/(3) applied to a "name" that happens to contain
        // "::", which the rule's own text never forbids.
        let joined = segs.join("::");
        if let Ok(Some(hit)) = self.resolve_unqualified(&joined, from_module) {
            return Ok(Some(hit));
        }
        // General case: resolve q1, then walk each further segment as an
        // item of the previous one.
        let (mut cur_key, mut cur_kind) = match self.resolve_unqualified(&segs[0], from_module)? {
            Some(x) => x,
            None => return Ok(None),
        };
        // The rule's own premise names only `visible(qk, name, M)` -- the
        // final hop. This implementation checks every hop instead, which
        // is what `rule.module.visibility`'s prose requires ("An export
        // item inside a private module is reachable only where that
        // module is") but the compressed final-hop-only premise does not,
        // by itself, make explicit: reaching `a::b::foo` through a private
        // intermediate `b` must not bypass `b`'s own privacy merely
        // because `foo` itself is exported.
        for seg in segs.iter().skip(1) {
            let candidate = format!("{}::{}", cur_key, seg);
            // `E::V`: a variant of the enum reached so far, which is not an
            // item; written with its enum's qualified key, so execution
            // dispatches on that enum, not on the bare name.
            if cur_kind == Kind::Enum && self.enum_variants.get(&cur_key).map_or(false, |vs| vs.contains(seg)) {
                cur_key = candidate;
                continue;
            }
            // An item of a module is looked up among the module's own
            // names, which hold its re-exports too (D-0136); an item of a
            // type (`T::name`) by its key.
            let found = if cur_kind == Kind::Module {
                self.modules.get(&cur_key).and_then(|n| n.items.get(seg.as_str())).map(|(qk, export, kind)| (qk.clone(), *export, *kind, cur_key.clone()))
            } else {
                self.all_items.get(&candidate).map(|(export, kind, m)| (candidate.clone(), *export, *kind, m.clone()))
            };
            match found {
                Some((qk, export, kind, decl_module)) => {
                    if !self.visible(&decl_module, export, from_module) {
                        let shown = candidate.strip_prefix("std::").unwrap_or(&candidate);
                        return Err(named("diag.name-not-visible", format!("`{}` is not exported from its module", shown)));
                    }
                    cur_key = qk;
                    cur_kind = kind;
                }
                None => {
                    // `m::V`: a variant of an enum declared directly in
                    // module `m` (or re-exported by it) is reachable only
                    // where that enum is.
                    if cur_kind == Kind::Module {
                        let mut owners: Vec<(String, bool)> = Vec::new();
                        for (simple, (qk, export, kind)) in &self.modules[&cur_key].items {
                            if *kind == Kind::Enum && !simple.contains("::") && self.enum_variants.get(qk).map_or(false, |vs| vs.contains(seg)) && !owners.iter().any(|(k, _)| k == qk) {
                                owners.push((qk.clone(), *export));
                            }
                        }
                        if let [(ek, export)] = owners.as_slice() {
                            if !self.visible(&cur_key, *export, from_module) {
                                let shown = ek.strip_prefix("std::").unwrap_or(ek);
                                return Err(named("diag.name-not-visible", format!("`{}` is a variant of `{}`, which is not exported from its module", seg, shown)));
                            }
                        }
                    }
                    return Ok(None);
                }
            }
        }
        Ok(Some((cur_key, cur_kind)))
    }
}

struct RewriteCtx<'a> {
    resolver: &'a Resolver,
    module: String,
    locals: Vec<HashSet<String>>,
    type_params: HashSet<String>,
}

impl<'a> RewriteCtx<'a> {
    fn is_local(&self, name: &str) -> bool {
        self.locals.iter().rev().any(|s| s.contains(name))
    }
    fn push(&mut self) {
        self.locals.push(HashSet::default());
    }
    fn pop(&mut self) {
        self.locals.pop();
    }
    fn declare(&mut self, name: &str) {
        self.locals.last_mut().unwrap().insert(name.to_string());
    }

    fn resolve_type(&self, ty: &mut Type) -> Result<(), String> {
        match ty {
            Type::Named(name, args) => {
                for a in args.iter_mut() {
                    self.resolve_type(a)?;
                }
                if self.type_params.contains(name.as_str()) {
                    return Ok(());
                }
                let segs: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
                match self.resolver.resolve_qualified(&segs, &self.module)? {
                    Some((qk, Kind::Struct)) | Some((qk, Kind::Enum)) => *name = qk,
                    // `[Resolve-Unbound]`: a type name that yields no item.
                    None => {
                        // A private type of the same name in another module
                        // (not `std`'s): say so, rather than that none exists.
                        let hidden = self.resolver.all_items.iter().filter(|(k, (export, kind, m))| {
                            !*export && matches!(kind, Kind::Struct | Kind::Enum) && m != "std" && !m.starts_with("std::") && k.rsplit("::").next() == Some(name.as_str()) && k.contains("::")
                        }).min_by(|a, b| a.0.cmp(b.0));
                        return Err(named("diag.unbound-name", match hidden {
                            Some((k, _)) => format!("no type named `{}` is visible here; `{}` is not exported from its module", name, k),
                            None if self.resolver.all_items.contains_key(&format!("std::{}", name)) => format!("no type named `{}`; the standard library's `{}` is brought in by `import std;`", name, name),
                            None => format!("no type named `{}`", name),
                        }));
                    }
                    _ => {} // a non-type item: leave for existing checks
                }
                Ok(())
            }
            Type::Ref(t, _) | Type::Slice(t, _) | Type::Rawptr(t) | Type::Array(t, _) | Type::Handle(t) | Type::Mutex(t) | Type::Guard(t) => {
                self.resolve_type(t)
            }
            Type::Fn(ps, r) => {
                for p in ps.iter_mut() {
                    self.resolve_type(p)?;
                }
                self.resolve_type(r)
            }
            _ => Ok(()),
        }
    }

    fn resolve_type_name(&self, name: &mut String) -> Result<(), String> {
        if self.type_params.contains(name.as_str()) {
            return Ok(());
        }
        if let Some((qk, Kind::Struct)) | Some((qk, Kind::Enum)) = self.resolver.resolve_unqualified(name, &self.module)? {
            *name = qk;
        }
        Ok(())
    }

    fn walk_block(&mut self, b: &mut Block) -> Result<(), String> {
        self.push();
        for s in &mut b.stmts {
            self.walk_stmt(s)?;
        }
        if let Some(t) = &mut b.tail {
            self.walk_expr(t)?;
        }
        self.pop();
        Ok(())
    }

    fn walk_stmt(&mut self, s: &mut Stmt) -> Result<(), String> {
        match s {
            Stmt::Let { ty, name, init, line } => {
                if let Some(t) = ty {
                    // A `Type` carries no line; the statement's.
                    let l = *line;
                    self.resolve_type(t).map_err(|d| tag(d, l))?;
                }
                if let Some(e) = init {
                    self.walk_expr(e)?;
                }
                self.declare(name);
            }
            Stmt::Destructure { struct_name, fields, init, .. } => {
                self.resolve_type_name(struct_name)?;
                self.walk_expr(init)?;
                for field in fields.iter() {
                    self.declare(field);
                }
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => self.walk_expr(e)?,
        }
        Ok(())
    }

    fn walk_expr(&mut self, e: &mut Expr) -> Result<(), String> {
        let line = e.line;
        self.walk_expr_untagged(e).map_err(|d| tag(d, line))
    }

    fn walk_expr_untagged(&mut self, e: &mut Expr) -> Result<(), String> {
        match &mut e.kind {
            ExprKind::Path(segs, targs) => {
                for t in targs.iter_mut() {
                    self.resolve_type(t)?;
                }
                if segs.len() == 1 && self.is_local(&segs[0]) {
                    return Ok(());
                }
                if let Some((qk, _kind)) = self.resolver.resolve_qualified(segs, &self.module)? {
                    // A `const` (D-0036): its use is a call of it.
                    if self.resolver.consts.contains(&qk) {
                        let path = Expr { kind: ExprKind::Path(qk.split("::").map(|s| s.to_string()).collect(), Vec::new()), line: e.line };
                        e.kind = ExprKind::Call(Box::new(path), Vec::new());
                        return Ok(());
                    }
                    // Restored as genuine segments (not collapsed to one
                    // compound string): `eval_path`/`items.*.get` already
                    // reconstruct their lookup key via `segs.join("::")`
                    // regardless of segment count, and keeping a resolved
                    // multi-part reference at `segs.len() > 1` matters
                    // elsewhere too -- `typecheck.rs`'s closure free-
                    // variable scan (`diag.capture-list-mismatch`) treats
                    // any `segs.len() == 1` path as a candidate local, so
                    // collapsing e.g. `Vec::len` to one segment made it
                    // look like an uncaptured local variable named
                    // `"Vec::len"`.
                    *segs = qk.split("::").map(|s| s.to_string()).collect();
                }
            }
            ExprKind::StructLit(segs, targs, fields) => {
                for t in targs.iter_mut() {
                    self.resolve_type(t)?;
                }
                if let Some((qk, Kind::Struct)) = self.resolver.resolve_qualified(segs, &self.module)? {
                    *segs = qk.split("::").map(|s| s.to_string()).collect();
                }
                for (_, fe) in fields.iter_mut() {
                    self.walk_expr(fe)?;
                }
            }
            ExprKind::ArrayLit(es) => {
                for x in es.iter_mut() {
                    self.walk_expr(x)?;
                }
            }
            ExprKind::ArrayRepeat(x, _) => self.walk_expr(x)?,
            ExprKind::Unary(_, x) | ExprKind::Borrow(_, x) | ExprKind::Deref(x) | ExprKind::Field(x, _) | ExprKind::Propagate(x) | ExprKind::Paren(x) => {
                self.walk_expr(x)?;
            }
            ExprKind::SliceOf(_, b, lo, hi) => {
                self.walk_expr(b)?;
                self.walk_expr(lo)?;
                self.walk_expr(hi)?;
            }
            ExprKind::Dollar => {}
            ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) | ExprKind::Assign(l, r) => {
                self.walk_expr(l)?;
                self.walk_expr(r)?;
            }
            ExprKind::Call(callee, args) => {
                self.walk_expr(callee)?;
                for a in args.iter_mut() {
                    self.walk_expr(a)?;
                }
                // D-0038: `printf(f, a…)` is `print($fmt(f, a…))`, and
                // `String::appendf(s, f, a…)` is `String::append(s, $fmt(f,
                // a…))`. `$fmt` is `std`'s formatter, a name no program can
                // write; it checks `f` against the arguments.
                let key = match &callee.kind {
                    ExprKind::Path(segs, _) => segs.join("::"),
                    _ => String::new(),
                };
                let line = e.line;
                let fmt_call = |rest: Vec<Expr>| Expr {
                    kind: ExprKind::Call(Box::new(Expr { kind: ExprKind::Path(vec!["$fmt".to_string()], Vec::new()), line }), rest),
                    line,
                };
                let path = |p: &[&str]| Box::new(Expr { kind: ExprKind::Path(p.iter().map(|s| s.to_string()).collect(), Vec::new()), line });
                if key == "checked_neg" && args.len() == 1 {
                    // D-0072: `checked_neg(a)` is `checked_sub(0, a)`; the
                    // `0` takes `a`'s type (`rule.arith.alt`).
                    let a = args.remove(0);
                    let zero = Expr { kind: ExprKind::IntLit(0, None), line };
                    e.kind = ExprKind::Call(path(&["checked_sub"]), vec![zero, a]);
                } else if key == "std::printf" {
                    let rest = std::mem::take(args);
                    e.kind = ExprKind::Call(path(&["std", "print"]), vec![fmt_call(rest)]);
                } else if key == "std::eprintf" {
                    // D-0040: the same text, to standard error.
                    let rest = std::mem::take(args);
                    e.kind = ExprKind::Call(path(&["std", "eprint_str"]), vec![fmt_call(rest)]);
                } else if key == "std::sprintf" {
                    // D-0039: a new `String` holding the text.
                    let rest = std::mem::take(args);
                    e.kind = ExprKind::Call(path(&["std", "String", "from_str"]), vec![fmt_call(rest)]);
                } else if key == "std::String::appendf" && !args.is_empty() {
                    let mut rest = std::mem::take(args);
                    let s = rest.remove(0);
                    e.kind = ExprKind::Call(path(&["std", "String", "append"]), vec![s, fmt_call(rest)]);
                } else if key == "std::File::printf" && !args.is_empty() {
                    // D-0059: `File::write_formatted(f, $fmt(format, a…))`.
                    let mut rest = std::mem::take(args);
                    let f = rest.remove(0);
                    e.kind = ExprKind::Call(path(&["std", "File", "write_formatted"]), vec![f, fmt_call(rest)]);
                } else if key == "std::TcpStream::printf" && !args.is_empty() {
                    // D-0144: `TcpStream::write_formatted(s, $fmt(format, a…))`.
                    let mut rest = std::mem::take(args);
                    let s = rest.remove(0);
                    e.kind = ExprKind::Call(path(&["std", "TcpStream", "write_formatted"]), vec![s, fmt_call(rest)]);
                } else if key == "std::assert" && !args.is_empty() {
                    // D-0065: `assert(c)` is `if (!(c)) { $assert_fail(); }`,
                    // and `assert(c, format, a…)` passes `$fmt(format, a…)`
                    // to it: the message, and its arguments, only when `c`
                    // is false.
                    let mut rest = std::mem::take(args);
                    let c = rest.remove(0);
                    let not_c = Expr { kind: ExprKind::Unary(crate::ast::UnOp::Not, Box::new(Expr { kind: ExprKind::Paren(Box::new(c)), line })), line };
                    let msg = if rest.is_empty() { Vec::new() } else { vec![fmt_call(rest)] };
                    let fail = Expr {
                        kind: ExprKind::Call(Box::new(Expr { kind: ExprKind::Path(vec!["$assert_fail".to_string()], Vec::new()), line }), msg),
                        line,
                    };
                    e.kind = ExprKind::If(Box::new(not_c), crate::ast::Block { stmts: vec![crate::ast::Stmt::Expr(Box::new(fail))], tail: None }, None);
                }
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.walk_block(b)?,
            ExprKind::If(c, then_b, else_e) => {
                self.walk_expr(c)?;
                self.walk_block(then_b)?;
                if let Some(x) = else_e {
                    self.walk_expr(x)?;
                }
            }
            ExprKind::While(c, b, step) => {
                self.walk_expr(c)?;
                self.walk_block(b)?;
                if let Some(s) = step {
                    self.walk_expr(s)?;
                }
            }
            ExprKind::Match(scrut, arms) => {
                self.walk_expr(scrut)?;
                for arm in arms.iter_mut() {
                    // D-0056/D-0058: an identifier in a pattern (at the top
                    // or inside parentheses) that names a variant of some
                    // enum is that (unit) variant, not a binder.
                    if let Some(b) = arm.binder.clone() {
                        // D-0058: a name that resolves to a `const` is that
                        // constant's value, a literal by name (`consts`
                        // folds it); it is not a binder.
                        let segs: Vec<String> = b.split("::").map(|x| x.to_string()).collect();
                        if let Ok(Some((qk, _))) = self.resolver.resolve_qualified(&segs, &self.module) {
                            if self.resolver.consts.contains(&qk) {
                                let line = arm.body.line;
                                let callee = Expr { kind: ExprKind::Path(qk.split("::").map(|s| s.to_string()).collect(), Vec::new()), line };
                                arm.lit = Some(Box::new(Expr { kind: ExprKind::Call(Box::new(callee), Vec::new()), line }));
                                arm.binder = None;
                            }
                        }
                    }
                    if let Some(b) = arm.binder.clone() {
                        // A qualified name that is not a constant is a
                        // variant, named by its last segment (D-0056).
                        let b = match b.rsplit_once("::") {
                            Some((_, last)) => {
                                arm.binder = Some(last.to_string());
                                last.to_string()
                            }
                            None => b,
                        };
                        if self.resolver.enum_variants.values().any(|vs| vs.contains(&b)) {
                            match arm.variant {
                                Some(_) => arm.nested.push(b),
                                None => arm.variant = Some(b),
                            }
                            arm.binder = None;
                        }
                    }
                    self.push();
                    if let Some(b) = &arm.binder {
                        self.declare(b);
                    }
                    self.walk_expr(&mut arm.body)?;
                    self.pop();
                }
            }
            ExprKind::Return(opt) => {
                if let Some(x) = opt {
                    self.walk_expr(x)?;
                }
            }
            ExprKind::Closure { params, ret, body, .. } => {
                self.push();
                for p in params.iter() {
                    self.declare(&p.name);
                }
                for p in params.iter_mut() {
                    self.resolve_type(&mut p.ty)?;
                }
                if let Some(t) = ret {
                    self.resolve_type(t)?;
                }
                self.walk_block(std::sync::Arc::make_mut(body))?;
                self.pop();
            }
            ExprKind::IntLit(..)
            | ExprKind::FloatLit(..)
            | ExprKind::StrLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit
            | ExprKind::Break
            | ExprKind::Continue => {}
        }
        Ok(())
    }
}

fn resolve_items(items: &mut [Item], path: &mut Vec<String>, resolver: &Resolver) -> Result<(), String> {
    for it in items.iter_mut() {
        match it {
            Item::Fn(f) => {
                let decl = std::sync::Arc::get_mut(f).expect("fresh AST: item Rc must be uniquely owned before build_items");
                let mut ctx = RewriteCtx {
                    resolver,
                    module: path.join("::"),
                    locals: Vec::new(),
                    type_params: decl.type_params.iter().cloned().collect(),
                };
                let line = decl.line;
                for p in decl.params.iter_mut() {
                    ctx.resolve_type(&mut p.ty).map_err(|d| tag(d, line))?;
                }
                ctx.resolve_type(&mut decl.ret).map_err(|d| tag(d, line))?;
                ctx.push();
                for p in decl.params.iter() {
                    ctx.declare(&p.name);
                }
                ctx.walk_block(&mut decl.body)?;
                ctx.pop();
            }
            Item::Extern(e) => {
                let decl = std::sync::Arc::get_mut(e).expect("fresh AST: item Rc must be uniquely owned before build_items");
                let ctx = RewriteCtx { resolver, module: path.join("::"), locals: Vec::new(), type_params: HashSet::default() };
                let line = decl.line;
                for p in decl.params.iter_mut() {
                    ctx.resolve_type(&mut p.ty).map_err(|d| tag(d, line))?;
                }
                ctx.resolve_type(&mut decl.ret).map_err(|d| tag(d, line))?;
            }
            Item::Struct(s) => {
                let decl = std::sync::Arc::get_mut(s).expect("fresh AST: item Rc must be uniquely owned before build_items");
                let ctx = RewriteCtx {
                    resolver,
                    module: path.join("::"),
                    locals: Vec::new(),
                    type_params: decl.type_params.iter().cloned().collect(),
                };
                let line = decl.line;
                for f in decl.fields.iter_mut() {
                    ctx.resolve_type(&mut f.ty).map_err(|d| tag(d, line))?;
                }
            }
            Item::Enum(en) => {
                let decl = std::sync::Arc::get_mut(en).expect("fresh AST: item Rc must be uniquely owned before build_items");
                let ctx = RewriteCtx {
                    resolver,
                    module: path.join("::"),
                    locals: Vec::new(),
                    type_params: decl.type_params.iter().cloned().collect(),
                };
                let line = decl.line;
                for v in decl.variants.iter_mut() {
                    if let Some(t) = &mut v.payload {
                        ctx.resolve_type(t).map_err(|d| tag(d, line))?;
                    }
                }
            }
            Item::Module(_, name, inner, _) => {
                path.push(name.clone());
                resolve_items(inner, path, resolver)?;
                path.pop();
            }
            Item::Import(..) => {} // validated once, up front, in `Resolver::build`
            Item::ExternCode(..) => {}
        }
    }
    Ok(())
}

/// Entry point: resolves and rewrites every item reference in `program` to
/// its fully qualified key, enforcing `spec/17`'s visibility rule. Must run
/// exactly once, immediately after parsing and before `build_items` clones
/// any item `Arc` a second time (this pass needs `Arc::get_mut`, which
/// requires each item `Arc` still have exactly one owner).
pub fn resolve_program(program: &mut Program) -> Result<(), String> {
    let resolver = Resolver::build(program)?;
    resolve_items(&mut program.items, &mut Vec::new(), &resolver)
}
