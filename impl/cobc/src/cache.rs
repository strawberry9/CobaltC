// The object cache: pure-`std` functions compiled once, kept, and linked
// into every later program that uses them.
//
// A program's C (`lower::Generated`) comes in units, one per lowered
// function. A unit is cacheable when it is a `std` function instantiated
// at `std` types alone: its text is then the same in every program (the
// lowering numbers temporaries per function, names string literals by
// content, and gives types ids that are hashes of their names), so its
// object code is too. Each such unit is compiled in a translation unit
// of its own -- the runtime's header, the declarations it refers to
// (closed over the structs' fields), and the unit -- and the object is
// kept under a key that hashes that text with the C compiler, its
// options, the runtime's stamp and this `cobc`. The next program that
// needs the function links the object instead of compiling it. What is
// not cacheable (the program's own functions, `std` instantiated at the
// program's types, thunks, descriptors, `main`) is one translation unit
// compiled every time; everything is linked together.
//
// `COBC_CACHE=off` disables the cache (one translation unit, as before);
// `COBC_CACHE=<dir>` names its directory; by default it is `cobc-cache`
// beside the `cobc` executable, where the runtime library is too.

use crate::lower::Generated;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub struct Inputs<'a> {
    pub cc: &'a str,
    pub opt: &'a str,
    pub arch: Option<&'a str>,
    pub cache_dir: PathBuf,
    pub work_dir: &'a Path,
}

pub struct Built {
    pub objects: Vec<PathBuf>,
    pub compiled: usize,
    pub reused: usize,
}

// The cache directory, or None when the cache is off.
pub fn dir() -> Option<PathBuf> {
    match std::env::var_os("COBC_CACHE") {
        Some(v) if v == "off" || v == "0" || v.is_empty() => None,
        Some(v) => Some(PathBuf::from(v)),
        None => std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("cobc-cache"))),
    }
}

pub const CC_FLAGS: &[&str] = &["-std=gnu11", "-ffp-contract=off", "-fno-optimize-sibling-calls", "-w"];

// The C compiler's identity for the key: the first line of `cc --version`.
fn cc_identity(cc: &str) -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let out = Command::new(cc).arg("--version").output().ok();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        format!("{} {}", cc, text.lines().next().unwrap_or(""))
    })
    .clone()
}

// This `cobc`'s identity: its executable's size and modification time
// (a rebuilt compiler may lower differently).
fn cobc_identity() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let meta = std::env::current_exe().ok().and_then(|e| std::fs::metadata(e).ok());
        match meta {
            Some(m) => {
                let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                format!("{}:{}:{}", m.len(), t, cbrt::STAMP)
            }
            None => cbrt::STAMP.to_string(),
        }
    })
    .clone()
}

fn fnv1a64(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// Every C identifier in `text`.
pub fn identifiers(text: &str) -> HashSet<&str> {
    let mut out = HashSet::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'_' || c.is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.insert(&text[start..i]);
        } else {
            i += 1;
        }
    }
    out
}

// The declarations of a program's C, by name.
struct Decls<'a> {
    fwd: Vec<(&'a str, &'a str)>,              // struct name, its forward declaration line
    type_blocks: Vec<(&'a str, String)>,       // struct name, its definition (with its static asserts)
    type_hashes: Vec<u64>,                     // each block's text hash, for the key
    type_index: HashMap<&'a str, usize>,
    type_refs: Vec<Vec<usize>>,                // the blocks each block mentions
    strings: HashMap<&'a str, &'a str>,        // literal name, its definition line
    protos: Vec<(&'a str, &'a str)>,           // function name, its prototype line
    thunk_protos: HashMap<String, String>,     // thunk name, a prototype made from its definition
}

// A function definition's first line: at the start of a line, not a
// declaration, and followed by the line `{`.
pub fn is_definition(line: &str, next: Option<&str>) -> bool {
    next == Some("{") && !line.is_empty() && !line.starts_with([' ', '{', '}', '#']) && line.ends_with(')') && !line.starts_with("static ")
}

