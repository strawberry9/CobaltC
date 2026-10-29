// Tree-walking evaluator over an explicit abstract state, following
// spec/05, spec/06, spec/07, spec/08, spec/13, spec/14, spec/15, spec/16,
// spec/18, spec/19, spec/20 as directly as a dynamically-checked
// (rather than statically-checked) implementation reasonably can.
//
// Scope note (see impl/STATUS.md): this interpreter performs every
// `disposition: checked` safety check *dynamically*, at the point of
// access, exactly as the spec's own rules state the runtime guard. It
// does not additionally run spec/14 §6's static flow-analysis dataflow
// pass, so it does not reject at "compile time" the subset of programs
// that analysis would statically refute (e.g. a literal-operand overflow
// that FA would catch before running). Every program the spec accepts
// still runs correctly, and every program the spec's *dynamic* baseline
// would fault on still faults identically — this is a real, honestly-
// documented precision gap against full conformance, not a soundness gap
// in what actually executes.

use crate::ast::*;
use crate::value::*;
use std::cell::RefCell;
use crate::fxhash::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// The evaluator's parts, by concern (each `impl Interp` continues the one
// here).
mod access;
mod assign;
mod borrows;
mod calls;
mod closures;
mod destroy;
mod eval;
mod exec;
mod extern_calls;
mod frames;
mod index;
mod intrinsics;
mod intrinsics_raw;
mod intrinsics_runtime;
mod intrinsics_simd;
mod intrinsics_threads;
mod keys;
mod layout;
mod literals;
mod matching;
mod memory;
mod native_vec;
mod objects;
mod point_checks;
mod operators;
mod refs;
mod threads;
mod verify;

// Object ids, addresses and tokens are distinct `u64`s the interpreter
// makes itself: nothing adversarial reaches these tables, so they hash
// with one multiply instead of SipHash (which was an eighth of the time).
#[derive(Default, Clone, Copy)]
pub struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

pub type IdMap<V> = std::collections::HashMap<u64, V, std::hash::BuildHasherDefault<IdHasher>>;
pub type IdSet = std::collections::HashSet<u64, std::hash::BuildHasherDefault<IdHasher>>;

pub struct FnItem {
    pub decl: Arc<FnDecl>,
}

pub struct Items {
    pub fns: HashMap<String, Arc<FnDecl>>,
    pub externs: HashMap<String, Arc<ExternDecl>>,
    // D-0258: the program's statics (`static Atomic<T> NAME = …;`), by
    // qualified name; and the constants whose address the program takes
    // (`&K`), recorded by the static pass for the lowering.
    pub statics: HashMap<String, Arc<StaticDecl>>,
    pub const_addrs: Mutex<HashSet<String>>,
    pub structs: HashMap<String, Arc<StructDecl>>,
    pub enums: HashMap<String, Arc<EnumDecl>>,
    pub enum_of_variant: HashMap<String, String>, // variant name -> enum name (unqualified corpus)
    // Every `extern "…";` (spec/20 `rule.trust.extern-code`): its string and
    // declaring line in the combined text, in program order. `cobc` links
    // them; this interpreter only names them when a C function is missing.
    pub extern_code: Vec<(String, usize)>,
    // Filled by the static pass (`typecheck::check_match`): for each
    // `match` expression (by address) whose arms' expected type is fixed
    // only by its first arm, that type -- so the evaluator types a
    // literal in a later arm the same way (`rule.type.expected`).
    pub match_hints: Mutex<HashMap<usize, Type>>,
    // Filled by the static pass (D-0053): expressions (by address) that
    // are `StringView` operations, which the evaluators carry out by
    // evaluating the expression's own operands and calling `std`.
    pub view_ops: Mutex<HashMap<usize, ViewOp>>,
    // Filled by the static pass (D-0073): `&e` expressions (by address)
    // that borrow a temporary as a call's argument.
    pub temp_borrows: Mutex<HashSet<usize>>,
    // Filled by the static pass (D-0218): fields, elements and `*r` (by
    // address) that read or call a value holding `fn` values through a
    // shared reference; each tool checks there that no closure in it holds
    // an exclusive capture. `any_` lets the evaluator skip the lookup.
    pub shared_fn_reads: Mutex<HashSet<usize>>,
    pub any_shared_fn_reads: std::sync::atomic::AtomicBool,
    // Filled by the static pass: every function instantiation it checked,
    // `(key, type args)`. The pass checks the program's own items and the
    // `std` items they reach (through calls, function values, and the
    // destructors and clones of the types they use), so a consumer that
    // runs or lowers a function can confirm it was checked (`cobc` does).
    pub checked_fns: Mutex<HashSet<(String, Vec<Type>)>>,
    // The same by the function's address, for the evaluator's own fast
    // check at every call (`Interp::ensure_checked`); and every function's
    // key by its address, to ask the static pass for one more.
    pub checked_bodies: Mutex<HashSet<usize>>,
    pub fn_keys: HashMap<usize, String>,
    // Filled by the static pass (D-0103): `f().x` moving a field out of a
    // temporary, by address, and the `{ S { x, … } = f(); x }` both
    // evaluators run instead; `any_temp_moves` spares the lookup when
    // the program has none.
    pub temp_moves: Mutex<HashMap<usize, Arc<Expr>>>,
    // D-0111: after the statement (by address), the local reference
    // bindings that end there, at their last use; `any_early_ends` spares
    // the lookup when there are none.
    pub early_ends: Mutex<HashMap<usize, Vec<String>>>,
    pub any_early_ends: std::sync::atomic::AtomicBool,
    pub any_temp_moves: std::sync::atomic::AtomicBool,
    // Filled by the static pass (D-0073): each closure literal's type,
    // `fn(P…) : R`, by the literal's address.
    pub closure_types: Mutex<HashMap<usize, Type>>,
    // Filled by the static pass (D-0090): unsuffixed integer literals (by
    // address) written where a `number` type parameter is expected; at a
    // floating-point instantiation such a literal is that value as a float.
    pub number_literals: Mutex<HashSet<usize>>,
    // D-0282: the access summaries of the program's functions, computed
    // when the static pass first needs them (`tier::summaries`).
    pub tier_summaries: std::sync::OnceLock<Arc<crate::tier::Summaries>>,
    // D-0283: the point checks at calls (by the callee's line and column)
    // whose conflict depends on values: the run compares the places the
    // arguments reach when the call is made. `any_point_checks` spares
    // programs with none the lookup.
    pub point_checks: Mutex<HashMap<(usize, usize), Arc<Vec<crate::tier::PointCheck>>>>,
    pub any_point_checks: std::sync::atomic::AtomicBool,
    // D-0281 step 4c: the bodies (`tier::body_key`) in which the static
    // pass left a conflict to the run's tracking (a point check, a fact it
    // could not decide, a reference of unknown origin, a tier-D binding),
    // each with the first reason; `cobc` lowers a program pathless only
    // when none of its functions is here.
    pub tracking_needed: Mutex<HashMap<String, String>>,
}

impl Items {
    /// A `m::V` path whose prefix is a module, not an enum: the enum
    /// declared directly in module `m` that has a variant `V`, when
    /// exactly one does.
    pub fn module_variant_enum(&self, prefix: &str, vname: &str) -> Option<String> {
        let mut found = None;
        for (key, e) in self.enums.iter() {
            let Some(rest) = key.strip_prefix(prefix).and_then(|r| r.strip_prefix("::")) else { continue };
            if !rest.contains("::") && e.variants.iter().any(|v| v.name == vname) {
                if found.is_some() {
                    return None;
                }
                found = Some(key.clone());
            }
        }
        found
    }
}

