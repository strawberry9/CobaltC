// The rows of spec/conformance.md as runnable programs — shared by the
// interpreter's runner (tests/spec_rows.rs) and the compiler's
// (cobc/tests/compiled_spec_rows.rs), so both check exactly the same
// programs against exactly the same expectations.
//
// The fragment column is assembled into a program with the document's
// own conventions: items at the top level, statements inside
// `fn main()` with `bool c = true;` in scope, and a trailing value
// expression bound to a local. Rows that refer to an earlier row's
// context or to an `ex.*` program are completed from the override
// table; `ex.e2e-*` rows run the example verbatim; a fragment that is
// a path to a `.cb` file is that whole program (spec/conformance.md
// §14).

#![allow(dead_code)]

use std::collections::HashMap;

pub fn overrides() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("conf.i32-div-min-neg-one", "`fn d(i32 a, i32 b) : i32 { a / b }` `d(-2147483647 - 1, -1)`"),
        ("conf.cmp-le-ge-ne", "`bool a = 1: i32 <= 1: i32; bool b = 2: i32 >= 1: i32; bool c2 = 1: i32 != 2: i32;`"),
        ("conf.disjoint-field-borrows-ok", "`struct P { i32 a; i32 b; } auto p = P { .a = 1, .b = 2 }; auto r1 = &mut p.a; auto r2 = &mut p.b; *r1 + *r2`"),
        ("conf.exclusive-from-shared-rejected", "`struct P { i32 a; } fn g(ref<P, shared> p) { auto w = &mut p.a; }`"),
        ("conf.elision-conflict-rejected", "`struct Pair { i32 a; i32 b; } fn first(ref<Pair, shared> p) : ref<i32, shared> { &p.a }` `auto pr = Pair { .a = 1, .b = 2 }; auto y = first(&pr); pr.a = 3; *y`"),
        ("conf.index-out-of-bounds-static", "`array<i32, 3> a = [10, 20, 30]; a[3]`"),
        ("conf.composite-destroy-recurses", "`struct P { Vec<i32> a; } { auto p = P { .a = Vec::new() }; }`"),
        ("conf.match-non-exhaustive-rejected", "`enum Sign { Pos, Neg, Zero } fn d(Sign s) : i32 { match (s) { Pos : 1, Neg : -1 } }` `d(Neg)`"),
        ("conf.match-wildcard-ok", "`enum Sign { Pos, Neg, Zero } fn d(Sign s) : i32 { match (s) { Pos : 1, _ : 0 } }` `d(Neg)`"),
        ("conf.match-through-ref-rejected", "`enum E { Has(Vec<i32>), Empty } fn f(ref<E, shared> e) : usize { match (*e) { Has(x) : Vec::len(&x), Empty : 0 } }`"),
        ("conf.generic-call-inferred", "`fn id<T>(T x) : T { x }` `id(5)`"),
        ("conf.vec-index-shared-ok", "`Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); *Vec::index_shared(&v, 0)`"),
        ("conf.vec-index-out-of-bounds", "`Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); *Vec::index_shared(&v, 1)`"),
        ("conf.string-from-utf8-err", "`Vec<u8> b = Vec::new(); Vec::push(&mut b, 255); match (String::from_utf8(b)) { Ok(s) : String::len(&s), Err(_) : 99 }`"),
        ("conf.string-into-bytes-roundtrip", "`Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); auto s = match (String::from_utf8(b)) { Ok(v) : v, Err(_) : String::from_str(\"\") }; usize n1 = String::len(&s); auto bytes = String::into_bytes(s); usize n2 = Vec::len(&bytes);`"),
        ("conf.rc-last-drop-frees", "`auto a = Rc::new(1); auto b = Rc::clone(&a); drop(a); drop(b);`"),
        ("conf.unjoined-handle-waits", "`fn work(i32 n) : i32 { n * 2 }` `{ auto h = spawn(work, 1); }`"),
        ("conf.literal-default-i32", "`auto x = 5;`"),
        ("conf.literal-context-u8", "`u8 x = 200;`"),
        ("conf.let-synthesis", "`auto x = 3: i64 + 4: i64;`"),
        ("conf.rc-clone-shares-allocation", "`auto a = Rc::new(1); auto b = Rc::clone(&a);`"),
    ])
}

const ITEM_KW: &[&str] = &["fn ", "struct ", "enum ", "extern ", "resource ", "export ", "module ", "import ", "const "];

fn starts_item(s: &str) -> bool {
    ITEM_KW.iter().any(|k| s.starts_with(k))
}