pub fn name_before_paren(line: &str) -> Option<&str> {
    let open = line.find('(')?;
    let head = &line[..open];
    let end = head.trim_end().len();
    let start = head[..end].rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let name = &head[start..end];
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn decls(gen: &Generated) -> Decls<'_> {
    let mut fwd = Vec::new();
    for line in gen.fwd.lines() {
        if let Some(rest) = line.strip_prefix("struct ") {
            if let Some(name) = rest.strip_suffix(';') {
                fwd.push((name.trim(), line));
            }
        }
    }
    let mut type_blocks: Vec<(&str, String)> = Vec::new();
    for line in gen.types.lines() {
        if let Some(rest) = line.strip_prefix("struct ") {
            if line.contains('{') {
                let name = rest.split(|c: char| c == ' ' || c == '{').next().unwrap_or("");
                type_blocks.push((name, format!("{}\n", line)));
                continue;
            }
        }
        if let Some(last) = type_blocks.last_mut() {
            if !line.trim().is_empty() {
                last.1.push_str(line);
                last.1.push('\n');
            }
        }
    }
    let type_index: HashMap<&str, usize> = type_blocks.iter().enumerate().map(|(i, (n, _))| (*n, i)).collect();
    // The blocks each struct definition mentions (its fields' structs).
    let type_refs: Vec<Vec<usize>> = type_blocks
        .iter()
        .map(|(_, text)| {
            let ids = identifiers(text);
            let mut v: Vec<usize> = type_index.iter().filter(|(k, _)| ids.contains(*k)).map(|(_, &i)| i).collect();
            v.sort_unstable();
            v
        })
        .collect();
    let mut strings = HashMap::new();
    for line in gen.strings.lines() {
        if let Some(rest) = line.strip_prefix("static const uint8_t ") {
            if let Some(end) = rest.find('[') {
                strings.insert(&rest[..end], line);
            }
        }
    }
    let mut protos = Vec::new();
    for line in gen.protos.lines() {
        if let Some(name) = name_before_paren(line) {
            protos.push((name, line));
        }
    }
    let type_hashes: Vec<u64> = type_blocks.iter().map(|(_, t)| fnv1a64(0xcbf2_9ce4_8422_2325, t.as_bytes())).collect();
    let mut thunk_protos = HashMap::new();
    for line in gen.thunks.lines() {
        if !line.starts_with(|c: char| c == ' ' || c == '{' || c == '}') && line.ends_with(')') && line.contains('(') {
            if let Some(name) = name_before_paren(line) {
                thunk_protos.insert(name.to_string(), format!("{};", line));
            }
        }
    }
    Decls { fwd, type_blocks, type_hashes, type_index, type_refs, strings, protos, thunk_protos }
}

// What a unit's translation unit is made of: the struct blocks (closed
// over their fields), and the names of the literals, prototypes and thunk
// prototypes it mentions. The key is a hash over these parts' texts, so
// the text itself is assembled only for a unit that must be compiled.
struct Needs<'a> {
    types: Vec<usize>,
    names: HashSet<&'a str>,
}

fn needs<'a>(d: &Decls<'a>, unit: &'a str) -> Needs<'a> {
    let ids = identifiers(unit);
    let mut want: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut stack: Vec<usize> = d.type_index.iter().filter(|(k, _)| ids.contains(*k)).map(|(_, &i)| i).collect();
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        want.push(i);
        for &j in &d.type_refs[i] {
            stack.push(j);
        }
    }
    want.sort_unstable();
    Needs { types: want, names: ids }
}

fn key_for(d: &Decls, unit: &str, n: &Needs, salt: &str) -> String {
    let mut h = fnv1a64(0xcbf2_9ce4_8422_2325, salt.as_bytes());
    h = fnv1a64(h, unit.as_bytes());
    for &i in &n.types {
        h = fnv1a64(h ^ d.type_hashes[i], b"t");
    }
    let wanted_names: HashSet<&str> = n.types.iter().map(|&i| d.type_blocks[i].0).collect();
    for (name, line) in &d.fwd {
        if wanted_names.contains(name) || n.names.contains(name) {
            h = fnv1a64(h, line.as_bytes());
        }
    }
    let mut lits: Vec<&str> = d.strings.iter().filter(|(k, _)| n.names.contains(*k)).map(|(_, v)| *v).collect();
    lits.sort_unstable();
    for l in lits {
        h = fnv1a64(h, l.as_bytes());
    }
    for (name, line) in &d.protos {
        if n.names.contains(name) {
            h = fnv1a64(h, line.as_bytes());
        }
    }
    let mut thunks: Vec<&String> = d.thunk_protos.iter().filter(|(k, _)| n.names.contains(k.as_str())).map(|(_, v)| v).collect();
    thunks.sort();
    for t in thunks {
        h = fnv1a64(h, t.as_bytes());
    }
    // A second, independent hash of the unit alone widens the key.
    let g = fnv1a64(0x9E37_79B9_7F4A_7C15, unit.as_bytes());
    format!("{:016x}{:016x}", h, g ^ fnv1a64(0x1234_5678_9abc_def0, salt.as_bytes()))
}