/// A `StringView` operation written with the language's own syntax
/// (D-0053).
// The body a spawned thread runs (`[Spawn]`): a function's own, or a
// closure's, shared with the item table.
enum SpawnBody {
    Fn(Arc<FnDecl>),
    Closure(Arc<Block>),
}

impl SpawnBody {
    fn block(&self) -> &Block {
        match self {
            SpawnBody::Fn(f) => &f.body,
            SpawnBody::Closure(b) => b,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewOp {
    /// `&s[lo .. hi]` on a `String` place: `String::view(&s, lo, hi)`.
    SliceString,
    /// `&v[lo .. hi]` on a `StringView`: `StringView::sub(v, lo, hi)`.
    SliceView,
    /// `a == b` / `a != b` with a view on the `view_left` side and a view
    /// (`StringView::eq`) or a `str` (`StringView::eq_str`) on the other.
    Eq { view_left: bool, other_is_str: bool, negate: bool },
    /// D-0108: a comparison whose operands are `String`s in some
    /// instantiation: each is read where it is, by its bytes, and neither
    /// is moved (a temporary ends with its statement).
    TextCmp,
    /// D-0108: `a < b` (and `<=`, `>`, `>=`) on two views:
    /// `StringView::less`, its operands `swap`ped and the result
    /// `negate`d as the operator needs.
    Less { swap: bool, negate: bool },
    /// D-0271 `[Cmp-Key]`: a comparison whose operands are a struct or enum
    /// key type in some instantiation: each is read where it is and
    /// neither is moved; the order is `Vec::sort`'s (`key_cmp`). Text
    /// operands compare as `TextCmp`'s; any other type as the plain
    /// operator.
    KeyCmp,
}

pub struct Object {
    pub ty: Type,
    pub value: Value,
    pub is_resource: bool,
    // `state.authority` (spec/04): destroy authority is granted to the
    // thread that established the object and re-keyed by a cross-thread
    // transfer (spawn arguments, join results). Destroying or moving a
    // resource from any other thread is `diag.no-destroy-authority` /
    // `diag.transfer-without-authority`.
    pub owner_thread: u64,
}

pub struct Frame {
    pub bindings: HashMap<String, u64>,
    // Each binding's type when bound: what a new value takes after the
    // old one was moved away or destroyed (D-0033 (1)).
    pub types: HashMap<String, Type>,
    pub owned: Vec<u64>, // declaration order
    pub moved: HashSet<String>,
    pub is_loop: bool,
    // A called function's (or closure's) parameter frame: name lookup
    // stops here, since a body sees only its own frames (captures are
    // bound in its frame), so a call's cost does not grow with the depth
    // of the stack it runs on.
    pub is_call: bool,
}

impl Frame {
    fn new() -> Self {
        Frame { bindings: HashMap::default(), types: HashMap::default(), owned: Vec::new(), moved: HashSet::default(), is_loop: false, is_call: false }
    }
}

#[derive(Debug, Clone)]
pub enum EvalResult {
    Val(Value),
    // A place, together with the access-path tokens that were followed to
    // reach it (spec/08 `ancestors`): `*r` carries `r`'s token, `r.f`
    // carries every token auto-deref'd through, a bare binding carries
    // none. The chain belongs to *this access* and travels with the
    // result -- it is never stored against the place it reached, because
    // a later, unrelated access to the same place must not inherit it.
    Place(u64, Vec<Proj>, Vec<u64>),
    Temp(u64),
}

// `std::exit(s)` (D-0150) travels as a `Flow::Fault` whose text is this
// marker and the status, so the thread unwinds as `[Fault-Unwind]`
// unwinds it; `lib::run_items_raw` reports it as `ok(s)`.
pub const EXIT_MARK: &str = "$exit:";

pub enum Flow {
    Return(EvalResult),
    Break,
    Continue,
    Fault(String),
    // The whole program has already terminated with another thread's
    // fault (`[Fault-Unwind]`, spec/18: "other threads take no further
    // steps; their frames are not unwound"). Unwinds this thread's Rust
    // stack without running any destructor (`pop_frame` is a no-op once
    // `Interp::terminated` is set).
    Terminated,
}

pub type EvalOutcome = Result<EvalResult, Flow>;

pub struct ArenaBlock {
    pub bytes: Vec<u8>,
}

// D-0213: an exclusive token formed by spawned thread `ℓ` is numbered
// `id | ℓ << TOKEN_THREAD_SHIFT` (ids stay below 2^40); `TRANSIT` marks
// one lent at a thread boundary that no thread has accessed through yet.
const TOKEN_THREAD_SHIFT: u32 = 40;
const TRANSIT: u64 = u64::MAX;

// A spawned thread's lifecycle result (spec/19 `state.threads(ell).value`):
// `Running` until `[Thread-Body-Done]`, then `Done` until `[Join]`/
// `[Handle-Destructor]` claims it (`taken`).
pub enum ThreadStatus {
    Running,
    Done(Value, Type),
    // D-0204: the thread's fault was contained; its failure's id in
    // `Shared::failures`, until `[Join]` or `[Handle-Destructor]` takes it.
    Failed(u64, Type),
    Taken,
}

// D-0204: what a waiting thread waits for (`Interp::wait_for`): another
// thread's end, a mutex, a channel's count, or the clock. A wait on the
// clock always ends; the others can wait on each other for ever.
#[derive(Clone, PartialEq, Eq)]
pub enum Wait {
    Join(u64),
    Lock((u64, Vec<Proj>)),
    // A count, and the value it was seen at.
    Event(u64, u64),
    // D-0253: an atomic's wake count (its bucket), and the value it was
    // seen at.
    Atomic(u64, u64),
    Timer,
}

// Whether a waiting thread's wait still holds it: a thread that has been
// woken but not yet run is not stuck.
fn still_waits(s: &Shared, w: &Wait) -> bool {
    match w {
        Wait::Join(t) => matches!(s.thread_status.get(t), Some(ThreadStatus::Running)),
        Wait::Lock(key) => s.cobalt_locks.contains_key(key),
        Wait::Event(id, seen) => s.events.get(id) == Some(seen),
        Wait::Atomic(b, seen) => s.atomic_counts.get(b).copied().unwrap_or(0) == *seen,
        Wait::Timer => false,
    }
}

// The parts of Σ that another OS thread reads or writes while this one
// waits: thread results (`state.threads`), lock holders (`state.sync`),
// and the program's fault, if any. Kept behind their own mutex, outside
// the `Interp` the GIL guards, so a waiting thread can poll them
// without touching the interpreter it has released.
#[derive(Default)]
pub struct Shared {
    pub thread_status: HashMap<u64, ThreadStatus>,
    // Keyed by the mutex's place: its object and the path to it there (a
    // mutex may be a field, and two fields are two mutexes).
    pub cobalt_locks: HashMap<(u64, Vec<Proj>), u64>,
    pub program_fault: Option<String>,
    // `std::event_op`'s counts (D-0063), by id, and the last id made.
    pub events: HashMap<u64, u64>,
    pub last_event: u64,
    // D-0253: the atomics' wake counts, one per bucket of atomics (at most
    // 256), as `cbrt` keeps them.
    pub atomic_counts: HashMap<u64, u64>,
    // D-0204: threads asked to stop (`cancel`); threads lent an exclusive
    // reference (never cancelled, never contained); contained failures
    // by id, as raised and as reported; poisoned mutexes; what each
    // waiting thread waits for; threads in a wait outside the runtime (a
    // socket, input); threads found deadlocked, to fault as they wake.
    pub cancel: HashSet<u64>,
    pub lent_excl: HashSet<u64>,
    pub failures: Vec<(String, String)>,
    pub poisoned: HashSet<(u64, Vec<Proj>)>,
    pub waits: HashMap<u64, Wait>,
    pub in_io: HashSet<u64>,
    pub deadlocked: HashSet<u64>,
    // Each running program thread's OS thread, for `cancel` to interrupt a
    // socket wait (`netio::wake_thread`); removed as the thread ends.
    pub os: HashMap<u64, u64>,
}

// A global interpreter lock. Every CobaltC thread is a real OS thread,
// and exactly one of them runs the evaluator at a time; a thread that
// must wait for another (`[Join]`, `[Lock]`, `[Handle-Destructor]`)
// releases the GIL *in place* -- deep inside whatever calls it is
// nested in -- and reacquires it afterwards, so no unwinding-and-
// resuming machinery is needed and blocking works at any depth. The
// evaluator also yields at statement boundaries while other threads
// are live, which is what makes `[Thread-Step]`'s interleaving real.
//
// Safety: the `Interp` is reached through `Gil::interp` only by the
// thread holding the GIL; the `&mut Interp` a waiting thread still has
// on its stack is not used until it holds the GIL again, and the wait
// itself runs in a function that touches nothing but the GIL and
// `Shared` (`gil_wait`), with a `black_box` barrier around the
// reacquisition so no interpreter state is assumed unchanged across it.
pub struct Gil {
    cell: std::cell::UnsafeCell<Interp>,
    held: Mutex<GilState>,
    cv: std::sync::Condvar,
}

unsafe impl Send for Gil {}
unsafe impl Sync for Gil {}

// D-0204: the GIL's state. With a schedule seed (`--schedule-seed`,
// `--explore`), whose turn it is to take it: each release passes the turn
// to a live thread chosen by a generator seeded once, so a seed decides
// the whole interleaving -- the steps between turns are the interpreter's
// own, in order. A thread waiting outside the runtime (a socket, input)
// is not given the turn; when every one is, anyone may take it, so a
// program doing real input and output is reproducible only as far as its
// input and output are.
struct GilState {
    held: bool,
    sched: Option<Sched>,
}

struct Sched {
    rng: u64,
    turn: Option<u64>,
    members: Vec<u64>,
    io: Vec<u64>,
}

impl Sched {
    fn pick(&mut self) {
        let cands: Vec<u64> = self.members.iter().copied().filter(|m| !self.io.contains(m)).collect();
        if cands.is_empty() {
            self.turn = None;
            return;
        }
        // splitmix64
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        self.turn = Some(cands[(z % cands.len() as u64) as usize]);
    }
}

// D-0204: the seed of a deterministic schedule (0: none), set by
// `coby --schedule-seed` before the program runs.
// How many steps a thread takes between offers of the GIL, unscheduled.
const YIELD_SLICE: u32 = 200;

static SCHEDULE_SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn set_schedule_seed(seed: u64) {
    SCHEDULE_SEED.store(seed, std::sync::atomic::Ordering::SeqCst);
}

thread_local! {
    // The program thread this OS thread runs, for the GIL's turns.
    static GIL_ME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Gil {
    pub fn new(interp: Interp) -> Gil {
        let seed = SCHEDULE_SEED.load(std::sync::atomic::Ordering::SeqCst);
        let sched = if seed == 0 { None } else { Some(Sched { rng: seed, turn: None, members: vec![0], io: Vec::new() }) };
        Gil { cell: std::cell::UnsafeCell::new(interp), held: Mutex::new(GilState { held: false, sched }), cv: std::sync::Condvar::new() }
    }
    pub fn acquire(&self) {
        let me = GIL_ME.with(|c| c.get());
        let mut h = self.held.lock().unwrap();
        while h.held || h.sched.as_ref().map_or(false, |s| s.turn.map_or(false, |t| t != me)) {
            h = self.cv.wait(h).unwrap();
        }
        h.held = true;
    }
    pub fn release(&self) {
        let mut h = self.held.lock().unwrap();
        h.held = false;
        if let Some(s) = h.sched.as_mut() {
            s.pick();
        }
        drop(h);
        self.cv.notify_all();
    }
    // A thread joins the schedule (at its `spawn`, by the spawner) and
    // leaves it (at its end); it waits outside the runtime, and is back.
    pub fn join_member(&self, t: u64) {
        if let Some(s) = self.held.lock().unwrap().sched.as_mut() {
            s.members.push(t);
        }
    }
    pub fn leave_member(&self, t: u64) {
        if let Some(s) = self.held.lock().unwrap().sched.as_mut() {
            s.members.retain(|m| *m != t);
        }
    }
    pub fn io(&self, t: u64, on: bool) {
        if let Some(s) = self.held.lock().unwrap().sched.as_mut() {
            if on {
                s.io.push(t);
            } else {
                s.io.retain(|m| *m != t);
            }
        }
    }
    // The program thread this OS thread runs.
    pub fn set_me(t: u64) {
        GIL_ME.with(|c| c.set(t));
    }
    /// # Safety
    /// The caller must hold the GIL for as long as it uses the result.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn interp(&self) -> &mut Interp {
        &mut *self.cell.get()
    }
}

// Waits, with the GIL released, until `ready` holds or the program has
// terminated. Touches nothing but the GIL and `Shared`.
#[inline(never)]
// D-0108: a comparison of two byte strings, in byte order.
fn bytes_cmp(op: BinOp, a: &[u8], b: &[u8]) -> bool {
    let o = a.cmp(b);
    match op {
        BinOp::Eq => o.is_eq(),
        BinOp::Ne => o.is_ne(),
        BinOp::Lt => o.is_lt(),
        BinOp::Le => o.is_le(),
        BinOp::Gt => o.is_gt(),
        _ => o.is_ge(),
    }
}

fn gil_wait(gil: &Arc<Gil>, shared: &Arc<Mutex<Shared>>, ready: &mut dyn FnMut(&Shared) -> bool) -> Result<(), String> {
    loop {
        {
            let s = shared.lock().unwrap();
            if let Some(d) = &s.program_fault {
                return Err(d.clone());
            }
            if ready(&s) {
                return Ok(());
            }
        }
        gil.release();
        std::thread::yield_now();
        std::thread::sleep(std::time::Duration::from_micros(50));
        gil.acquire();
    }
}

// Whether `v` holds an exclusive reference anywhere (D-0204: a thread
// given one is lent another's data).
fn value_has_excl_ref(v: &Value) -> bool {
    match v {
        Value::Ref { mode, .. } => *mode == Mode::Exclusive,
        Value::Struct(fs) => fs.iter().any(value_has_excl_ref),
        Value::Array(es) => es.iter().any(value_has_excl_ref),
        Value::Enum(_, p) => value_has_excl_ref(p),
        _ => false,
    }
}

// D-0204: `netio`'s question while a socket waits (`set_cancel_probe`):
// has the thread running on this OS thread been asked to stop? Each
// program thread is an OS thread of its own, which records its shared
// state and its id here as it starts (`set_probe`).
thread_local! {
    static PROBE: RefCell<Option<(Arc<Mutex<Shared>>, u64)>> = const { RefCell::new(None) };
}

// D-0204: `t` is asked to stop; a socket wait it is in ends now (its OS
// thread signalled under the lock, so it cannot have ended in between).
fn request_cancel(shared: &Arc<Mutex<Shared>>, t: u64) {
    let mut s = shared.lock().unwrap();
    s.cancel.insert(t);
    if let Some(&o) = s.os.get(&t) {
        crate::netio::wake_thread(o);
    }
}

fn set_probe(shared: &Arc<Mutex<Shared>>, me: u64) {
    crate::netio::set_cancel_probe(cancel_probe);
    PROBE.with(|p| *p.borrow_mut() = Some((shared.clone(), me)));
}

fn cancel_probe() -> bool {
    PROBE.with(|p| {
        p.borrow().as_ref().map_or(false, |(s, me)| {
            let s = s.lock().unwrap();
            s.cancel.contains(me) && !s.lent_excl.contains(me)
        })
    })
}

// D-0204: whether `me` waiting for `kind` would close a cycle: following
// each wait to the thread it waits on (a join's thread, a mutex's holder)
// comes back to `me`.
fn closes_cycle(s: &Shared, me: u64, kind: &Wait) -> bool {
    let next = |k: &Wait| -> Option<u64> {
        match k {
            Wait::Join(t) => Some(*t),
            Wait::Lock(key) => s.cobalt_locks.get(key).copied(),
            _ => None,
        }
    };
    let mut at = next(kind);
    let mut steps = 0;
    while let Some(t) = at {
        if t == me {
            return true;
        }
        // A thread asked to stop ends at its next look: no cycle through it.
        if s.cancel.contains(&t) && !s.lent_excl.contains(&t) {
            return false;
        }
        steps += 1;
        if steps > s.waits.len() + 1 {
            return false;
        }
        at = s.waits.get(&t).and_then(|k| next(k));
    }
    false
}

// D-0204: when every live thread waits on another thread, a mutex or a
// channel, and none on the clock or outside the runtime, no wait can end.
// Where some of them wait on each other in a cycle of joins and locks, one
// of that cycle is deadlocked -- its fault, contained, ends its waits and
// may free the rest; otherwise every one of them is. Each faults as it
// wakes.
fn find_deadlock(s: &mut Shared) {
    if !s.in_io.is_empty() {
        return;
    }
    let mut live: Vec<u64> = vec![0];
    live.extend(s.thread_status.iter().filter(|(_, st)| matches!(st, ThreadStatus::Running)).map(|(t, _)| *t));
    // The main thread waiting alone on a channel is `[Channel-Deadlock]`, not
    // `[Deadlock]` (`spec/19` §3): its own wait reports it (D-0219).
    if live.len() == 1 && matches!(s.waits.get(&0), Some(Wait::Event(..))) {
        return;
    }
    if !live.iter().all(|t| s.waits.get(t).map_or(false, |w| still_waits(s, w)) && !(s.cancel.contains(t) && !s.lent_excl.contains(t))) {
        return;
    }
    let mut waiting: Vec<u64> = s.waits.keys().copied().collect();
    waiting.sort_unstable();
    for &t in waiting.iter().rev() {
        if let Some(k) = s.waits.get(&t).cloned() {
            if closes_cycle(s, t, &k) {
                s.deadlocked.insert(t);
                return;
            }
        }
    }
    // No cycle: the ends of the chains of waits -- the threads waiting on
    // a channel, not on another thread -- fault; the threads waiting on
    // them then learn of it as their joins and locks end. One outcome,
    // whatever the timing.
    let ends: Vec<u64> = live.iter().copied().filter(|t| !matches!(s.waits.get(t), Some(Wait::Join(_)) | Some(Wait::Lock(_)))).collect();
    for t in if ends.is_empty() { live } else { ends } {
        s.deadlocked.insert(t);
    }
}

#[inline(never)]
fn gil_yield(gil: &Arc<Gil>) {
    gil.release();
    std::thread::yield_now();
    gil.acquire();
}

// Reclaimed objects by id, with their address and type, and an index by
// address: `release` and `deallocate` ask for the objects in a range,
// which a scan of every reclaimed object made quadratic in a program
// holding many (a `Vec` of resources).
#[derive(Default)]
pub struct ReclaimedAddr {
    map: HashMap<u64, (u64, Type)>,
    by_addr: std::collections::BTreeSet<(u64, u64)>,
}

impl ReclaimedAddr {
    fn get(&self, id: &u64) -> Option<&(u64, Type)> {
        self.map.get(id)
    }
    fn contains_key(&self, id: &u64) -> bool {
        self.map.contains_key(id)
    }
    fn insert(&mut self, id: u64, v: (u64, Type)) {
        if let Some((old, _)) = self.map.get(&id) {
            self.by_addr.remove(&(*old, id));
        }
        self.by_addr.insert((v.0, id));
        self.map.insert(id, v);
    }
    fn remove(&mut self, id: &u64) -> Option<(u64, Type)> {
        let r = self.map.remove(id);
        if let Some((a, _)) = &r {
            self.by_addr.remove(&(*a, *id));
        }
        r
    }
    // The ids of the objects whose address is in `[lo, hi)`.
    fn ids_in(&self, lo: u64, hi: u64) -> Vec<u64> {
        if hi <= lo {
            return Vec::new();
        }
        self.by_addr.range((lo, 0)..(hi, 0)).map(|(_, id)| *id).collect()
    }
}

// `COBALTC_TRACE`, read once: `eval` asked the environment at every
// expression.
// D-0077: how far this thread's stack may grow. A thread the interpreter
// starts with a known stack (the runner in main.rs, a spawned program
// thread) records its floor: the address below which a call faults with
// `diag.stack-exhausted` instead of overflowing the host stack. Other
// threads (a library caller's) record none, and nothing is checked.
thread_local! {
    static STACK_FLOOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[inline(always)]
fn stack_here() -> usize {
    let probe = 0u8;
    std::hint::black_box(&probe) as *const u8 as usize
}

/// Records, for the calling thread, a stack of `bytes` beginning about
/// here; calls fault once less than a safety margin is left.
pub fn set_stack_budget(bytes: usize) {
    let margin = (bytes / 16).clamp(256 << 10, 64 << 20);
    let floor = stack_here().saturating_sub(bytes.saturating_sub(margin));
    STACK_FLOOR.with(|c| c.set(floor));
}

/// Whether the calling thread recorded a stack budget.
pub fn has_stack_budget() -> bool {
    STACK_FLOOR.with(|c| c.get()) != 0
}

fn stack_exhausted() -> bool {
    let floor = STACK_FLOOR.with(|c| c.get());
    floor != 0 && stack_here() < floor
}

pub fn trace_on_pub() -> bool {
    trace_on()
}

fn trace_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("COBALTC_TRACE").is_ok())
}

// D-0264: `COBALTC_TRACE_EVENTS`, read once. With it set, the interpreter
// writes one `EV` line on standard error for each event the trace
// comparison (`impl/tools/trace_compare.py`) matches between the two
// tools: a user destructor run (`EV drop <type>`), the program's fault
// (`EV fault <diag>`), an `exit` (`EV exit <status>`) and its end
// (`EV end <status>`).
pub fn trace_events_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("COBALTC_TRACE_EVENTS").map_or(false, |v| !v.is_empty() && v != "0"))
}

