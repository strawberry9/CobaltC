// The rows of spec/conformance.md as runnable programs — shared by the
// interpreter's runner (tests/spec_rows.rs) and the compiler's
// (cobc/tests/compiled_spec_rows.rs), so both check exactly the same
// programs against exactly the same expectations.
//
// The fragment column is assembled into a program with the document's
// own conventions: items at the top level, statements inside
// `fn main()` with `bool c = true;` in scope (from an enclosing block), and a trailing value
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
        ("conf.string-from-utf8-err", "`Vec<u8> b = Vec::new(); Vec::push(&mut b, 255); match (String::from_utf8(b)) { Ok(s) : StringView::len(&s), Err(_) : 99 }`"),
        ("conf.string-into-bytes-roundtrip", "`Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); auto s = match (String::from_utf8(b)) { Ok(v) : v, Err(_) : String::copy(\"\") }; usize n1 = StringView::len(&s); auto bytes = String::into_bytes(s); usize n2 = Vec::len(&bytes);`"),
        ("conf.rc-last-drop-frees", "`auto a = Rc::new(1); auto b = Rc::clone(&a); drop(a); drop(b);`"),
        ("conf.unjoined-handle-waits", "`fn work(i32 n) : i32 { n * 2 }` `{ auto h = spawn(work, 1); }`"),
        ("conf.literal-default-i32", "`auto x = 5;`"),
        ("conf.literal-context-u8", "`u8 x = 200;`"),
        ("conf.let-synthesis", "`auto x = 3: i64 + 4: i64;`"),
        ("conf.rc-clone-shares-allocation", "`auto a = Rc::new(1); auto b = Rc::clone(&a);`"),
    ])
}