// The translation unit of one cacheable unit: the runtime's header, what
// the unit refers to, and the unit.
fn translation_unit(d: &Decls, unit: &str, n: &Needs) -> String {
    let ids = &n.names;
    let want = &n.types;
    let wanted_names: HashSet<&str> = want.iter().map(|&i| d.type_blocks[i].0).collect();
    let mut out = String::with_capacity(unit.len() + 8192);
    out.push_str(Generated::header());
    out.push_str("\n/* ---- types ---- */\n");
    for (name, line) in &d.fwd {
        if wanted_names.contains(name) || ids.contains(name) {
            out.push_str(line);
            out.push('\n');
        }
    }
    for &i in want {
        out.push_str(&d.type_blocks[i].1);
    }
    out.push_str("\n/* ---- string literals ---- */\n");
    let mut lits: Vec<(&str, &str)> = d.strings.iter().filter(|(k, _)| ids.contains(*k)).map(|(k, v)| (*k, *v)).collect();
    lits.sort_unstable();
    for (_, line) in lits {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("\n/* ---- functions ---- */\n");
    for (name, line) in &d.protos {
        if ids.contains(name) {
            out.push_str(line);
            out.push('\n');
        }
    }
    let mut thunks: Vec<&String> = d.thunk_protos.iter().filter(|(k, _)| ids.contains(k.as_str())).map(|(_, v)| v).collect();
    thunks.sort();
    for p in thunks {
        out.push_str(p);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(unit);
    out
}

// The longest cached function, in lines, that the program's translation
// unit also gets an inline-only copy of (`inline_copies`).
const INLINE_LINES: usize = 50;

// The function definitions of a cacheable unit, each its name and text,
// or None if the unit holds anything else at file scope.
fn definitions(text: &str) -> Option<Vec<(&str, &str)>> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut offset = 0;
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.is_empty() {
            offset += 1;
            i += 1;
            continue;
        }
        if !is_definition(l, lines.get(i + 1).copied()) {
            return None;
        }
        let start = offset;
        let name = name_before_paren(l)?;
        // The body ends at the first `}` at the start of a line.
        let mut j = i + 1;
        while j < lines.len() && lines[j] != "}" {
            j += 1;
        }
        if j == lines.len() {
            return None;
        }
        for l in &lines[i..=j] {
            offset += l.len() + 1;
        }
        out.push((name, &text[start..offset.min(text.len())]));
        i = j + 1;
    }
    Some(out)
}