// Whether the program's fault event has been written (once per process:
// the first raise, or the run's end for a fault that reached no raise
// site, as a `dynamic` fault the checker folds).
pub static FAULT_EVENT_SENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn fault_event(d: &str) {
    if trace_events_on() && !FAULT_EVENT_SENT.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("EV fault {}", d.split(|c: char| c == crate::diagnostics::DETAIL_SEP || c == '@' || c.is_whitespace()).next().unwrap_or(d));
    }
}

pub struct Interp {
    pub items: Arc<Items>,
    pub objects: IdMap<Object>,
    pub next_id: u64,
    // Per-real-CobaltC-thread call stacks (spec/04 `Sigma.frame-stack(ell)`
    // is genuinely per-thread; a single shared `Vec<Frame>` was correct
    // only back when every thread ran synchronously on the one Rust call
    // stack). `current_thread` names whose stack `self.frames()`/
    // `frames_mut()` resolve to -- correct because only one real OS
    // thread ever holds `&mut Interp` at a time (it lives behind the one
    // `Mutex` in `self_shared`), so there is never a moment where two
    // threads' logical `current_thread` could disagree about the shared
    // struct's contents.
    frames_by_thread: HashMap<u64, Vec<Frame>>,
    // How many frame bindings of each thread currently name an object:
    // `lookup_owns`' index, kept in step at every bindings.insert/remove.
    // (A scan of every frame per `drop` made deep teardown quadratic:
    // 87% of a 120k-deep drop's cycles, found by perf.)
    bound_count_by_thread: HashMap<u64, HashMap<u64, u32>>,
    pub current_thread: u64,
    pub next_thread_id: u64,
    // raw byte arena for allocate/deallocate/rawptr; address 0 is never valid.
    pub arena: IdMap<u8>,
    pub next_addr: u64,
    pub allocations: IdMap<(u64, u64)>, // addr -> (len, align), for deallocate's trusted check
    pub extern_calls: Vec<(String, Vec<i128>)>,
    // The program's arguments (`rule.stdlib.args`): what follows the
    // file on `coby`'s command line, as bytes; `std::arg_bytes` reads them.
    pub prog_args: Arc<Vec<Vec<u8>>>,
    // D-0204: renders a fault as the program would report it (set by
    // `lib::run_source_raw`, which has the source map), for
    // `ThreadFailure::text`.
    pub render: Option<Arc<dyn Fn(&str) -> String + Send + Sync>>,
    pub trace_writes: bool,
    closures: RefCell<HashMap<u64, ClosureInfo>>,
    closure_params: RefCell<HashMap<u64, Vec<Param>>>,
    // D-0073: the result type a closure was formed at (`fn(…) : R` where
    // it was written), which its body's value takes, as a function's
    // declared result does.
    closure_rets: RefCell<HashMap<u64, Type>>,
    // Layouts computed once per instantiated struct/enum type: every
    // place read finds its offset through them, and a recursive walk of
    // the field types each time made reads cost the size of the type.
    struct_layouts: RefCell<HashMap<(String, Vec<Type>), Option<Layout>>>,
    enum_layouts: RefCell<HashMap<(String, Vec<Type>), Option<EnumLayout>>>,
    reclaimed_addr: ReclaimedAddr,
    reclaimed_by_addr: IdMap<u64>,
    // Values raw storage holds that bytes cannot carry — references,
    // guards, handles, `fn` values — by the address of their cell. The
    // arena keeps their (placeholder) bytes; `arena_read` puts the value
    // back. A reference here is held by the reclaimed object over its
    // cell (`spec/20` `[Rawptr-Write]`, `CHG-0031`) until that ends.
    arena_side: std::collections::BTreeMap<u64, Value>,
    // The address of an object's cells for the byte image of a reference
    // to it (`[Repr-Ref]`, spec/06 §7: `bytes(ref(a))` is the image of
    // `min(target(a))`), for an object the arena cannot carry (one holding
    // references or handles): assigned once, when first needed, and the
    // address `rawptr_of` then answers with too.
    image_addr: HashMap<u64, u64>,
    // `[Stmt-Exit]` for temporaries this interpreter would otherwise lose
    // track of -- one read through (`make().f`, `make()[i]`) or a closure
    // called straight from an expression: each statement ends the ones
    // registered while it ran, last first.
    // Per thread: a thread may be switched out between statements of a
    // call made mid-statement, so each keeps its own stack.
    stmt_temps: IdMap<Vec<Vec<u64>>>,
    // The objects in `stmt_temps` (a superset, never missing one): `bind`
    // scans the statements only for an object that may be a temporary.
    temp_index: IdSet,
    // D-0047: the lengths `$` stands for, innermost `[…]` last, and
    // whether the place being evaluated is to be written (an assignment's
    // target, `&mut`'s operand): a `Vec` element is reached through a
    // borrow of the `Vec` in that mode.
    dollar: HashMap<u64, Vec<usize>>,
    write_ctx: bool,
    // Program-data image of each distinct `str` value, minted on first
    // `str_ptr`/arena encoding (impl-defined: one address per content,
    // never reused).
    str_data: HashMap<Arc<[u8]>, u64>,
    // D-0134: for each text `StringView::of` has viewed, the object its
    // views borrow: a `Vec<u8>` over the text's interned bytes that never
    // ends, as the `str` value never does.
    str_views: HashMap<Arc<[u8]>, u64>,
    // D-0258: the statics' objects, and the addressed constants' (`&K`),
    // by qualified name, each made once and never ended.
    static_objs: HashMap<String, u64>,
    // For each object, the objects whose value has held a reference or
    // guard to it (a superset: an id is added whenever a reference-bearing
    // value is stored, and removed only with the object; a stale holder
    // simply contributes no occurrence),
    // and each holder's targets, to forget it when it ends. An access
    // checks only the references to its own target: a deep recursion that
    // passes a reference down keeps one holder per frame, and walking all
    // of them at every access made each access cost the recursion depth.
    holders_by_target: IdMap<IdSet>,
    targets_by_holder: IdMap<Vec<u64>>,
    // Real mutex state (spec/19 `rule.conc.lock`): the CobaltC thread id
    // currently holding each `mutex<T>` object, if any -- distinct from
    // `Running`/lock-free by whether the object id has an entry at all.
    pub shared: Arc<Mutex<Shared>>,
    // The GIL this interpreter runs under (`lib::run_source_raw`); `None`
    // only for an interpreter driven without one, which cannot spawn.
    pub gil: Option<std::sync::Weak<Gil>>,
    // Live CobaltC threads (main included): the evaluator yields the GIL
    // at statement boundaries while this exceeds one.
    pub active_threads: usize,
    // D-0213 `[Thread-Affine]`: the thread an exclusive token belongs to
    // where it is not the one its number names (`token_owner`): lent at a
    // thread boundary (`TRANSIT`) or taken over since.
    tok_owner: HashMap<u64, u64>,
    // Steps left before this thread offers the GIL to the others (`yield_step`).
    pub yield_budget: u32,
    // Set once the program has terminated with a fault: every frame exit
    // from then on skips destruction (`[Fault-Unwind]`).
    pub terminated: bool,
    // Set once a fault is unwinding the current thread's frames
    // (`[Fault-Unwind]`): destructors still run, but a handle's does not
    // wait for its thread, which takes no further steps.
    pub faulting: bool,
    // A fault (not `exit`'s unwind) is on its way out: set where it is
    // first seen (an expression's evaluation, a destructor), so that a
    // destructor faulting during its unwind does not replace it.
    pub fault_pending: bool,
    // `exit` (D-0150) is unwinding the thread.
    pub exit_pending: bool,
    // rule.init.definite-assignment (spec/11): object ids established by
    // `[Let-Uninit]` (`τ x;`, no initializer) that have not yet received
    // their first whole-object write. Removed the moment such a write
    // happens (`eval_assign_place`'s `path.is_empty()` case); a read
    // while still present faults `diag.use-of-uninitialized`
    // (`result_to_value`). Plain (non-resource) bindings only -- a
    // resource's own destruction-obligation tracking already covers
    // this concern for resource types.
    uninit_bindings: HashSet<u64>,
    // Per-call cells holding a borrow-capturing closure's capture
    // references while its body runs (`bind_closure_captures`). A name
    // bound to such a cell denotes `*self.f_i` (spec/15 [Closure-Call]):
    // `eval_path` resolves it through the stored reference, so every
    // access inside the body carries the capture token as its ancestor
    // instead of finding that token as a conflicting stranger. Removed,
    // with their objects, when the call returns.
    capture_cells: HashSet<u64>,
    // Persistent, per-token ancestor set (spec/08 `ancestors`), distinct
    // from the per-access chain an `EvalResult::Place` carries: that chain
    // is *transient*, built while navigating to one place for one access
    // — it has no memory of a chain that was walked once to *form* a new
    // token and is never walked again to reach the same place. A token
    // minted by a genuine new borrow (`eval_borrow`'s `[Borrow]`, a
    // by-reference closure capture) permanently descends from whatever
    // tokens were excluded at the moment it was formed, transitively
    // through those tokens' own recorded ancestors — recorded once, here,
    // at formation time, and consulted by every later `clash` check made
    // through this token, however many frames or bindings removed from
    // where it was formed. Without this, a reference formed by
    // dereferencing a reference *parameter* (`&*v`) forgets that it is a
    // sub-borrow of `v`'s own still-live borrow the moment control leaves
    // the statement that formed it, and a later access made through it
    // spuriously conflicts with `v`.
    token_ancestors: IdMap<std::sync::Weak<Vec<u64>>>,
    // The table's size at which dead entries are next dropped.
    token_prune_at: usize,
    // Lexical `unsafe { }` nesting depth (spec/20 §1 `[Unsafe-Rejected]`):
    // every `discharge: trusted` construct must occur textually inside
    // one of these blocks. This is the one purely-static rejection this
    // interpreter enforces without a separate type-checking pass, since
    // it needs only a scope counter, not dataflow. Per-thread for the
    // same reason `frames_by_thread` is: each real CobaltC thread has its
    // own independent lexical nesting.
    unsafe_depth_by_thread: HashMap<u64, u32>,
    // Per closure-object field types, since a closure's own `.ty` is the
    // opaque `Type::Closure(cid)` tag, not a `Named` struct `type_at` can
    // look a field up on directly.
    closure_field_types: IdMap<Vec<Type>>,
    // The same, by closure literal (`closure_id`): for an object that holds
    // a closure's captures without having been formed as one -- a closure
    // moved into another's capture, or a call's view of a captured
    // closure -- whose own entry was never recorded.
    closure_cid_types: HashMap<u64, Vec<Type>>,
    // D-0089: the closure objects whose last `fn`-value handle was
    // dropped (`value::ClosureRef`), destroyed at the next object end or
    // statement end; and whether that is under way.
    ended_closures: Arc<Mutex<Vec<u64>>>,
    ending_closures: bool,
    // A Box teardown loop is running (`destroy_box_value`), and the
    // referent a Box in tail position left to it.
    box_draining: bool,
    box_next: Option<(u64, u64, u64)>,
    // A move capture's per-call cell -> the closure object and field its
    // value returns to when the call ends.
    capture_writeback: IdMap<(u64, usize)>,
    // A handle to "myself, behind the lock real OS threads share" --
    // `None` until `run_source` (lib.rs) wraps a freshly built `Interp`
    // in `Arc::new(Mutex::new(..))` and locks it once, immediately, to
    // set this field to a clone of that same `Arc`. `spawn` is the only
    // reader: it clones this to hand a new `std::thread::spawn`'d thread
    // a way to take its own turn locking the same `Interp`.
    // True while `run_body` binds a spawned thread's parameters in that
    // thread: the one legitimate cross-thread transfer (spec/07
    // `[Authority-Transfer]`'s `ℓ2 ≠ ℓ` case).
    binding_thread_params: bool,
    // [Fault-Unwind] (spec/18): set the instant *any* thread (main or
    // spawned) hits a `disposition: checked` fault -- every other
    // thread's own dispatch loop (see `spawn`'s closure and
    // `run_source` in lib.rs) checks this and stops taking further
    // steps as soon as it next has a chance to, matching "other threads
    // take no further steps; their frames are not unwound." Recorded as
    // a plain field behind this same `Mutex`, not a process exit, so a
    // faulting spawned thread cannot ever tear down a *host* process
    // this interpreter is embedded in (e.g. the conformance test
    // binary, which runs many independent programs' interpreters in one
    // process).
    // The source line of the innermost expression `eval()` is currently
    // (or most recently was) evaluating. Updated as a side effect of
    // every `eval()` call, purely so a `Flow::Fault` constructed
    // *anywhere* -- including the handful of sites that raise one
    // directly (`destroy_object`, `store_binding`, block-exit's
    // automatic-destruction sweep) rather than as an `eval()` return
    // value -- has a real source line to attach to it, without having
    // to thread an explicit line parameter through every one of those
    // call sites individually. Only ever read synchronously, within the
    // same thread's own uninterrupted call chain that is about to
    // construct or propagate a fault (see `eval`, `exec_stmt`,
    // `exec_block_stmts`'s wrapping) -- never across a `WouldBlock`
    // boundary, so one flat field (not per-thread) is sound even though
    // several real CobaltC threads share one `Interp`: only one OS
    // thread ever holds `&mut Interp` at a time.
    pub current_line: usize,
    // D-0283: the site (the callee's line and column) of the call
    // `eval_call` is making, for its point checks; taken as the call starts.
    pub pc_site: Option<(usize, usize)>,
    // The last line of the program's own code this thread ran (`std`'s lines
    // are not one): where a fault raised by a block's exit is reported when
    // the exit began inside a call into `std` (D-0219).
    pub last_user_line: usize,

