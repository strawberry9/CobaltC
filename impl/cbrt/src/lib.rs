// cbrt -- the CobaltC runtime linked into every `cobc`-compiled program
// (impl/COBC-PLAN.md §2): `spec/04`'s dynamic Σ over real addresses.
//
// Objects are storage ranges with an identity; access paths are tokens
// with a target object, a projection, a mode and a validity flag. A
// path takes part in aliasing checks only while some live object's
// storage holds it as a reference value (`held-by`, spec/04): the slot
// table maps a slot's address to the token it holds, and a token's
// occurrence count is the number of such slots. That is exactly what
// the interpreter's `scan_refs` computes by walking values, so `clash`
// and `solitary` here are its functions over these tables.
//
// Frames and statement scopes form one LIFO stack: a frame ends the
// objects it owns in reverse origin order (`[Block-Exit]`), a scope ends
// its temporaries (`[Stmt-Exit]`), and `[Fault-Unwind]` folds the whole
// stack the same way before exiting. Destructors are generated C
// functions the type table points at; the runtime calls them.
//
// Threads (impl/COBC-PLAN.md §9, §10 T3): every CobaltC thread is an
// OS thread, and they run in parallel. The shared state (objects, paths,
// slots, threads, locks) is one global guarded by a reentrant runtime
// lock, held for the duration of each runtime call — reentrant because
// a destructor the runtime calls re-enters it — and released in full
// while a thread waits (`join`, `lock`, a handle's destructor). The lock
// is taken only once a second thread exists, so a single-threaded
// program pays nothing. Generated code between runtime calls runs
// unlocked: any memory two threads can both reach is reached through
// access paths the runtime checks, under the lock, before each access.
// The frame/scope stack, the in-flight queue and the current location
// are the thread's own (`TState`, thread-local).

#![allow(clippy::missing_safety_doc)]

#[path = "../../src/diagnostics.rs"]
#[allow(dead_code)]
mod diagnostics;

use std::cell::{Cell, UnsafeCell};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;
use std::ffi::{c_char, CStr};
use std::io::Write;

// The runtime's tables are keyed by object and path ids (sequential
// integers) and by addresses: a multiplicative hash is enough, and the
// standard library's SipHash, built to resist hostile keys, cost more
// than half of a compiled program's instructions.
#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0 ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
    }
}

type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<IdHasher>>;

// The object and path tables: every runtime call looks entries up by id,
// several times, so they are arenas indexed by the id itself rather than
// hash maps. An id is `generation << 32 | (slot + 1)` -- never 0, which
// the runtime uses for "none". A removed entry's slot is reused under the
// next generation, so a stale id (a token surviving in a register after
// its path ended) finds nothing, exactly as a removed hash-map key did.
struct Arena<T> {
    slots: Vec<(u32, Option<T>)>,
    free: Vec<u32>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena { slots: Vec::new(), free: Vec::new() }
    }
}

impl<T> Arena<T> {
    #[inline]
    fn split(k: u64) -> (usize, u32) {
        (((k & 0xffff_ffff) as usize).wrapping_sub(1), (k >> 32) as u32)
    }
    // A fresh id whose entry `insert` then fills.
    fn reserve(&mut self) -> u64 {
        let i = match self.free.pop() {
            Some(i) => i as usize,
            None => {
                self.slots.push((1, None));
                self.slots.len() - 1
            }
        };
        ((self.slots[i].0 as u64) << 32) | (i as u64 + 1)
    }
    fn insert(&mut self, k: u64, v: T) {
        let (i, g) = Self::split(k);
        debug_assert_eq!(self.slots[i].0, g, "cbrt: arena insert under a stale id");
        self.slots[i].1 = Some(v);
    }
    #[inline]
    fn get(&self, k: &u64) -> Option<&T> {
        let (i, g) = Self::split(*k);
        match self.slots.get(i) {
            Some((sg, Some(v))) if *sg == g => Some(v),
            _ => None,
        }
    }
    #[inline]
    fn get_mut(&mut self, k: &u64) -> Option<&mut T> {
        let (i, g) = Self::split(*k);
        match self.slots.get_mut(i) {
            Some((sg, Some(v))) if *sg == g => Some(v),
            _ => None,
        }
    }
    #[inline]
    fn contains_key(&self, k: &u64) -> bool {
        self.get(k).is_some()
    }
    fn remove(&mut self, k: &u64) -> Option<T> {
        let (i, g) = Self::split(*k);
        match self.slots.get_mut(i) {
            Some((sg, v @ Some(_))) if *sg == g => {
                let out = v.take();
                *sg = sg.wrapping_add(1).max(1);
                self.free.push(i as u32);
                out
            }
            _ => None,
        }
    }
}

// Raw address -> the reclaimed object there. `[Release]` asks for every
// object in a range, so persistent objects are kept ordered; ephemeral
// element objects (`Obj::ephemeral`) are established and ended at every
// `Vec` element access, and live briefly, so they sit in a hash map and a
// range query scans it -- it holds only the few whose borrow is still
// alive.
#[derive(Default)]
// Both maps ordered: `range` is asked for every `release` (a `Vec`'s
// elements one at a time, `Vec::clear`), so scanning every ephemeral
// entry there made clearing a `Vec` whose elements had been indexed
// quadratic.
struct Reclaimed {
    ordered: BTreeMap<usize, u64>,
    ephemeral: BTreeMap<usize, u64>,
}

impl Reclaimed {
    fn get(&self, a: &usize) -> Option<&u64> {
        self.ephemeral.get(a).or_else(|| self.ordered.get(a))
    }
    fn insert(&mut self, a: usize, obj: u64) {
        self.ephemeral.remove(&a);
        self.ordered.insert(a, obj);
    }
    // Only where no live object lies at `a` (`cb_elem_borrow`); an ordered
    // entry left there names an ended object, which every reader skips.
    fn insert_ephemeral(&mut self, a: usize, obj: u64) {
        self.ephemeral.insert(a, obj);
    }
    fn remove(&mut self, a: &usize) {
        if self.ephemeral.remove(a).is_none() {
            self.ordered.remove(a);
        }
    }
    // Removes the entry at `a` if it is `obj`.
    fn remove_if(&mut self, a: usize, obj: u64, ephemeral: bool) {
        if ephemeral {
            if let std::collections::btree_map::Entry::Occupied(e) = self.ephemeral.entry(a) {
                if *e.get() == obj {
                    e.remove();
                }
            }
        } else if self.get(&a) == Some(&obj) {
            self.remove(&a);
        }
    }
    // Every (address, object) in `[lo, hi)`, in address order.
    fn range(&self, lo: usize, hi: usize) -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = self.ordered.range(lo..hi).map(|(a, o)| (*a, *o)).collect();
        let before = v.len();
        v.extend(self.ephemeral.range(lo..hi).map(|(a, o)| (*a, *o)));
        if v.len() > before {
            v.sort_unstable();
        }
        v
    }
}

impl<T> std::ops::Index<&u64> for Arena<T> {
    type Output = T;
    #[inline]
    fn index(&self, k: &u64) -> &T {
        self.get(k).expect("cbrt: no entry for id")
    }
}

const NOLOC: u32 = u32::MAX;
const NOTYPE: u32 = u32::MAX;

#[repr(C)]
pub struct CbField {
    off: u64,
    ty: u32,
}