// Inline-only copies of the small cached functions the program's own code
// calls (and those such a copy calls): each definition made
// `extern inline __attribute__((gnu_inline))`, which the C compiler may
// inline but never emits, so a call it does not inline still goes to the
// cached object. Compiled in translation units of their own, `Vec::push`,
// `Vec::len`, the element accessors and `HashMap::entry`'s accessor could
// not be inlined into the program's loops (FINDINGS-2026-10-07).
fn inline_copies(gen: &Generated) -> String {
    let small: Vec<(&str, &str)> = gen
        .units
        .iter()
        .filter(|u| u.cacheable)
        .filter_map(|u| definitions(&u.text))
        .flatten()
        .filter(|(_, text)| text.lines().count() <= INLINE_LINES && !hands_runtime_an_address(text))
        .collect();
    let mut wanted: HashSet<&str> = HashSet::new();
    for u in gen.units.iter().filter(|u| !u.cacheable) {
        wanted.extend(identifiers(&u.text));
    }
    for t in [&gen.thunks, &gen.descs, &gen.tail] {
        wanted.extend(identifiers(t));
    }
    let mut taken = vec![false; small.len()];
    loop {
        let mut more = false;
        for (k, (name, text)) in small.iter().enumerate() {
            if !taken[k] && wanted.contains(name) {
                taken[k] = true;
                more = true;
                wanted.extend(identifiers(text));
            }
        }
        if !more {
            break;
        }
    }
    let mut out = String::new();
    for (k, (_, text)) in small.iter().enumerate() {
        if taken[k] {
            out.push_str("extern inline __attribute__((gnu_inline)) ");
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

// Whether a function hands the runtime an address (`cb_new(&r, …)`,
// `cb_recv_datum(&p_v, …)`, `cb_move_to(o, &t)`): the runtime keys
// objects and reference slots by address, and once gcc inlined such a
// copy it reused the dead locals' stack slots, so a slot still on record
// was overwritten and a borrow seemed to end early (a missed
// `diag.destroy-while-aliased`, view_of_temporary_kept_faults.cb, gcc
// -O1 and up). Such a function stays a call into its cached object, as
// before inline copies; the copies that matter (`Vec::push`, `len`, the
// element accessors) hand it none.
fn hands_runtime_an_address(text: &str) -> bool {
    text.lines().any(|line| {
        let mut s = line;
        while let Some(i) = s.find("cb_") {
            let rest = &s[i + 3..];
            let name = rest.bytes().take_while(|b| *b == b'_' || b.is_ascii_alphanumeric()).count();
            let after = &rest[name..];
            if after.starts_with('(') && after.split(';').next().unwrap_or("").contains('&') {
                return true;
            }
            s = after;
        }
        false
    })
}

// The program's own translation unit: everything that is not cached.
fn program_unit(gen: &Generated) -> String {
    let mut out = String::new();
    out.push_str(Generated::header());
    out.push_str("\n/* ---- types ---- */\n");
    out.push_str(&gen.fwd);
    out.push_str(&gen.types);
    out.push_str("\n/* ---- string literals ---- */\n");
    out.push_str(&gen.strings);
    out.push_str("\n/* ---- functions ---- */\n");
    out.push_str(&gen.protos);
    out.push_str("\n/* ---- call thunks for fn values ---- */\n");
    out.push_str(&gen.thunks);
    out.push_str("\n/* ---- type descriptors ---- */\n");
    out.push_str(&gen.descs);
    // After the thunks: a copy may call a `cb_eq_…` function, which has
    // no prototype of its own (`StringView::contains`'s `!= None`).
    out.push_str("\n/* ---- inline-only copies of cached std functions ---- */\n");
    out.push_str(&inline_copies(gen));
    for u in gen.units.iter().filter(|u| !u.cacheable) {
        out.push_str(&u.text);
    }
    out.push_str(&gen.tail);
    out
}

fn compile_one(inp: &Inputs, c_path: &Path, obj: &Path) -> Result<(), String> {
    let mut cmd = Command::new(inp.cc);
    cmd.args(CC_FLAGS).arg(inp.opt).args(inp.arch).arg("-c").arg("-o").arg(obj).arg(c_path);
    crate::show(&cmd);
    let out = cmd.output().map_err(|e| format!("cannot run the C compiler `{}`: {}", inp.cc, e))?;
    if !out.status.success() {
        return Err(format!("{}{} failed on {} (kept for inspection)", String::from_utf8_lossy(&out.stderr), inp.cc, c_path.display()));
    }
    Ok(())
}

// Compiles the program: cacheable units from the cache or into it (the
// missing ones in parallel), the rest as one unit; gives the objects to
// link.
pub fn build(gen: &Generated, inp: &Inputs) -> Result<Built, String> {
    std::fs::create_dir_all(&inp.cache_dir).map_err(|e| format!("cannot create the object cache {}: {}", inp.cache_dir.display(), e))?;
    let d = decls(gen);
    let salt = format!("{}\n{}\n{}\n{}\n{}", cc_identity(inp.cc), inp.opt, inp.arch.unwrap_or(""), CC_FLAGS.join(" "), cobc_identity());
    let mut objects = Vec::new();
    let mut todo: Vec<(PathBuf, PathBuf)> = Vec::new(); // (C file to write and compile, object)
    let mut texts: Vec<String> = Vec::new();
    let mut reused = 0;
    for u in gen.units.iter().filter(|u| u.cacheable) {
        let n = needs(&d, &u.text);
        let key = key_for(&d, &u.text, &n, &salt);
        let obj = inp.cache_dir.join(format!("{}.o", key));
        if obj.is_file() {
            reused += 1;
        } else {
            let c_path = inp.work_dir.join(format!("{}.c", key));
            todo.push((c_path, obj.clone()));
            texts.push(translation_unit(&d, &u.text, &n));
        }
        objects.push(obj);
    }
    let prog_c = inp.work_dir.join("program.c");
    let prog_o = inp.work_dir.join("program.o");
    std::fs::write(&prog_c, program_unit(gen)).map_err(|e| format!("cannot write {}: {}", prog_c.display(), e))?;
    for ((c_path, _), text) in todo.iter().zip(&texts) {
        std::fs::write(c_path, text).map_err(|e| format!("cannot write {}: {}", c_path.display(), e))?;
    }
    // The program's unit and the missing cached units, compiled in
    // parallel, as many at a time as there are processors.
    let mut jobs: Vec<(PathBuf, PathBuf, bool)> = vec![(prog_c.clone(), prog_o.clone(), false)];
    jobs.extend(todo.iter().map(|(c, o)| (c.clone(), o.clone(), true)));
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let errors: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.min(jobs.len()) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((c_path, obj, cached)) = jobs.get(i) else { break };
                let target = if *cached { c_path.with_extension("o.tmp") } else { obj.clone() };
                match compile_one(inp, c_path, &target) {
                    Ok(()) => {
                        if *cached {
                            // Into the cache under its final name only when
                            // whole: a reader never sees a partial object.
                            if std::fs::rename(&target, obj).is_err() {
                                if let Ok(bytes) = std::fs::read(&target) {
                                    let _ = std::fs::write(obj, bytes);
                                }
                            }
                        }
                    }
                    Err(e) => errors.lock().unwrap().push(e),
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if let Some(e) = errors.into_iter().next() {
        return Err(e);
    }
    let compiled = todo.len();
    let mut all = vec![prog_o];
    all.extend(objects);
    Ok(Built { objects: all, compiled, reused })
}