    // The enclosing function's own declared return type, pushed/popped
    // around a call's body execution: consulted by an explicit
    // `return expr;` (`[T-Return]`); a body's implicit trailing
    // expression gets the same type through
    // `exec_block_body_expected` (`rule.type.expected`).
    // Per thread: the declared return type of each open call.
    return_ty_by_thread: HashMap<u64, Vec<Option<Type>>>,
}

const MAIN_ADDR_BASE: u64 = 0x1000;

impl Interp {
    pub fn new(items: Arc<Items>) -> Self {
        let mut frames_by_thread = HashMap::default();
        frames_by_thread.insert(0u64, Vec::new()); // ell_0, the main thread
        Interp {
            items,
            objects: Default::default(),
            next_id: 1,
            frames_by_thread,
            bound_count_by_thread: HashMap::default(),
            current_thread: 0,
            next_thread_id: 1,
            arena: Default::default(),
            next_addr: MAIN_ADDR_BASE,
            allocations: Default::default(),
            extern_calls: Vec::new(),
            prog_args: Arc::new(Vec::new()),
            render: None,
            trace_writes: false,
            closures: RefCell::new(HashMap::default()),
            closure_params: RefCell::new(HashMap::default()),
            closure_rets: RefCell::new(HashMap::default()),
            struct_layouts: RefCell::new(HashMap::default()),
            enum_layouts: RefCell::new(HashMap::default()),
            reclaimed_addr: ReclaimedAddr::default(),
            reclaimed_by_addr: Default::default(),
            arena_side: std::collections::BTreeMap::new(),
            image_addr: HashMap::default(),
            stmt_temps: Default::default(),
            temp_index: Default::default(),
            dollar: HashMap::default(),
            write_ctx: false,
            str_data: HashMap::default(),
            str_views: HashMap::default(),
            static_objs: HashMap::default(),
            holders_by_target: Default::default(),
            targets_by_holder: Default::default(),
            shared: Arc::new(Mutex::new(Shared::default())),
            gil: None,
            active_threads: 1,
            tok_owner: Default::default(),
            yield_budget: 0,
            terminated: false,
            faulting: false,
            fault_pending: false,
            exit_pending: false,
            uninit_bindings: HashSet::default(),
            capture_cells: HashSet::default(),
            token_ancestors: Default::default(),
            token_prune_at: 4096,
            unsafe_depth_by_thread: HashMap::default(),
            closure_field_types: Default::default(),
            closure_cid_types: Default::default(),
            ended_closures: Default::default(),
            ending_closures: false,
            box_draining: false,
            box_next: None,
            capture_writeback: Default::default(),
            binding_thread_params: false,
            current_line: 0,
            pc_site: None,
            last_user_line: 0,
            return_ty_by_thread: HashMap::default(),
        }
    }

