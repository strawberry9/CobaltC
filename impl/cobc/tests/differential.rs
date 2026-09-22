// Differential testing: random, well-typed CobaltC programs, each run by
// `coby` and by `cobc --run`, must produce the same exit status, stdout
// and stderr (the diagnostic, location included). The programs lean on
// what the compiler proves things about: `Vec` pushes, element reads and
// writes, element references held across pushes, reference parameters
// (passed on, recursive, returning a reference, two vectors or one twice),
// moves, resources ending in nested blocks, and arithmetic that may
// overflow, divide by zero or narrow badly. Many fault; that is the point,
// as long as both tools fault the same way.
//
//   cargo test --release -p cobc --test differential
//   COBC_DIFF_N=2000 COBC_DIFF_SEED=7 cargo test --release -p cobc --test differential -- --nocapture
//
// `COBC_DIFF_N` programs (default 100) from seeds `COBC_DIFF_SEED`,
// `COBC_DIFF_SEED + 1`, … (default 1). A program that disagrees is kept
// in `target/differential-failures/seed-<n>.cb`; rerun one seed with
// `COBC_DIFF_SEED=<n> COBC_DIFF_N=1`. `COBC_DIFF_KEEP=<dir>` keeps every
// generated program there, to read or to compile by hand.

#[allow(dead_code)]
mod oracle;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

// xorshift64*: deterministic across platforms and Rust versions.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Ty {
    I32,
    Usize,
    Vec,
    LocalVec, // used only through push, len and element access: confinable
    HelperVec, // also passed to the confining helpers: confinable across calls
    Tracer,
    Stash, // Vec<ref<i32, shared>>: element references held across statements
}

const PRELUDE: &str = r#"import std;

fn byte(u8 b)
{
    auto buf = [b];
    rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf));
    unsafe
    {
        write(p, 1);
    }
}

fn digits(u64 n)
{
    if (n >= 10)
    {
        digits(n / 10);
    }
    byte(narrow<u8>(n % 10) + 48);
}

fn show(i64 n)
{
    if (n < 0)
    {
        printf("-");
        digits(wrapping_sub(0: u64, reinterpret<u64>(n)));
    }
    else
    {
        digits(reinterpret<u64>(n));
    }
    printf("\n");
}

fn showu(u64 n)
{
    digits(n);
    printf("\n");
}

resource struct Tracer
{
    i32 id;
}

fn Tracer::drop(ref<Tracer, exclusive> self)
{
    printf("drop ");
    show(widen<i64>(self.id));
}

fn make(i32 id) : Tracer
{
    Tracer { .id = id }
}

fn sum(ref<Vec<i32>, shared> v) : i64
{
    i64 s = 0;
    usize i = 0;
    while (i < Vec::len(v))
    {
        s = s + widen<i64>(*Vec::index_shared(v, i));
        i = i + 1;
    }
    s
}

fn fill(ref<Vec<i32>, exclusive> v, i32 n)
{
    i32 k = 0;
    while (k < n)
    {
        Vec::push(v, k * 3 - 4);
        k = k + 1;
    }
}

fn set(ref<Vec<i32>, exclusive> v, usize i, i32 x)
{
    *Vec::index_exclusive(v, i) = x;
}

fn takes(Vec<i32> v) : usize
{
    Vec::len(&v)
}

fn fill_twice(ref<Vec<i32>, exclusive> v, i32 n)
{
    fill(v, n);
    fill(&mut *v, n);
}

fn sum_from(ref<Vec<i32>, shared> v, usize i) : i64
{
    if (i >= Vec::len(v))
    {
        0
    }
    else
    {
        widen<i64>(*Vec::index_shared(v, i)) + sum_from(v, i + 1)
    }
}

fn first_ref(ref<Vec<i32>, shared> v) : ref<i32, shared>
{
    Vec::index_shared(v, 0)
}

fn copy_first(ref<Vec<i32>, exclusive> a, ref<Vec<i32>, shared> b)
{
    if (Vec::len(b) > 0)
    {
        Vec::push(a, *Vec::index_shared(b, 0));
    }
}

