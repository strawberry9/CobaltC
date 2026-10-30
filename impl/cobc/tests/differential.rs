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
// Since 2026-10-12 (WI-2) they also reach threads (a borrow lent to a
// thread, a reference held in a local and lent, an exclusive reference in a
// struct passed to a thread, two threads at once), a mutex several threads
// add under, channels of values and of exclusive references, closures that
// write their captures called directly and through `ref<fn, exclusive>` or
// `ref<fn, shared>` (D-0218), a closure borrowing an outer variable, `match`
// on `HashMap::get_mut` and `Vec::pop`, and a `StringView` held across an
// append. Later the same day: a vector lent shared to one or two
// reading threads and exclusive to a pushing one, a `Vec<ref>` lent to a
// thread, structs holding exclusive references sent through a channel,
// `move` closures that copy an integer or take a vector, and
// `HashMap::insert`/`remove`. Each has one outcome whatever the schedule; a program with a
// thread is compiled once per C compiler and run as threads really run and
// under schedule seeds 1 to 3, and every run must be coby's. Shapes whose
// outcome depends on the schedule are the model's (`impl/model`).
//
//   cargo test --release -p cobc --test differential
//   COBC_DIFF_N=2000 COBC_DIFF_SEED=7 cargo test --release -p cobc --test differential -- --nocapture
//
// `COBC_DIFF_ALL_CHECKS=1` compiles with `cobc --all-checks`.
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
    Map,   // HashMap<i64, i64>, driven through the tally/mget helpers
    Str,   // a String of space-separated numbers, split and re-parsed
    PtVec, // Vec<Pt>: elements updated field by field
}

const PRELUDE: &str = r#"import std;