    /// Tags a bare `"diag.xxx"` string with the current source line as
    /// `"diag.xxx@N"`, for a fault raised somewhere `eval()`'s own
    /// wrapping doesn't reach directly (see `current_line`'s doc
    /// comment). A no-op if `d` is already tagged (an already-`eval()`-
    /// wrapped fault propagating through, most commonly), so the
    /// *innermost* attachment point always wins -- the most precise
    /// line available, never overwritten by a coarser outer one.
    fn tag_current_line(&self, d: String) -> String {
        if crate::diagnostics::has_location(&d) || !user_line(self.current_line) {
            d
        } else {
            format!("{d}@{}", self.current_line)
        }
    }

    // This thread's own call stack. Never absent while `self` is
    // reachable: `Interp::new` seeds ell_0's, and every other thread id
    // is inserted (`spawn`) before `current_thread` is ever set to it.
    // The result type of each function or closure body running on this
    // thread, innermost last (`None`: a closure with none written).
    fn ret_stack(&mut self) -> &mut Vec<Option<Type>> {
        self.return_ty_by_thread.entry(self.current_thread).or_default()
    }

    // This thread's statement-temporary stack.
    fn st(&mut self) -> &mut Vec<Vec<u64>> {
        self.stmt_temps.entry(self.current_thread).or_default()
    }