"#;

struct Gen {
    rng: Rng,
    out: String,
    ind: usize,
    scopes: Vec<Vec<(String, Ty)>>,
    next: u32,
}

impl Gen {
    fn line(&mut self, s: &str) {
        for _ in 0..self.ind {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }
    fn fresh(&mut self, p: &str) -> String {
        self.next += 1;
        format!("{}{}", p, self.next)
    }
    fn vars(&self, ty: Ty) -> Vec<String> {
        self.scopes.iter().flatten().filter(|(_, t)| *t == ty).map(|(n, _)| n.clone()).collect()
    }
    // Every vector, for the uses both kinds allow.
    fn any_vecs(&self) -> Vec<String> {
        self.scopes.iter().flatten().filter(|(_, t)| matches!(t, Ty::Vec | Ty::LocalVec | Ty::HelperVec)).map(|(n, _)| n.clone()).collect()
    }
    // The vectors the confining helpers (`sum`, `fill`, `set`,
    // `fill_twice`, `sum_from`, `copy_first` on two) may be given.
    fn helper_vecs(&self) -> Vec<String> {
        self.scopes.iter().flatten().filter(|(_, t)| matches!(t, Ty::Vec | Ty::HelperVec)).map(|(n, _)| n.clone()).collect()
    }
    fn bind(&mut self, name: &str, ty: Ty) {
        self.scopes.last_mut().unwrap().push((name.to_string(), ty));
    }
    fn forget(&mut self, name: &str) {
        for s in &mut self.scopes {
            s.retain(|(n, _)| n != name);
        }
    }

    fn int_lit(&mut self) -> String {
        match self.rng.below(10) {
            0 => format!("{}", 2_147_483_000 + self.rng.below(647)),
            1 => format!("-{}", self.rng.below(50)),
            _ => format!("{}", self.rng.below(20)),
        }
    }

    // An `i32` expression.
    fn iexpr(&mut self, depth: u32) -> String {
        let vars = self.vars(Ty::I32);
        let vecs = self.any_vecs();
        let shared_vecs = self.helper_vecs();
        let k = if depth == 0 { self.rng.below(3) } else { self.rng.below(9) };
        match k {
            0 => self.int_lit(),
            1 | 2 if !vars.is_empty() => self.rng.pick(&vars).clone(),
            3 | 4 => {
                let op = *self.rng.pick(&["+", "-", "*", "+", "-", "/", "%"]);
                let (a, b) = (self.iexpr(depth - 1), self.iexpr(depth - 1));
                format!("({} {} {})", a, op, b)
            }
            5 | 6 if !vecs.is_empty() => {
                let v = self.rng.pick(&vecs).clone();
                let i = self.uexpr(depth - 1, Some(&v));
                format!("*Vec::index_shared(&{}, {})", v, i)
            }
            7 if !vecs.is_empty() => {
                let v = self.rng.pick(&vecs).clone();
                format!("narrow<i32>(Vec::len(&{}))", v)
            }
            8 if !shared_vecs.is_empty() => {
                let v = self.rng.pick(&shared_vecs).clone();
                format!("narrow<i32>(sum(&{}))", v)
            }
            _ => self.int_lit(),
        }
    }

    // A `usize` expression, usually an index into `v` when one is given.
    fn uexpr(&mut self, depth: u32, v: Option<&str>) -> String {
        let vars = self.vars(Ty::Usize);
        match self.rng.below(6) {
            0 | 1 if !vars.is_empty() => self.rng.pick(&vars).clone(),
            2 if v.is_some() => format!("Vec::len(&{}) - 1", v.unwrap()),
            3 if depth > 0 => {
                let e = self.iexpr(depth - 1);
                format!("narrow<usize>({})", e)
            }
            _ => format!("{}", self.rng.below(4)),
        }
    }

    fn bexpr(&mut self, depth: u32) -> String {
        let op = *self.rng.pick(&["<", "<=", "==", "!=", ">", ">="]);
        let (a, b) = (self.iexpr(depth), self.iexpr(depth));
        format!("{} {} {}", a, op, b)
    }

    fn block(&mut self, n: u64, depth: u32) {
        self.scopes.push(Vec::new());
        for _ in 0..n {
            self.stmt(depth);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, depth: u32) {
        let vecs = self.vars(Ty::Vec);
        let helper = self.helper_vecs();
        let all_vecs = self.any_vecs();
        let ints = self.vars(Ty::I32);
        let tracers = self.vars(Ty::Tracer);
        let stashes = self.vars(Ty::Stash);
        match self.rng.below(27) {
            0 | 1 => {
                let x = self.fresh("x");
                let e = self.iexpr(2);
                self.line(&format!("i32 {} = {};", x, e));
                self.bind(&x, Ty::I32);
            }
            2 if !ints.is_empty() => {
                let x = self.rng.pick(&ints).clone();
                let e = self.iexpr(2);
                self.line(&format!("{} = {};", x, e));
            }
            3 => {
                let v = self.fresh("v");
                self.line(&format!("Vec<i32> {} = Vec::new();", v));
                let ty = *self.rng.pick(&[Ty::LocalVec, Ty::HelperVec, Ty::HelperVec, Ty::Vec]);
                self.bind(&v, ty);
            }
            4 | 5 | 6 if !all_vecs.is_empty() => {
                let v = self.rng.pick(&all_vecs).clone();
                let e = self.iexpr(1);
                self.line(&format!("Vec::push(&mut {}, {});", v, e));
            }
            7 | 8 if !all_vecs.is_empty() => {
                let v = self.rng.pick(&all_vecs).clone();
                let i = self.uexpr(1, Some(&v));
                let e = self.iexpr(1);
                self.line(&format!("*Vec::index_exclusive(&mut {}, {}) = {};", v, i, e));
            }
            9 | 10 => {
                let e = self.iexpr(2);
                self.line(&format!("show(widen<i64>({}));", e));
            }
            11 if !all_vecs.is_empty() => {
                let v = self.rng.pick(&all_vecs).clone();
                if self.rng.chance(50) || !helper.contains(&v) {
                    self.line(&format!("showu(widen<u64>(Vec::len(&{})));", v));
                } else {
                    self.line(&format!("show(sum(&{}));", v));
                }
            }
            12 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                let n = self.rng.below(12);
                self.line(&format!("fill(&mut {}, {});", v, n));
            }
            13 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                let i = self.uexpr(1, Some(&v));
                let e = self.iexpr(1);
                self.line(&format!("set(&mut {}, {}, {});", v, i, e));
            }
            14 if depth > 0 => {
                let k = self.fresh("k");
                let n = self.rng.below(5);
                self.line(&format!("usize {} = 0;", k));
                self.line(&format!("while ({} < {})", k, n));
                self.line("{");
                self.ind += 1;
                self.scopes.push(vec![(k.clone(), Ty::Usize)]);
                let m = 1 + self.rng.below(4);
                for _ in 0..m {
                    self.stmt(depth - 1);
                }
                self.scopes.pop();
                self.line(&format!("{} = {} + 1;", k, k));
                self.ind -= 1;
                self.line("}");
            }
            15 if depth > 0 => {
                let c = self.bexpr(1);
                self.line(&format!("if ({})", c));
                self.line("{");
                self.ind += 1;
                let m = 1 + self.rng.below(3);
                self.block(m, depth - 1);
                self.ind -= 1;
                self.line("}");
                if self.rng.chance(50) {
                    self.line("else");
                    self.line("{");
                    self.ind += 1;
                    let m = 1 + self.rng.below(3);
                    self.block(m, depth - 1);
                    self.ind -= 1;
                    self.line("}");
                }
            }
            16 if depth > 0 => {
                // A resource that ends with its block, after what the
                // block does (or during the unwind, if that faults).
                let t = self.fresh("t");
                let id = self.rng.below(100);
                self.line("{");
                self.ind += 1;
                self.scopes.push(Vec::new());
                self.line(&format!("auto {} = make({});", t, id));
                self.bind(&t, Ty::Tracer);
                let m = 1 + self.rng.below(3);
                for _ in 0..m {
                    self.stmt(depth - 1);
                }
                self.scopes.pop();
                self.ind -= 1;
                self.line("}");
            }
            17 if !tracers.is_empty() => {
                let t = self.rng.pick(&tracers).clone();
                self.line(&format!("drop({});", t));
                self.forget(&t);
            }
            18 if !vecs.is_empty() => {
                // A move: later uses of the vector are rejected statically,
                // or fault at run time inside a loop.
                let v = self.rng.pick(&vecs).clone();
                self.line(&format!("showu(widen<u64>(takes({})));", v));
                self.forget(&v);
            }
            19 if !vecs.is_empty() => {
                // Hold a reference to an element in another vector, so the
                // static pass cannot see it: a push that reallocates makes
                // it stale.
                let v = self.rng.pick(&vecs).clone();
                let h = self.fresh("h");
                let i = self.uexpr(0, Some(&v));
                self.line(&format!("Vec<ref<i32, shared>> {} = Vec::new();", h));
                self.line(&format!("Vec::push(&mut {}, Vec::index_shared(&{}, {}));", h, v, i));
                self.bind(&h, Ty::Stash);
            }
            20 if !stashes.is_empty() => {
                let h = self.rng.pick(&stashes).clone();
                self.line(&format!("show(widen<i64>(**Vec::index_shared(&{}, 0)));", h));
            }
            21 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                let n = self.rng.below(6);
                self.line(&format!("fill_twice(&mut {}, {});", v, n));
            }
            22 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                self.line(&format!("show(sum_from(&{}, 0));", v));
            }
            23 if !vecs.is_empty() => {
                let v = self.rng.pick(&vecs).clone();
                self.line(&format!("show(widen<i64>(*first_ref(&{})));", v));
            }
            24 if !vecs.is_empty() => {
                // Two vectors, or one passed twice (a conflict at the push).
                let a = self.rng.pick(&vecs).clone();
                let b = if self.rng.chance(30) { a.clone() } else { self.rng.pick(&vecs).clone() };
                self.line(&format!("copy_first(&mut {}, &{});", a, b));
            }
            25 if helper.len() >= 2 => {
                // Two distinct confinable vectors: both confined in the call.
                let a = self.rng.pick(&helper).clone();
                let b = self.rng.pick(&helper).clone();
                if a != b {
                    self.line(&format!("copy_first(&mut {}, &{});", a, b));
                }
            }
            _ => {
                let e = self.iexpr(1);
                self.line(&format!("show(widen<i64>({}));", e));
            }
        }
    }
}