const ITEM_KW: &[&str] = &["fn ", "struct ", "enum ", "bitstruct ", "extern ", "unsafe extern ", "resource ", "export ", "module ", "import ", "const "];

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
            if text[i..].starts_with("extern ") || text[i..].starts_with("unsafe extern ") || text[i..].starts_with("import ") {
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
                    // Not before `else`: `if (…) { … } else { … }` is one value.
                    if depth == 0 && (!value_row || k + 1 < b.len()) && !stmts[k + 1..].trim_start().starts_with("else") {
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
    // `c` belongs to an enclosing block, so a fragment may declare its
    // own `c` (D-0131: only a nested block may reuse a name).
    format!("import std;\n{}\nfn main()\n{{\n    bool c = true;\n    {{\n    {}\n    }}\n}}\n", items.join("\n"), stmts)
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

// A file case's tool flags, from its `flags:` header (D-0303), given to
// the tool before the case's path: `--allow-deprecated`. No header, none.
pub fn case_flags(src: &str) -> Vec<std::ffi::OsString> {
    src.lines()
        .take_while(|l| l.trim_start().starts_with("//"))
        .find_map(|l| l.trim_start().trim_start_matches('/').trim().strip_prefix("flags:"))
        .map_or(Vec::new(), |h| h.split_whitespace().map(std::ffi::OsString::from).collect())
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

// Whether a file case's `platform:` header (D-0143) names a platform
// this is not: `// platform: unix` runs only on Unix systems (`linux`, on
// Linux alone), for a case
// that needs a program every Unix system has (`/bin/sh`, `/bin/cat`) or
// prints Unix paths. Such a case is skipped elsewhere, saying so.
pub fn platform_skipped(src: &str) -> bool {
    let Some(p) = src.lines().take_while(|l| l.trim_start().starts_with("//")).find_map(|l| l.trim_start().trim_start_matches('/').trim().strip_prefix("platform:")) else {
        return false;
    };
    match p.trim() {
        "unix" => !cfg!(unix),
        // D-0205: Linux alone (`Child::kill_tree` finds descendants there).
        "linux" => !cfg!(target_os = "linux"),
        "windows" => !cfg!(windows),
        other => panic!("platform: unknown platform `{}`", other),
    }
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

// ---- the corpus runners' battery knobs (impl/battery.sh) ----
//
// `COBALTC_TEST_SHARD=i/n` (1 <= i <= n): this process runs only the
// cases whose position in the runner's fixed order is congruent to i-1
// modulo n, so n processes side by side cover the corpus once. The
// runners are single-threaded (a file case runs in its own directory),
// and this is how the battery spreads them over the machine's cores.
// Unset: every case.
pub fn shard() -> (usize, usize) {
    match std::env::var("COBALTC_TEST_SHARD") {
        Err(_) => (1, 1),
        Ok(s) => {
            let bad = || -> ! { panic!("COBALTC_TEST_SHARD must be i/n with 1 <= i <= n, not {:?}", s) };
            let (i, n) = s.split_once('/').unwrap_or_else(|| bad());
            let (i, n): (usize, usize) = (i.trim().parse().unwrap_or_else(|_| bad()), n.trim().parse().unwrap_or_else(|_| bad()));
            if i < 1 || n < 1 || i > n {
                bad();
            }
            (i, n)
        }
    }
}

// Whether the case at `index` (0-based, in the runner's order) is this
// process's to run.
pub fn in_shard(index: usize) -> bool {
    let (i, n) = shard();
    index % n == i - 1
}

// The number of shards, for a runner's "at least this many cases ran"
// floor: each shard sees about a 1/n share.
pub fn shard_count() -> usize {
    shard().1
}

// A case with a `slow:` header (the reason follows) runs only when
// `COBALTC_TEST_SLOW=1`, which the full battery sets and a scoped one
// does not; `Some(reason)` says to skip it here. The budget: a case
// should take under 10 s interpreted and under 20 s compiled; one that
// cannot be trimmed below that is marked `slow:` instead.
pub fn slow_skipped(src: &str) -> Option<String> {
    let reason = src.lines().take_while(|l| l.trim_start().starts_with("//")).find_map(|l| l.trim_start().trim_start_matches('/').trim().strip_prefix("slow:"))?;
    if std::env::var("COBALTC_TEST_SLOW").map_or(false, |v| v == "1") {
        return None;
    }
    Some(reason.trim().to_string())
}

// The slowest cases a runner saw, reported at its end so the budget
// above can be enforced (`cargo test -- --nocapture`, or the battery's
// direct runs, show it).
pub struct Slowest {
    seen: Vec<(std::time::Duration, String)>,
    total: std::time::Duration,
    count: usize,
}

impl Slowest {
    pub fn new() -> Slowest {
        Slowest { seen: Vec::new(), total: std::time::Duration::ZERO, count: 0 }
    }

    // Records one case's wall time.
    pub fn note(&mut self, label: &str, took: std::time::Duration) {
        self.total += took;
        self.count += 1;
        self.seen.push((took, label.to_string()));
        self.seen.sort_by(|a, b| b.0.cmp(&a.0));
        self.seen.truncate(10);
    }

    // Prints the ten slowest, and the total, under `what`.
    pub fn report(&self, what: &str) {
        eprintln!("{}: {} cases in {:.1} s; slowest:", what, self.count, self.total.as_secs_f64());
        for (took, label) in &self.seen {
            eprintln!("  {:7.2} s  {}", took.as_secs_f64(), label);
        }
    }
}

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


// How long any one process a test starts may run (wall clock) before it
// is killed: a runaway program fails its test instead of being left
// spinning. Portable (`Child::kill`), so `coby`'s tests keep building on
// Windows.
pub const RUN_LIMIT_SECS: u64 = 600;

// Runs `cmd` with `stdin` as its input, to completion or to
// `RUN_LIMIT_SECS`, whichever is first; a killed process's output is what
// it wrote, and its status is not success.
pub fn output_limited(cmd: &mut std::process::Command, stdin: &[u8]) -> std::process::Output {
    use std::io::{Read, Write};
    use std::process::Stdio;
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("start the process");
    let input = stdin.to_vec();
    let mut sin = child.stdin.take().unwrap();
    let w = std::thread::spawn(move || {
        let _ = sin.write_all(&input);
    });
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ro = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = so.read_to_end(&mut v);
        v
    });
    let re = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = se.read_to_end(&mut v);
        v
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().expect("wait for the process") {
            break st;
        }
        if start.elapsed().as_secs() >= RUN_LIMIT_SECS {
            let _ = child.kill();
            break child.wait().expect("wait for the killed process");
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let _ = w.join();
    std::process::Output { status, stdout: ro.join().unwrap_or_default(), stderr: re.join().unwrap_or_default() }
}

// D-0307: the gate's store of results (`impl/gate.sh`). With
// `COBALTC_RESULT_STORE` (a directory) and `COBALTC_TOOLCHAIN_KEY` (a hash
// of everything that builds the tools, which the gate computes) both set,
// a case's result is kept under a key made of that toolchain key, the
// case's own text, the support files beside it and how it was run; a later
// run with the same key reuses it instead of running the case again. Only
// results that met their check are kept, so a failure always runs again.
// Without the two variables nothing is stored and every case runs, as a
// plain `cargo test` always has.
pub mod store {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    fn dir() -> Option<&'static (PathBuf, String)> {
        static DIR: OnceLock<Option<(PathBuf, String)>> = OnceLock::new();
        DIR.get_or_init(|| {
            let d = std::env::var_os("COBALTC_RESULT_STORE")?;
            let k = std::env::var("COBALTC_TOOLCHAIN_KEY").ok().filter(|k| !k.is_empty())?;
            let d = PathBuf::from(d);
            std::fs::create_dir_all(&d).ok()?;
            Some((d, k))
        })
        .as_ref()
    }

    pub fn enabled() -> bool {
        dir().is_some()
    }

    // Two FNV-1a hashes of 64 bits with different offsets: a key of 128
    // bits, from the standard library alone (Rust 1.77.2, no new crates).
    pub fn hash(parts: &[&[u8]]) -> String {
        let mut a: u64 = 0xcbf2_9ce4_8422_2325;
        let mut b: u64 = 0x6c62_272e_07bb_0142;
        let mut eat = |bytes: &[u8]| {
            for &x in bytes {
                a = (a ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3);
                b = (b ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(5);
            }
        };
        for p in parts {
            eat(&(p.len() as u64).to_le_bytes());
            eat(p);
        }
        format!("{:016x}{:016x}", a, b)
    }

    // The files a case of the file-based corpus may read besides itself:
    // every file under the corpus directory that is not a case (fixtures,
    // data, C code; not the runner `run.py` or the `.md` files), hashed once
    // per process. Outside that directory, none.
    fn support(path: &Path) -> &'static str {
        static SUPPORT: OnceLock<String> = OnceLock::new();
        let in_corpus = path.components().any(|c| c.as_os_str() == "conformance");
        if !in_corpus {
            return "";
        }
        SUPPORT.get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(if Path::new(env!("CARGO_MANIFEST_DIR")).ends_with("cobc") { "../conformance" } else { "conformance" });
            let mut files = Vec::new();
            walk(&root, &mut files);
            files.sort();
            let mut parts: Vec<Vec<u8>> = Vec::new();
            for f in files {
                let bytes = std::fs::read(&f).unwrap_or_default();
                let is_case = f.extension().map_or(false, |e| e == "cb") && !bytes.starts_with(b"// CONFORMANCE-FIXTURE");
                // The corpus's own runner and its documentation are no case's input.
                let tooling = f.extension().map_or(false, |e| e == "py" || e == "md");
                if !is_case && !tooling {
                    parts.push(f.strip_prefix(&root).unwrap_or(&f).to_string_lossy().into_owned().into_bytes());
                    parts.push(bytes);
                }
            }
            let refs: Vec<&[u8]> = parts.iter().map(|p| p.as_slice()).collect();
            hash(&refs)
        })
    }

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }

    // The key of one run: `what` names the runner's kind of result, `path`
    // is the case as it is passed (diagnostics print it), `extra` anything
    // else the result depends on (a C compiler, the stdin, the expectation).
    pub fn key(what: &str, path: &Path, src: &[u8], extra: &[&[u8]]) -> Option<String> {
        let (_, tool) = dir()?;
        let p = path.to_string_lossy();
        let mut parts: Vec<&[u8]> = vec![tool.as_bytes(), what.as_bytes(), p.as_bytes(), src, support(path).as_bytes()];
        let checks = std::env::var("COBALTC_ALL_CHECKS").unwrap_or_default();
        parts.push(checks.as_bytes());
        parts.extend_from_slice(extra);
        Some(hash(&parts))
    }

    fn file(key: &str) -> Option<PathBuf> {
        let (d, _) = dir()?;
        Some(d.join(&key[..2]).join(key))
    }

    // A verdict: the case met its check under this key.
    pub fn passed(key: &Option<String>) -> bool {
        key.as_deref().and_then(file).map_or(false, |f| f.exists())
    }

    pub fn mark(key: &Option<String>) {
        put(key, b"pass");
    }

    // A run's exit code, stdout and stderr, kept so another runner checking
    // the same program the same way reads it instead of running it again.
    pub fn get_run(key: &Option<String>) -> Option<(i32, Vec<u8>, Vec<u8>)> {
        let bytes = std::fs::read(file(key.as_deref()?)?).ok()?;
        let nl = bytes.iter().position(|&b| b == b'\n')?;
        let head = std::str::from_utf8(&bytes[..nl]).ok()?;
        let mut it = head.split(' ');
        let code: i32 = it.next()?.parse().ok()?;
        let no: usize = it.next()?.parse().ok()?;
        let ne: usize = it.next()?.parse().ok()?;
        let body = &bytes[nl + 1..];
        if body.len() != no + ne {
            return None;
        }
        Some((code, body[..no].to_vec(), body[no..].to_vec()))
    }

    pub fn put_run(key: &Option<String>, code: i32, stdout: &[u8], stderr: &[u8]) {
        let mut v = format!("{} {} {}\n", code, stdout.len(), stderr.len()).into_bytes();
        v.extend_from_slice(stdout);
        v.extend_from_slice(stderr);
        put(key, &v);
    }

    fn put(key: &Option<String>, bytes: &[u8]) {
        let Some(f) = key.as_deref().and_then(file) else { return };
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        // Written aside and renamed: a reader never sees half a record.
        let tmp = f.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &f);
        }
    }

    // The exit status as a run records it: the code, or -1 for a signal.
    pub fn code_of(status: &std::process::ExitStatus) -> i32 {
        status.code().unwrap_or(-1)
    }

    // An `ExitStatus` with this code, for a run read from the store.
    #[cfg(unix)]
    pub fn status_of(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        if code < 0 { std::process::ExitStatus::from_raw(9) } else { std::process::ExitStatus::from_raw(code << 8) }
    }
    #[cfg(windows)]
    pub fn status_of(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}