// Splits `text` into top-level items and the remaining statement text.
fn split_items(text: &str) -> (Vec<String>, String) {
    let b = text.as_bytes();
    let n = b.len();
    let mut items = Vec::new();
    let mut stmts = Vec::new();
    let mut i = 0;
    while i < n {
        while i < n && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        if starts_item(&text[i..]) {
            if text[i..].starts_with("extern ") || text[i..].starts_with("import ") {
                let j = text[i..].find(';').map(|k| i + k + 1).unwrap_or(n);
                items.push(text[i..j].to_string());
                i = j;
                continue;
            }
            // `const τ N = e;` ends at its `;`, outside any braces `e` has.
            if text[i..].starts_with("const ") || text[i..].starts_with("export const ") {
                let mut depth = 0i32;
                let mut j = i;
                while j < n {
                    match b[j] as char {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        ';' if depth == 0 => {
                            j += 1;
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                items.push(text[i..j].to_string());
                i = j;
                continue;
            }
            let mut depth = 0i32;
            let mut started = false;
            let mut j = i;
            while j < n {
                match b[j] as char {
                    '{' => {
                        depth += 1;
                        started = true;
                    }
                    '}' => {
                        depth -= 1;
                        if started && depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            items.push(text[i..j].to_string());
            i = j;
        } else {
            let mut depth = 0i32;
            let mut j = i;
            while j < n {
                match b[j] as char {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 && j > i && (b[j - 1] as char).is_whitespace() && starts_item(&text[j..]) {
                    break;
                }
                j += 1;
            }
            stmts.push(text[i..j].trim().to_string());
            i = j;
        }
    }
    (items, stmts.join(" "))
}

pub fn assemble(frag: &str, value_row: bool) -> String {
    let frag = frag.replace("\\|", "|");
    let parts: Vec<&str> = frag.split('`').enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, s)| s).collect();
    let text = if parts.is_empty() { frag.clone() } else { parts.join(" ") };
    let (items, mut stmts) = split_items(&text);
    stmts = stmts.trim().to_string();
    let needs_wrap = !stmts.is_empty() && (!(stmts.ends_with(';') || stmts.ends_with('}')) || (value_row && stmts.ends_with('}')));
    if needs_wrap {
        // Bind only the trailing expression (after the last top-level
        // `;`, or the last top-level `}` that is not the final one).
        let b = stmts.as_bytes();
        let mut depth = 0i32;
        let mut cut = 0;
        for (k, ch) in b.iter().enumerate() {
            match *ch as char {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 && (!value_row || k + 1 < b.len()) {
                        cut = k + 1;
                    }
                }
                ';' if depth == 0 => cut = k + 1,
                _ => {}
            }
        }
        let head = &stmts[..cut];
        let tail = stmts[cut..].trim();
        stmts = format!("{} auto __v = {};", head, tail);
    }
    // CHG-0033: a fragment is assembled into a program whose root begins
    // with `import std;` (`spec/conformance.md` §0).
    if items.iter().any(|it| it.trim().starts_with("fn main")) {
        return format!("import std;\n{}", items.join("\n"));
    }
    format!("import std;\n{}\nfn main()\n{{\n    bool c = true;\n    {}\n}}\n", items.join("\n"), stmts)
}

pub fn example_code(examples: &str, id: &str) -> Option<String> {
    let head = format!("### `{}`", id);
    let mut lines = examples.lines();
    lines.by_ref().find(|l| l.starts_with(&head))?;
    let mut code = Vec::new();
    for l in lines {
        if let Some(rest) = l.strip_prefix("    ") {
            // strip the annotation comments
            let c = match rest.find("//") {
                Some(k) => &rest[..k],
                None => rest,
            };
            code.push(c.to_string());
        } else if l.starts_with("**") || l.starts_with('→') || l.starts_with("## ") || l.starts_with("### ") {
            break;
        }
    }
    Some(code.join("\n"))
}

#[derive(Debug, PartialEq, Clone)]
pub enum Exp {
    Ok(u8),                                 // terminates ok(s): `ok`, or `ok, exit status s`
    Diag(String, Option<&'static str>),
}

pub fn expected(out: &str) -> Exp {
    if let Some(k) = out.find("`diag.") {
        let rest = &out[k + 1..];
        let end = rest.find('`').unwrap();
        let id = rest[..end].to_string();
        let phase = if out.contains("(static)") {
            Some("static")
        } else if out.contains("(dynamic)") || out.contains("terminates") {
            Some("dynamic")
        } else {
            None
        };
        return Exp::Diag(id, phase);
    }
    match out.find("exit status ") {
        Some(k) => {
            let digits: String = out[k + "exit status ".len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
            Exp::Ok(digits.parse().expect("exit status is a u8"))
        }
        None => Exp::Ok(0),
    }
}

// Does an observed outcome — `None` for ok with exit status `status`,
// `Some((diag, phase))` — satisfy the row's expectation?
pub fn agrees(exp: &Exp, got: Option<(&str, &str)>, status: u8) -> bool {
    match (exp, got) {
        (Exp::Ok(s), None) => *s == status,
        (Exp::Diag(d, Some(p)), Some((g, gp))) => d == g && *p == gp,
        (Exp::Diag(d, None), Some((g, _))) => d == g,
        _ => false,
    }
}

pub enum Program {
    Source(String),
    // A path relative to the repository root.
    File(String),
}

pub struct Row {
    pub id: String,
    pub program: Program,
    pub exp: Exp,
}

// Every row that can be run, in table order.
pub fn rows(spec: &str, examples: &str) -> Vec<Row> {
    let ov = overrides();
    let mut out = Vec::new();
    for line in spec.lines() {
        let Some(rest) = line.strip_prefix("| `conf.") else { continue };
        let cols: Vec<&str> = line.split(" | ").collect();
        if cols.len() < 3 {
            continue;
        }
        let id = format!("conf.{}", &rest[..rest.find('`').unwrap()]);
        let frag = if let Some(o) = ov.get(id.as_str()) { o.to_string() } else { cols[1].to_string() };
        let outcome = cols[2];
        let value_row = outcome.trim().starts_with("`→");
        let exp = expected(outcome);
        if let Some(path) = frag.strip_prefix('`').and_then(|f| f.split('`').next()).filter(|f| f.ends_with(".cb")) {
            out.push(Row { id, program: Program::File(path.to_string()), exp });
            continue;
        }
        let prog = if let Some(ex_id) = frag.strip_prefix('`').and_then(|f| f.split('`').next()).filter(|f| f.starts_with("ex.")) {
            let code = example_code(examples, ex_id).unwrap_or_else(|| panic!("{}: no example {}", id, ex_id));
            if !(ex_id.starts_with("ex.e2e") || ex_id == "ex.extern-write") {
                continue; // other examples mix deliberately invalid lines
            }
            if code.contains("fn main") { code } else { assemble(&format!("`{}`", code), value_row) }
        } else {
            assemble(&frag, value_row)
        };
        out.push(Row { id, program: Program::Source(prog), exp });
    }
    out
}

// A file case's program arguments, from its `args:` header (CHG-0040):
// separated by spaces, `\xHH` for any byte, `""` for an empty argument;
// `$TMP` is a fresh, empty directory made for this run (CHG-0043), so
// runs that write files never share one. No header, no arguments.
pub fn case_args(src: &str) -> Vec<Vec<u8>> {
    let Some(h) = src.lines().take_while(|l| l.trim_start().starts_with("//")).find_map(|l| l.trim_start().trim_start_matches('/').trim().strip_prefix("args:")) else {
        return Vec::new();
    };
    // One directory per run, put in after splitting: its path may hold a space.
    let tmp = if h.contains("$TMP") { fresh_dir() } else { String::new() };
    h.split_whitespace()
        .map(|a| {
            if a == "\"\"" {
                return Vec::new();
            }
            if a.contains("$TMP") {
                return a.replace("$TMP", &tmp).into_bytes();
            }
            let b = a.as_bytes();
            let mut out = Vec::new();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'\\' && b.get(i + 1) == Some(&b'x') && i + 4 <= b.len() {
                    out.push(u8::from_str_radix(&a[i + 2..i + 4], 16).expect("args: bad \\xHH"));
                    i += 4;
                } else {
                    out.push(b[i]);
                    i += 1;
                }
            }
            out
        })
        .collect()
}

// The same, ready to pass to a `Command`; `None` where the platform
// cannot pass them: Windows arguments are UTF-16 text, so an argument
// that is not UTF-8 cannot be given there.
pub fn case_args_os(src: &str) -> Option<Vec<std::ffi::OsString>> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(case_args(src).into_iter().map(std::ffi::OsString::from_vec).collect())
    }
    #[cfg(not(unix))]
    {
        case_args(src).into_iter().map(|a| String::from_utf8(a).ok().map(std::ffi::OsString::from)).collect()
    }
}

// Whether `coby` calls C functions on this platform (`src/ffi.rs`), and
// the refusal it gives where it does not: a case that calls C is
// skipped there, saying so, and must pass on x86-64 Linux.
pub const COBY_CALLS_C: bool = cfg!(all(unix, target_arch = "x86_64"));
pub const NO_C_CALLS: &str = "coby calls C functions only on x86-64 Linux";

// A new, empty directory under the system's temporary directory.
pub fn fresh_dir() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!("cobaltc-case-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&d).expect("temporary directory");
    d.to_string_lossy().into_owned()
}

// The directory a file case runs in: its own (CHG-0043), so a relative
// path in its `args:` names a fixture beside it.
pub fn case_dir(path: &std::path::Path) -> &std::path::Path {
    path.parent().unwrap_or(std::path::Path::new("."))
}