fn program(seed: u64) -> String {
    let mut g = Gen { rng: Rng::new(seed), out: String::new(), ind: 1, scopes: vec![Vec::new()], next: 0 };
    g.out.push_str(PRELUDE);
    writeln!(g.out, "// seed {}", seed).unwrap();
    g.out.push_str("fn main()\n{\n");
    let n = 6 + g.rng.below(20);
    for _ in 0..n {
        g.stmt(2);
    }
    g.out.push_str("}\n");
    g.out
}

fn run(cmd: &mut Command, limit: Duration) -> Option<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn");
    let start = Instant::now();
    loop {
        if child.try_wait().expect("wait").is_some() {
            return Some(child.wait_with_output().expect("output"));
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

enum Verdict {
    // How the program ended: ran to completion, or its diagnostic.
    Same(String),
    Skipped,
    Differs(String),
}

// A program compiled under every C compiler must behave as `coby` runs it.
fn check(dir: &Path, seed: u64) -> Verdict {
    let path = dir.join(format!("seed-{}.cb", seed));
    fs::write(&path, program(seed)).unwrap();
    let limit = Duration::from_secs(60);
    let Some(a) = run(Command::new(oracle::coby()).arg(&path), limit) else { return Verdict::Differs("coby timed out".into()) };
    let mut result = Verdict::Skipped;
    for cc in oracle::c_compilers() {
        let Some(c) = run(oracle::cobc(cc).arg("--run").arg(&path), limit) else { return Verdict::Differs(format!("cobc [{}] timed out", cc)) };
        if c.status.code() == Some(3) {
            return Verdict::Skipped;
        }
        match compare(&a, &c) {
            Verdict::Differs(why) => return Verdict::Differs(format!("[{}] {}", cc, why)),
            v => result = v,
        }
    }
    result
}

fn compare(a: &Output, c: &Output) -> Verdict {
    let mut why = String::new();
    if a.status.code() != c.status.code() {
        writeln!(why, "exit status: coby {:?}, cobc {:?}", a.status.code(), c.status.code()).unwrap();
    }
    if a.stdout != c.stdout {
        writeln!(why, "stdout:\n--- coby\n{}--- cobc\n{}", String::from_utf8_lossy(&a.stdout), String::from_utf8_lossy(&c.stdout)).unwrap();
    }
    if a.stderr != c.stderr {
        writeln!(why, "stderr:\n--- coby\n{}--- cobc\n{}", String::from_utf8_lossy(&a.stderr), String::from_utf8_lossy(&c.stderr)).unwrap();
    }
    if why.is_empty() {
        let first = String::from_utf8_lossy(&a.stderr).lines().next().unwrap_or("").to_string();
        let how = if a.status.success() {
            "ran to completion".to_string()
        } else {
            first.trim_start_matches("error: ").to_string()
        };
        Verdict::Same(how)
    } else {
        Verdict::Differs(why)
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[test]
fn random_programs_agree_under_coby_and_cobc() {
    let n = env_u64("COBC_DIFF_N", 100);
    let first = env_u64("COBC_DIFF_SEED", 1);
    let target = Path::new(env!("CARGO_BIN_EXE_cobc")).parent().unwrap().parent().unwrap().to_path_buf();
    let work: PathBuf = std::env::temp_dir().join(format!("cobc-diff-{}", std::process::id()));
    let keep = target.join("differential-failures");
    fs::create_dir_all(&work).unwrap();
    oracle::coby(); // build it once, before the workers race to

    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()) as u64;
    let results: Vec<(u64, Verdict)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..workers)
            .map(|w| {
                let work = &work;
                s.spawn(move || (first + w..first + n).step_by(workers as usize).map(|seed| (seed, check(work, seed))).collect::<Vec<_>>())
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });

    let mut tally: HashMap<&str, usize> = HashMap::new();
    let mut ends: HashMap<&str, usize> = HashMap::new();
    let mut failures = Vec::new();
    for (seed, v) in &results {
        match v {
            Verdict::Same(how) => {
                *tally.entry("same").or_default() += 1;
                *ends.entry(how.as_str()).or_default() += 1;
            }
            Verdict::Skipped => *tally.entry("skipped").or_default() += 1,
            Verdict::Differs(why) => {
                fs::create_dir_all(&keep).unwrap();
                let kept = keep.join(format!("seed-{}.cb", seed));
                fs::copy(work.join(format!("seed-{}.cb", seed)), &kept).unwrap();
                failures.push(format!("seed {} ({}):\n{}", seed, kept.display(), why));
            }
        }
    }
    if let Ok(dir) = std::env::var("COBC_DIFF_KEEP") {
        fs::create_dir_all(&dir).unwrap();
        for (seed, _) in &results {
            let name = format!("seed-{}.cb", seed);
            fs::copy(work.join(&name), Path::new(&dir).join(&name)).unwrap();
        }
    }
    let _ = fs::remove_dir_all(&work);
    eprintln!(
        "differential: {} programs from seed {}: {} same, {} skipped, {} differ",
        n,
        first,
        tally.get("same").unwrap_or(&0),
        tally.get("skipped").unwrap_or(&0),
        failures.len()
    );
    let mut ends: Vec<_> = ends.into_iter().collect();
    ends.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (how, k) in ends {
        eprintln!("  {:>5}  {}", k, how);
    }
    assert!(failures.is_empty(), "{} program(s) disagree:\n{}", failures.len(), failures.join("\n"));
}