    // This thread's `$` stack (D-0047).
    fn dl(&mut self) -> &mut Vec<usize> {
        self.dollar.entry(self.current_thread).or_default()
    }
}

#[derive(Clone)]
struct Layout {
    size: u64,
    align: u64,
    offsets: Vec<u64>,
}

#[derive(Clone)]
struct EnumLayout {
    size: u64,
    align: u64,
    dw: u64,
    payload_off: u64,
    payload_types: Vec<Type>,
}

fn r_is_temp(_obj: u64, _objects: &IdMap<Object>) -> bool {
    // simplification: we do not currently distinguish "temp root" scrutinees
    // for consumption purposes beyond `path.is_empty()`.
    true
}

fn closure_id(e: &Expr) -> u64 {
    e as *const Expr as u64
}

// `[Float-To-Int-Checked]`/`[Float-To-Int-Invalid]`: truncate toward
// zero; infinities, NaN and out-of-range values fault. `u128`'s range
// exceeds the `i128` payload, so it is handled on the unsigned side.
fn float_to_int(f: f64, dt: IntTy) -> Result<Value, Flow> {
    if !f.is_finite() {
        return Err(Flow::Fault("diag.narrowing-overflow".into()));
    }
    let tr = f.trunc();
    if is_u128(dt) {
        if tr >= 0.0 && tr < 340282366920938463463374607431768211456.0 {
            return Ok(Value::Int(dt, tr as u128 as i128));
        }
        return Err(Flow::Fault("diag.narrowing-overflow".into()));
    }
    let (min, max) = int_min_max(dt);
    if tr >= min as f64 && tr <= max as f64 && (tr as i128) >= min && (tr as i128) <= max {
        Ok(Value::Int(dt, tr as i128))
    } else {
        Err(Flow::Fault("diag.narrowing-overflow".into()))
    }
}