#[repr(C)]
pub struct CbType {
    name: *const c_char,
    size: u64,
    kind: u32,
    is_resource: u8,
    has_refs: u8,
    drop: Option<unsafe extern "C" fn(CbRef)>,
    nfields: u32,
    fields: *const CbField,
    nvariants: u32,
    variants: *const u32,
    payload_off: u64,
    elem: u32,
    count: u64,
    owner: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CbRef {
    p: *mut u8,
    tok: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CbProj {
    kind: u32,
    idx: u64,
}

const K_STRUCT: u32 = 1;
const K_ENUM: u32 = 2;
const K_ARRAY: u32 = 3;
const K_REF: u32 = 4;
const K_FN: u32 = 6;
const K_HANDLE: u32 = 7;
const K_GUARD: u32 = 8;

#[repr(C)]
pub struct CbFnBox {
    code: *mut u8,
    data: *mut u8,
    root: u64,
    ty: u32,
    closure: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Proj {
    Field(u64),
    Index(u64),
    Payload,
}

struct TypeInfo {
    size: u64,
    kind: u32,
    is_resource: bool,
    has_refs: bool,
    drop: Option<unsafe extern "C" fn(CbRef)>,
    fields: Vec<(u64, u32)>,
    variants: Vec<u32>,
    payload_off: u64,
    elem: u32,
    count: u64,
    owner: bool,
}

struct Obj {
    addr: usize,
    ty: u32,
    alive: bool,
    init: bool,
    is_resource: bool,
    reclaimed: bool,      // storage-kind reclaimed: at a raw address, owned by no frame
    root: Option<u64>,
    frame: Option<usize>, // index into `stack` of the owning frame
    scope: Option<usize>, // index into `stack` of the scope this temporary belongs to
    tid: u64,             // the thread whose stack `frame`/`scope` index
    // A reclaimed object of a plain, reference-free type established by
    // `cb_elem_borrow` and never re-attached by `[Reclaim]`: its root
    // token never leaves the runtime, so once its last derived path is
    // gone nothing can observe it and it ends (see `retire_token`).
    ephemeral: bool,
    // Every path whose `of` is this object (live or held-but-invalid).
    toks: Vec<u64>,
}

struct PathRec {
    of: u64,
    proj: Vec<Proj>,
    exclusive: bool,
    valid: bool,
    is_root: bool,
    ancestors: Vec<u64>,
    occ: u32,
    scope: Option<usize>, // the statement scope this path was formed in, while unheld
    // `lock-derived` (spec/19 §2): the projection of the mutex the lock
    // path this one was formed through locks, for `sync-exempt`.
    lock_of: Option<Vec<Proj>>,
}

#[derive(Default)]
struct Scope {
    temps: Vec<u64>,
    toks: Vec<u64>,
}

enum Entry {
    Frame(Vec<u64>),
    Scope(Scope),
}

enum Inflight {
    Obj(u64),
    Ref(u64),
    Datum(Vec<Option<u64>>),
}

// A thread's own part of the state.
#[derive(Default)]
struct TState {
    stack: Vec<Entry>,
    inflight: VecDeque<Inflight>,
}

// `state.threads`: a spawned thread's argument block (its result at
// offset 0), and whether the result is still there to be claimed.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Status {
    Running,
    Done,
    Taken,
}

struct ThreadRec {
    status: Status,
    env: usize,
    env_size: u64,
    res_ty: u32,
    res_obj: u64,
}

struct Rt {
    files: Vec<String>,
    types: Vec<TypeInfo>,
    objs: Arena<Obj>,
    paths: Arena<PathRec>,
    slots: BTreeMap<usize, u64>,
    reclaimed: Reclaimed,
    fn_items: HashMap<usize, usize>,  // code address -> its interned box
    fn_boxes: HashMap<usize, u64>,    // closure box handle -> the closure object it owns
    unwinding: bool,
    threads: HashMap<u64, ThreadRec>,
    locks: HashMap<usize, u64>, // a mutex's address -> the thread holding it (`state.sync`)
    live: usize,
    next_tid: u64,
    // Emptied id lists, reused: a frame's, a scope's and an object's
    // path list are created and dropped at every block, statement and
    // borrow, and allocating each was a tenth of a program's run.
    pool: Vec<Vec<u64>>,
}

fn take_vec(pool: &mut Vec<Vec<u64>>) -> Vec<u64> {
    pool.pop().unwrap_or_default()
}

fn give_vec(pool: &mut Vec<Vec<u64>>, mut v: Vec<u64>) {
    if v.capacity() > 0 && pool.len() < 4096 {
        v.clear();
        pool.push(v);
    }
}

struct RtCell(UnsafeCell<Option<Rt>>);
unsafe impl Sync for RtCell {}
static RT: RtCell = RtCell(UnsafeCell::new(None));

fn rt() -> &'static mut Rt {
    unsafe { (*RT.0.get()).as_mut().expect("cb_init not called") }
}

// ---- the runtime lock ----

// Set when the second thread is spawned; never cleared.
static MULTI: AtomicBool = AtomicBool::new(false);

// The runtime lock: a standard mutex (it spins briefly before sleeping,
// which a runtime taken and released at every call needs — a condition
// variable hand-off convoyed), made reentrant by recording its owner
// and this thread's depth; the owning thread keeps the guard.
static RTLOCK: Mutex<()> = Mutex::new(());
static OWNER: AtomicU64 = AtomicU64::new(0);

// What blocked threads wait for, by key — a thread id (`join`, a
// handle's destructor) or a mutex's address with the top bit set
// (`lock`): (key, times signalled, threads waiting). A signal wakes
// anyone only if someone waits on that very key; a mutex unlocked in a
// loop must not wake a thread that is only waiting for a join.
static EVENTS: Mutex<Vec<(u64, u64, u32)>> = Mutex::new(Vec::new());
static EVENT_CV: Condvar = Condvar::new();

fn mutex_key(addr: usize) -> u64 {
    addr as u64 | 1 << 63
}

thread_local! {
    static TID: Cell<u64> = const { Cell::new(1) };
    // A spawned thread's own state; null on the main thread.
    static TSP: Cell<*mut TState> = const { Cell::new(std::ptr::null_mut()) };
    static HELD: Cell<u32> = const { Cell::new(0) };
    static GUARD: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> = const { std::cell::RefCell::new(None) };
}

// Until the first spawn only the main thread exists, so its state and
// its runtime-call depth are plain statics and no thread-local is read.
static mut MAIN_TS: TState = TState { stack: Vec::new(), inflight: VecDeque::new() };
// How many destructor callbacks the main thread is inside: the only way
// a spawn (the switch to locking) can happen while an outer runtime
// call is still active, one entry per level (`go_multi`).
static mut CALLBACK_DEPTH: u32 = 0;

#[inline(always)]
fn multi() -> bool {
    MULTI.load(Ordering::Relaxed)
}

fn me() -> u64 {
    if multi() {
        TID.with(|t| t.get())
    } else {
        1
    }
}

// This thread's frame/scope stack, in-flight queue and location.
#[inline(always)]
fn ts() -> &'static mut TState {
    let p = if multi() { TSP.with(|c| c.get()) } else { std::ptr::null_mut() };
    if p.is_null() {
        unsafe { &mut *std::ptr::addr_of_mut!(MAIN_TS) }
    } else {
        unsafe { &mut *p }
    }
}

fn lock_acquire(count: u32) {
    let me = TID.with(|t| t.get());
    if OWNER.load(Ordering::Relaxed) == me {
        HELD.with(|h| h.set(h.get() + count));
        return;
    }
    let g = RTLOCK.lock().unwrap_or_else(|e| e.into_inner());
    OWNER.store(me, Ordering::Relaxed);
    HELD.with(|h| h.set(count));
    GUARD.with(|c| *c.borrow_mut() = Some(g));
}

fn lock_release() {
    let left = HELD.with(|h| {
        let n = h.get() - 1;
        h.set(n);
        n
    });
    if left == 0 {
        OWNER.store(0, Ordering::Relaxed);
        GUARD.with(|c| drop(c.borrow_mut().take()));
    }
}

// Gives the lock up entirely (a blocking wait); returns the depth to restore.
fn lock_release_all() -> u32 {
    let n = HELD.with(|h| h.replace(0));
    OWNER.store(0, Ordering::Relaxed);
    GUARD.with(|c| drop(c.borrow_mut().take()));
    n
}

// Entering the runtime from generated code (every `extern "C"` entry
// that touches shared state). `.0`: whether this entry took the lock.
struct Entered(bool);

#[inline(always)]
fn enter() -> Entered {
    let m = multi();
    if m {
        lock_acquire(1);
    }
    Entered(m)
}

impl Drop for Entered {
    #[inline(always)]
    fn drop(&mut self) {
        // An entry made before the switch to locking releases the count
        // `go_multi` took for it.
        if self.0 || multi() {
            lock_release();
        }
    }
}

// The first spawn turns locking on. The main thread is then inside
// `cb_spawn`'s own entry plus one entry per destructor callback it is
// running (`CALLBACK_DEPTH`); the lock is taken once for each, so each
// of their exits releases one. Only the main thread exists before this,
// and a spawned thread starts after it.
fn go_multi() {
    if !multi() {
        lock_acquire(1 + unsafe { CALLBACK_DEPTH });
        MULTI.store(true, Ordering::Release);
    }
}

fn signal(key: u64) {
    let mut g = EVENTS.lock().unwrap();
    if let Some(ev) = g.iter_mut().find(|ev| ev.0 == key) {
        ev.1 += 1;
        if ev.2 > 0 {
            drop(g);
            EVENT_CV.notify_all();
        }
    }
}

// Blocks, with the runtime lock released in full, until `ready` holds.
// `ready` is evaluated only under the lock, and the waiter registers on
// `key` before letting the lock go; signals are raised only under the
// lock, so none can slip between the check and the wait.
fn wait_until(key: u64, ready: &mut dyn FnMut(&Rt) -> bool) {
    loop {
        if ready(rt()) {
            return;
        }
        let e = {
            let mut g = EVENTS.lock().unwrap();
            match g.iter_mut().find(|ev| ev.0 == key) {
                Some(ev) => {
                    ev.2 += 1;
                    ev.1
                }
                None => {
                    g.push((key, 0, 1));
                    0
                }
            }
        };
        let held = lock_release_all();
        {
            let mut g = EVENTS.lock().unwrap();
            loop {
                let now = g.iter().find(|ev| ev.0 == key).map_or(e, |ev| ev.1);
                if now != e {
                    break;
                }
                let (g2, to) = EVENT_CV.wait_timeout(g, Duration::from_millis(50)).unwrap();
                g = g2;
                if to.timed_out() {
                    break;
                }
            }
            if let Some(i) = g.iter().position(|ev| ev.0 == key) {
                g[i].2 -= 1;
                if g[i].2 == 0 {
                    g.swap_remove(i);
                }
            }
        }
        lock_acquire(held);
    }
}

extern "C" {
    fn cb_where(file: *mut u32, line: *mut u32);
    fn cb_set_where(file: u32, line: u32);
}

// The executing statement's location (`cb_at`, a C thread-local in the
// generated program).
fn get_at() -> (u32, u32) {
    let (mut f, mut l) = (NOLOC, 0u32);
    unsafe { cb_where(&mut f, &mut l) };
    (f, l)
}

fn set_at(file: u32, line: u32) {
    unsafe { cb_set_where(file, line) };
}

fn flush_stdout() {
    let _ = std::io::stdout().flush();
}

// ---- program ----

#[no_mangle]
pub unsafe extern "C" fn cb_init(files: *const *const c_char, nfiles: usize, types: *const CbType, ntypes: usize) {
    let mut fs = Vec::new();
    for i in 0..nfiles {
        fs.push(CStr::from_ptr(*files.add(i)).to_string_lossy().into_owned());
    }
    let mut ts = Vec::new();
    for i in 0..ntypes {
        let t = &*types.add(i);
        let fields = (0..t.nfields as usize).map(|k| {
            let f = &*t.fields.add(k);
            (f.off, f.ty)
        });
        let variants = (0..t.nvariants as usize).map(|k| *t.variants.add(k));
        ts.push(TypeInfo {
            size: t.size,
            kind: t.kind,
            is_resource: t.is_resource != 0,
            has_refs: t.has_refs != 0,
            drop: t.drop,
            fields: fields.collect(),
            variants: variants.collect(),
            payload_off: t.payload_off,
            elem: t.elem,
            count: t.count,
            owner: t.owner != 0,
        });
    }
    *RT.0.get() = Some(Rt {
        files: fs,
        types: ts,
        objs: Arena::default(),
        paths: Arena::default(),
        slots: BTreeMap::new(),
        reclaimed: Reclaimed::default(),
        fn_items: HashMap::default(),
        fn_boxes: HashMap::default(),
        unwinding: false,
        threads: HashMap::default(),
        locks: HashMap::default(),
        live: 1,
        next_tid: 2,
        pool: Vec::new(),
    });
}

#[no_mangle]
pub unsafe extern "C" fn cb_fault(diag: *const c_char, file: u32, line: u32) -> ! {
    let _e = enter();
    let id = CStr::from_ptr(diag).to_string_lossy().into_owned();
    fault(&id, file, line)
}

fn fault(id: &str, file: u32, line: u32) -> ! {
    // The GIL is never released again: `[Fault-Unwind]` — other threads
    // take no further steps and their frames are not unwound.
    let r = rt();
    let (file, line) = if file == NOLOC && line == 0 { get_at() } else { (file, line) };
    flush_stdout();
    if r.unwinding {
        // A destructor faulting during the unwind: the program's outcome
        // is the first fault, already reported.
        std::process::exit(1);
    }
    let at = if file == NOLOC { None } else { r.files.get(file as usize).map(|f| (f.as_str(), line as usize)) };
    eprint!("{}", diagnostics::render(id, "dynamic", at));
    r.unwinding = true;
    // [Fault-Unwind]: fold every open scope and frame, innermost first.
    while let Some(entry) = ts().stack.pop() {
        match entry {
            Entry::Frame(owned) => end_owned(owned),
            Entry::Scope(s) => end_scope(s),
        }
    }
    flush_stdout();
    std::process::exit(1);
}

