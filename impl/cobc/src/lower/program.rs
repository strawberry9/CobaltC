// The generator's entry: Gen::new, generate, and its request queues.

use super::*;
use super::functions::dump_pass;

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
            desc_ids: Vec::new(),
            type_pos: HashMap::new(),
            desc_aux: String::new(),
            strings: String::new(),
            str_names: HashMap::new(),
            str_log: Vec::new(),
            units: Vec::new(),
            nstr: 0,
            protos: String::new(),
            fns: String::new(),
            substring_fn: false,
            append_substring_fn: false,
            native_ctx: None,
            pending_field_confined: None,
            pending_pred_closure: false,
            pending_reader_closure: false,
            pending_bare: false,
            pending_raw_ret: false,
            frozen_mode: false,
            inline_sigs: HashSet::new(),
            arrive_twins: HashSet::new(),
            pending_tested: false,
            all_checks: std::env::var("COBALTC_ALL_CHECKS").map_or(false, |v| !v.is_empty() && v != "0") || bootstrap_form(),
            bootstrap: bootstrap_level(),
            line_directives: true,
            pure_closures: HashSet::new(),
            fn_names: HashMap::new(),
            queue: VecDeque::new(),
            uid: 0,
            closure_base: 0,
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
            view_memo: std::cell::RefCell::new(HashMap::new()),
            reader_memo: std::cell::RefCell::new(HashMap::new()),
            writer_memo: std::cell::RefCell::new(HashMap::new()),
            fn_values: std::cell::RefCell::new(None),
            lib: false,
            pathless: false,
            decided: false,
            globals: HashSet::new(),
        }
    }

    // The program's C, in parts (`Generated::combined` joins them into the
    // one C file the driver compiles, D-0277).
    pub fn generate(mut self) -> R<Generated> {
        // The names `std`'s own bodies use as values (`used_as_value`), for
        // impl/selfhost/back/std_values.cb (`gen_std_values.sh`): cbc parses
        // a `std` body only when it reaches it, and takes this part as given.
        if std::env::var_os("COBALTC_STD_FN_VALUES").is_some() {
            let mut names: Vec<String> = self.value_names(|k| k.starts_with("std::")).into_iter().collect();
            names.sort();
            for n in names {
                println!("{}", n);
            }
            std::process::exit(0);
        }
        self.debug_lent();
        self.confining = if self.all_checks { HashMap::new() } else { self.confining_table() };
        // D-0257: a library has no `main`; its entry points (`extern fn`
        // with a body) are requested instead, each with a C-callable stub
        // in the tail.
        let main_c = if self.lib { String::new() } else { self.request_fn("main", vec![])? };
        let main_u8 = self.items.fns.get("main").map_or(false, |m| m.ret != Type::Void);
        let mut entries: Vec<(String, std::sync::Arc<FnDecl>)> = self.items.fns.iter().filter(|(_, f)| f.c_entry).map(|(k, f)| (k.clone(), f.clone())).collect();
        entries.sort_by_key(|(_, f)| f.line);
        let mut entry_c: Vec<(String, std::sync::Arc<FnDecl>)> = Vec::new();
        if self.lib {
            for (k, f) in &entries {
                let c = self.request_fn(k, vec![])?;
                entry_c.push((c, f.clone()));
            }
        }
        while let Some((name, targs, mask)) = self.queue.pop_front() {
            let start = self.fns.len();
            let pure = name.starts_with("std::") && targs.iter().all(std_only);
            self.lower_fn(&name, &targs, mask)?;
            let mut text = self.fns[start..].to_string();
            // D-0286: a decided program's natives and helpers, whose fixed
            // text puts values in flight, made to hand them over instead
            // (`ground_native`); text it does not know keeps the program
            // on the first lowering.
            if self.decided && (text.contains("cb_send(") || text.contains("cb_recv(")) {
                let f = self.items.fns.get(&name).cloned().expect("a lowered function");
                let subst = subst_of(&f.type_params, &targs);
                let mut tids = Vec::new();
                for p in &f.params {
                    let pty = apply(&p.ty, &subst);
                    if self.is_resource(&pty) {
                        tids.push((format!("p_{}", san(&p.name)), self.type_id(&pty)?));
                    }
                }
                text = match flights::ground_native(&text, &tids) {
                    Ok(t) => t,
                    Err(e) => return unsupported(&format!("a value in flight in `{}`: {}", name, e)),
                };
                if text.contains("cb_send(") || text.contains("cb_recv(") {
                    return unsupported(&format!("a value in flight in `{}`", name));
                }
            }
            // A `spawn`'s block (`struct cb_env_N`) is defined with its
            // trampoline among the thunks, in the program's translation
            // unit: a function that spawns is compiled there too.
            let pure = pure && !text.contains("struct cb_env_");
            self.units.push(Unit { text, pure_std: pure });
        }
        // D-0219: each call's `ARRIVE_MARK` names the callee's `__arrive`
        // twin where it has one, and the callee itself where it has none.
        let twins = std::mem::take(&mut self.arrive_twins);
        let mut twin_list: Vec<&String> = twins.iter().collect();
        twin_list.sort();
        let twins_text: String = twin_list.iter().map(|t| format!("{}\n", t)).collect();
        for u in self.units.iter_mut() {
            if u.text.contains(ARRIVE_MARK) {
                let r = resolve_arrive(&u.text, &twins);
                dump_pass("resolve_arrive", &[("in", &u.text), ("arg", &twins_text), ("out", &r)]);
                u.text = r;
            }
        }
        if self.thunks_out.contains(ARRIVE_MARK) {
            let r = resolve_arrive(&self.thunks_out, &twins);
            dump_pass("resolve_arrive", &[("in", &self.thunks_out), ("arg", &twins_text), ("out", &r)]);
            self.thunks_out = r;
        }
        self.arrive_twins = twins;
        if !self.boot(1) {
            self.dumped_localize(&main_c);
        }
        // Last, when no pass reads the text line by line any more.
        if self.line_directives {
            for u in self.units.iter_mut().filter(|u| !u.pure_std) {
                if u.text.contains("cb_at(") || u.text.contains(LINE_MARK) {
                    u.text = line_directives(&u.text, &self.files);
                }
            }
        }
        let mut descs = String::new();
        descs.push_str(&self.desc_aux);
        descs.push_str("static const cb_type cb_types[] = {\n");
        for d in &self.descs {
            writeln!(descs, "    {},", d.as_deref().unwrap_or("{0}")).unwrap();
        }
        descs.push_str("    {0}\n};\n");
        descs.push_str("static const uint32_t cb_type_ids[] = {");
        for id in &self.desc_ids {
            write!(descs, "{}u, ", id).unwrap();
        }
        descs.push_str("0};\n");
        let mut out = String::new();
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
        let mut header_text = String::new();
        if self.lib {
            // D-0257: no `main`. `cb_lib_init` is the runtime's start, run
            // once by the first entry; each entry point is a stub that
            // enters, calls the function, leaves; its `_try` twin contains
            // a fault (`cb_lib_jmp`) and gives C a status and the result
            // through a pointer. `cobalt_init`, `cobalt_shutdown` and
            // `cobalt_last_fault` are the library's own API.
            writeln!(out, "static void cb_lib_init(void)\n{{\n    cb_init(cb_files, {}, cb_types, {}, cb_type_ids);\n}}", self.files.len(), self.descs.len()).unwrap();
            out.push_str("void cobalt_init(void)\n{\n    cb_lib_enter(cb_lib_init);\n    cb_lib_leave();\n}\n");
            out.push_str("void cobalt_shutdown(void)\n{\n    cb_lib_shutdown();\n}\n");
            out.push_str("int cobalt_last_fault(char *buf, size_t n)\n{\n    return cb_lib_last_fault(buf, n);\n}\n");
            for (c, f) in &entry_c {
                let mut params = Vec::new();
                let mut args = Vec::new();
                for (i, p) in f.params.iter().enumerate() {
                    let pc = self.ctype(&p.ty)?;
                    params.push(format!("{} a{}", pc, i));
                    args.push(format!("a{}", i));
                }
                let plist = if params.is_empty() { "void".to_string() } else { params.join(", ") };
                let call = format!("{}({})", c, args.join(", "));
                if f.ret == Type::Void {
                    writeln!(out, "void {}({})\n{{\n    cb_lib_enter(cb_lib_init);\n    {};\n    cb_lib_leave();\n}}", f.name, plist, call).unwrap();
                    let tplist = if params.is_empty() { "void".to_string() } else { params.join(", ") };
                    writeln!(
                        out,
                        "int {n}_try({pl})\n{{\n    cb_lib_enter(cb_lib_init);\n    jmp_buf cb_jb;\n    if (setjmp(cb_jb) == 0) {{\n        cb_lib_jmp((void *)cb_jb);\n        {call};\n        cb_lib_jmp(0);\n        cb_lib_leave();\n        return 0;\n    }}\n    cb_lib_leave();\n    return cb_lib_fault_code();\n}}",
                        n = f.name, pl = tplist, call = call
                    )
                    .unwrap();
                } else {
                    let rc = self.ctype(&f.ret)?;
                    writeln!(out, "{} {}({})\n{{\n    cb_lib_enter(cb_lib_init);\n    {} cb_r = {};\n    cb_lib_leave();\n    return cb_r;\n}}", rc, f.name, plist, rc, call).unwrap();
                    let mut tparams = params.clone();
                    tparams.push(format!("{} *cb_out", rc));
                    writeln!(
                        out,
                        "int {n}_try({pl})\n{{\n    cb_lib_enter(cb_lib_init);\n    jmp_buf cb_jb;\n    if (setjmp(cb_jb) == 0) {{\n        cb_lib_jmp((void *)cb_jb);\n        {rc} cb_r = {call};\n        cb_lib_jmp(0);\n        cb_lib_leave();\n        *cb_out = cb_r;\n        return 0;\n    }}\n    cb_lib_leave();\n    return cb_lib_fault_code();\n}}",
                        n = f.name, pl = tparams.join(", "), rc = rc, call = call
                    )
                    .unwrap();
                }
            }
            header_text = self.header_text(&entry_c)?;
        } else {
            writeln!(
                out,
                // D-0077: the program runs on a thread with a large stack
                // (reserved, not committed), as `coby` runs it; `main`'s own
                // thread only starts it.
                "static void cb_program(void)\n{{\n    cb_init(cb_files, {}, cb_types, {}, cb_type_ids);\n    cb_frame_push();\n    {}\n    cb_frame_pop();\n    cb_terminate_ok({});\n}}\nint main(int argc, char **argv)\n{{\n    cb_set_args(argc, argv);\n    cb_run_main(cb_program);\n}}",
                self.files.len(),
                self.descs.len(),
                // `ok(s)`: `fn main() : u8`'s value, or 0 (`rule.fn.program`).
                if main_u8 { format!("uint8_t cb_status = {}();", main_c) } else { format!("{}();", main_c) },
                if main_u8 { "cb_status" } else { "0" }
            )
            .unwrap();
        }
        let pathless = if self.pathless { self.pathless_blocker().map_or(Ok(()), Err) } else { Err("COBALTC_PATHLESS=0".to_string()) };
        Ok(Generated {
            decided: self.decided && pathless.is_ok(),
            pathless,
            fwd: self.fwd,
            types: self.types,
            strings: self.strings,
            protos: self.protos,
            thunks: self.thunks_out,
            descs,
            units: self.units,
            tail: out,
            header_text,
        })
    }

    // D-0281 step 4c: why the program must keep its paths, if it must: it
    // is a library (C callers keep what they like), it asks for every
    // check, a function it lowers has a conflict the static pass left to
    // the run (`Items::tracking_needed`), a type it uses keeps references
    // in data (tier D), or it spawns a thread (references lent to a
    // thread are tier D until scoped lending).
    fn pathless_blocker(&self) -> Option<String> {
        if self.lib {
            return Some("a library (`--lib`)".to_string());
        }
        if self.all_checks {
            return Some("every check was asked for".to_string());
        }
        let needed = self.items.tracking_needed.lock().unwrap();
        let mut fns: Vec<String> = self.fn_names.keys().map(|(name, targs)| coby::tier::body_key(name, targs)).collect();
        fns.sort();
        for key in fns {
            if let Some(why) = needed.get(&key) {
                // "line N: …" in the combined text's line space, shown as
                // the file and line it came from.
                let why = match why.strip_prefix("line ").and_then(|r| r.split_once(": ")) {
                    Some((n, rest)) => match n.parse().ok().and_then(|n| coby::locate(n, self.map)) {
                        Some((f, l)) => format!("{}:{}: {}", f, l, rest),
                        None => why.clone(),
                    },
                    None => why.clone(),
                };
                return Some(format!("`{}`, {}", key.strip_prefix("std::").unwrap_or(&key), why));
            }
        }
        // A closure that borrows what it captures, or captures a reference,
        // keeps references in its environment: a `fn` value kept in data
        // may then be one. In a frame the static pass holds what such a
        // closure reaches as a borrow (D-0288), so only data keeps paths.
        let fn_refs = self.closures.values().any(|c| (!c.is_move && !c.fields.is_empty()) || c.fields.iter().any(|(_, t)| coby::typecheck::keeps_refs_in_data(self.items, t, true) || matches!(t, Type::Ref(..) | Type::Slice(..)) || matches!(t, Type::Named(n, _) if n == "std::StringView")));
        let mut tys: Vec<String> = self.type_ids.keys().filter(|t| coby::typecheck::keeps_refs_in_data(self.items, t, fn_refs)).map(|t| t.to_string()).collect();
        tys.sort();
        if let Some(t) = tys.first() {
            return Some(format!("the type `{}` keeps references in data (tier D)", t));
        }
        if self.units.iter().any(|u| u.text.contains("cb_point_check(")) {
            return Some("a call is checked when it is made (D-0283)".to_string());
        }
        None
    }

    // External linkage only where it is needed. A function defined in a
    // unit that is not pure `std`, and named by no pure-`std` unit, is
    // `static`: nothing outside the program's translation unit
    // translation unit calls it by name (the runtime reaches functions
    // through pointers, and foreign code is only called,
    // `rule.trust.extern-code`), so the C compiler may inline it wherever
    // it is called and drop it where it is not. Not `main`: inlined into
    // `cb_program`, which only C's `main` calls, GCC would compile the
    // whole program as code run once, for size (a `div` for a `%` by a
    // constant, say).
    // `localize`, dumped (`dump_pass`): the units' flags (`1` pure `std`)
    // and texts, the prototypes, the inlined signatures and `main`'s name,
    // before and after.
    fn dumped_localize(&mut self, main_c: &str) {
        if std::env::var_os("COBC_DUMP_PASSES").is_none() {
            return self.localize(main_c);
        }
        let mut sigs: Vec<&String> = self.inline_sigs.iter().collect();
        sigs.sort();
        let sigs: String = sigs.iter().map(|t| format!("{}\n", t)).collect();
        let flags: String = self.units.iter().map(|u| if u.pure_std { "1\n" } else { "0\n" }).collect();
        let before: Vec<String> = self.units.iter().map(|u| u.text.clone()).collect();
        let protos_in = self.protos.clone();
        self.localize(main_c);
        let mut parts: Vec<(String, &str)> = vec![("main".to_string(), main_c), ("sigs".to_string(), &sigs), ("flags".to_string(), &flags), ("protos_in".to_string(), &protos_in), ("protos_out".to_string(), &self.protos)];
        for (k, b) in before.iter().enumerate() {
            parts.push((format!("unit{}_in", k), b));
            parts.push((format!("unit{}_out", k), &self.units[k].text));
        }
        let parts: Vec<(&str, &str)> = parts.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        dump_pass("localize", &parts);
    }

    pub(super) fn localize(&mut self, main_c: &str) {
        let mut std_names: HashSet<String> = HashSet::new();
        for u in self.units.iter().filter(|u| u.pure_std) {
            std_names.extend(crate::cc::identifiers(&u.text).into_iter().map(str::to_string));
        }
        let mut local: HashSet<String> = HashSet::new();
        let inline_sigs = std::mem::take(&mut self.inline_sigs);
        for u in self.units.iter_mut().filter(|u| !u.pure_std) {
            let mut out = String::with_capacity(u.text.len() + 64);
            let mut lines = u.text.split('\n').peekable();
            while let Some(l) = lines.next() {
                if crate::cc::is_definition(l, lines.peek().copied()) {
                    if let Some(n) = crate::cc::name_before_paren(l) {
                        if n != main_c && !std_names.contains(n) {
                            local.insert(l.to_string());
                            // D-0209: a direct-parameter dispatcher is
                            // inlined at every call (`lower_fn_direct`).
                            out.push_str(if inline_sigs.contains(l) { "static inline __attribute__((always_inline)) " } else { "static " });
                        }
                    }
                }
                out.push_str(l);
                if lines.peek().is_some() {
                    out.push('\n');
                }
            }
            u.text = out;
        }
        let mut protos = String::with_capacity(self.protos.len() + 7 * local.len());
        for l in self.protos.lines() {
            if l.strip_suffix(';').map_or(false, |sig| local.contains(sig)) {
                protos.push_str("static ");
            }
            protos.push_str(l);
            protos.push('\n');
        }
        self.protos = protos;
    }

    // A closure's id (`closure_base`): unique in the program, the same
    // whatever else the program contains.
    pub(super) fn closure_id(&mut self) -> u64 {
        self.uid += 1;
        self.closure_base | self.uid as u64
    }

    pub(super) fn fresh(&mut self, prefix: &str) -> String {
        self.uid += 1;
        format!("{}{}", prefix, self.uid)
    }

    pub(super) fn loc(&mut self, line: usize) -> String {
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

    pub(super) fn request_fn(&mut self, name: &str, targs: Vec<Type>) -> R<String> {
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
    pub(super) fn request_variant(&mut self, name: &str, targs: Vec<Type>, mask: u64) -> R<String> {
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
}

// D-0274: `cobc --bootstrap-form` (`COBALTC_BOOTSTRAP_FORM=1`): every
// optimisation off, the general checked form that the code generator
// written in CobaltC reproduces.
pub(crate) fn bootstrap_form() -> bool {
    bootstrap_level() > 0
}

// The highest D-0274 stage switched off: `COBALTC_BOOTSTRAP_UPTO=N`
// switches off stages 1..=N only (each a state the suites passed in
// D-0274's order; the code generator written in CobaltC restores them from
// the last, CHG-0368); `COBALTC_BOOTSTRAP_FORM` all ten.
pub(crate) const BOOTSTRAP_STAGES: u8 = 10;

pub(crate) fn bootstrap_level() -> u8 {
    if let Ok(v) = std::env::var("COBALTC_BOOTSTRAP_UPTO") {
        if let Ok(n) = v.trim().parse::<u8>() {
            return n.min(BOOTSTRAP_STAGES);
        }
    }
    if std::env::var("COBALTC_BOOTSTRAP_FORM").map_or(false, |v| !v.is_empty() && v != "0") {
        BOOTSTRAP_STAGES
    } else {
        0
    }
}

impl Gen<'_> {
    // D-0274 stage `stage` is switched off (`bootstrap_level`).
    pub(crate) fn boot(&self, stage: u8) -> bool {
        self.bootstrap >= stage
    }
}