// The value of an unsuffixed float literal, possibly negated or
// parenthesized; `None` for anything else.
fn bare_float_literal(e: &Expr) -> Option<f64> {
    match &e.kind {
        ExprKind::FloatLit(v, None) => Some(*v),
        ExprKind::Unary(UnOp::Neg, inner) => bare_float_literal(inner).map(|v| -v),
        ExprKind::Paren(inner) => bare_float_literal(inner),
        _ => None,
    }
}

// Does `t` still name one of `params` (a type parameter not yet known)?
fn mentions_type_param(t: &Type, params: &[String]) -> bool {
    match t {
        Type::Named(n, args) => (args.is_empty() && params.contains(n)) || args.iter().any(|a| mentions_type_param(a, params)),
        Type::Ref(i, _) | Type::Slice(i, _) | Type::Rawptr(i) | Type::Array(i, _) | Type::Handle(i) | Type::Mutex(i) | Type::Guard(i) => mentions_type_param(i, params),
        Type::Fn(ps, r) => ps.iter().any(|p| mentions_type_param(p, params)) || mentions_type_param(r, params),
        _ => false,
    }
}

// The temporary a `return` carries out, which the frames it leaves must
// not destroy: it is the caller's result.
fn returned_temp(flow: &Flow) -> Option<u64> {
    match flow {
        Flow::Return(EvalResult::Temp(o)) => Some(*o),
        _ => None,
    }
}

// The object a finished body's result keeps alive through its frame's pop.
fn result_keep(r: &Result<EvalResult, Flow>) -> Option<u64> {
    match r {
        Ok(EvalResult::Temp(o)) => Some(*o),
        Err(flow) => returned_temp(flow),
        _ => None,
    }
}

fn is_bare_literal_expr(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) => true,
        ExprKind::Unary(UnOp::Neg, inner) => matches!(inner.kind, ExprKind::IntLit(_, None)),
        ExprKind::Paren(inner) => is_bare_literal_expr(inner),
        _ => false,
    }
}

pub(crate) fn closure_body_writes(body: &Block, name: &str) -> bool {
    // conservative syntactic scan: any assignment, exclusive borrow, move,
    // or drop whose place root is `name` marks it exclusive (spec/15 §6).
    fn scan_expr(e: &Expr, name: &str, found: &mut bool) {
        match &e.kind {
            ExprKind::Assign(lhs, rhs) => {
                if root_is(lhs, name) {
                    *found = true;
                }
                scan_expr(lhs, name, found);
                scan_expr(rhs, name, found);
            }
            ExprKind::Borrow(Mode::Exclusive, inner) => {
                if root_is(inner, name) {
                    *found = true;
                }
                scan_expr(inner, name, found);
            }
            ExprKind::Call(c, args) => {
                scan_expr(c, name, found);
                for a in args {
                    scan_expr(a, name, found);
                }
                if let ExprKind::Path(segs, _) = &c.kind {
                    if segs.last().map(|s| s.as_str()) == Some("drop") {
                        if let Some(a0) = args.get(0) {
                            if root_is(a0, name) {
                                *found = true;
                            }
                        }
                    }
                }
            }
            ExprKind::Binary(_, a, b) => {
                scan_expr(a, name, found);
                scan_expr(b, name, found);
            }
            ExprKind::Unary(_, a) => scan_expr(a, name, found),
            ExprKind::Deref(a) => scan_expr(a, name, found),
            ExprKind::Field(a, _) => scan_expr(a, name, found),
            ExprKind::Index(a, b) => {
                scan_expr(a, name, found);
                scan_expr(b, name, found);
            }
            ExprKind::Paren(a) => scan_expr(a, name, found),
            ExprKind::Propagate(a) => scan_expr(a, name, found),
            ExprKind::Block(b) => scan_block(b, name, found),
            ExprKind::Unsafe(b) => scan_block(b, name, found),
            ExprKind::If(c, t, f) => {
                scan_expr(c, name, found);
                scan_block(t, name, found);
                if let Some(f) = f {
                    scan_expr(f, name, found);
                }
            }
            ExprKind::While(c, b, step) => {
                scan_expr(c, name, found);
                scan_block(b, name, found);
                if let Some(s) = step {
                    scan_expr(s, name, found);
                }
            }
            ExprKind::Match(s, arms) => {
                scan_expr(s, name, found);
                for a in arms {
                    scan_expr(&a.body, name, found);
                }
            }
            ExprKind::Return(Some(e)) => scan_expr(e, name, found),
            ExprKind::StructLit(_, _, fields) => {
                for (_, e) in fields {
                    scan_expr(e, name, found);
                }
            }
            ExprKind::ArrayLit(items) => {
                for e in items {
                    scan_expr(e, name, found);
                }
            }
            ExprKind::ArrayRepeat(e, _) => scan_expr(e, name, found),
            // A nested closure literal is part of this body's text: a
            // write to `name` inside it is a write whose place root is
            // `name` (spec/15 §6 -- the substitution `x_i := *self.f_i`
            // reaches into the nested literal too, so the inner closure's
            // exclusive capture must be derivable from an exclusive one).
            ExprKind::Closure { captures, body, .. } => {
                if captures.iter().any(|c| c == name) {
                    scan_block(body, name, found);
                }
            }
            _ => {}
        }
    }
    fn root_is(e: &Expr, name: &str) -> bool {
        match &e.kind {
            ExprKind::Path(segs, _) => segs.len() == 1 && segs[0] == name,
            ExprKind::Field(b, _) => root_is(b, name),
            ExprKind::Index(b, _) => root_is(b, name),
            ExprKind::Deref(b) => root_is(b, name),
            _ => false,
        }
    }
    fn scan_block(b: &Block, name: &str, found: &mut bool) {
        for s in &b.stmts {
            match s {
                Stmt::Expr(e) | Stmt::BlockLike(e) => scan_expr(e, name, found),
                Stmt::Let { init: Some(e), .. } => scan_expr(e, name, found),
                _ => {}
            }
        }
        if let Some(t) = &b.tail {
            scan_expr(t, name, found);
        }
    }
    let mut found = false;
    scan_block(body, name, &mut found);
    found
}