// `[Terminate-Ok]`: `ok(s)`, reported as exit status `s` (`rule.fn.program`).
#[no_mangle]
pub extern "C" fn cb_terminate_ok(status: u8) -> ! {
    flush_stdout();
    std::process::exit(status as i32);
}

// The program's arguments (`rule.stdlib.args`): `argv` without the
// program's own name, copied once at startup, before `main` begins.
static ARGS: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

#[no_mangle]
pub unsafe extern "C" fn cb_set_args(argc: i32, argv: *const *const c_char) {
    let args = (1..argc.max(0) as usize).map(|i| CStr::from_ptr(*argv.add(i)).to_bytes().to_vec()).collect();
    let _ = ARGS.set(args);
}

// `std::arg_bytes`: up to `n` bytes of argument `i` into `p`; the
// argument's whole length, or -1 when there is no argument `i`.
#[no_mangle]
pub unsafe extern "C" fn cb_arg_bytes(i: u64, p: *mut u8, n: u64) -> i64 {
    match ARGS.get().and_then(|a| a.get(i as usize)) {
        Some(a) => {
            let k = a.len().min(n as usize);
            std::ptr::copy_nonoverlapping(a.as_ptr(), p, k);
            a.len() as i64
        }
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_write_out(p: *const u8, n: u64) -> i64 {
    let bytes = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n as usize) };
    let mut out = std::io::stdout().lock();
    match out.write_all(bytes).and_then(|_| out.flush()) {
        Ok(()) => n as i64,
        Err(_) => -1,
    }
}

// `std::write_err` (`eprintf`, D-0040): as `cb_write_out`, to standard
// error. Standard output is flushed at each write, so the two interleave
// in program order.
#[no_mangle]
pub unsafe extern "C" fn cb_write_err(p: *const u8, n: u64) -> i64 {
    let bytes = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n as usize) };
    let mut out = std::io::stderr().lock();
    match out.write_all(bytes).and_then(|_| out.flush()) {
        Ok(()) => n as i64,
        Err(_) => -1,
    }
}

// `std::read`: up to `n` bytes of standard input into `p`; 0 at its end,
// -1 on an error. Rust's stdin is buffered, so `read_line`'s byte at a
// time costs no system call per byte. Output is flushed first, so a
// prompt is on the screen before the program waits.
#[no_mangle]
pub unsafe extern "C" fn cb_read_in(p: *mut u8, n: u64) -> i64 {
    flush_stdout();
    if n == 0 {
        return 0;
    }
    let buf = std::slice::from_raw_parts_mut(p, n as usize);
    let mut input = std::io::stdin().lock();
    loop {
        match std::io::Read::read(&mut input, buf) {
            Ok(k) => return k as i64,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return -1,
        }
    }
}

// `print`'s text for a float (`rule.stdlib.print` [Print-Float], spec/21
// §2a): the shortest decimal digits that read back as `v` in its own
// type, laid out positionally for a decimal exponent k with -7 < k < 21
// (always with a fraction, `1.0`), and as `d.ddde±k` otherwise (`1.0e21`,
// `1.5e-7`). `coby`'s `value::format_float` is the same function, so the two
// implementations print the same bytes.
pub fn format_float(v: f64, is_f32: bool) -> String {
    fmt::format_float(v, is_f32)
}

fn print_text(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes()).and_then(|_| out.flush());
}

// `print` of an integer (`rule.stdlib.print` [Print-Int]): its 128-bit
// image in two halves, read as signed or unsigned.
#[no_mangle]
pub extern "C" fn cb_print_int(hi: u64, lo: u64, signed: u32) {
    let bits = ((hi as u128) << 64) | lo as u128;
    print_text(&if signed != 0 { (bits as i128).to_string() } else { bits.to_string() });
}

#[no_mangle]
pub extern "C" fn cb_print_f64(v: f64) {
    print_text(&format_float(v, false));
}

#[no_mangle]
pub extern "C" fn cb_print_f32(v: f32) {
    print_text(&format_float(v as f64, true));
}

#[no_mangle]
pub extern "C" fn cb_print_bool(b: u32) {
    print_text(if b != 0 { "true" } else { "false" });
}

// `String::append`'s `text(v)` for a number or `bool` (`rule.stdlib.text`,
// `std`'s `text_write`): written to `buf`, which holds 64 bytes (the
// longest text, an `i128`'s, is 40); returns its length.
unsafe fn put_text(buf: *mut u8, t: &str) -> u64 {
    std::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len());
    t.len() as u64
}

#[no_mangle]
pub unsafe extern "C" fn cb_text_int(hi: u64, lo: u64, signed: u32, buf: *mut u8) -> u64 {
    let bits = ((hi as u128) << 64) | lo as u128;
    put_text(buf, &if signed != 0 { (bits as i128).to_string() } else { bits.to_string() })
}

#[no_mangle]
pub unsafe extern "C" fn cb_text_f64(v: f64, buf: *mut u8) -> u64 {
    put_text(buf, &format_float(v, false))
}

#[no_mangle]
pub unsafe extern "C" fn cb_text_f32(v: f32, buf: *mut u8) -> u64 {
    put_text(buf, &format_float(v as f64, true))
}

#[no_mangle]
pub unsafe extern "C" fn cb_text_bool(b: u32, buf: *mut u8) -> u64 {
    put_text(buf, if b != 0 { "true" } else { "false" })
}

// `parse<T>` (`rule.stdlib.text` `[Parse]`, `std`'s `parse_check` and
// `parse_value`): the scanner `coby` uses too. -1 for a number, whose
// value goes to `out` if it is not null; else `numtext`'s code.
#[path = "../../src/numtext.rs"]
mod numtext;

// `printf`'s and `String::appendf`'s text (`rule.stdlib.format`, `$fmt`):
// the formatter `coby` and the front end use too.
#[path = "../../src/fmt.rs"]
mod fmt;

#[repr(C)]
pub struct CbFmtArg {
    kind: u32, // 0 unsigned integer, 1 signed integer, 2 float, 3 text
    width: u32,
    hi: u64,
    lo: u64,
    f: f64,
    p: *const u8,
    n: u64,
}

#[repr(C)]
pub struct CbStr {
    p: *const u8,
    n: u64,
}