fn byte(u8 b)
{
    auto buf = [b];
    rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf));
    unsafe
    {
        stdout_write(p, 1);
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

fn tally(ref<HashMap<i64, i64>, exclusive> m, i64 k)
{
    *HashMap::entry(m, k, 0) += 1;
}

fn mget(ref<HashMap<i64, i64>, shared> m, i64 k) : i64
{
    match (HashMap::get(m, &k))
    {
        Some(v) : *v,
        None : 0 - 1,
    }
}

struct Pt
{
    i32 x;
    i32 y;
}

fn grow_pts(ref<Vec<Pt>, exclusive> v) : i32
{
    usize n = 0;
    while (n < 40)
    {
        Vec::push(v, Pt { .x = 1, .y = 2 });
        n = n + 1;
    }
    3
}

fn two(i32 a, i32 b) : i64
{
    widen<i64>(a) * 1000 + widen<i64>(b)
}

fn vcount(StringView s, str t) : i64
{
    i64 n = 0;
    StringView rest = s;
    while (Some(i) = StringView::find(rest, StringView::of(t)))
    {
        n = n + 1;
        rest = StringView::sub(rest, i + 1, StringView::len(rest));
    }
    n * 100 + widen<i64>(narrow<i32>(StringView::len(s)))
}

fn owned_total(Vec<i32> v) : Vec<i32>
{
    i64 t = 0;
    usize i = 0;
    while (i < Vec::len(&v))
    {
        t = t + widen<i64>(v[i]);
        i = i + 1;
    }
    show(t);
    v
}

fn mword(ref<String, shared> s, usize i) : i64
{
    auto parts = StringView::split(String::view(s, 0, String::len(s)), " ");
    match (StringView::parse<i64>(parts[i]))
    {
        Ok(n) : n,
        Err(_) : 0 - 2,
    }
}

fn bump(ref<i32, exclusive> p, i32 k)
{
    *p = *p + k;
}

struct Hold
{
    ref<i32, exclusive> r;
}

fn bump_hold(Hold h)
{
    *h.r = *h.r + 1;
}

fn read_hold(ref<Hold, shared> h) : i32
{
    *h.r
}

fn add_locked(ref<mutex<i64>, shared> m, i64 k)
{
    auto g = lock(m);
    *g = *g + k;
}

fn sum_chan(ref<Channel<i64>, shared> ch) : i64
{
    i64 s = 0;
    while (Some(x) = Channel::recv(ch))
    {
        s = s + x;
    }
    s
}

fn take_refs(ref<Channel<ref<i32, exclusive>>, shared> ch)
{
    while (Some(r) = Channel::recv(ch))
    {
        *r = *r + 100;
    }
}

fn apply_mut(ref<fn(i32) : i32, exclusive> f, i32 k) : i32
{
    (*f)(k)
}

fn apply_shared(ref<fn(i32) : i32, shared> f, i32 k) : i32
{
    (*f)(k)
}

fn sum_refs(ref<Vec<ref<i32, shared>>, shared> rs) : i64
{
    i64 s = 0;
    usize i = 0;
    while (i < Vec::len(rs))
    {
        s = s + widen<i64>(**Vec::index_shared(rs, i));
        i = i + 1;
    }
    s
}

fn take_holds(ref<Channel<Hold>, shared> ch)
{
    while (Some(h) = Channel::recv(ch))
    {
        *h.r = *h.r + 10;
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
        let maps = self.vars(Ty::Map);
        let strs = self.vars(Ty::Str);
        let pts = self.vars(Ty::PtVec);
        match self.rng.below(66) {
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
                if self.rng.chance(50) {
                    // A plain variable beside the vector, used for the last
                    // time in the call: the vector stays confined.
                    let c = self.fresh("c");
                    self.line(&format!("i32 {} = {};", c, n));
                    self.line(&format!("fill_twice(&mut {}, {});", v, c));
                } else {
                    self.line(&format!("fill_twice(&mut {}, {});", v, n));
                }
            }
            22 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                if self.rng.chance(50) {
                    let c = self.fresh("c");
                    self.line(&format!("usize {} = 0;", c));
                    self.line(&format!("show(sum_from(&{}, {}));", v, c));
                } else {
                    self.line(&format!("show(sum_from(&{}, 0));", v));
                }
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
            27 => {
                let m = self.fresh("m");
                self.line(&format!("HashMap<i64, i64> {} = HashMap::new();", m));
                self.bind(&m, Ty::Map);
            }
            28 | 29 if !maps.is_empty() => {
                let m = self.rng.pick(&maps).clone();
                let e = self.iexpr(1);
                self.line(&format!("tally(&mut {}, widen<i64>({}));", m, e));
            }
            30 if !maps.is_empty() => {
                let m = self.rng.pick(&maps).clone();
                let e = self.iexpr(1);
                self.line(&format!("show(mget(&{}, widen<i64>({})));", m, e));
            }
            31 if !maps.is_empty() => {
                let m = self.rng.pick(&maps).clone();
                if self.rng.chance(50) {
                    // In or out of bounds: index parity through the map's
                    // public insertion order.
                    let i = self.uexpr(1, None);
                    self.line(&format!("show(*HashMap::value_at(&{}, {}));", m, i));
                } else {
                    let e = self.iexpr(1);
                    self.line(&format!("show(Option::unwrap_or(HashMap::swap_remove(&mut {}, &widen<i64>({})), 0 - 7));", m, e));
                }
            }
            32 => {
                let s = self.fresh("s");
                let (a, b, c) = (self.iexpr(1), self.iexpr(1), self.iexpr(1));
                self.line(&format!("String {} = sprintf(\"%v %v %v\", {}, {}, {});", s, a, b, c));
                self.bind(&s, Ty::Str);
            }
            33 if !strs.is_empty() => {
                let s = self.rng.pick(&strs).clone();
                let i = self.uexpr(1, None);
                self.line(&format!("show(mword(&{}, {}));", s, i));
            }
            34 if depth > 0 => {
                // A counter closure in its own scope, so the exclusive
                // capture never collides with later statements.
                let x = self.fresh("x");
                let c = self.fresh("c");
                let e = self.iexpr(1);
                let n = 1 + self.rng.below(3);
                self.line("{");
                self.ind += 1;
                self.line(&format!("i32 {} = {};", x, e));
                self.line(&format!("auto {} = [{}]() : i64 {{ {} += 1; widen<i64>({}) }};", c, x, x, x));
                for _ in 0..n {
                    self.line(&format!("show({}());", c));
                }
                self.ind -= 1;
                self.line("}");
            }
            35 => {
                let p = self.fresh("p");
                self.line(&format!("Vec<Pt> {} = Vec::new();", p));
                let n = 1 + self.rng.below(5);
                for _ in 0..n {
                    let (a, b) = (self.iexpr(1), self.iexpr(1));
                    self.line(&format!("Vec::push(&mut {}, Pt {{ .x = {}, .y = {} }});", p, a, b));
                }
                self.bind(&p, Ty::PtVec);
            }
            36 | 37 if !pts.is_empty() => {
                let p = self.rng.pick(&pts).clone();
                let i = self.uexpr(1, Some(&p));
                let f = *self.rng.pick(&["x", "y"]);
                let op = *self.rng.pick(&["=", "+=", "-=", "*="]);
                let e = if self.rng.chance(30) { format!("{}[{}].{}", p, self.uexpr(0, Some(&p)), *self.rng.pick(&["x", "y"])) } else { self.iexpr(1) };
                self.line(&format!("{}[{}].{} {} {};", p, i, f, op, e));
            }
            38 if !pts.is_empty() => {
                let p = self.rng.pick(&pts).clone();
                let q = self.fresh("q");
                let e = self.iexpr(1);
                self.line(&format!("foreach ({} in &mut {})", q, p));
                self.line("{");
                self.line(&format!("    {}.x = {}.y + {};", q, q, e));
                self.line(&format!("    {}.y -= 1;", q));
                self.line("}");
            }
            39 if !pts.is_empty() => {
                let p = self.rng.pick(&pts).clone();
                let r = self.fresh("r");
                let i = self.uexpr(1, Some(&p));
                let e = self.iexpr(0);
                self.line("{");
                self.line(&format!("    ref<Pt, exclusive> {} = &mut {}[{}];", r, p, i));
                self.line(&format!("    {}.y = {}.x * 2 + {};", r, r, e));
                self.line(&format!("    (*{}).x += 1;", r));
                self.line("}");
            }
            40 if !pts.is_empty() => {
                let p = self.rng.pick(&pts).clone();
                let i = self.uexpr(0, Some(&p));
                let mut line = format!("show(widen<i64>({}[{}].x) + widen<i64>({}[{}].y));", p, i, p, i);
                if self.rng.chance(15) {
                    // The vector grows under the element the write reached:
                    // stale, in both tools.
                    line = format!("{}[{}].x = grow_pts(&mut {});", p, i, p);
                }
                self.line(&line);
            }
            41 if !ints.is_empty() => {
                let x = self.rng.pick(&ints).clone();
                let e = self.iexpr(1);
                let e2 = self.iexpr(0);
                self.line(&format!("show(two({}, {{ {} = {}; {} }}));", x, x, e, e2));
            }
            42 if !all_vecs.is_empty() && depth > 0 => {
                let v = self.rng.pick(&all_vecs).clone();
                let i = self.uexpr(0, Some(&v));
                let e = self.iexpr(0);
                self.line(&format!("show(two(*Vec::index_shared(&{}, {}), {{ *Vec::index_exclusive(&mut {}, {}) = {}; 1 }}));", v, i, v, i, e));
            }
            43 if !strs.is_empty() => {
                let s = self.rng.pick(&strs).clone();
                let t = *self.rng.pick(&["\" \"", "\"1\"", "\"-\"", "\"0\""]);
                if self.rng.chance(50) {
                    self.line(&format!("show(vcount(String::as_view(&{}), {}));", s, t));
                } else {
                    let w = self.fresh("w");
                    self.line("{");
                    self.line(&format!("    StringView {} = String::as_view(&{});", w, s));
                    self.line(&format!("    show(vcount({}, {}) + vcount(StringView::trim({}), {}));", w, t, w, t));
                    self.line("}");
                }
            }
            44 if !helper.is_empty() => {
                let v = self.rng.pick(&helper).clone();
                if self.rng.chance(50) {
                    let i = self.uexpr(0, Some(&v));
                    self.line(&format!("show(widen<i64>(Vec::remove(&mut {}, {})));", v, i));
                } else {
                    let e = self.iexpr(0);
                    self.line(&format!("if (Vec::contains(&{}, &({}))) {{ show(1); }} else {{ show(0); }}", v, e));
                }
            }
            45 if !vecs.is_empty() => {
                let v = self.rng.pick(&vecs).clone();
                let w = self.fresh("v");
                self.line(&format!("Vec<i32> {} = owned_total({});", w, v));
                self.forget(&v);
                self.bind(&w, Ty::Vec);
            }
            // Threads, closures, `match`, channels, maps and views (WI-2,
            // 2026-10-12). Only shapes with one outcome whatever the
            // schedule: a racy shape belongs to the model (`impl/model`),
            // which knows every outcome the rules allow.
            46 | 47 if !ints.is_empty() => {
                // A thread lent a borrow made in the call, joined at once.
                let x = self.rng.pick(&ints).clone();
                let e = self.iexpr(0);
                self.line(&format!("{{ auto t = spawn(bump, &mut {}, {}); join(t); }}", x, e));
            }
            48 if !ints.is_empty() => {
                // A reference held in a local, lent to a thread: the
                // spawner's copy is usable again after the join. Used
                // before it, or lent twice, is rejected statically.
                let x = self.rng.pick(&ints).clone();
                let r = self.fresh("r");
                let t = self.fresh("t");
                self.line("{");
                self.line(&format!("    auto {} = &mut {};", r, x));
                self.line(&format!("    auto {} = spawn(bump, {}, 2);", t, r));
                match self.rng.below(10) {
                    0 => self.line(&format!("    *{} = 3;", r)),
                    1 => self.line(&format!("    join(spawn(bump, {}, 1));", r)),
                    _ => {}
                }
                self.line(&format!("    join({});", t));
                self.line(&format!("    *{} = *{} + 1;", r, r));
                self.line("}");
            }
            49 if !ints.is_empty() => {
                // An exclusive reference inside a struct passed by value:
                // the copy a thread receives is lent (D-0213); read
                // through a shared reference to the struct (D-0218).
                let x = self.rng.pick(&ints).clone();
                let h = self.fresh("h");
                let t = self.fresh("t");
                self.line("{");
                self.line(&format!("    Hold {} = Hold {{ .r = &mut {} }};", h, x));
                self.line(&format!("    show(widen<i64>(read_hold(&{})));", h));
                self.line(&format!("    auto {} = spawn(bump_hold, {});", t, h));
                self.line(&format!("    join({});", t));
                self.line(&format!("    *{}.r = *{}.r + 1;", h, h));
                self.line("}");
            }
            50 => {
                // Threads adding under one mutex: the sum is the same in
                // every order.
                let m = self.fresh("m");
                let n = 1 + self.rng.below(3);
                self.line("{");
                self.line("    i64 z = 0;");
                self.line(&format!("    mutex<i64> {} = Mutex::new(z);", m));
                let mut ts = Vec::new();
                for _ in 0..n {
                    let t = self.fresh("t");
                    let k = self.rng.below(50);
                    self.line(&format!("    auto {} = spawn(add_locked, &{}, {});", t, m, k));
                    ts.push(t);
                }
                for t in ts {
                    self.line(&format!("    join({});", t));
                }
                self.line(&format!("    auto g = lock(&{});", m));
                self.line("    show(*g);");
                self.line("}");
            }
            51 => {
                // A channel to one receiving thread, closed, its sum joined.
                let c = self.fresh("ch");
                let cap = 1 + self.rng.below(3);
                let n = self.rng.below(5);
                self.line("{");
                self.line(&format!("    Channel<i64> {} = Channel::new({});", c, cap));
                self.line(&format!("    auto t = spawn(sum_chan, &{});", c));
                for _ in 0..n {
                    let e = self.iexpr(0);
                    self.line(&format!("    _ = Channel::send(&{}, widen<i64>({}));", c, e));
                }
                self.line(&format!("    Channel::close(&{});", c));
                self.line("    show(join(t));");
                self.line("}");
            }
            52 if !ints.is_empty() => {
                // An exclusive reference sent through a channel: lent to the
                // receiver, which writes through it; read after the join.
                let x = self.rng.pick(&ints).clone();
                let c = self.fresh("ch");
                self.line("{");
                self.line(&format!("    Channel<ref<i32, exclusive>> {} = Channel::new(1);", c));
                self.line(&format!("    auto t = spawn(take_refs, &{});", c));
                self.line(&format!("    _ = Channel::send(&{}, &mut {});", c, x));
                self.line(&format!("    Channel::close(&{});", c));
                self.line("    join(t);");
                self.line("}");
                self.line(&format!("show(widen<i64>({}));", x));
            }
            53 | 54 => {
                // A closure writing its capture, called directly and through
                // an exclusive reference; through a shared one it faults
                // (D-0218). One that only reads may go through either.
                let n = self.fresh("n");
                let c = self.fresh("c");
                let (e0, e1, e2) = (self.iexpr(0), self.iexpr(0), self.iexpr(0));
                let writes = self.rng.chance(70);
                self.line("{");
                self.line(&format!("    i32 {} = {};", n, e0));
                if writes {
                    self.line(&format!("    auto {} = [{}](i32 k) : i32 {{ {} += k; {} }};", c, n, n, n));
                } else {
                    self.line(&format!("    auto {} = [{}](i32 k) : i32 {{ {} - k }};", c, n, n));
                }
                self.line(&format!("    show(widen<i64>({}({})));", c, e1));
                let through = if self.rng.chance(25) { "apply_shared(&" } else { "apply_mut(&mut " };
                self.line(&format!("    show(widen<i64>({}{}, {})));", through, c, e2));
                self.line("}");
            }
            55 if !ints.is_empty() => {
                // A closure borrowing an outer variable: using the variable
                // while the closure lives is rejected statically.
                let x = self.rng.pick(&ints).clone();
                let c = self.fresh("c");
                self.line("{");
                self.line(&format!("    auto {} = [{}](i32 k) : i32 {{ {} += k; {} }};", c, x, x, x));
                self.line(&format!("    show(widen<i64>({}(1)));", c));
                if self.rng.chance(20) {
                    self.line(&format!("    show(widen<i64>({}));", x));
                    self.line(&format!("    show(widen<i64>({}(2)));", c));
                }
                self.line("}");
                self.line(&format!("show(widen<i64>({}));", x));
            }
            56 if !maps.is_empty() => {
                // `match` on a map lookup's reference; the map used while the
                // binder lives is rejected statically.
                let m = self.rng.pick(&maps).clone();
                let e = self.iexpr(0);
                let k = self.fresh("k");
                self.line("{");
                self.line(&format!("    i64 {} = widen<i64>({});", k, e));
                self.line(&format!("    match (HashMap::get_mut(&mut {}, &{}))", m, k));
                self.line("    {");
                if self.rng.chance(15) {
                    self.line(&format!("        Some(c) : {{ tally(&mut {}, 5); *c += 1; }},", m));
                } else {
                    self.line("        Some(c) : *c += 5,");
                }
                self.line(&format!("        None    : tally(&mut {}, {}),", m, k));
                self.line("    }");
                self.line(&format!("    show(mget(&{}, {}));", m, k));
                self.line("}");
            }
            57 if !all_vecs.is_empty() => {
                let v = self.rng.pick(&all_vecs).clone();
                self.line(&format!("match (Vec::pop(&mut {}))", v));
                self.line("{");
                self.line("    Some(a) : show(widen<i64>(a)),");
                self.line("    None    : show(0 - 9),");
                self.line("}");
            }
            58 if !strs.is_empty() => {
                // A view of a String, then the String appended to: fine
                // after the view's last use, rejected while it lives.
                let s = self.rng.pick(&strs).clone();
                let w = self.fresh("w");
                let late = self.rng.chance(20);
                self.line("{");
                self.line(&format!("    StringView {} = String::as_view(&{});", w, s));
                self.line(&format!("    show(vcount({}, \" \"));", w));
                self.line(&format!("    String::append(&mut {}, \" 7\");", s));
                if late {
                    self.line(&format!("    show(vcount({}, \"7\"));", w));
                }
                self.line("}");
            }
            59 if !ints.is_empty() => {
                // Two threads lent borrows of two different variables.
                let a = self.rng.pick(&ints).clone();
                let b = self.rng.pick(&ints).clone();
                if a != b {
                    self.line(&format!("{{ auto t1 = spawn(bump, &mut {}, 1); auto t2 = spawn(bump, &mut {}, 2); join(t1); join(t2); }}", a, b));
                } else {
                    self.line(&format!("{{ auto t1 = spawn(bump, &mut {}, 1); join(t1); auto t2 = spawn(bump, &mut {}, 2); join(t2); }}", a, b));
                }
            }
            60 if !helper.is_empty() => {
                // A vector lent shared to one or two reading threads; a
                // push before the join is rejected statically.
                let v = self.rng.pick(&helper).clone();
                self.line("{");
                self.line(&format!("    auto t = spawn(sum, &{});", v));
                if self.rng.chance(30) {
                    self.line(&format!("    auto u = spawn(sum, &{});", v));
                    self.line("    show(join(u));");
                }
                if self.rng.chance(10) {
                    self.line(&format!("    Vec::push(&mut {}, 1);", v));
                }
                self.line("    show(join(t));");
                self.line("}");
            }
            61 if !helper.is_empty() => {
                // A vector lent exclusive to a thread that pushes; read
                // after the join, or (rejected) before it.
                let v = self.rng.pick(&helper).clone();
                let n = self.rng.below(5);
                let early = self.rng.chance(10);
                self.line("{");
                self.line(&format!("    auto t = spawn(fill, &mut {}, {});", v, n));
                if early {
                    self.line(&format!("    show(sum(&{}));", v));
                }
                self.line("    join(t);");
                self.line("}");
                self.line(&format!("show(sum(&{}));", v));
            }
            62 if !stashes.is_empty() || !vecs.is_empty() => {
                // A `Vec<ref>` lent to a thread that reads through each
                // reference: one made stale by a push faults in the thread.
                if vecs.is_empty() || (!stashes.is_empty() && self.rng.chance(50)) {
                    let h = self.rng.pick(&stashes).clone();
                    self.line(&format!("{{ auto t = spawn(sum_refs, &{}); show(join(t)); }}", h));
                } else {
                    let v = self.rng.pick(&vecs).clone();
                    let h = self.fresh("h");
                    let i = self.uexpr(0, Some(&v));
                    let j = self.uexpr(0, Some(&v));
                    self.line("{");
                    self.line(&format!("    Vec<ref<i32, shared>> {} = Vec::new();", h));
                    self.line(&format!("    Vec::push(&mut {}, Vec::index_shared(&{}, {}));", h, v, i));
                    self.line(&format!("    Vec::push(&mut {}, Vec::index_shared(&{}, {}));", h, v, j));
                    if self.rng.chance(20) {
                        self.line(&format!("    fill(&mut {}, 20);", v));
                    }
                    self.line(&format!("    auto t = spawn(sum_refs, &{});", h));
                    self.line("    show(join(t));");
                    self.line("}");
                }
            }
            63 if !ints.is_empty() => {
                // Values holding exclusive references sent through a channel.
                // Sent before the receiver exists, they wait in the channel:
                // reading the variable then is a conflict at run time, and a
                // second send to a full channel a deadlock. (Read once the
                // receiver runs, the outcome depends on the schedule.)
                let a = self.rng.pick(&ints).clone();
                let b = self.rng.pick(&ints).clone();
                let c = self.fresh("ch");
                let cap = 1 + self.rng.below(2);
                let early = self.rng.chance(25);
                self.line("{");
                self.line(&format!("    Channel<Hold> {} = Channel::new({});", c, cap));
                if !early {
                    self.line(&format!("    auto t = spawn(take_holds, &{});", c));
                }
                self.line(&format!("    _ = Channel::send(&{}, Hold {{ .r = &mut {} }});", c, a));
                if a != b {
                    self.line(&format!("    _ = Channel::send(&{}, Hold {{ .r = &mut {} }});", c, b));
                }
                if early {
                    if self.rng.chance(40) {
                        self.line(&format!("    show(widen<i64>({}));", a));
                    }
                    self.line(&format!("    auto t = spawn(take_holds, &{});", c));
                }
                self.line(&format!("    Channel::close(&{});", c));
                self.line("    join(t);");
                self.line("}");
                self.line(&format!("show(widen<i64>({}) + widen<i64>({}));", a, b));
            }
            64 => {
                // `move` closures: an integer copied in, or a vector moved
                // in (using it afterwards is rejected statically); the
                // closure keeps its state between calls.
                let c = self.fresh("c");
                let (e0, e1, e2) = (self.iexpr(0), self.iexpr(0), self.iexpr(0));
                if !vecs.is_empty() && self.rng.chance(50) {
                    let v = self.rng.pick(&vecs).clone();
                    self.line("{");
                    self.line(&format!("    auto {} = move [{}](i32 k) : i32 {{ Vec::push(&mut {}, k); narrow<i32>(Vec::len(&{})) }};", c, v, v, v));
                    self.line(&format!("    show(widen<i64>({}({})));", c, e1));
                    self.line(&format!("    show(widen<i64>(apply_mut(&mut {}, {})));", c, e2));
                    if self.rng.chance(15) {
                        self.line(&format!("    show(sum(&{}));", v));
                    }
                    self.line("}");
                    self.forget(&v);
                } else {
                    let n = self.fresh("n");
                    self.line("{");
                    self.line(&format!("    i32 {} = {};", n, e0));
                    self.line(&format!("    auto {} = move [{}](i32 k) : i32 {{ {} += k; {} }};", c, n, n, n));
                    self.line(&format!("    show(widen<i64>({}({})));", c, e1));
                    self.line(&format!("    show(widen<i64>(apply_mut(&mut {}, {})));", c, e2));
                    self.line(&format!("    show(widen<i64>({}));", n));
                    self.line("}");
                }
            }
            65 if !maps.is_empty() => {
                // `HashMap::insert` and `HashMap::remove`, each returning
                // the value it displaced.
                let m = self.rng.pick(&maps).clone();
                let k = self.iexpr(0);
                if self.rng.chance(50) {
                    let e = self.iexpr(1);
                    self.line(&format!("show(Option::unwrap_or(HashMap::insert(&mut {}, widen<i64>({}), widen<i64>({})), 0 - 3));", m, k, e));
                } else {
                    self.line(&format!("show(Option::unwrap_or(HashMap::remove(&mut {}, &widen<i64>({})), 0 - 4));", m, k));
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
    let Some(a) = run(oracle::coby_cmd().arg(&path), limit) else { return Verdict::Differs("coby timed out".into()) };
    let threads = fs::read_to_string(&path).unwrap().contains("spawn(");
    let mut result = Verdict::Skipped;
    for cc in oracle::c_compilers() {
        if threads {
            // Compiled once, then run as threads really run and under three
            // schedules (`COBALTC_SCHEDULE_SEED`, as `cobc --explore` does):
            // the generator makes only programs with one outcome, so every
            // run must be coby's.
            // The compiler may be named by a path (a sanitizer wrapper).
            let exe = dir.join(format!("seed-{}-{}", seed, cc.rsplit('/').next().unwrap_or(cc)));
            let Some(b) = run(cobc_cmd(cc).arg("-o").arg(&exe).arg(&path), limit) else { return Verdict::Differs(format!("cobc [{}] timed out", cc)) };
            if b.status.code() == Some(3) {
                return Verdict::Skipped;
            }
            if !b.status.success() || !exe.exists() {
                match compare(&a, &b) {
                    Verdict::Differs(why) => return Verdict::Differs(format!("[{}] {}", cc, why)),
                    v => result = v,
                }
                continue;
            }
            for s in [None, Some(1), Some(2), Some(3)] {
                let mut cmd = Command::new(&exe);
                cmd.current_dir(dir).env("COBALTC_CPU_LIMIT", "60");
                if let Some(s) = s {
                    cmd.env("COBALTC_SCHEDULE_SEED", s.to_string());
                }
                oracle::limited(&mut cmd);
                let Some(c) = run(&mut cmd, limit) else { return Verdict::Differs(format!("[{}] schedule {:?} timed out", cc, s)) };
                match compare(&a, &c) {
                    Verdict::Differs(why) => return Verdict::Differs(format!("[{}] schedule {:?}: {}", cc, s, why)),
                    v => result = v,
                }
            }
            let _ = fs::remove_file(&exe);
            continue;
        }
        let Some(c) = run(cobc_cmd(cc).arg("--run").arg(&path), limit) else { return Verdict::Differs(format!("cobc [{}] timed out", cc)) };
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

// `cobc` for one C compiler; with COBC_DIFF_ALL_CHECKS=1, `--all-checks`
// too, so that the elided forms are compared against every check made.
fn cobc_cmd(cc: &str) -> Command {
    let mut c = oracle::cobc(cc);
    if std::env::var("COBC_DIFF_ALL_CHECKS").map_or(false, |v| v == "1") {
        c.arg("--all-checks");
    }
    c
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
            coby::diagnostics::first_line_id(&first).map_or(first.clone(), |(id, phase)| format!("{} ({})", id, phase))
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