/// Every `diag.*` id this interpreter's own source (`interp.rs`,
/// `typecheck.rs`, `modres.rs`) can actually construct, hand-derived by
/// grepping every `"diag.xxx"` string literal plus the one dynamically
/// built id (`fault(alloc_failure)`, `interp.rs`'s `"fault"` intrinsic
/// arm) -- see `src/diagnostics.rs`'s
/// `every_diag_id_this_interpreter_emits_is_in_the_registry` test,
/// which cross-checks every entry here against
/// `spec/registry/diagnostics.md` and fails if this list and the
/// registry ever disagree. Re-derive this list (not just append to it)
/// whenever a new `Flow::Fault(...)`/`Err("diag....)` site is added, so
/// it stays an honest cross-check rather than a rubber stamp.
pub const ALL_EMITTED_DIAGS: &[&str] = &[
    "diag.unittest-name",
    "diag.aliasing-conflict",
    "diag.alloc-failure",
    "diag.unwrap-failed",
    "diag.ambiguous-name",
    "diag.assert-failed",
    "diag.arith-overflow",
    "diag.bad-destructor-signature",
    "diag.destructor-on-plain-type",
    "diag.stack-exhausted",
    "diag.result-discarded",
    "diag.borrow-of-non-place",
    "diag.borrow-of-temporary",
    "diag.capture-list-mismatch",
    "diag.channel-deadlock",
    "diag.channel-zero-capacity",
    "diag.barrier-zero-parties",
    "diag.destroy-while-aliased",
    "diag.direct-destructor-call",
    "diag.div-by-zero",
    "diag.div-overflow",
    "diag.duplicate-item",
    "diag.foreign-associated-fn",
    "diag.index-out-of-bounds",
    "diag.literal-out-of-range",
    "diag.module-cycle",
    "diag.module-file-duplicate",
    "diag.module-file-not-found",
    "diag.move-out-of-field",
    "diag.move-while-aliased",
    "diag.mutex-reentrant-lock",
    "diag.name-not-visible",
    "diag.narrowing-overflow",
    "diag.no-main",
    "diag.non-exhaustive-match",
    "diag.not-ascii",
    "diag.not-char-boundary",
    "diag.static-assert-failed",
    "diag.static-assert-not-constant",
    "diag.syntax-error",
    "diag.overwrite-of-live-resource",
    "diag.read-of-resource",
    "diag.recursive-type",
    "diag.shift-amount-out-of-range",
    "diag.stale-binding",
    "diag.trusted-outside-unsafe",
    "diag.type-mismatch",
    "diag.unbound-name",
];

fn arith_err_diag(e: ArithError) -> String {
    match e {
        ArithError::Overflow => "diag.arith-overflow".into(),
        ArithError::DivByZero => "diag.div-by-zero".into(),
        ArithError::DivOverflow => "diag.div-overflow".into(),
        ArithError::ShiftOutOfRange => "diag.shift-amount-out-of-range".into(),
        ArithError::NarrowOverflow => "diag.narrowing-overflow".into(),
    }
}

fn reinterpret_sign_pub(t: IntTy, bits: u128, bw: u32) -> i128 {
    if t.signed() {
        let sign_bit = 1u128 << (bw - 1);
        if bw < 128 && bits & sign_bit != 0 {
            let full: u128 = !0u128 << bw;
            (bits | full) as i128
        } else {
            bits as i128
        }
    } else {
        bits as i128
    }
}

#[derive(Clone)]
struct ClosureInfo {
    captures: Vec<String>,
    body: std::sync::Arc<Block>,
    subst: HashMap<String, Type>,
    is_move: bool,
    // D-0239: a `move` closure whose body writes a capture changes its
    // own storage when called (`[Closure-Call]`).
    writes_self: bool,
}

// `text(v)` for a number or `bool` (`rule.stdlib.print` `[Print-Int]`,
// `[Print-Float]`): what `print` writes and `String::append` adds.
fn number_text(v: &Value) -> Option<String> {
    Some(match v {
        Value::Int(IntTy::U128, x) => (*x as u128).to_string(),
        Value::Int(_, x) => x.to_string(),
        Value::F64(f) => format_float(*f, false),
        Value::F32(f) => format_float(*f as f64, true),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    })
}

// Whether a line of the combined text is the program's own, not `std`'s
// (the prelude comes first): only such a line locates a diagnostic.
fn user_line(line: usize) -> bool {
    line > crate::prelude::line_count()
}

// A fault from the evaluation of the expression at `line`, tagged with
// that line as `eval`'s own wrapper tags one (a line inside `std` is not
// one: it is left to the calling expression).
fn tag_at(r: EvalOutcome, line: usize) -> EvalOutcome {
    match r {
        Err(Flow::Fault(d)) if !d.contains('@') && user_line(line) => Err(Flow::Fault(format!("{d}@{line}"))),
        other => other,
    }
}

// D-0057: whether the value `v` is the pattern literal `l` (an integer,
// negated or not, or `true`/`false`; the checker typed it as `v`'s type).
fn lit_equals(l: &Expr, v: &Value) -> bool {
    let want: Option<i128> = match &l.kind {
        ExprKind::IntLit(n, _) => Some(*n as i128),
        ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
            ExprKind::IntLit(n, _) => Some((*n as i128).wrapping_neg()),
            _ => None,
        },
        _ => None,
    };
    match (&l.kind, v) {
        (ExprKind::BoolLit(b), Value::Bool(x)) => b == x,
        (_, Value::Int(_, x)) => want == Some(*x),
        _ => false,
    }
}


// An unsuffixed numeric literal, possibly negated or parenthesized
// (D-0079: not evidence for a type parameter while other evidence remains).
fn is_bare_num_literal(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Unary(crate::ast::UnOp::Neg, inner) => is_bare_num_literal(inner),
        ExprKind::Paren(inner) => is_bare_num_literal(inner),
        _ => false,
    }
}