thread_local! {
    // The last `$fmt` text of this thread: valid until its next `$fmt`,
    // after `print` or `String::append` has taken it.
    static FORMATTED: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[no_mangle]
pub unsafe extern "C" fn cb_format(f: *const u8, fn_: u64, args: *const CbFmtArg, n: u64, out: *mut CbStr) {
    let pieces = fmt::parse(bytes(f, fn_)).unwrap_or_default();
    let args: Vec<fmt::Arg> = (0..n as usize)
        .map(|i| {
            let a = &*args.add(i);
            match a.kind {
                0 | 1 => fmt::Arg::Int { bits: ((a.hi as u128) << 64) | a.lo as u128, signed: a.kind == 1, width: a.width },
                2 => fmt::Arg::Float(a.f, a.width == 32),
                _ => fmt::Arg::Text(bytes(a.p, a.n).to_vec()),
            }
        })
        .collect();
    let text = fmt::render(&pieces, &args);
    FORMATTED.with(|b| {
        let mut b = b.borrow_mut();
        *b = text;
        *out = CbStr { p: b.as_ptr(), n: b.len() as u64 };
    });
}

// `std::file_read` / `std::file_write` (`rule.stdlib.file`): the file
// access `coby` uses too.
#[path = "../../src/fileio.rs"]
mod fileio;

#[no_mangle]
pub unsafe extern "C" fn cb_file_read(path: *const u8, path_len: u64, buf: *mut u8, cap: u64) -> i64 {
    match fileio::read(bytes(path, path_len)) {
        Ok(data) => {
            let k = data.len().min(cap as usize);
            if k > 0 {
                std::ptr::copy_nonoverlapping(data.as_ptr(), buf, k);
            }
            data.len() as i64
        }
        Err(c) => c,
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_file_write(path: *const u8, path_len: u64, buf: *const u8, len: u64) -> i64 {
    match fileio::write(bytes(path, path_len), bytes(buf, len)) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

// `std::file_op` (`rule.stdlib.file-handle`, D-0054): `File`'s
// primitive, over the open-file table `coby` uses too.
#[no_mangle]
pub unsafe extern "C" fn cb_file_op(op: u64, h: u64, buf: *mut u8, n: u64) -> i64 {
    let input = if fileio::takes_input(op) { bytes(buf, n) } else { &[] };
    let (r, out) = fileio::call(op, h, input, n);
    if !out.is_empty() {
        std::ptr::copy_nonoverlapping(out.as_ptr(), buf, out.len());
    }
    r
}

// `std::file_at`: `seek` and `len`, whose number is a position.
#[no_mangle]
pub extern "C" fn cb_file_at(op: u64, h: u64, pos: u64) -> i64 {
    fileio::call(op, h, &[], pos).0
}

unsafe fn bytes<'a>(p: *const u8, n: u64) -> &'a [u8] {
    if n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, n as usize)
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_parse_int(p: *const u8, n: u64, signed: u32, bits: u32, out: *mut u64) -> i64 {
    match numtext::parse_int(bytes(p, n), signed != 0, bits) {
        Ok(v) => {
            if !out.is_null() {
                *out = (v >> 64) as u64;
                *out.add(1) = v as u64;
            }
            -1
        }
        Err(code) => code,
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_parse_float(p: *const u8, n: u64, is_f32: u32, out: *mut f64) -> i64 {
    match numtext::parse_float(bytes(p, n), is_f32 != 0) {
        Ok(v) => {
            if !out.is_null() {
                *out = v;
            }
            -1
        }
        Err(code) => code,
    }
}

// ---- frames and scopes ----

#[no_mangle]
pub extern "C" fn cb_frame_push() {
    let _e = enter();
    let r = rt();
    let v = take_vec(&mut r.pool);
    ts().stack.push(Entry::Frame(v));
}

#[no_mangle]
pub extern "C" fn cb_frame_pop() {
    let _e = enter();
    match ts().stack.pop() {
        Some(Entry::Frame(owned)) => end_owned(owned),
        Some(Entry::Scope(_)) => panic!("cbrt: frame pop closed a statement scope"),
        None => panic!("cbrt: frame pop on an empty stack"),
    }
}

#[no_mangle]
pub extern "C" fn cb_stmt_push() {
    let _e = enter();
    let r = rt();
    let (temps, toks) = (take_vec(&mut r.pool), take_vec(&mut r.pool));
    ts().stack.push(Entry::Scope(Scope { temps, toks }));
}

#[no_mangle]
pub extern "C" fn cb_stmt_pop() {
    let _e = enter();
    match ts().stack.pop() {
        Some(Entry::Scope(s)) => end_scope(s),
        Some(Entry::Frame(_)) => panic!("cbrt: statement pop closed a frame"),
        None => panic!("cbrt: statement pop on an empty stack"),
    }
}

// `[Stmt-Exit]`: the scope's temporaries end, and every path formed
// in it that no object holds ends too (`Unheld`); a held path lives
// on with its holder.
fn end_scope(s: Scope) {
    end_temps(&s.temps);
    let r = rt();
    for &t in &s.toks {
        let unheld = matches!(r.paths.get(&t), Some(p) if p.occ == 0);
        if unheld {
            retire_token(r, t);
        } else if let Some(p) = r.paths.get_mut(&t) {
            p.scope = None;
        }
    }
    give_vec(&mut r.pool, s.temps);
    give_vec(&mut r.pool, s.toks);
}

// A path stops being valid. One that nothing holds is removed at once:
// any later use through it is `diag.stale-binding` either way, and
// keeping dead paths made every check on a long-lived object (a mutex
// locked in a loop) scan all of them. A held one is removed when its
// last holder lets go (`drop_occ`).
fn invalidate(r: &mut Rt, tok: u64) {
    match r.paths.get_mut(&tok) {
        Some(p) if p.occ == 0 => retire_token(r, tok),
        Some(p) => p.valid = false,
        None => {}
    }
}

// One fewer slot holds `tok`.
fn drop_occ(r: &mut Rt, tok: u64) {
    if let Some(p) = r.paths.get_mut(&tok) {
        p.occ = p.occ.saturating_sub(1);
        if p.occ == 0 && !p.valid {
            retire_token(r, tok);
        }
    }
}

fn retire_token(r: &mut Rt, t: u64) {
    if let Some(p) = r.paths.remove(&t) {
        let mut rest = None;
        if let Some(o) = r.objs.get_mut(&p.of) {
            o.toks.retain(|&x| x != t);
            if o.toks.len() == 1 {
                rest = Some(o.toks[0]);
            }
        }
        // An ephemeral element object left with only its root path is
        // unobservable: a later `[Reclaim]` of the same cells establishes
        // an object no program can tell from it (plain, reference-free,
        // `init = valid`, no authority), so it ends now rather than at
        // `[Release]`. This is what keeps a loop over a Vec from holding
        // one object per element it ever touched.
        if !p.is_root {
            if let Some(o) = r.objs.get(&p.of) {
                let unobserved = match o.root {
                    None => o.toks.is_empty(),
                    Some(root) => rest == Some(root),
                };
                if o.ephemeral && unobserved {
                    end_identity(r, p.of);
                }
            }
        }
    }
}

fn scope_toks(_r: &mut Rt, i: usize) -> Option<&mut Vec<u64>> {
    match ts().stack.get_mut(i) {
        Some(Entry::Scope(s)) => Some(&mut s.toks),
        _ => None,
    }
}

// `[Block-Exit]`: owned objects, last established first.
fn end_owned(owned: Vec<u64>) {
    for &obj in owned.iter().rev() {
        let r = rt();
        let (alive, root) = match r.objs.get(&obj) {
            Some(o) => (o.alive, o.root),
            None => continue,
        };
        if alive {
            destroy_obj(obj, root);
        }
    }
    give_vec(&mut rt().pool, owned);
}

// `[Stmt-Exit]`: temporaries not adopted, sent, or re-stamped outward.
fn end_temps(temps: &[u64]) {
    for &obj in temps.iter().rev() {
        let r = rt();
        let still_temp = matches!(r.objs.get(&obj), Some(o) if o.alive && o.frame.is_none() && o.root.is_none());
        if still_temp {
            destroy_obj(obj, None);
        }
    }
}

fn top_frame(_r: &Rt) -> Option<usize> {
    ts().stack.iter().rposition(|e| matches!(e, Entry::Frame(_)))
}

fn top_scope(_r: &Rt) -> Option<usize> {
    ts().stack.iter().rposition(|e| matches!(e, Entry::Scope(_)))
}

fn detach(r: &mut Rt, obj: u64) {
    let (frame, scope) = match r.objs.get(&obj) {
        // Only the owning thread edits its own stack; an index into
        // another thread's stack is never followed.
        Some(o) if o.tid == me() || (o.frame.is_none() && o.scope.is_none()) => (o.frame, o.scope),
        Some(_) => {
            debug_assert!(false, "cbrt: detach of another thread's frame or scope entry");
            (None, None)
        }
        None => return,
    };
    if let Some(i) = frame {
        if let Some(Entry::Frame(v)) = ts().stack.get_mut(i) {
            v.retain(|&x| x != obj);
        }
    }
    if let Some(i) = scope {
        if let Some(Entry::Scope(s)) = ts().stack.get_mut(i) {
            s.temps.retain(|&x| x != obj);
        }
    }
    let o = r.objs.get_mut(&obj).unwrap();
    o.frame = None;
    o.scope = None;
}

fn stamp_temp(r: &mut Rt, obj: u64, scope: Option<usize>) {
    if let Some(i) = scope {
        if let Some(Entry::Scope(s)) = ts().stack.get_mut(i) {
            s.temps.push(obj);
        }
    }
    let o = r.objs.get_mut(&obj).unwrap();
    o.scope = scope;
    if scope.is_some() {
        o.tid = me();
    }
}

fn detach_tok(r: &mut Rt, tok: u64) {
    let scope = match r.paths.get(&tok) {
        Some(p) => p.scope,
        None => return,
    };
    if let Some(i) = scope {
        if let Some(v) = scope_toks(r, i) {
            v.retain(|&x| x != tok);
        }
    }
    r.paths.get_mut(&tok).unwrap().scope = None;
}

fn stamp_tok(r: &mut Rt, tok: u64, scope: Option<usize>) {
    if let Some(i) = scope {
        if let Some(v) = scope_toks(r, i) {
            v.push(tok);
        }
    }
    if let Some(p) = r.paths.get_mut(&tok) {
        p.scope = scope;
    }
}

// ---- objects ----

fn new_obj(addr: usize, ty: u32, init: bool) -> u64 {
    let r = rt();
    let id = r.objs.reserve();
    let is_resource = r.types.get(ty as usize).map_or(false, |t| t.is_resource);
    r.objs.insert(id, Obj { addr, ty, alive: true, init, is_resource, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: Vec::new() });
    let scope = top_scope(r);
    stamp_temp(r, id, scope);
    id
}

#[no_mangle]
pub extern "C" fn cb_new(addr: *mut u8, ty: u32, _file: u32, _line: u32) -> u64 {
    let _e = enter();
    new_obj(addr as usize, ty, true)
}

#[no_mangle]
pub extern "C" fn cb_new_uninit(addr: *mut u8, ty: u32, _file: u32, _line: u32) -> u64 {
    let _e = enter();
    new_obj(addr as usize, ty, false)
}

fn mint(r: &mut Rt, of: u64, proj: Vec<Proj>, exclusive: bool, is_root: bool, ancestors: Vec<u64>) -> u64 {
    mint_locked(r, of, proj, exclusive, is_root, ancestors, None)
}

fn mint_locked(r: &mut Rt, of: u64, proj: Vec<Proj>, exclusive: bool, is_root: bool, ancestors: Vec<u64>, lock_of: Option<Vec<Proj>>) -> u64 {
    mint_in(r, of, proj, exclusive, is_root, ancestors, lock_of, !is_root)
}

// `stamp`: record a derived path in the current statement scope. A path
// that leaves for the caller at once (`cb_elem_borrow`) is not stamped:
// `cb_recv_ref` stamps it into the caller's scope.
fn mint_in(r: &mut Rt, of: u64, proj: Vec<Proj>, exclusive: bool, is_root: bool, ancestors: Vec<u64>, lock_of: Option<Vec<Proj>>, stamp: bool) -> u64 {
    let tok = r.paths.reserve();
    r.paths.insert(tok, PathRec { of, proj, exclusive, valid: true, is_root, ancestors, occ: 0, scope: None, lock_of });
    if let Some(o) = r.objs.get_mut(&of) {
        if o.toks.capacity() == 0 {
            o.toks = take_vec(&mut r.pool);
        }
        o.toks.push(tok);
    }
    if stamp {
        let scope = top_scope(r);
        stamp_tok(r, tok, scope);
    }
    tok
}

#[no_mangle]
pub extern "C" fn cb_bind(obj: u64) -> u64 {
    let _e = enter();
    let r = rt();
    detach(r, obj);
    let frame = top_frame(r).expect("a binding outside any frame");
    if let Entry::Frame(v) = &mut ts().stack[frame] {
        v.push(obj);
    }
    let tok = mint(r, obj, Vec::new(), true, true, Vec::new());
    let o = r.objs.get_mut(&obj).unwrap();
    o.frame = Some(frame);
    o.tid = me();
    o.root = Some(tok);
    tok
}

// D-0033 (1): the frame the calling block binds into (`cb_bind`), recorded
// for a binding the program may give a new value after its old one moved
// away or ended.
#[no_mangle]
pub extern "C" fn cb_frame_here() -> u64 {
    let _e = enter();
    top_frame(rt()).expect("a binding outside any frame") as u64
}

// `x = e` for a whole binding: while `root` still reaches a live object,
// `root` itself; otherwise (its value moved away or ended, before or
// while `e` was evaluated) a new, uninitialized object at `addr`, bound
// in `frame` (x's own, from `cb_frame_here`) as `cb_bind` binds one, and
// its root. The write that follows initializes it.
#[no_mangle]
pub extern "C" fn cb_rebind(root: u64, addr: *mut u8, ty: u32, frame: u64, _file: u32, _line: u32) -> u64 {
    let _e = enter();
    let r = rt();
    let live = r.paths.get(&root).map_or(false, |p| p.valid && r.objs.get(&p.of).map_or(false, |o| o.alive));
    if live {
        return root;
    }
    let obj = new_obj(addr as usize, ty, false);
    let r = rt();
    detach(r, obj);
    let frame = frame as usize;
    if let Entry::Frame(v) = &mut ts().stack[frame] {
        v.push(obj);
    }
    let tok = mint(r, obj, Vec::new(), true, true, Vec::new());
    let o = r.objs.get_mut(&obj).unwrap();
    o.frame = Some(frame);
    o.tid = me();
    o.root = Some(tok);
    tok
}

// Re-keys the slots inside an object's old extent to its new address.
fn rekey_slots(r: &mut Rt, old: usize, new: usize, size: u64) {
    if old == new || size == 0 {
        return;
    }
    let moved: Vec<(usize, u64)> = r.slots.range(old..old + size as usize).map(|(a, t)| (*a, *t)).collect();
    for (a, _) in &moved {
        r.slots.remove(a);
    }
    for (a, t) in moved {
        // A slot already at the destination (a copy made before the move
        // re-homed the object) stops holding what it held: overwriting it
        // silently left that path counted as held forever.
        if let Some(prev) = r.slots.insert(new + (a - old), t) {
            drop_occ(r, prev);
        }
    }
}

#[no_mangle]
pub extern "C" fn cb_move_to(obj: u64, addr: *mut u8) {
    let _e = enter();
    let r = rt();
    let (old, size) = match r.objs.get(&obj) {
        Some(o) => (o.addr, r.types[o.ty as usize].size),
        None => return,
    };
    rekey_slots(r, old, addr as usize, size);
    r.objs.get_mut(&obj).unwrap().addr = addr as usize;
}

fn path_target(r: &Rt, tok: u64, file: u32, line: u32) -> (u64, Vec<Proj>, Vec<u64>) {
    let (of, proj, excludes, _) = path_target_locked(r, tok, file, line);
    (of, proj, excludes)
}

fn path_target_locked(r: &Rt, tok: u64, file: u32, line: u32) -> (u64, Vec<Proj>, Vec<u64>, Option<Vec<Proj>>) {
    let rec = match r.paths.get(&tok) {
        Some(p) if p.valid => p,
        _ => fault("diag.stale-binding", file, line),
    };
    if !r.objs.get(&rec.of).map_or(false, |o| o.alive) {
        fault("diag.stale-binding", file, line);
    }
    let excludes = if rec.is_root {
        Vec::new()
    } else {
        let mut v = vec![tok];
        v.extend(rec.ancestors.iter().cloned());
        v
    };
    (rec.of, rec.proj.clone(), excludes, rec.lock_of.clone())
}

// A path to a temporary for an access that needs one (calling a closure
// temporary): owner-like, like a root, but the object stays a temporary
// of its statement and ends there.
#[no_mangle]
pub extern "C" fn cb_temp_path(obj: u64) -> u64 {
    let _e = enter();
    let r = rt();
    mint(r, obj, Vec::new(), true, true, Vec::new())
}

#[no_mangle]
pub extern "C" fn cb_take(root: u64, file: u32, line: u32) -> u64 {
    let _e = enter();
    let r = rt();
    let (obj, _, _) = path_target(r, root, file, line);
    // [Authority-Transfer-Aliased]: nothing but the owner reaches it.
    if !solitary(r, obj, &[root]) {
        fault("diag.move-while-aliased", file, line);
    }
    invalidate(r, root);
    detach(r, obj);
    r.objs.get_mut(&obj).unwrap().root = None;
    let scope = top_scope(r);
    stamp_temp(r, obj, scope);
    obj
}

#[no_mangle]
pub extern "C" fn cb_absorb(obj: u64) {
    let _e = enter();
    let r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

#[no_mangle]
pub extern "C" fn cb_result(obj: u64) {
    let _e = enter();
    let r = rt();
    let cur = match r.objs.get(&obj) {
        Some(o) if o.alive => o.scope,
        _ => return,
    };
    detach(r, obj);
    let parent = match cur {
        Some(i) => ts().stack[..i].iter().rposition(|e| matches!(e, Entry::Scope(_))),
        None => None,
    };
    stamp_temp(r, obj, parent);
}

#[no_mangle]
pub extern "C" fn cb_destroy(root: u64, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let (obj, _, _) = path_target(r, root, file, line);
    set_at(file, line);
    destroy_obj(obj, Some(root));
}

// `drop(*r)`: `[Destroy]` reached through a reference. The owner's
// own root is another valid path on the object, so it is never
// solitary while owned; an unowned temporary is destroyed.
#[no_mangle]
pub extern "C" fn cb_destroy_via(tok: u64, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let (obj, proj, mut excludes) = path_target(r, tok, file, line);
    if !proj.is_empty() {
        fault("diag.move-out-of-field", file, line);
    }
    excludes.push(tok);
    if r.objs[&obj].root.is_some() || !solitary(r, obj, &excludes) {
        fault("diag.destroy-while-aliased", file, line);
    }
    set_at(file, line);
    destroy_obj(obj, Some(tok));
}

#[no_mangle]
pub extern "C" fn cb_consume(obj: u64, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    if r.objs.get(&obj).map_or(false, |o| o.alive) {
        set_at(file, line);
        destroy_obj(obj, None);
    }
}

#[no_mangle]
pub extern "C" fn cb_end_moved_out(obj: u64) {
    let _e = enter();
    let r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

// `[Destroy]` for a resource, `[Object-End]` for a plain object.
fn destroy_obj(obj: u64, root: Option<u64>) {
    let r = rt();
    let (addr, ty, is_resource, alive) = match r.objs.get(&obj) {
        Some(o) => (o.addr, o.ty, o.is_resource, o.alive),
        None => return,
    };
    if !alive {
        return;
    }
    if is_resource {
        let ex: Vec<u64> = root.into_iter().collect();
        if !solitary(r, obj, &ex) {
            fault("diag.destroy-while-aliased", NOLOC, 0);
        }
        drop_in_place(addr, ty, obj);
    } else if r.types[ty as usize].kind == K_FN {
        drop_in_place(addr, ty, obj);
    }
    let r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

// Runs the destructor and destroys sub-resources of the value at
// `addr` of type `ty` (`[Run-Destructor]`, `[Destroy-Composite]`).
// `owner` is the object being destroyed, or 0 for a field.
fn drop_in_place(addr: usize, ty: u32, owner: u64) {
    let r = rt();
    let info = &r.types[ty as usize];
    if info.kind == K_FN {
        // A fn-typed slot owns the closure box its handle names, if any.
        drop_fn_handle(unsafe { *(addr as *const usize) });
        return;
    }
    if !info.is_resource {
        return;
    }
    if info.kind == K_HANDLE {
        handle_destructor(unsafe { *(addr as *const u64) });
        return;
    }
    if info.kind == K_GUARD {
        guard_drop(addr);
        return;
    }
    if let Some(drop) = info.drop {
        // `self`: a fresh exclusive root into the value.
        let of = if owner != 0 { owner } else { new_anonymous(r, addr, ty) };
        let tok = mint(r, of, Vec::new(), true, true, Vec::new());
        let counted = !multi();
        if counted {
            unsafe { CALLBACK_DEPTH += 1 };
        }
        unsafe { drop(CbRef { p: addr as *mut u8, tok }) };
        if counted {
            unsafe { CALLBACK_DEPTH -= 1 };
        }
        let r = rt();
        invalidate(r, tok);
        if owner == 0 {
            end_identity(r, of);
        }
    }
    let r = rt();
    let info = &r.types[ty as usize];
    match info.kind {
        K_STRUCT => {
            let fields = info.fields.clone();
            for (off, fty) in fields.into_iter().rev() {
                drop_in_place(addr + off as usize, fty, 0);
            }
        }
        K_ENUM => {
            let tag = unsafe { *(addr as *const u32) } as usize;
            let variants = info.variants.clone();
            let off = info.payload_off as usize;
            if let Some(&pty) = variants.get(tag) {
                if pty != NOTYPE {
                    drop_in_place(addr + off, pty, 0);
                }
            }
        }
        K_ARRAY => {
            let (elem, count) = (info.elem, info.count);
            let esize = r.types[elem as usize].size as usize;
            for i in (0..count as usize).rev() {
                drop_in_place(addr + i * esize, elem, 0);
            }
        }
        _ => {}
    }
}

fn drop_fn_handle(handle: usize) {
    let r = rt();
    if let Some(obj) = r.fn_boxes.remove(&handle) {
        let root = r.objs.get(&obj).and_then(|o| o.root);
        destroy_obj(obj, root);
        unsafe {
            let b = handle as *mut CbFnBox;
            std::alloc::dealloc((*b).data, std::alloc::Layout::from_size_align_unchecked(rt().types[(*b).ty as usize].size.max(1) as usize, 16));
            drop(Box::from_raw(b));
        }
    }
}

// A field being destroyed needs an identity for its destructor's
// `self` path; it exists only for the duration of the call.
fn new_anonymous(r: &mut Rt, addr: usize, ty: u32) -> u64 {
    let id = r.objs.reserve();
    r.objs.insert(id, Obj { addr, ty, alive: true, init: true, is_resource: true, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: Vec::new() });
    id
}

// `[Object-End]`: every path into the object dies, every reference
// its storage held stops being an occurrence, the identity is gone.
fn end_identity(r: &mut Rt, obj: u64) {
    let (addr, size, ephemeral, toks) = match r.objs.get_mut(&obj) {
        Some(o) => (o.addr, r.types[o.ty as usize].size, o.ephemeral, std::mem::take(&mut o.toks)),
        None => return,
    };
    // Every path into the object is dead; a token that turns up later
    // (in a register somewhere) is reported stale by its absence.
    for &t in &toks {
        r.paths.remove(&t);
    }
    give_vec(&mut r.pool, toks);
    // An ephemeral object's type holds no reference, so no slot lies in
    // its storage.
    if !ephemeral {
        forget_slots(r, addr, size);
    }
    let o = r.objs.remove(&obj).unwrap();
    if o.reclaimed {
        r.reclaimed.remove_if(addr, obj, ephemeral);
    }
}

// The references stored in `[addr, addr + size)` stop being occurrences.
fn forget_slots(r: &mut Rt, addr: usize, size: u64) {
    if size == 0 {
        return;
    }
    let held: Vec<(usize, u64)> = r.slots.range(addr..addr + size as usize).map(|(a, t)| (*a, *t)).collect();
    for (a, t) in held {
        r.slots.remove(&a);
        drop_occ(r, t);
    }
}

// ---- raw storage ----

#[no_mangle]
pub extern "C" fn cb_reclaim(addr: *mut u8, ty: u32) -> u64 {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    if let Some(&obj) = r.reclaimed.get(&a) {
        if let Some(o) = r.objs.get_mut(&obj) {
            if o.alive && (o.root.is_some() || o.ephemeral) {
                // Its root token is about to be in the program's hands.
                if o.ephemeral {
                    o.ephemeral = false;
                    r.reclaimed.insert(a, obj);
                }
                return match r.objs[&obj].root {
                    Some(root) => root,
                    None => {
                        let root = mint(r, obj, Vec::new(), true, true, Vec::new());
                        r.objs.get_mut(&obj).unwrap().root = Some(root);
                        root
                    }
                };
            }
        }
    }
    establish_reclaimed(r, a, ty, false).0
}

// `[Reclaim]`'s fresh object over the cells at `a`: its root token (0 for
// an ephemeral object, which has none) and its id.
fn establish_reclaimed(r: &mut Rt, a: usize, ty: u32, ephemeral: bool) -> (u64, u64) {
    let id = r.objs.reserve();
    let is_resource = r.types.get(ty as usize).map_or(false, |t| t.is_resource);
    r.objs.insert(id, Obj { addr: a, ty, alive: true, init: true, is_resource, reclaimed: true, root: None, frame: None, scope: None, tid: 0, ephemeral, toks: Vec::new() });
    if ephemeral {
        // No root path: nothing outside the runtime ever holds one, and
        // `cb_reclaim` mints it if the program re-attaches the object.
        r.reclaimed.insert_ephemeral(a, id);
        return (0, id);
    }
    r.reclaimed.insert(a, id);
    let tok = mint(r, id, Vec::new(), true, true, Vec::new());
    r.objs.get_mut(&id).unwrap().root = Some(tok);
    (tok, id)
}

// `Vec::index_shared`/`Vec::index_exclusive` (`spec/21` §1) realized
// natively, as `spec/21` §0 permits: the caller has already performed the
// body's checked read of `v.len` and its bounds check. The body's second
// read, of `v.ptr`, is through the same path with nothing in between, so
// it cannot fail where the first passed. What remains is `[Reclaim]` of
// the element's cells, `[Borrow]` of it in `mode`, and the result's
// return to the caller (`cb_send_ref`): the same state the prelude body
// leaves, without its frame, parameter binding and scopes.
// `&place` / `&mut place` passed straight to the prelude's `Vec::index_*`
// (`cobc`'s `…_pre` call). The borrow's own check, then the checked read
// of `v.len` the body performs first -- which, through a path formed just
// now with nothing in between, can fail only for initialization. No token
// is minted: the borrow would be unheld and die with its statement, and
// no check counts an unheld path (D-0018), so it is unobservable.
#[no_mangle]
pub unsafe extern "C" fn cb_borrow_check(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    let of = check_access(r, base, extra, mode == 1, file, line);
    if !r.objs[&of].init {
        // The body's read reports no location (`spec/21`'s own text).
        set_at(NOLOC, 0);
        fault("diag.use-of-uninitialized", NOLOC, 0);
    }
}

#[no_mangle]
pub extern "C" fn cb_elem_borrow(addr: *mut u8, ty: u32, mode: u32) -> u64 {
    let _e = enter();
    let tok = elem_borrow(addr, ty, mode, false);
    ts().inflight.push_back(Inflight::Ref(tok));
    tok
}

// The same for a `…_pre` body, whose caller receives the reference in
// the statement scope that is current: no frame or scope lies between
// the native body and its call, so the path is recorded there directly
// instead of travelling through `cb_send_ref`/`cb_recv_ref`.
#[no_mangle]
pub extern "C" fn cb_elem_borrow_here(addr: *mut u8, ty: u32, mode: u32) -> u64 {
    let _e = enter();
    elem_borrow(addr, ty, mode, true)
}

// `*Vec::index_*(&v, i)` read, or assigned a stateless value, at once,
// for a plain element type (`cobc`'s `elem_access_place`): the element
// borrow `cb_elem_borrow_here` makes, then the read's or write's check
// through it. Where no live object lies over the element's cells, the
// borrow would establish a fresh ephemeral object whose only path is
// this one: neither check can fail, and the object ends unseen with the
// path at `[Stmt-Exit]`. So nothing is done. Otherwise the same checks
// are made, and the path, unheld, ends now rather than at `[Stmt-Exit]`:
// no check counts an unheld path (D-0018), so only the ephemeral
// object's earlier end differs, and that is unobservable (D-0023).
#[no_mangle]
pub unsafe extern "C" fn cb_elem_access(addr: *mut u8, ty: u32, mode: u32, write: u32, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    let live = r.reclaimed.get(&a).copied().map_or(false, |o| r.objs.get(&o).map_or(false, |o| o.alive && (o.root.is_some() || o.ephemeral)));
    if !live {
        return;
    }
    let tok = elem_borrow(addr, ty, mode, false);
    if write != 0 {
        cb_write(tok, std::ptr::null(), 0, 0, file, line);
    } else {
        cb_read(tok, std::ptr::null(), 0, file, line);
    }
    retire_token(rt(), tok);
}

fn elem_borrow(addr: *mut u8, ty: u32, mode: u32, stamp: bool) -> u64 {
    let r = rt();
    let a = addr as usize;
    let exclusive = mode == 1;
    let existing = r.reclaimed.get(&a).copied().filter(|o| r.objs.get(o).map_or(false, |o| o.alive && (o.root.is_some() || o.ephemeral)));
    let obj = match existing {
        Some(obj) => obj,
        None => {
            let t = &r.types[ty as usize];
            let plain = !t.is_resource && !t.has_refs && t.kind != K_FN;
            establish_reclaimed(r, a, ty, plain).1
        }
    };
    // `[Borrow]` from the object's root path: through a root with no
    // projection, `check_access` is `check_root_access`, and the new
    // path's projection, ancestors and lock origin are all empty.
    check_root_access(r, obj, exclusive, NOLOC, 0);
    mint_in(r, obj, Vec::new(), exclusive, false, Vec::new(), None, stamp)
}

// `Vec::drop`'s element loop (`spec/21` §1) for a plain, reference-free
// element type, realized natively (`spec/21` §0): each `drop(reclaim(…))`
// of such a value is `[Reclaim]` and a checked read. Where no reclaimed
// object lies over an element's cells, `[Reclaim]` would establish a
// fresh one with no path but its root, whose read cannot fail, and the
// `deallocate` that follows ends it unseen; so only the elements that
// already have an object are checked, in index order as the loop would.
#[no_mangle]
pub extern "C" fn cb_vec_drop_plain(addr: *mut u8, len: u64, size: u64) {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    let n = (len * size) as usize;
    let size = size as usize;
    let existing: Vec<(usize, u32)> = r
        .reclaimed
        .range(a, a + n)
        .into_iter()
        .filter(|(at, _)| (*at - a) % size == 0)
        .filter_map(|(at, o)| r.objs.get(&o).map(|o| (at, o.ty)))
        .collect();
    for (at, ty) in existing {
        let root = cb_reclaim(at as *mut u8, ty);
        unsafe { cb_read(root, std::ptr::null(), 0, NOLOC, 0) };
    }
}

#[no_mangle]
pub extern "C" fn cb_raw_move_in(obj: u64, addr: *mut u8) {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    if let Some(&old) = r.reclaimed.get(&a) {
        if old != obj {
            end_identity(r, old);
        }
    }
    cb_move_to(obj, addr);
    let r = rt();
    detach(r, obj);
    let o = r.objs.get_mut(&obj).unwrap();
    o.reclaimed = true;
    if let Some(root) = o.root.take() {
        invalidate(r, root);
    }
    r.reclaimed.insert(a, obj);
}

#[no_mangle]
pub extern "C" fn cb_raw_move_out(addr: *mut u8, dst: *mut u8, ty: u32) {
    let _e = enter();
    let r = rt();
    // `[Rawptr-Move-Out]` re-keys held-by to the new object: the
    // references the value holds move with its bytes (already copied to
    // `dst`), and only then does the reclaimed identity at `addr` end.
    let size = r.types[ty as usize].size;
    rekey_slots(r, addr as usize, dst as usize, size);
    if let Some(&obj) = r.reclaimed.get(&(addr as usize)) {
        end_identity(r, obj);
    }
}

#[no_mangle]
pub extern "C" fn cb_release(addr: *mut u8, n: u64) {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    let stale: Vec<u64> = r.reclaimed.range(a, a + n as usize).into_iter().map(|(_, o)| o).collect();
    for obj in stale {
        end_identity(r, obj);
    }
    // References written into these cells were held by the object over
    // them (`[Rawptr-Write]`, `CHG-0031`), reclaimed or not: none is held
    // once the cells are released.
    forget_slots(r, a, n);
}

static ALLOCS: std::sync::Mutex<Vec<(usize, usize, usize)>> = std::sync::Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn cb_allocate(size: u64, align: u64) -> *mut u8 {
    let _e = enter();
    if size == 0 {
        return 1 as *mut u8;
    }
    let align = (align.max(1) as usize).next_power_of_two();
    let Ok(layout) = std::alloc::Layout::from_size_align(size as usize, align) else { return std::ptr::null_mut() };
    let p = unsafe { std::alloc::alloc(layout) };
    if !p.is_null() {
        ALLOCS.lock().unwrap().push((p as usize, size as usize, align));
    }
    p
}

#[no_mangle]
pub extern "C" fn cb_deallocate(addr: *mut u8, size: u64, _align: u64) {
    let _e = enter();
    cb_release(addr, size);
    let mut allocs = ALLOCS.lock().unwrap();
    if let Some(i) = allocs.iter().position(|(a, _, _)| *a == addr as usize) {
        let (a, s, al) = allocs.remove(i);
        if let Ok(layout) = std::alloc::Layout::from_size_align(s, al) {
            unsafe { std::alloc::dealloc(a as *mut u8, layout) };
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_copy_raw(dst: *mut u8, src: *const u8, n: u64) {
    let _e = enter();
    if n == 0 {
        return;
    }
    std::ptr::copy(src, dst, n as usize);
    let r = rt();
    let (s, d) = (src as usize, dst as usize);
    let moved: Vec<(usize, u64)> = r.slots.range(s..s + n as usize).map(|(a, t)| (*a, *t)).collect();
    for (a, t) in moved {
        cb_store_ref((d + (a - s)) as *mut u8, t);
    }
}

// ---- fn values ----

#[no_mangle]
pub extern "C" fn cb_fn_item(code: *mut u8) -> *mut u8 {
    let _e = enter();
    let r = rt();
    if let Some(&h) = r.fn_items.get(&(code as usize)) {
        return h as *mut u8;
    }
    let b = Box::new(CbFnBox { code, data: std::ptr::null_mut(), root: 0, ty: NOTYPE, closure: 0 });
    let h = Box::into_raw(b) as usize;
    r.fn_items.insert(code as usize, h);
    h as *mut u8
}

fn box_storage(size: u64) -> *mut u8 {
    let layout = std::alloc::Layout::from_size_align(size.max(1) as usize, 16).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

// The closure object moves into a heap box the handle owns.
#[no_mangle]
pub unsafe extern "C" fn cb_fn_box(obj: u64, code: *mut u8, ty: u32) -> *mut u8 {
    let _e = enter();
    let r = rt();
    let size = r.types[ty as usize].size;
    let data = box_storage(size);
    let old = r.objs[&obj].addr;
    if size > 0 {
        std::ptr::copy_nonoverlapping(old as *const u8, data, size as usize);
    }
    cb_move_to(obj, data);
    let r = rt();
    detach(r, obj);
    let root = match r.objs[&obj].root {
        Some(t) => t,
        None => {
            let t = mint(r, obj, Vec::new(), true, true, Vec::new());
            r.objs.get_mut(&obj).unwrap().root = Some(t);
            t
        }
    };
    let b = Box::new(CbFnBox { code, data, root, ty, closure: 1 });
    let h = Box::into_raw(b) as usize;
    r.fn_boxes.insert(h, obj);
    h as *mut u8
}

// `[Read]` of a fn value: an item handle copies; a closure box is
// cloned (its own object, its own reference data) — unless the closure
// owns a resource, which cannot be read in value position.
#[no_mangle]
pub unsafe extern "C" fn cb_fn_copy(handle: *mut u8, file: u32, line: u32) -> *mut u8 {
    let _e = enter();
    let r = rt();
    let Some(&obj) = r.fn_boxes.get(&(handle as usize)) else { return handle };
    let b = &*(handle as *const CbFnBox);
    let ty = b.ty;
    if r.types[ty as usize].is_resource {
        fault("diag.read-of-resource", file, line);
    }
    let size = r.types[ty as usize].size;
    let data = box_storage(size);
    if size > 0 {
        std::ptr::copy_nonoverlapping(b.data as *const u8, data, size as usize);
    }
    let _ = obj;
    let id = r.objs.reserve();
    r.objs.insert(id, Obj { addr: data as usize, ty, alive: true, init: true, is_resource: false, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: Vec::new() });
    let root = mint(r, id, Vec::new(), true, true, Vec::new());
    r.objs.get_mut(&id).unwrap().root = Some(root);
    cb_copy_datum(data, b.data, ty);
    let nb = Box::new(CbFnBox { code: b.code, data, root, ty, closure: 1 });
    let h = Box::into_raw(nb) as usize;
    rt().fn_boxes.insert(h, id);
    h as *mut u8
}

// ---- aliasing ----

fn excluded(r: &Rt, excludes: &[u64], tok: u64) -> bool {
    if excludes.contains(&tok) {
        return true;
    }
    excludes.iter().any(|e| r.paths.get(e).map_or(false, |p| p.ancestors.contains(&tok)))
}

fn solitary(r: &Rt, of: u64, excludes: &[u64]) -> bool {
    let Some(toks) = r.objs.get(&of).map(|o| &o.toks) else { return true };
    !toks.iter().any(|&t| {
        let p = &r.paths[&t];
        p.valid && !p.is_root && p.occ > 0 && !excluded(r, excludes, t)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cb_borrow(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) -> u64 {
    let _e = enter();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    borrow(rt(), base, extra, mode == 1, file, line)
}

fn borrow(r: &mut Rt, base: u64, extra: &[CbProj], exclusive: bool, file: u32, line: u32) -> u64 {
    let of = check_access(r, base, extra, exclusive, file, line);
    let rec = &r.paths[&base];
    let mut proj = rec.proj.clone();
    proj.extend(extra.iter().map(|q| conv(*q)));
    // `ancestors`: the chain this borrow was formed through, closed
    // over those tokens' own ancestors (record_token_ancestors).
    let mut anc = Vec::new();
    if !rec.is_root {
        anc.push(base);
        anc.extend(rec.ancestors.iter().copied());
        for e in std::iter::once(&base).chain(rec.ancestors.iter()) {
            if let Some(pe) = r.paths.get(e) {
                for a in &pe.ancestors {
                    if !anc.contains(a) {
                        anc.push(*a);
                    }
                }
            }
        }
    }
    let lock_of = rec.lock_of.clone();
    mint_locked(r, of, proj, exclusive, false, anc, lock_of)
}

fn conv(q: CbProj) -> Proj {
    match q.kind {
        0 => Proj::Field(q.idx),
        1 => Proj::Index(q.idx),
        _ => Proj::Payload,
    }
}

// Element `i` of the projection `base ++ extra`, without building it.
fn proj_at(base: &[Proj], extra: &[CbProj], i: usize) -> Proj {
    if i < base.len() {
        base[i]
    } else {
        conv(extra[i - base.len()])
    }
}

// `[Read]`/`[Write]`'s path checks (stale binding, `clash`) for an access
// through `tok` narrowed by `extra`, as `path_target` + `clash` compute
// them, without allocating: this runs at every access. Returns the
// accessed object.
// `check_access` through an object's root path with no projection: the
// root excludes no path, every path's projection extends the empty one,
// and a root carries no lock origin.
fn check_root_access(r: &Rt, of: u64, exclusive: bool, file: u32, line: u32) {
    let toks = match r.objs.get(&of) {
        Some(o) if o.alive => &o.toks,
        _ => fault("diag.stale-binding", file, line),
    };
    for &t in toks {
        let p = &r.paths[&t];
        if !p.valid || p.is_root || p.occ == 0 {
            continue;
        }
        if !exclusive && p.lock_of.as_ref().map_or(false, |lo| lo.is_empty()) {
            continue;
        }
        if exclusive || p.exclusive {
            fault("diag.aliasing-conflict", file, line);
        }
    }
}

fn check_access(r: &Rt, tok: u64, extra: &[CbProj], exclusive: bool, file: u32, line: u32) -> u64 {
    let rec = match r.paths.get(&tok) {
        Some(p) if p.valid => p,
        _ => fault("diag.stale-binding", file, line),
    };
    let toks = match r.objs.get(&rec.of) {
        Some(o) if o.alive => &o.toks,
        _ => fault("diag.stale-binding", file, line),
    };
    let n = rec.proj.len() + extra.len();
    for &t in toks {
        let p = &r.paths[&t];
        if !p.valid || p.is_root || p.occ == 0 {
            continue;
        }
        // `excluded`: the accessing path itself and its ancestors.
        if !rec.is_root
            && (t == tok
                || rec.ancestors.contains(&t)
                || rec.ancestors.iter().any(|e| r.paths.get(e).map_or(false, |pe| pe.ancestors.contains(&t))))
        {
            continue;
        }
        let m = n.min(p.proj.len());
        if (0..m).any(|i| proj_at(&rec.proj, extra, i) != p.proj[i]) {
            continue;
        }
        // `sync-exempt` (spec/19 §2), both directions.
        if !p.exclusive && rec.lock_of.as_ref() == Some(&p.proj) {
            continue;
        }
        if !exclusive {
            if let Some(lo) = &p.lock_of {
                if lo.len() == n && (0..n).all(|i| proj_at(&rec.proj, extra, i) == lo[i]) {
                    continue;
                }
            }
        }
        if exclusive || p.exclusive {
            fault("diag.aliasing-conflict", file, line);
        }
    }
    rec.of
}

#[no_mangle]
pub unsafe extern "C" fn cb_read(base: u64, p: *const CbProj, n: usize, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    // Staleness first, then initialization, then `clash` — the order
    // `path_target` and the checks after it always had.
    let of = match r.paths.get(&base) {
        Some(rec) if rec.valid && r.objs.get(&rec.of).map_or(false, |o| o.alive) => rec.of,
        _ => fault("diag.stale-binding", file, line),
    };
    if !r.objs[&of].init {
        fault("diag.use-of-uninitialized", file, line);
    }
    check_access(r, base, extra, false, file, line);
}

// D-0049: whether the value of type `ty` at `addr` owns a resource now
// (`live-resource-at`, spec/05): a `None` of an `Option<Box<T>>` does
// not. A type declared `resource` or with a destructor (`owner`), a
// handle and a guard always do.
fn owns(r: &Rt, addr: usize, ty: u32) -> bool {
    let info = &r.types[ty as usize];
    if !info.is_resource {
        return false;
    }
    if info.owner || info.drop.is_some() {
        return true;
    }
    match info.kind {
        K_STRUCT => info.fields.iter().any(|&(off, fty)| owns(r, addr + off as usize, fty)),
        K_ENUM => {
            let tag = unsafe { *(addr as *const u32) } as usize;
            match info.variants.get(tag) {
                Some(&pty) if pty != NOTYPE => owns(r, addr + info.payload_off as usize, pty),
                Some(_) => false,
                None => true,
            }
        }
        K_ARRAY => {
            let esize = r.types[info.elem as usize].size;
            (0..info.count).any(|i| owns(r, addr + (i * esize) as usize, info.elem))
        }
        _ => true,
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_owns(addr: *const u8, ty: u32) -> u32 {
    let _e = enter();
    owns(rt(), addr as usize, ty) as u32
}

#[no_mangle]
pub unsafe extern "C" fn cb_write(base: u64, p: *const CbProj, n: usize, target_resource: u32, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    let of = check_access(r, base, extra, true, file, line);
    let o = r.objs.get_mut(&of).unwrap();
    // [Write-Resource-Overwrite-Rejected]: the target still holds a
    // live resource (a moved-out binding is stale, not overwritable).
    if target_resource != 0 && o.init {
        fault("diag.overwrite-of-live-resource", file, line);
    }
    o.init = true;
}

// ---- reference data ----

#[no_mangle]
pub extern "C" fn cb_store_ref(slot: *mut u8, tok: u64) {
    let _e = enter();
    let r = rt();
    if let Some(old) = r.slots.insert(slot as usize, tok) {
        drop_occ(r, old);
    }
    if let Some(p) = r.paths.get_mut(&tok) {
        p.occ += 1;
    }
}

// `[Swap-Places]`: each reference names a place that is still there
// (temporally valid, its object alive); no conflict check.
#[no_mangle]
pub extern "C" fn cb_valid(tok: u64, file: u32, line: u32) {
    let _e = enter();
    path_target(rt(), tok, file, line);
}

#[no_mangle]
pub extern "C" fn cb_load_ref(slot: *const u8) -> u64 {
    let _e = enter();
    rt().slots.get(&(slot as usize)).copied().unwrap_or(0)
}

// The offsets of every reference slot inside a value of type `ty`
// (enums by the active variant, read from the value itself).
fn ref_offsets(r: &Rt, addr: usize, ty: u32, base: u64, out: &mut Vec<u64>) {
    let info = &r.types[ty as usize];
    if !info.has_refs {
        return;
    }
    match info.kind {
        K_REF | K_GUARD => out.push(base),
        K_STRUCT => {
            for &(off, fty) in &info.fields {
                ref_offsets(r, addr + off as usize, fty, base + off, out);
            }
        }
        K_ENUM => {
            let tag = unsafe { *(addr as *const u32) } as usize;
            if let Some(&pty) = info.variants.get(tag) {
                if pty != NOTYPE {
                    ref_offsets(r, addr + info.payload_off as usize, pty, base + info.payload_off, out);
                }
            }
        }
        K_ARRAY => {
            let esize = r.types[info.elem as usize].size;
            for i in 0..info.count {
                ref_offsets(r, addr + (i * esize) as usize, info.elem, base + i * esize, out);
            }
        }
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn cb_copy_datum(dst: *mut u8, src: *const u8, ty: u32) {
    let _e = enter();
    let r = rt();
    let mut offs = Vec::new();
    ref_offsets(r, src as usize, ty, 0, &mut offs);
    for off in offs {
        if let Some(&t) = r.slots.get(&(src as usize + off as usize)) {
            cb_store_ref(unsafe { dst.add(off as usize) }, t);
        }
    }
}

// ---- values in flight ----

#[no_mangle]
pub extern "C" fn cb_send(obj: u64) {
    let _e = enter();
    let r = rt();
    detach(r, obj);
    ts().inflight.push_back(Inflight::Obj(obj));
}

#[no_mangle]
pub extern "C" fn cb_recv(addr: *mut u8) -> u64 {
    let _e = enter();
    let obj = match ts().inflight.pop_front() {
        Some(Inflight::Obj(o)) => o,
        _ => panic!("cbrt: receive of an object that was not sent"),
    };
    cb_move_to(obj, addr);
    let r = rt();
    let scope = top_scope(r);
    stamp_temp(r, obj, scope);
    obj
}

// A reference leaving a function: out of the callee's scopes, into the
// caller's current statement — the same journey an object makes.
#[no_mangle]
pub extern "C" fn cb_send_ref(tok: u64) {
    let _e = enter();
    let r = rt();
    if r.paths.contains_key(&tok) {
        detach_tok(r, tok);
        ts().inflight.push_back(Inflight::Ref(tok));
    } else {
        ts().inflight.push_back(Inflight::Ref(0));
    }
}

#[no_mangle]
pub extern "C" fn cb_recv_ref() {
    let _e = enter();
    let r = rt();
    let tok = match ts().inflight.pop_front() {
        Some(Inflight::Ref(t)) => t,
        _ => panic!("cbrt: receive of a reference that was not sent"),
    };
    if tok != 0 && r.paths.contains_key(&tok) {
        let scope = top_scope(r);
        stamp_tok(r, tok, scope);
    }
}

// A reference that is a statement's result: re-stamped outward.
#[no_mangle]
pub extern "C" fn cb_result_ref(tok: u64) {
    let _e = enter();
    let r = rt();
    let cur = match r.paths.get(&tok) {
        Some(p) => p.scope,
        None => return,
    };
    let Some(i) = cur else { return };
    detach_tok(r, tok);
    let parent = ts().stack[..i].iter().rposition(|e| matches!(e, Entry::Scope(_)));
    stamp_tok(r, tok, parent);
}

#[no_mangle]
pub extern "C" fn cb_send_datum(src: *const u8, ty: u32) {
    let _e = enter();
    let r = rt();
    let mut offs = Vec::new();
    ref_offsets(r, src as usize, ty, 0, &mut offs);
    let toks: Vec<Option<u64>> = offs.iter().map(|off| r.slots.get(&(src as usize + *off as usize)).copied()).collect();
    // In flight, as `cb_send_ref`'s: a path formed in the sending
    // statement (`return Some(&m.v);`) must not end with that statement;
    // `cb_recv_datum` stores it, held again, at the receiver.
    for t in toks.iter().flatten() {
        detach_tok(r, *t);
    }
    ts().inflight.push_back(Inflight::Datum(toks));
}

#[no_mangle]
pub extern "C" fn cb_recv_datum(dst: *mut u8, ty: u32) {
    let _e = enter();
    let r = rt();
    let toks = match ts().inflight.pop_front() {
        Some(Inflight::Datum(t)) => t,
        _ => panic!("cbrt: receive of reference data that was not sent"),
    };
    let mut offs = Vec::new();
    ref_offsets(r, dst as usize, ty, 0, &mut offs);
    for (off, tok) in offs.into_iter().zip(toks) {
        if let Some(t) = tok {
            cb_store_ref(unsafe { dst.add(off as usize) }, t);
        }
    }
}

// ---- threads (spec/19 §1) ----

extern "C" {
    fn free(p: *mut u8);
    fn calloc(n: usize, size: usize) -> *mut u8;
}

// A spawned thread's argument block, zeroed; its result goes at offset 0.
#[no_mangle]
pub extern "C" fn cb_env_alloc(size: u64) -> *mut u8 {
    unsafe { calloc(1, size.max(1) as usize) }
}

struct SendPtr(usize, usize);
unsafe impl Send for SendPtr {}

// `[Spawn]`: the arguments are already in the block; the new thread
// runs `body(env)`, which makes the call and reports its result.
#[no_mangle]
pub extern "C" fn cb_spawn(body: unsafe extern "C" fn(*mut u8), env: *mut u8, env_size: u64, res_ty: u32) -> u64 {
    let _e = enter();
    let r = rt();
    let tid = r.next_tid;
    r.next_tid += 1;
    r.threads.insert(tid, ThreadRec { status: Status::Running, env: env as usize, env_size, res_ty, res_obj: 0 });
    r.live += 1;
    go_multi();
    let at = get_at();
    let job = SendPtr(body as usize, env as usize);
    std::thread::spawn(move || {
        let job = job;
        TID.with(|t| t.set(tid));
        let own = Box::into_raw(Box::new(TState { stack: Vec::new(), inflight: VecDeque::new() }));
        set_at(at.0, at.1);
        TSP.with(|c| c.set(own));
        let body: unsafe extern "C" fn(*mut u8) = unsafe { std::mem::transmute(job.0) };
        unsafe { body(job.1 as *mut u8) };
        let _e = enter();
        rt().live -= 1;
        TSP.with(|c| c.set(std::ptr::null_mut()));
        drop(unsafe { Box::from_raw(own) });
    });
    tid
}

// `[Thread-Body-Done]`: the result (and its object, for a resource) is
// in the block, waiting to be claimed.
#[no_mangle]
pub extern "C" fn cb_thread_done(res_obj: u64) {
    let _e = enter();
    let r = rt();
    if let Some(t) = r.threads.get_mut(&me()) {
        t.status = Status::Done;
        t.res_obj = res_obj;
    }
    signal(me());
}

// A resource argument waiting in a spawned thread's block: detached
// from the spawning statement, owned by nothing until the new thread
// sends it to the callee.
#[no_mangle]
pub extern "C" fn cb_hold(obj: u64) {
    let _e = enter();
    detach(rt(), obj);
}

// The argument block's references stop being occurrences once the call
// that received them has returned.
#[no_mangle]
pub extern "C" fn cb_env_forget(addr: *mut u8, size: u64) {
    let _e = enter();
    forget_slots(rt(), addr as usize, size);
}

// The spawned callee, a `fn` value, is destroyed once its call is over.
#[no_mangle]
pub extern "C" fn cb_drop_fn(handle: *mut u8) {
    let _e = enter();
    drop_fn_handle(handle as usize);
}

fn wait_done(tid: u64) {
    wait_until(tid, &mut |r: &Rt| r.threads.get(&tid).map_or(true, |t| t.status != Status::Running));
}

// The block's remaining references stop being occurrences, the block
// is freed, and the result counts as taken.
fn free_env(tid: u64) {
    let r = rt();
    let Some(t) = r.threads.get(&tid) else { return };
    let (env, size) = (t.env, t.env_size);
    forget_slots(r, env, size);
    let t = r.threads.get_mut(&tid).unwrap();
    t.status = Status::Taken;
    t.env = 0;
    if env != 0 {
        unsafe { free(env as *mut u8) };
    }
}

// `[Join]`: waits, claims the result into `dst` (its object, if any,
// moving with it into this statement), and frees the block. Returns the
// result's object, or 0.
#[no_mangle]
pub extern "C" fn cb_join(tid: u64, dst: *mut u8) -> u64 {
    let _e = enter();
    wait_done(tid);
    let r = rt();
    let (env, res_ty, res_obj) = match r.threads.get(&tid) {
        Some(t) if t.status == Status::Done => (t.env, t.res_ty, t.res_obj),
        _ => panic!("cbrt: join of a thread whose result was already taken"),
    };
    let size = r.types[res_ty as usize].size;
    if size > 0 {
        unsafe { std::ptr::copy_nonoverlapping(env as *const u8, dst, size as usize) };
    }
    if res_obj != 0 {
        cb_move_to(res_obj, dst);
        let r = rt();
        let scope = top_scope(r);
        stamp_temp(r, res_obj, scope);
    } else {
        rekey_slots(r, env, dst as usize, size);
    }
    free_env(tid);
    res_obj
}

// `[Handle-Destructor]`: waits; an unclaimed result is discarded.
fn handle_destructor(tid: u64) {
    wait_done(tid);
    let r = rt();
    let (status, res_obj) = match r.threads.get(&tid) {
        Some(t) => (t.status, t.res_obj),
        None => return,
    };
    if status == Status::Done && res_obj != 0 && r.objs.contains_key(&res_obj) {
        destroy_obj(res_obj, None);
    }
    if status == Status::Done {
        free_env(tid);
    }
    rt().threads.remove(&tid);
}

// ---- mutexes (spec/19 §2) ----

// `[Lock]`: `tok` is the shared reference to the mutex at `m`. Returns
// the lock path's token, which the guard holds.
#[no_mangle]
pub extern "C" fn cb_lock(tok: u64, m: *mut u8, file: u32, line: u32) -> u64 {
    let _e = enter();
    let key = m as usize;
    let me = me();
    // [Lock-Reentrant]: a fault, not a deadlock.
    if rt().locks.get(&key) == Some(&me) {
        fault("diag.mutex-reentrant-lock", file, line);
    }
    let _ = path_target(rt(), tok, file, line);
    wait_until(mutex_key(key), &mut |r: &Rt| !r.locks.contains_key(&key));
    let r = rt();
    let (of, proj, excludes) = path_target(r, tok, file, line);
    r.locks.insert(key, me);
    let mut anc = excludes.clone();
    for e in &excludes {
        if let Some(pe) = r.paths.get(e) {
            for a in &pe.ancestors {
                if !anc.contains(a) {
                    anc.push(*a);
                }
            }
        }
    }
    if !anc.contains(&tok) {
        anc.push(tok);
    }
    let mut gproj = proj.clone();
    gproj.push(Proj::Field(0));
    mint_locked(r, of, gproj, true, false, anc, Some(proj))
}

// `[Guard-Drop]`: the guard at `addr` holds the address of the mutex's
// interior (its first field, so the mutex's own address) and, in the
// slot table, the lock path.
fn guard_drop(addr: usize) {
    let r = rt();
    let key = unsafe { *(addr as *const usize) };
    r.locks.remove(&key);
    if let Some(&tok) = r.slots.get(&addr) {
        invalidate(r, tok);
    }
    signal(mutex_key(key));
}
