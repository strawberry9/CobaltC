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

/// This runtime's sources, hashed (`build.rs`): `cobc` finds the marker
/// in `libcbrt.a` and compares it with the stamp it was built with.
pub const STAMP: &str = env!("CBRT_STAMP");

#[used]
#[no_mangle]
pub static CB_RUNTIME_STAMP: [u8; 27] = stamp_marker();

const fn stamp_marker() -> [u8; 27] {
    let mut m = *b"CBRT-STAMP:0000000000000000";
    let s = STAMP.as_bytes();
    let mut i = 0;
    while i < 16 {
        m[11 + i] = s[i];
        i += 1;
    }
    m
}

/// The staticlib's file name beside `cobc` (a Rust staticlib is
/// `cbrt.lib` under MSVC, `libcbrt.a` everywhere else). What links a
/// program against this runtime lives here, with the runtime, not in
/// `cobc`'s driver.
pub const LIB_FILE: &str = if cfg!(target_env = "msvc") { "cbrt.lib" } else { "libcbrt.a" };

/// The system libraries this staticlib needs, as `-l` names for the C
/// compiler after `-lcbrt`. The authoritative list for a target is
/// `cargo rustc -p cbrt --release -- --print native-static-libs`, less
/// what the C compiler links on its own (libc, libgcc; msvcrt).
#[cfg(unix)]
pub const LINK_LIBS: &[&str] = &["pthread", "dl", "m"];
#[cfg(windows)]
pub const LINK_LIBS: &[&str] = &["kernel32", "ntdll", "userenv", "ws2_32", "dbghelp"];

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

// The reference slots by address (`Rt::slots`): a hash of 4 KiB pages,
// each a direct array of its 512 pointer-sized positions. Point
// operations (`cb_store_ref`, `cb_load_ref`) are a hash probe and an
// index; a range is visited page by page, in address order. An ordered
// tree of every slot was a deep descent per operation once a program
// held many references (a `Vec<StringView>` of a long text); sorted
// lists per page shifted on every slot a recursion's frames took.
struct SlotMap {
    pages: HashMap<usize, Box<SlotPage>>,
    // Slots at addresses not 8-aligned (none in practice: a reference
    // field is pointer-aligned), kept apart so pages stay direct arrays.
    odd: BTreeMap<usize, u64>,
    // Emptied pages, all zeros again, for reuse without clearing.
    spare: Vec<Box<SlotPage>>,
    // The page used last and its (boxed, so stable) address: consecutive
    // operations mostly touch one page (a frame's slots, one object's).
    last: std::cell::Cell<(usize, *mut SlotPage)>,
}

impl Default for SlotMap {
    fn default() -> SlotMap {
        SlotMap { pages: HashMap::default(), odd: BTreeMap::new(), spare: Vec::new(), last: std::cell::Cell::new((usize::MAX, std::ptr::null_mut())) }
    }
}

// One page's slots by 8-byte position; 0 is "no slot" (no token is 0).
struct SlotPage {
    s: [u64; 512],
    n: u32,
}

const SLOT_PAGE: usize = 12;

impl SlotMap {
    #[inline]
    fn split(a: usize) -> (usize, usize) {
        (a >> SLOT_PAGE, (a >> 3) & 511)
    }
    #[inline]
    fn page(&self, p: usize) -> *mut SlotPage {
        let (lp, ptr) = self.last.get();
        if lp == p {
            return ptr;
        }
        match self.pages.get(&p) {
            Some(b) => {
                let ptr = &**b as *const SlotPage as *mut SlotPage;
                self.last.set((p, ptr));
                ptr
            }
            None => std::ptr::null_mut(),
        }
    }
    fn drop_page(&mut self, p: usize) {
        if self.last.get().0 == p {
            self.last.set((usize::MAX, std::ptr::null_mut()));
        }
        if let Some(pg) = self.pages.remove(&p) {
            if self.spare.len() < 256 {
                self.spare.push(pg);
            }
        }
    }
    fn get(&self, a: &usize) -> Option<&u64> {
        if a & 7 != 0 {
            return self.odd.get(a);
        }
        let (p, i) = Self::split(*a);
        let pg = self.page(p);
        if pg.is_null() {
            return None;
        }
        let t = unsafe { &(*pg).s[i] };
        if *t == 0 { None } else { Some(t) }
    }
    fn insert(&mut self, a: usize, t: u64) -> Option<u64> {
        if a & 7 != 0 {
            return self.odd.insert(a, t);
        }
        let (p, i) = Self::split(a);
        let mut pg = self.page(p);
        if pg.is_null() {
            let b = self.spare.pop().unwrap_or_else(|| Box::new(SlotPage { s: [0; 512], n: 0 }));
            self.pages.insert(p, b);
            pg = self.page(p);
        }
        let pg = unsafe { &mut *pg };
        let old = std::mem::replace(&mut pg.s[i], t);
        if old == 0 {
            pg.n += 1;
            None
        } else {
            Some(old)
        }
    }
    fn remove(&mut self, a: &usize) -> Option<u64> {
        if a & 7 != 0 {
            return self.odd.remove(a);
        }
        let (p, i) = Self::split(*a);
        let pg = self.page(p);
        if pg.is_null() {
            return None;
        }
        let pg = unsafe { &mut *pg };
        let old = std::mem::replace(&mut pg.s[i], 0);
        if old == 0 {
            return None;
        }
        pg.n -= 1;
        if pg.n == 0 {
            self.drop_page(p);
        }
        Some(old)
    }
    // The slots in `[lo, hi)`, in address order; `take`: removed as well.
    fn scan(&mut self, lo: usize, hi: usize, take: bool) -> Vec<(usize, u64)> {
        let mut out = Vec::new();
        self.scan_into(lo, hi, take, &mut out);
        out
    }
    fn scan_into(&mut self, lo: usize, hi: usize, take: bool, out: &mut Vec<(usize, u64)>) {
        if lo >= hi || (self.pages.is_empty() && self.odd.is_empty()) {
            return;
        }
        let start = out.len();
        let (p0, p1) = (lo >> SLOT_PAGE, (hi - 1) >> SLOT_PAGE);
        let mut emptied = Vec::new();
        let mut visit = |p: usize, pg: &mut SlotPage, out: &mut Vec<(usize, u64)>| {
            let base = p << SLOT_PAGE;
            let from = lo.max(base);
            let to = hi.min(base + (1 << SLOT_PAGE));
            let mut a = (from + 7) & !7;
            while a < to {
                let e = &mut pg.s[(a >> 3) & 511];
                if *e != 0 {
                    out.push((a, *e));
                    if take {
                        *e = 0;
                        pg.n -= 1;
                    }
                }
                a += 8;
            }
            if take && pg.n == 0 {
                emptied.push(p);
            }
        };
        if p1 == p0 {
            let pg = self.page(p0);
            if !pg.is_null() {
                visit(p0, unsafe { &mut *pg }, out);
            }
        } else if p1 - p0 >= self.pages.len() {
            // Fewer pages held than the range spans: visit those.
            for (&p, pg) in self.pages.iter_mut() {
                if p >= p0 && p <= p1 {
                    visit(p, pg, out);
                }
            }
            out[start..].sort_unstable_by_key(|e| e.0);
        } else {
            for p in p0..=p1 {
                if let Some(pg) = self.pages.get_mut(&p) {
                    visit(p, pg, out);
                }
            }
        }
        for p in emptied {
            self.drop_page(p);
        }
        if !self.odd.is_empty() {
            let odd: Vec<(usize, u64)> = self.odd.range(lo..hi).map(|(a, t)| (*a, *t)).collect();
            if take {
                for (a, _) in &odd {
                    self.odd.remove(a);
                }
            }
            out.extend(odd);
            out[start..].sort_unstable_by_key(|e| e.0);
        }
    }
    fn range(&mut self, lo: usize, hi: usize) -> Vec<(usize, u64)> {
        self.scan(lo, hi, false)
    }
    fn drain(&mut self, lo: usize, hi: usize) -> Vec<(usize, u64)> {
        self.scan(lo, hi, true)
    }
}

// The paths of one object (`Obj::toks`, `Obj::xtoks`): appended when a
// path is minted, removed when it is retired, scanned by every access
// check. Removal was a scan of the whole list, so an object with many
// live paths -- a `String` with a view per character, `String::chars`
// -- paid for every short-lived borrow of it in proportion to all the
// others, quadratically in all. Small lists stay a plain scan; past
// `TOK_INDEX_AT` entries a position index makes removal a constant-time
// `swap_remove`. The order of the entries is not meaningful: every check
// over them faults with the same diagnostic whichever clash it meets.
// A plain list, or, once long, the list with its position index, boxed:
// an `Obj` holds two lists and is moved whole on creation and removal, so
// a list must stay the size of a `Vec` (the enum's tag fits in the
// `Vec`'s capacity niche).
enum TokList {
    Small(Vec<u64>),
    Big(Box<(Vec<u64>, HashMap<u64, usize>)>),
}

impl Default for TokList {
    fn default() -> TokList {
        TokList::Small(Vec::new())
    }
}

const TOK_INDEX_AT: usize = 32;

const _: () = assert!(std::mem::size_of::<TokList>() == std::mem::size_of::<Vec<u64>>());

impl TokList {
    fn from_vec(v: Vec<u64>) -> TokList {
        TokList::Small(v)
    }
    fn capacity(&self) -> usize {
        match self {
            TokList::Small(v) => v.capacity(),
            TokList::Big(b) => b.0.capacity(),
        }
    }
    fn push(&mut self, t: u64) {
        match self {
            TokList::Small(v) => {
                v.push(t);
                if v.len() > TOK_INDEX_AT {
                    let v = std::mem::take(v);
                    let at = v.iter().enumerate().map(|(i, &x)| (x, i)).collect();
                    *self = TokList::Big(Box::new((v, at)));
                }
            }
            TokList::Big(b) => {
                b.0.push(t);
                let n = b.0.len();
                b.1.insert(t, n - 1);
            }
        }
    }
    // Each path is minted once, so it is in the list at most once.
    fn remove(&mut self, t: u64) {
        match self {
            TokList::Small(v) => {
                if let Some(i) = v.iter().position(|&x| x == t) {
                    v.swap_remove(i);
                }
            }
            TokList::Big(b) => {
                let (v, at) = &mut **b;
                let Some(i) = at.remove(&t) else { return };
                v.swap_remove(i);
                if let Some(&moved) = v.get(i) {
                    at.insert(moved, i);
                }
            }
        }
    }
    fn into_vec(self) -> Vec<u64> {
        match self {
            TokList::Small(v) => v,
            TokList::Big(b) => b.0,
        }
    }
}

impl std::ops::Deref for TokList {
    type Target = [u64];
    fn deref(&self) -> &[u64] {
        match self {
            TokList::Small(v) => v,
            TokList::Big(b) => &b.0,
        }
    }
}

impl<'a> IntoIterator for &'a TokList {
    type Item = &'a u64;
    type IntoIter = std::slice::Iter<'a, u64>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

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
    // `ordered` again, by hash: every element or `Box` access looks its
    // address up, and a tree of every reclaimed object (a tree of
    // `Box`es) was a deep descent each time.
    ordered_ix: HashMap<usize, u64>,
    ephemeral: BTreeMap<usize, u64>,
}

// How many entries `Reclaimed` holds, at least as many as the live
// objects over raw cells: while it is 0 none lies over any element, so
// the generated code's `cb_elem_access` (`cbrt.h`) need not ask
// (`cb_elem_access_slow` would return at once). Kept by every change to
// the maps; read without the runtime's lock, which is enough: a program
// reaches an element another thread placed an object over only through
// a synchronization that makes the count's change visible.
#[no_mangle]
#[allow(non_upper_case_globals)]
pub static cb_reclaimed_entries: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Reclaimed {
    fn count(&self) {
        cb_reclaimed_entries.store(self.ordered.len() + self.ephemeral.len(), std::sync::atomic::Ordering::Relaxed);
    }
    fn get(&self, a: &usize) -> Option<&u64> {
        self.ephemeral.get(a).or_else(|| self.ordered_ix.get(a))
    }
    fn insert(&mut self, a: usize, obj: u64) {
        self.ephemeral.remove(&a);
        self.ordered.insert(a, obj);
        self.ordered_ix.insert(a, obj);
        self.count();
    }
    // Only where no live object lies at `a` (`cb_elem_borrow`); an ordered
    // entry left there names an ended object, which every reader skips.
    fn insert_ephemeral(&mut self, a: usize, obj: u64) {
        self.ephemeral.insert(a, obj);
        self.count();
    }
    fn remove(&mut self, a: &usize) {
        if self.ephemeral.remove(a).is_none() {
            self.ordered.remove(a);
            self.ordered_ix.remove(a);
        }
        self.count();
    }
    // Removes the entry at `a` if it is `obj`.
    fn remove_if(&mut self, a: usize, obj: u64, ephemeral: bool) {
        if ephemeral {
            if let std::collections::btree_map::Entry::Occupied(e) = self.ephemeral.entry(a) {
                if *e.get() == obj {
                    e.remove();
                }
            }
            self.count();
        } else if self.get(&a) == Some(&obj) {
            self.remove(&a);
        }
    }
    // Whether any entry lies in `[lo, hi)`: one descent per map, nothing
    // collected (most released or dropped ranges hold none).
    fn any_in(&self, lo: usize, hi: usize) -> bool {
        let first = |m: &BTreeMap<usize, u64>| m.range(lo..).next().map_or(false, |(a, _)| *a < hi);
        lo < hi && (first(&self.ordered) || first(&self.ephemeral))
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
    // 0: a fn item; 1: a closure, called with an exclusive borrow of
    // its object as `self`; 2: one that captures nothing, whose code
    // never reaches `self` (`Fx::lower_closure_body`): no borrow formed.
    closure: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Proj {
    Field(u64),
    Index(u64),
    Payload,
    // D-0070: the elements `lo .. hi` a slice borrows, as the last step of
    // its path; an index after it (an element through the slice) is
    // relative to `lo`.
    Range(u64, u64),
}

fn has_range(p: &[Proj]) -> bool {
    p.iter().any(|x| matches!(x, Proj::Range(..)))
}

// A path with each range that an index follows folded into that index,
// made absolute: `[…, Range(l, h), Index(r)]` is `[…, Index(l + r)]`.
fn normalize(p: impl Iterator<Item = Proj>) -> Vec<Proj> {
    let mut out: Vec<Proj> = Vec::new();
    for x in p {
        match (out.last().copied(), x) {
            (Some(Proj::Range(l, _)), Proj::Index(r)) => {
                out.pop();
                out.push(Proj::Index(l + r));
            }
            _ => out.push(x),
        }
    }
    out
}

// `normalize` into a stack buffer: a path of a few steps (every path a
// program forms in practice) is compared without allocating.
const NORM_INLINE: usize = 12;

struct Norm {
    buf: [Proj; NORM_INLINE],
    len: usize,
    spill: Vec<Proj>,
}

impl Norm {
    fn of(p: impl Iterator<Item = Proj>) -> Norm {
        let mut n = Norm { buf: [Proj::Payload; NORM_INLINE], len: 0, spill: Vec::new() };
        for x in p {
            let last = if n.spill.is_empty() { n.buf[..n.len].last().copied() } else { n.spill.last().copied() };
            match (last, x) {
                (Some(Proj::Range(l, _)), Proj::Index(r)) => n.set_last(Proj::Index(l + r)),
                _ => n.push(x),
            }
        }
        n
    }

    fn push(&mut self, x: Proj) {
        if !self.spill.is_empty() {
            self.spill.push(x);
        } else if self.len < NORM_INLINE {
            self.buf[self.len] = x;
            self.len += 1;
        } else {
            self.spill.extend_from_slice(&self.buf);
            self.spill.push(x);
        }
    }

    fn set_last(&mut self, x: Proj) {
        if let Some(l) = self.spill.last_mut() {
            *l = x;
        } else {
            self.buf[self.len - 1] = x;
        }
    }

    fn get(&self) -> &[Proj] {
        if self.spill.is_empty() { &self.buf[..self.len] } else { &self.spill }
    }
}

// `overlap` over normalized paths: one a prefix of the other, step by
// step, a range meeting an index inside it or a range it intersects.
// `normalize` as an iterator, for comparing a stored path without
// building it.
struct NormIter<'a> {
    s: &'a [Proj],
    i: usize,
}

impl<'a> Iterator for NormIter<'a> {
    type Item = Proj;
    fn next(&mut self) -> Option<Proj> {
        let x = *self.s.get(self.i)?;
        self.i += 1;
        if let (Proj::Range(l, _), Some(Proj::Index(r))) = (x, self.s.get(self.i).copied()) {
            self.i += 1;
            return Some(Proj::Index(l + r));
        }
        Some(x)
    }
}

// `ranged_overlap(a, normalize(b))` without allocating.
fn ranged_overlap_with(a: &[Proj], b: &[Proj]) -> bool {
    a.iter().copied().zip(NormIter { s: b, i: 0 }).all(|(x, y)| match (x, y) {
        (Proj::Range(l, h), Proj::Index(i)) | (Proj::Index(i), Proj::Range(l, h)) => l <= i && i < h,
        (Proj::Range(l1, h1), Proj::Range(l2, h2)) => l1.max(l2) < h1.min(h2),
        (x, y) => x == y,
    })
}

fn ranged_overlap(a: &[Proj], b: &[Proj]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| match (*x, *y) {
        (Proj::Range(l, h), Proj::Index(i)) | (Proj::Index(i), Proj::Range(l, h)) => l <= i && i < h,
        (Proj::Range(l1, h1), Proj::Range(l2, h2)) => l1.max(l2) < h1.min(h2),
        (x, y) => x == y,
    })
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
    // D-0089: a value of this type holds a `fn` value somewhere (a field,
    // an element, a payload), whose closure ends with it.
    holds_fn: bool,
    // `Box<T>`: T's type (cobc marks a Box's descriptor with `elem` =
    // T's id + 1; a struct's `elem` is otherwise 0).
    box_of: Option<u32>,
    // `Vec<T>` whose `drop` is cobc's native body for a plain `T`: the
    // element size (marked by `count` = size + 1; a struct's is otherwise 0).
    vec_plain: Option<u64>,
    // A struct's fields that own something (a resource, or a `fn` value),
    // in declaration order: the only ones its destruction has to visit.
    owning: Vec<(u64, u32)>,
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
    toks: TokList,
    // The non-root exclusive ones among `toks`: all a shared access can
    // clash with (`check_access`). A path's mode and object never change.
    xtoks: TokList,
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
    // The statement's location (`cb_stmt_push_at`), for a destruction
    // fault when its temporaries end; `NOLOC` when none was given.
    loc: (u32, u32),
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
    // The positions in `stack` of its statement scopes, in order: the
    // nearest one is found at once, however many frames lie above it (a
    // deep recursion through calls made as whole statements pushes
    // frames only, and looking down the stack for a scope cost its depth).
    scopes: Vec<usize>,
    inflight: VecDeque<Inflight>,
    // Each frame's owning construct's location (`cb_frame_push_at`), and
    // the one whose frame is ending now, for a destruction fault.
    frame_locs: Vec<(u32, u32)>,
    dloc: (u32, u32),
    // A Box teardown loop is running (`box_drop`), and the referent a Box
    // in tail position left to it: (its root, its object, its cells).
    box_draining: bool,
    box_next: Option<(u64, u64, usize, u64, u32)>,
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
    slots: SlotMap,
    // `forget_slots`'s reusable buffer.
    slot_buf: Vec<(usize, u64)>,
    reclaimed: Reclaimed,
    fn_items: HashMap<usize, usize>,  // code address -> its interned box
    fn_boxes: HashMap<usize, u64>,    // closure box handle -> the closure object it owns
    unwinding: bool,
    // The first fault's rendered report, printed once the unwind is done
    // (`[Fault-Unwind]`: unwind, then terminate reporting it).
    fault_report: Option<String>,
    threads: HashMap<u64, ThreadRec>,
    locks: HashMap<usize, u64>, // a mutex's address -> the thread holding it (`state.sync`)
    live: usize,
    next_tid: u64,
    // `std::event_op`'s counts (D-0063), by id, and the last id made.
    events: HashMap<u64, u64>,
    last_event: u64,
    // Emptied id lists, reused: a frame's, a scope's and an object's
    // path list are created and dropped at every block, statement and
    // borrow, and allocating each was a tenth of a program's run.
    pool: Vec<Vec<u64>>,
    // The same for a path's projection: every borrow makes one, and every
    // path's end drops it.
    proj_pool: Vec<Vec<Proj>>,
}

fn give_proj(pool: &mut Vec<Vec<Proj>>, mut v: Vec<Proj>) {
    if v.capacity() > 0 && pool.len() < 4096 {
        v.clear();
        pool.push(v);
    }
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
// handle's destructor), a mutex's address with the top bit set (`lock`)
// or a channel's event count (`event_key`): (key, times signalled,
// threads waiting, their condition variable). A signal wakes only the
// threads waiting on that very key; a mutex unlocked in a loop must not
// wake a thread that is only waiting for a join.
struct Waiters {
    key: u64,
    signalled: u64,
    waiting: u32,
    cv: std::sync::Arc<Condvar>,
}
static EVENTS: Mutex<Vec<Waiters>> = Mutex::new(Vec::new());

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
static mut MAIN_TS: TState = TState { stack: Vec::new(), scopes: Vec::new(), inflight: VecDeque::new(), frame_locs: Vec::new(), dloc: (NOLOC, 0), box_draining: false, box_next: None };
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
    if let Some(ev) = g.iter_mut().find(|ev| ev.key == key) {
        ev.signalled += 1;
        if ev.waiting > 0 {
            let cv = ev.cv.clone();
            drop(g);
            cv.notify_all();
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
        let (e, cv) = {
            let mut g = EVENTS.lock().unwrap();
            match g.iter_mut().find(|ev| ev.key == key) {
                Some(ev) => {
                    ev.waiting += 1;
                    (ev.signalled, ev.cv.clone())
                }
                None => {
                    let cv = std::sync::Arc::new(Condvar::new());
                    g.push(Waiters { key, signalled: 0, waiting: 1, cv: cv.clone() });
                    (0, cv)
                }
            }
        };
        let held = lock_release_all();
        {
            let mut g = EVENTS.lock().unwrap();
            loop {
                let now = g.iter().find(|ev| ev.key == key).map_or(e, |ev| ev.signalled);
                if now != e {
                    break;
                }
                let (g2, to) = cv.wait_timeout(g, Duration::from_millis(50)).unwrap();
                g = g2;
                if to.timed_out() {
                    break;
                }
            }
            if let Some(i) = g.iter().position(|ev| ev.key == key) {
                g[i].waiting -= 1;
                if g[i].waiting == 0 {
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
    set_stack_floor();
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
            holds_fn: t.kind == K_FN,
            box_of: if t.kind == K_STRUCT && t.elem != 0 { Some(t.elem - 1) } else { None },
            vec_plain: if t.kind == K_STRUCT && t.count != 0 && t.nfields == 3 { Some(t.count - 1) } else { None },
            owning: Vec::new(),
        });
    }
    // D-0089: `holds_fn` through fields, elements and payloads.
    loop {
        let mut changed = false;
        for i in 0..ts.len() {
            if ts[i].holds_fn {
                continue;
            }
            let t = &ts[i];
            let inner = t.fields.iter().any(|&(_, f)| ts.get(f as usize).map_or(false, |x| x.holds_fn))
                || t.variants.iter().any(|&v| v != NOTYPE && ts.get(v as usize).map_or(false, |x| x.holds_fn))
                || (t.kind == K_ARRAY && ts.get(t.elem as usize).map_or(false, |x| x.holds_fn));
            if inner {
                ts[i].holds_fn = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for i in 0..ts.len() {
        if ts[i].kind == K_STRUCT {
            let owning: Vec<(u64, u32)> = ts[i].fields.iter().copied().filter(|&(_, f)| ts.get(f as usize).map_or(false, |x| x.is_resource || x.holds_fn)).collect();
            ts[i].owning = owning;
        }
    }
    *RT.0.get() = Some(Rt {
        files: fs,
        types: ts,
        objs: Arena::default(),
        paths: Arena::default(),
        slots: SlotMap::default(),
        slot_buf: Vec::new(),
        reclaimed: Reclaimed::default(),
        fn_items: HashMap::default(),
        fn_boxes: HashMap::default(),
        unwinding: false,
        fault_report: None,
        threads: HashMap::default(),
        locks: HashMap::default(),
        live: 1,
        next_tid: 2,
        events: HashMap::default(),
        last_event: 0,
        pool: Vec::new(),
        proj_pool: Vec::new(),
    });
}

#[no_mangle]
pub unsafe extern "C" fn cb_fault(diag: *const c_char, file: u32, line: u32) -> ! {
    let _e = enter();
    let id = CStr::from_ptr(diag).to_string_lossy().into_owned();
    fault(&id, file, line)
}

// A fault carrying the program's message (`assert`, D-0065).
#[no_mangle]
pub unsafe extern "C" fn cb_fault_msg(diag: *const c_char, msg: *const u8, n: u64, file: u32, line: u32) -> ! {
    let _e = enter();
    let id = CStr::from_ptr(diag).to_string_lossy().into_owned();
    let m = if n == 0 { String::new() } else { String::from_utf8_lossy(std::slice::from_raw_parts(msg, n as usize)).into_owned() };
    fault(&format!("{}{}{}", id, diagnostics::DETAIL_SEP, m), file, line)
}

fn fault(id: &str, file: u32, line: u32) -> ! {
    // The GIL is never released again: `[Fault-Unwind]` — other threads
    // take no further steps and their frames are not unwound.
    let r = rt();
    let (file, line) = if file == NOLOC && line == 0 { get_at() } else { (file, line) };
    flush_stdout();
    if r.unwinding {
        // A destructor faulting during the unwind: the program's outcome
        // is the first fault.
        if let Some(rep) = r.fault_report.take() {
            eprint!("{}", diagnostics::for_stderr(&rep));
        }
        std::process::exit(1);
    }
    let at = if file == NOLOC { None } else { r.files.get(file as usize).map(|f| (f.as_str(), line as usize)) };
    let (id, msg) = match id.split_once(diagnostics::DETAIL_SEP) {
        Some((i, m)) => (i, Some(m)),
        None => (id, None),
    };
    let out = diagnostics::render(id, "dynamic", at);
    r.fault_report = Some(match msg {
        Some(m) => diagnostics::with_message(out, m),
        None => out,
    });
    r.unwinding = true;
    // [Fault-Unwind]: fold every open scope and frame, innermost first.
    while let Some(entry) = ts().stack.pop() {
        match entry {
            Entry::Frame(owned) => end_owned(owned),
            Entry::Scope(s) => {
                ts().scopes.pop();
                end_scope(s)
            }
        }
    }
    flush_stdout();
    if let Some(rep) = rt().fault_report.take() {
        eprint!("{}", diagnostics::for_stderr(&rep));
    }
    std::process::exit(1);
}

// `[Terminate-Ok]`: `ok(s)`, reported as exit status `s` (`rule.fn.program`).
#[no_mangle]
pub extern "C" fn cb_terminate_ok(status: u8) -> ! {
    flush_stdout();
    // A test hook (CHG-0093): what the runtime still records once the
    // program has ended normally, which a program's own bookkeeping
    // must not make grow with its running time.
    if std::env::var_os("COBALTC_RT_STATS").is_some() {
        let r = rt();
        let objects = r.objs.slots.iter().filter(|(_, o)| matches!(o, Some(o) if o.alive)).count();
        let paths = r.paths.slots.iter().filter(|(_, p)| p.is_some()).count();
        eprintln!("cbrt: live at exit: {} objects, {} paths", objects, paths);
    }
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

// `std::stderr_write` (`eprintf`, D-0040): as `cb_write_out`, to standard
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

// `std::stdin_read`: up to `n` bytes of standard input into `p`; 0 at its end,
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
    // Parsed formats, most recent last (`cb_format`).
    static FMT_CACHE: std::cell::RefCell<Vec<(Vec<u8>, Vec<fmt::Piece>)>> = const { std::cell::RefCell::new(Vec::new()) };
    // `cb_format`'s arguments, copied out of the call: kept between calls,
    // the list and each text argument's bytes reuse their storage.
    static FMT_ARGS: std::cell::RefCell<Vec<fmt::Arg>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[no_mangle]
pub unsafe extern "C" fn cb_format(f: *const u8, fn_: u64, args: *const CbFmtArg, n: u64, out: *mut CbStr) {
    let fb = bytes(f, fn_);
    let cargs = args;
    FMT_ARGS.with(|fa| {
    let mut args = fa.borrow_mut();
    let n = n as usize;
    args.truncate(n);
    for i in 0..n {
        let a = &*cargs.add(i);
        let v = match a.kind {
            0 | 1 => fmt::Arg::Int { bits: ((a.hi as u128) << 64) | a.lo as u128, signed: a.kind == 1, width: a.width },
            2 => fmt::Arg::Float(a.f, a.width == 32),
            _ => {
                // The bytes copied into the slot's own buffer when it has one.
                if let Some(fmt::Arg::Text(t)) = args.get_mut(i) {
                    t.clear();
                    t.extend_from_slice(bytes(a.p, a.n));
                    continue;
                }
                fmt::Arg::Text(bytes(a.p, a.n).to_vec())
            }
        };
        if i < args.len() {
            args[i] = v;
        } else {
            args.push(v);
        }
    }
    let args = &*args;
    // The format's pieces, parsed once per distinct format (by its bytes:
    // a format built at run time may reuse an address with other text),
    // and the text rendered into this thread's buffer. The arguments were
    // copied out above, so a text argument that was the previous result
    // is not overwritten under them.
    FMT_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let i = match c.iter().position(|(k, _)| k.as_slice() == fb) {
            Some(i) => i,
            None => {
                if c.len() >= 64 {
                    c.remove(0);
                }
                c.push((fb.to_vec(), fmt::parse(fb).unwrap_or_default()));
                c.len() - 1
            }
        };
        FORMATTED.with(|b| {
            let mut b = b.borrow_mut();
            b.clear();
            fmt::render_into(&c[i].1, args, &mut b);
            *out = CbStr { p: b.as_ptr(), n: b.len() as u64 };
        });
    });
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

// `Vec::sort` (D-0062) of a plain integer or `bool` element type,
// realized natively: a stable sort of the `n` elements at `p` in place.
// `kind`: the element's width in bytes, plus 256 when it is signed; 0
// for `bool`.
#[no_mangle]
pub unsafe extern "C" fn cb_sort_plain(p: *mut u8, n: u64, kind: u32) {
    let n = n as usize;
    macro_rules! sort_as {
        ($t:ty) => {
            std::slice::from_raw_parts_mut(p as *mut $t, n).sort()
        };
    }
    match kind {
        0 => sort_as!(u8),
        1 => sort_as!(u8),
        2 => sort_as!(u16),
        4 => sort_as!(u32),
        8 => sort_as!(u64),
        16 => sort_as!(u128),
        257 => sort_as!(i8),
        258 => sort_as!(i16),
        260 => sort_as!(i32),
        264 => sort_as!(i64),
        272 => sort_as!(i128),
        _ => {}
    }
}

// `key_less` (D-0062) on text: byte order, a prefix first.
#[no_mangle]
pub unsafe extern "C" fn cb_bytes_less(a: *const u8, an: u64, b: *const u8, bn: u64) -> u8 {
    (bytes(a, an) < bytes(b, bn)) as u8
}

// `std::fs_op`, `std::fs_query`, `std::clock_read` (D-0061): the
// filesystem, the environment and the clocks, as `coby` has them.
#[no_mangle]
pub unsafe extern "C" fn cb_fs_op(op: u64, a: *const u8, an: u64, b: *const u8, bn: u64) -> i64 {
    fileio::fs_op(op, bytes(a, an), bytes(b, bn))
}

#[no_mangle]
pub unsafe extern "C" fn cb_fs_query(op: u64, a: *const u8, an: u64, buf: *mut u8, cap: u64) -> i64 {
    match fileio::fs_query(op, bytes(a, an)) {
        Ok(out) => {
            let k = out.len().min(cap as usize);
            if k > 0 {
                std::ptr::copy_nonoverlapping(out.as_ptr(), buf, k);
            }
            out.len() as i64
        }
        Err(c) => c,
    }
}

#[no_mangle]
pub extern "C" fn cb_clock_read(which: u64) -> i64 {
    fileio::clock_read(which)
}

// `std::sleep_ns` (D-0124).
#[no_mangle]
pub extern "C" fn cb_sleep_ns(ns: u64) -> i64 {
    fileio::sleep_ns(ns);
    0
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
    ts().frame_locs.push((NOLOC, 0));
}

#[no_mangle]
pub extern "C" fn cb_frame_push_at(file: u32, line: u32) {
    cb_frame_push();
    if let Some(l) = ts().frame_locs.last_mut() {
        *l = (file, line);
    }
}

#[no_mangle]
pub extern "C" fn cb_frame_pop() {
    let _e = enter();
    let loc = ts().frame_locs.pop().unwrap_or((NOLOC, 0));
    match ts().stack.pop() {
        Some(Entry::Frame(owned)) => {
            let saved = std::mem::replace(&mut ts().dloc, loc);
            end_owned(owned);
            ts().dloc = saved;
        }
        Some(Entry::Scope(_)) => panic!("cbrt: frame pop closed a statement scope"),
        None => panic!("cbrt: frame pop on an empty stack"),
    }
}

// A reference binding with no object (`cobc`'s held locals): the frame
// holds its path, as the binding's object would, until the frame ends
// or the binding ends early (`cb_frame_unhold`, D-0111). A held path
// lives on past its statement (`end_scope`) and ends with its last
// holder (`drop_occ`).
#[no_mangle]
pub extern "C" fn cb_frame_hold(tok: u64) {
    let _e = enter();
    let r = rt();
    if let Some(p) = r.paths.get_mut(&tok) {
        p.occ += 1;
    } else {
        return;
    }
    let t = ts();
    if let Some(Entry::Frame(owned)) = t.stack.iter_mut().rev().find(|e| matches!(e, Entry::Frame(_))) {
        owned.push(tok | HOLD);
    }
}

#[no_mangle]
pub extern "C" fn cb_frame_unhold(tok: u64) {
    let _e = enter();
    let t = ts();
    for e in t.stack.iter_mut().rev() {
        if let Entry::Frame(owned) = e {
            if let Some(i) = owned.iter().rposition(|&x| x == tok | HOLD) {
                owned.remove(i);
                drop_occ(rt(), tok);
                return;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn cb_stmt_push() {
    let _e = enter();
    // Its lists start empty and come from the pool on first use
    // (`stamp_temp`, `stamp_tok`): most statements record nothing.
    let t = ts();
    t.scopes.push(t.stack.len());
    t.stack.push(Entry::Scope(Scope { temps: Vec::new(), toks: Vec::new(), loc: (NOLOC, 0) }));
}

// A statement scope whose end reports `file:line` for a destruction
// fault, as `cb_frame_push_at` does for a frame: a temporary that ends
// with the statement while a kept reference holds it.
#[no_mangle]
pub extern "C" fn cb_stmt_push_at(file: u32, line: u32) {
    cb_stmt_push();
    if let Some(Entry::Scope(s)) = ts().stack.last_mut() {
        s.loc = (file, line);
    }
}

#[no_mangle]
pub extern "C" fn cb_stmt_pop() {
    let _e = enter();
    match ts().stack.pop() {
        Some(Entry::Scope(s)) => {
            ts().scopes.pop();
            if s.loc.0 == NOLOC {
                end_scope(s)
            } else {
                let saved = std::mem::replace(&mut ts().dloc, s.loc);
                end_scope(s);
                ts().dloc = saved;
            }
        }
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

// One fewer slot holds `tok`. A path its last holder let go of, owned
// by no statement scope and not in flight (a flight holds it), is
// unreachable: it ends now (D-0018: a path lives while something holds
// it). Left, it would stay on its object's list, and every later check
// of that object would scan it -- a loop borrowing into one array, or
// re-pointing one reference down a tree (`cur = &cur.kids[i]`), made
// each access slower than the last.
fn drop_occ(r: &mut Rt, tok: u64) {
    if let Some(p) = r.paths.get_mut(&tok) {
        p.occ = p.occ.saturating_sub(1);
        if p.occ == 0 && (!p.valid || (p.scope.is_none() && !p.is_root)) {
            retire_token(r, tok);
        }
    }
}

fn retire_token(r: &mut Rt, t: u64) {
    if let Some(mut p) = r.paths.remove(&t) {
        give_proj(&mut r.proj_pool, std::mem::take(&mut p.proj));
        give_vec(&mut r.pool, std::mem::take(&mut p.ancestors));
        let mut rest = None;
        if let Some(o) = r.objs.get_mut(&p.of) {
            o.toks.remove(t);
            if p.exclusive && !p.is_root {
                o.xtoks.remove(t);
            }
            // Its root ended: the object keeps no name for a dead path,
            // so a later `[Reclaim]` of its cells mints a new root rather
            // than handing back this one (`cb_reclaim`).
            if o.root == Some(t) {
                o.root = None;
            }
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
// A frame's list holds objects and, marked by `HOLD`, paths it holds for
// a reference binding with no object (`cb_frame_hold`).
const HOLD: u64 = 1 << 63;

fn end_owned(owned: Vec<u64>) {
    for &obj in owned.iter().rev() {
        let r = rt();
        if obj & HOLD != 0 {
            drop_occ(r, obj & !HOLD);
            continue;
        }
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

// The statement scope nearest below position `i` of the stack.
fn scope_below(i: usize) -> Option<usize> {
    let sc = &ts().scopes;
    let k = sc.partition_point(|&x| x < i);
    if k == 0 {
        None
    } else {
        Some(sc[k - 1])
    }
}

fn top_scope(_r: &Rt) -> Option<usize> {
    ts().scopes.last().copied()
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
            if s.temps.capacity() == 0 {
                s.temps = take_vec(&mut r.pool);
            }
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
        let fresh = matches!(ts().stack.get(i), Some(Entry::Scope(s)) if s.toks.capacity() == 0);
        let v0 = if fresh { Some(take_vec(&mut r.pool)) } else { None };
        if let Some(v) = scope_toks(r, i) {
            if let Some(nv) = v0 {
                *v = nv;
            }
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
    r.objs.insert(id, Obj { addr, ty, alive: true, init, is_resource, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: TokList::default(), xtoks: TokList::default() });
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
            o.toks = TokList::from_vec(take_vec(&mut r.pool));
        }
        o.toks.push(tok);
        if exclusive && !is_root {
            o.xtoks.push(tok);
        }
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
    let moved: Vec<(usize, u64)> = r.slots.drain(old, old + size as usize);
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
        Some(i) => scope_below(i),
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

// D-0111: a local reference binding ends after its last use, as at its
// block's exit: its object ends, and with it the path it holds.
#[no_mangle]
pub extern "C" fn cb_end_binding(root: u64) {
    let _e = enter();
    let r = rt();
    let Some(obj) = r.paths.get(&root).map(|p| p.of) else { return };
    destroy_obj(obj, Some(root));
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
    destroy_obj_tail(obj, root, false)
}

// `destroy_obj`; `tail`: nothing observable follows it in the Box teardown
// that asked for it.
fn destroy_obj_tail(obj: u64, root: Option<u64>, tail: bool) {
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
            let (file, line) = ts().dloc;
            fault("diag.destroy-while-aliased", file, line);
        }
        drop_in_place_tail(addr, ty, obj, tail);
    } else if r.types[ty as usize].holds_fn {
        // D-0089: plain, but its `fn` values own their closures.
        drop_in_place_tail(addr, ty, obj, tail);
    }
    let r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

// Runs the destructor and destroys sub-resources of the value at
// `addr` of type `ty` (`[Run-Destructor]`, `[Destroy-Composite]`).
// `owner` is the object being destroyed, or 0 for a field.
fn drop_in_place(addr: usize, ty: u32, owner: u64) {
    drop_in_place_tail(addr, ty, owner, false)
}

// `drop_in_place`; `tail` as for `destroy_obj_tail`: what this destroys
// last is then in tail position too.
fn drop_in_place_tail(addr: usize, ty: u32, owner: u64, tail: bool) {
    let r = rt();
    let info = &r.types[ty as usize];
    if let Some(t) = info.box_of {
        let full_off = info.fields.get(1).map_or(8, |f| f.0) as usize;
        box_drop(addr, full_off, t, tail);
        return;
    }
    if info.kind == K_FN {
        // A fn-typed slot owns the closure box its handle names, if any.
        drop_fn_handle(unsafe { *(addr as *const usize) });
        return;
    }
    if !info.is_resource && !info.holds_fn {
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
    if let (Some(esz), true) = (info.vec_plain, info.drop.is_some()) {
        // cobc's native `Vec::drop` for a plain element (`native_vec_drop`):
        // its checked read of `self.len` through the root just minted for
        // it cannot fail, so that root and its object are not made; the
        // element checks and the buffer's release are as the body makes
        // them. The vector's own fields are plain, so nothing follows.
        let (po, lo, co) = (info.fields[0].0 as usize, info.fields[1].0 as usize, info.fields[2].0 as usize);
        let (ptr, len, cap) = unsafe { (*((addr + po) as *const *mut u8), *((addr + lo) as *const u64), *((addr + co) as *const u64)) };
        if len > 0 {
            cb_vec_drop_plain(ptr, len, esz);
        }
        if cap > 0 {
            cb_deallocate(ptr, cap * esz, 1);
        }
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
        // The destructor is code of its own, run in the middle of the
        // caller's transfers: a `return` of a resource sends its value
        // before the frame's objects are destroyed, and a destructor's own
        // calls send and receive through the same queue. It gets an empty
        // queue, and the caller's is put back when it returns.
        let saved = std::mem::take(&mut ts().inflight);
        unsafe { drop(CbRef { p: addr as *mut u8, tok }) };
        let leftover = std::mem::replace(&mut ts().inflight, saved);
        debug_assert!(leftover.is_empty(), "cbrt: a destructor left values in flight");
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
            // The owning fields (`TypeInfo::owning`; a plain one's
            // destruction does nothing), last declared first, the first
            // declared last; read by index each time, not copied out (one
            // list per struct destroyed).
            let n = info.owning.len();
            for i in (0..n).rev() {
                let (off, fty) = rt().types[ty as usize].owning[i];
                drop_in_place_tail(addr + off as usize, fty, 0, tail && i == 0);
            }
        }
        K_ENUM => {
            // The one payload type needed, copied out (not the list: a
            // copy of it per enum destroyed was an allocation each).
            let tag = unsafe { *(addr as *const u32) } as usize;
            let off = info.payload_off as usize;
            if let Some(&pty) = info.variants.get(tag) {
                if pty != NOTYPE {
                    drop_in_place_tail(addr + off, pty, 0, tail);
                }
            }
        }
        K_ARRAY => {
            let (elem, count) = (info.elem, info.count);
            let esize = r.types[elem as usize].size as usize;
            for i in (0..count as usize).rev() {
                drop_in_place_tail(addr + i * esize, elem, 0, tail && i == 0);
            }
        }
        _ => {}
    }
}

// A `Box<T>` at `addr` destroyed as `std`'s `Box::drop` does it
// (`drop(reclaim<T>(ptr))`, then `deallocate`), natively. Its referent's
// destruction is its last observable effect, so a Box in tail position
// hands it to the nearest enclosing teardown's loop: a Box-linked chain of
// any length is destroyed in constant stack, in the same order.
fn box_drop(addr: usize, full_off: usize, t: u32, tail: bool) {
    let p = unsafe { *(addr as *const *mut u8) };
    let full = unsafe { *((addr + full_off) as *const u8) } != 0;
    let cells = rt().types[t as usize].size.max(1);
    if !full {
        cb_deallocate(p, cells, 0);
        return;
    }
    // With no live object over the contents, `[Reclaim]` would establish
    // a fresh one, no path but its root, which the destruction ends at
    // once: unobservable, so the contents are destroyed in place (root
    // and object 0) -- a tree's teardown makes no object per node.
    let none_over = cb_reclaimed_entries.load(std::sync::atomic::Ordering::Relaxed) == 0 || live_reclaimed(rt(), p as usize).is_none();
    let (root, obj) = if none_over {
        (0, 0)
    } else {
        let root = cb_reclaim(p, t);
        let obj = {
            let r = rt();
            let (file, line) = ts().dloc;
            path_target(r, root, file, line).0
        };
        (root, obj)
    };
    let st = ts();
    if tail && st.box_draining && st.box_next.is_none() {
        st.box_next = Some((root, obj, p as usize, cells, t));
        return;
    }
    let was = std::mem::replace(&mut st.box_draining, true);
    let saved = st.box_next.take();
    let mut next = Some((root, obj, p as usize, cells, t));
    while let Some((root, obj, p, cells, t)) = next.take() {
        if obj == 0 {
            drop_in_place_tail(p, t, 0, true);
        } else {
            destroy_obj_tail(obj, Some(root), true);
        }
        cb_deallocate(p as *mut u8, cells, 0);
        next = ts().box_next.take();
    }
    let st = ts();
    st.box_next = saved;
    st.box_draining = was;
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
    r.objs.insert(id, Obj { addr, ty, alive: true, init: true, is_resource: true, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: TokList::default(), xtoks: TokList::default() });
    id
}

// `[Object-End]`: every path into the object dies, every reference
// its storage held stops being an occurrence, the identity is gone.
fn end_identity(r: &mut Rt, obj: u64) {
    let (addr, size, ephemeral, toks) = match r.objs.get_mut(&obj) {
        Some(o) => {
            o.xtoks = TokList::default();
            (o.addr, r.types[o.ty as usize].size, o.ephemeral, std::mem::take(&mut o.toks).into_vec())
        }
        None => return,
    };
    // Every path into the object is dead; a token that turns up later
    // (in a register somewhere) is reported stale by its absence.
    for &t in &toks {
        if let Some(mut p) = r.paths.remove(&t) {
            give_proj(&mut r.proj_pool, std::mem::take(&mut p.proj));
            give_vec(&mut r.pool, std::mem::take(&mut p.ancestors));
        }
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
    // Into a buffer kept for this (an object's end is the commonest
    // runtime event); a nested end, through `drop_occ`, takes a new one.
    let mut held = std::mem::take(&mut r.slot_buf);
    r.slots.scan_into(addr, addr + size as usize, true, &mut held);
    for &(_, t) in &held {
        drop_occ(r, t);
    }
    held.clear();
    let r = rt();
    if r.slot_buf.capacity() < held.capacity() {
        r.slot_buf = held;
    }
}

// ---- raw storage ----

#[no_mangle]
pub extern "C" fn cb_reclaim(addr: *mut u8, ty: u32) -> u64 {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    if let Some(obj) = live_reclaimed(r, a) {
        // Its root token is about to be in the program's hands.
        let o = r.objs.get_mut(&obj).unwrap();
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
    establish_reclaimed(r, a, ty, false).0
}

// `[Reclaim]`'s fresh object over the cells at `a`: its root token (0 for
// an ephemeral object, which has none) and its id.
fn establish_reclaimed(r: &mut Rt, a: usize, ty: u32, ephemeral: bool) -> (u64, u64) {
    let id = r.objs.reserve();
    let is_resource = r.types.get(ty as usize).map_or(false, |t| t.is_resource);
    r.objs.insert(id, Obj { addr: a, ty, alive: true, init: true, is_resource, reclaimed: true, root: None, frame: None, scope: None, tid: 0, ephemeral, toks: TokList::default(), xtoks: TokList::default() });
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
// `&place` / `&mut place` passed to a parameter that never reads the
// reference's path (`cobc`'s pure direct parameters): `[Borrow]`'s
// check alone -- stale binding, then `clash` -- with no path minted.
// Token 0 names no path (the arenas' "none"): it is what `cobc` passes for
// a reference whose every check the caller has proven -- an element of a
// `Vec` a `foreach` holds shared, handed to a *reader* parameter
// (`Gen::reader_param`), which only reads through it and keeps nothing.
// A check through it is proven, and a borrow from it is another such
// reference (`cb_borrow` gives 0).
#[no_mangle]
pub unsafe extern "C" fn cb_borrow_unminted(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) {
    if base == 0 {
        return;
    }
    let _e = enter();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    check_access(rt(), base, extra, mode == 1, file, line);
}

#[no_mangle]
pub unsafe extern "C" fn cb_borrow_check(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) {
    if base == 0 {
        return;
    }
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
    if let Some(p) = rt().paths.get_mut(&tok) {
        p.occ += 1;
    }
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
pub unsafe extern "C" fn cb_elem_access_slow(addr: *mut u8, ty: u32, mode: u32, write: u32, file: u32, line: u32) {
    let _e = enter();
    let r = rt();
    let a = addr as usize;
    if live_reclaimed(r, a).is_none() {
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

// The live object over the cells at `a`, if any: one with a root, an
// ephemeral one, or a value `cb_raw_move_in` placed there (its root
// ended with the move, but it is still the object over these cells).
// Establishing a second object over them instead left the first alive
// and unreachable, one more per element on every access.
fn live_reclaimed(r: &Rt, a: usize) -> Option<u64> {
    r.reclaimed.get(&a).copied().filter(|o| {
        r.objs.get(o).map_or(false, |o| {
            o.alive && (o.root.is_some() || o.ephemeral || (o.reclaimed && o.frame.is_none() && o.scope.is_none()))
        })
    })
}

fn elem_borrow(addr: *mut u8, ty: u32, mode: u32, stamp: bool) -> u64 {
    let r = rt();
    let a = addr as usize;
    let exclusive = mode == 1;
    let existing = live_reclaimed(r, a);
    let obj = match existing {
        Some(obj) => obj,
        None => {
            // Ephemeral unless the element holds references (the object
            // holds them, `[Rawptr-Write]`) or is a function value. A
            // resource element qualifies too: an object established by a
            // borrow alone stays `init = valid` with no authority handed
            // out (no move out of a reference; `cb_reclaim`, which gives
            // the program its root, makes it persistent), so a later
            // `[Reclaim]` establishes one no program can tell from it.
            let t = &r.types[ty as usize];
            let plain = !t.has_refs && t.kind != K_FN;
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
    if !r.reclaimed.any_in(a, a + n) {
        return;
    }
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

// Whether a live object lies over the cells at `addr` (`live_reclaimed`):
// for a native body that must take the general path only for such an
// element (`cobc`'s native `Vec<String>::drop`).
#[no_mangle]
pub extern "C" fn cb_live_at(addr: *mut u8) -> u8 {
    let _e = enter();
    live_reclaimed(rt(), addr as usize).is_some() as u8
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
    let ty = o.ty;
    if let Some(root) = o.root.take() {
        invalidate(r, root);
    }
    // A value holding no reference and no function, its root ended by the
    // move, has no path left: its object is one no program can tell from
    // the one a later `[Reclaim]` or element borrow establishes (as for an
    // ephemeral element object), so it ends now instead of lasting until
    // `[Release]` (one per element a `Vec<String>` was ever pushed).
    let t = &r.types[ty as usize];
    if !t.has_refs && t.kind != K_FN && r.objs.get(&obj).map_or(false, |o| o.toks.is_empty()) {
        end_identity(r, obj);
        return;
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
    // No entry at all (`cb_reclaimed_entries`): no object over these cells.
    if cb_reclaimed_entries.load(std::sync::atomic::Ordering::Relaxed) != 0 && r.reclaimed.any_in(a, a + n as usize) {
        let stale: Vec<u64> = r.reclaimed.range(a, a + n as usize).into_iter().map(|(_, o)| o).collect();
        for obj in stale {
            end_identity(r, obj);
        }
    }
    // References written into these cells were held by the object over
    // them (`[Rawptr-Write]`, `CHG-0031`), reclaimed or not: none is held
    // once the cells are released.
    forget_slots(r, a, n);
}

// Allocations come from the C allocator (`malloc`, `free`), whose blocks
// are aligned for any type up to `MALLOC_ALIGN` and need no size to be
// freed. Only an allocation aligned beyond that goes through Rust's
// allocator, which does: those live ones, by address, with (size,
// alignment), and how many there are, so that freeing an ordinary block
// takes neither the lock nor a lookup while there are none. (A registry of
// every allocation cost a lock and a hash insert and remove per `Box`.)
// `deallocate` of anything `allocate` did not return is a false trusted
// claim (`[Deallocate]`, `spec/21` §0; `[Trusted-Claim-False]`).
const MALLOC_ALIGN: usize = if cfg!(target_pointer_width = "64") { 16 } else { 8 };
static ALLOCS: std::sync::Mutex<Option<HashMap<usize, (usize, usize)>>> = std::sync::Mutex::new(None);
static OVER_ALIGNED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" {
    fn malloc(n: usize) -> *mut u8;
}

#[no_mangle]
pub extern "C" fn cb_allocate(size: u64, align: u64) -> *mut u8 {
    let _e = enter();
    if size == 0 {
        return 1 as *mut u8;
    }
    let align = (align.max(1) as usize).next_power_of_two();
    if align <= MALLOC_ALIGN {
        return unsafe { malloc(size as usize) };
    }
    let Ok(layout) = std::alloc::Layout::from_size_align(size as usize, align) else { return std::ptr::null_mut() };
    let p = unsafe { std::alloc::alloc(layout) };
    if !p.is_null() {
        ALLOCS.lock().unwrap().get_or_insert_with(HashMap::default).insert(p as usize, (size as usize, align));
        OVER_ALIGNED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    p
}

#[no_mangle]
pub extern "C" fn cb_deallocate(addr: *mut u8, size: u64, _align: u64) {
    let _e = enter();
    cb_release(addr, size);
    if size == 0 {
        return;
    }
    if OVER_ALIGNED.load(std::sync::atomic::Ordering::Relaxed) > 0 {
        let mut allocs = ALLOCS.lock().unwrap();
        if let Some((s, al)) = allocs.as_mut().and_then(|m| m.remove(&(addr as usize))) {
            OVER_ALIGNED.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(layout) = std::alloc::Layout::from_size_align(s, al) {
                unsafe { std::alloc::dealloc(addr, layout) };
            }
            return;
        }
    }
    unsafe { free(addr) };
}

// `cb_deallocate` for cells that never held a reference (a `String`'s
// bytes, a buffer of elements with none in them): no slot to forget, so
// only an object over the cells, if any could be there, ends first.
#[no_mangle]
pub extern "C" fn cb_deallocate_plain(addr: *mut u8, size: u64) {
    if size == 0 {
        return;
    }
    if cb_reclaimed_entries.load(std::sync::atomic::Ordering::Relaxed) != 0 || OVER_ALIGNED.load(std::sync::atomic::Ordering::Relaxed) != 0 {
        cb_deallocate(addr, size, 1);
        return;
    }
    unsafe { free(addr) };
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
    let moved: Vec<(usize, u64)> = r.slots.range(s, s + n as usize);
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
    r.objs.insert(id, Obj { addr: data as usize, ty, alive: true, init: true, is_resource: false, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: TokList::default(), xtoks: TokList::default() });
    let root = mint(r, id, Vec::new(), true, true, Vec::new());
    r.objs.get_mut(&id).unwrap().root = Some(root);
    cb_copy_datum(data, b.data, ty);
    let nb = Box::new(CbFnBox { code: b.code, data, root, ty, closure: b.closure });
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
    if base == 0 {
        return 0;
    }
    let _e = enter();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    borrow(rt(), base, extra, mode == 1, file, line)
}

// `&m e[lo .. hi]` (D-0070): a borrow of the elements `lo .. hi` of what
// `base ++ extra` reaches, or, when that is itself a slice's range, of
// its elements `lo .. hi` (the range made absolute). Checked as an access
// to that range.
#[no_mangle]
pub unsafe extern "C" fn cb_borrow_range(base: u64, p: *const CbProj, n: usize, lo: u64, hi: u64, mode: u32, file: u32, line: u32) -> u64 {
    borrow_range_in(base, p, n, lo, hi, mode, file, line, true)
}

// `cb_borrow_range`'s checks with no path minted: a slice formed for a
// parameter that never reads its path (`cobc`'s pure direct slices).
#[no_mangle]
pub unsafe extern "C" fn cb_borrow_range_unminted(base: u64, p: *const CbProj, n: usize, lo: u64, hi: u64, mode: u32, file: u32, line: u32) {
    borrow_range_in(base, p, n, lo, hi, mode, file, line, false);
}

unsafe fn borrow_range_in(base: u64, p: *const CbProj, n: usize, lo: u64, hi: u64, mode: u32, file: u32, line: u32, minted: bool) -> u64 {
    let _e = enter();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    let r = rt();
    let rec = match r.paths.get(&base) {
        Some(p) if p.valid => p,
        _ => fault("diag.stale-binding", file, line),
    };
    let mut path: Vec<Proj> = rec.proj.iter().copied().chain(extra.iter().map(|q| conv(*q))).collect();
    let range = match path.last().copied() {
        Some(Proj::Range(pl, _)) => {
            path.pop();
            Proj::Range(pl + lo, pl + hi)
        }
        _ => Proj::Range(lo, hi),
    };
    path.push(range);
    // The access check over the range: `base` itself (and its ancestors)
    // excluded, as for any borrow through it.
    let r = rt();
    let rec_of = r.paths[&base].of;
    let rec_root = r.paths[&base].is_root;
    let anc: Vec<u64> = r.paths[&base].ancestors.clone();
    let exclusive = mode == 1;
    // A shared borrow can clash only with an exclusive path.
    let toks = match r.objs.get(&rec_of) {
        Some(o) if o.alive => if exclusive { o.toks.to_vec() } else { o.xtoks.to_vec() },
        _ => fault("diag.stale-binding", file, line),
    };
    let mine = normalize(path.iter().copied());
    for t in toks {
        let q = &r.paths[&t];
        if !q.valid || q.is_root || q.occ == 0 {
            continue;
        }
        debug_assert!(
            rec_root || anc.contains(&t) || !anc.iter().any(|e| r.paths.get(e).map_or(false, |pe| pe.ancestors.contains(&t))),
            "cbrt: an ancestors list is not closed"
        );
        if !rec_root && (t == base || anc.contains(&t)) {
            continue;
        }
        if !ranged_overlap(&mine, &normalize(q.proj.iter().copied())) {
            continue;
        }
        if q.lock_of.is_some() != r.paths[&base].lock_of.is_some() && !(if r.paths[&base].lock_of.is_some() { q.exclusive } else { exclusive }) {
            continue;
        }
        if exclusive || q.exclusive {
            fault("diag.aliasing-conflict", file, line);
        }
    }
    let mut anc2 = Vec::new();
    if !rec_root {
        anc2.push(base);
        anc2.extend(anc.iter().copied());
        for e in std::iter::once(&base).chain(anc.iter()) {
            if let Some(pe) = r.paths.get(e) {
                for a in &pe.ancestors {
                    if !anc2.contains(a) {
                        anc2.push(*a);
                    }
                }
            }
        }
    }
    if !minted {
        return 0;
    }
    let lock_of = r.paths[&base].lock_of.clone();
    mint_locked(r, rec_of, path, exclusive, false, anc2, lock_of)
}

fn borrow(r: &mut Rt, base: u64, extra: &[CbProj], exclusive: bool, file: u32, line: u32) -> u64 {
    borrow_in(r, base, extra, exclusive, file, line, true)
}

// `stamp` false: the path is recorded in no statement scope; its taker
// ends it (`cb_call_done`) or its last holder does (`drop_occ`).
#[inline]
fn borrow_in(r: &mut Rt, base: u64, extra: &[CbProj], exclusive: bool, file: u32, line: u32, stamp: bool) -> u64 {
    let of = check_access(r, base, extra, exclusive, file, line);
    // The two lists from the runtime's pools (`give_proj`, `give_vec`).
    let mut proj = r.proj_pool.pop().unwrap_or_default();
    let mut anc = if r.paths[&base].is_root { Vec::new() } else { take_vec(&mut r.pool) };
    let rec = &r.paths[&base];
    proj.extend_from_slice(&rec.proj);
    proj.extend(extra.iter().map(|q| conv(*q)));
    // `ancestors`: the chain this borrow was formed through, closed
    // over those tokens' own ancestors (record_token_ancestors).
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
    mint_in(r, of, proj, exclusive, false, anc, lock_of, stamp)
}

// A call through a closure's box (`cobc`'s `cb_call_…` thunks): the
// exclusive borrow of its object that is the call's `self`, ended by
// `cb_call_done` once the call returns -- unheld, no check could count
// it (D-0018), so ending it then rather than with the calling statement
// is unobservable, and the thunk records nothing in the caller's scopes.
#[no_mangle]
pub extern "C" fn cb_call_self(root: u64) -> u64 {
    let _e = enter();
    borrow_in(rt(), root, &[], true, NOLOC, 0, false)
}

// A borrow recorded in no statement scope, for a callee that keeps
// nothing of it (`cobc`'s inline `Vec::push` handing `grow` its path);
// ended by `cb_call_done`.
#[no_mangle]
pub unsafe extern "C" fn cb_borrow_unstamped(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) -> u64 {
    let _e = enter();
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    borrow_in(rt(), base, extra, mode == 1, file, line, false)
}

#[no_mangle]
pub extern "C" fn cb_call_done(tok: u64) {
    let _e = enter();
    let r = rt();
    if matches!(r.paths.get(&tok), Some(p) if p.occ == 0) {
        retire_token(r, tok);
    }
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
        Some(o) if o.alive => if exclusive { &o.toks } else { &o.xtoks },
        _ => fault("diag.stale-binding", file, line),
    };
    for &t in toks {
        let p = &r.paths[&t];
        if !p.valid || p.is_root || p.occ == 0 {
            continue;
        }
        if !exclusive && p.lock_of.is_some() {
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
    // A shared access can clash only with an exclusive path.
    let toks = match r.objs.get(&rec.of) {
        Some(o) if o.alive => if exclusive { &o.toks } else { &o.xtoks },
        _ => fault("diag.stale-binding", file, line),
    };
    let n = rec.proj.len() + extra.len();
    let ranged = has_range(&rec.proj);
    // The accessing path normalized, once per access, when some path
    // involved has a range (`mine` is built on first need).
    let mut mine: Option<Norm> = None;
    for &t in toks {
        let p = &r.paths[&t];
        if !p.valid || p.is_root || p.occ == 0 {
            continue;
        }
        // `excluded`: the accessing path itself and its ancestors (a
        // closed list: an ancestor's ancestors are in it).
        debug_assert!(
            rec.is_root || rec.ancestors.contains(&t) || !rec.ancestors.iter().any(|e| r.paths.get(e).map_or(false, |pe| pe.ancestors.contains(&t))),
            "cbrt: an ancestors list is not closed"
        );
        if !rec.is_root && (t == tok || rec.ancestors.contains(&t)) {
            continue;
        }
        if ranged || has_range(&p.proj) {
            let m = mine.get_or_insert_with(|| Norm::of((0..n).map(|i| proj_at(&rec.proj, extra, i))));
            if !ranged_overlap_with(m.get(), &p.proj) {
                continue;
            }
        } else {
            let m = n.min(p.proj.len());
            if (0..m).any(|i| proj_at(&rec.proj, extra, i) != p.proj[i]) {
                continue;
            }
        }
        // `sync-exempt` (spec/19 §2, CHG-0072), both directions: a
        // lock-derived path and a shared path that is not.
        if rec.lock_of.is_some() != p.lock_of.is_some() && !(if rec.lock_of.is_some() { p.exclusive } else { exclusive }) {
            continue;
        }
        if exclusive || p.exclusive {
            fault("diag.aliasing-conflict", file, line);
        }
    }
    rec.of
}

#[no_mangle]
pub unsafe extern "C" fn cb_read(base: u64, p: *const CbProj, n: usize, file: u32, line: u32) {
    if base == 0 {
        return;
    }
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
    // Token 0 (a proven reference, `cb_read`'s note): the slot stays empty,
    // and `cb_load_ref` gives 0 back.
    if tok == 0 {
        if let Some(old) = r.slots.remove(&(slot as usize)) {
            drop_occ(r, old);
        }
        return;
    }
    if let Some(old) = r.slots.insert(slot as usize, tok) {
        if old == tok {
            return;
        }
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

// D-0089: the offsets of the `fn` slots in the value at `addr`.
fn fn_offsets(r: &Rt, addr: usize, ty: u32, base: u64, out: &mut Vec<u64>) {
    let info = &r.types[ty as usize];
    if !info.holds_fn {
        return;
    }
    match info.kind {
        K_FN => out.push(base),
        K_STRUCT => {
            for &(off, fty) in &info.fields {
                fn_offsets(r, addr + off as usize, fty, base + off, out);
            }
        }
        K_ENUM => {
            let tag = unsafe { *(addr as *const u32) } as usize;
            if let Some(&pty) = info.variants.get(tag) {
                if pty != NOTYPE {
                    fn_offsets(r, addr + info.payload_off as usize, pty, base + info.payload_off, out);
                }
            }
        }
        K_ARRAY => {
            let esize = r.types[info.elem as usize].size;
            for i in 0..info.count {
                fn_offsets(r, addr + (i * esize) as usize, info.elem, base + i * esize, out);
            }
        }
        _ => {}
    }
}

// D-0089: whether the closure a handle names owns a resource.
fn fn_owns_resource(r: &Rt, h: *mut u8) -> bool {
    if h.is_null() || !r.fn_boxes.contains_key(&(h as usize)) {
        return false;
    }
    let b = unsafe { &*(h as *const CbFnBox) };
    r.types[b.ty as usize].is_resource
}

// D-0089: `[Read]` of a `fn` value by value. A closure that owns a
// resource moves out, leaving the slot empty (a call through it is
// `diag.stale-binding`); any other is copied.
#[no_mangle]
pub unsafe extern "C" fn cb_fn_read(slot: *mut *mut u8, file: u32, line: u32) -> *mut u8 {
    let h = *slot;
    let owns = {
        let _e = enter();
        fn_owns_resource(rt(), h)
    };
    if owns {
        *slot = std::ptr::null_mut();
        return h;
    }
    if h.is_null() {
        return h;
    }
    cb_fn_copy(h, file, line)
}

// D-0089: after `*dst = *src` (a value holding `fn` slots read by
// value): each slot's closure is copied into `dst`, or, when it owns a
// resource, moved there and emptied in `src`.
#[no_mangle]
pub unsafe extern "C" fn cb_fn_split(dst: *mut u8, src: *mut u8, ty: u32, file: u32, line: u32) {
    let (offs, owns): (Vec<u64>, Vec<bool>) = {
        let _e = enter();
        let r = rt();
        let mut offs = Vec::new();
        fn_offsets(r, src as usize, ty, 0, &mut offs);
        let owns = offs.iter().map(|&o| fn_owns_resource(r, *(src.add(o as usize) as *mut *mut u8))).collect();
        (offs, owns)
    };
    for (o, own) in offs.into_iter().zip(owns) {
        let s = src.add(o as usize) as *mut *mut u8;
        let d = dst.add(o as usize) as *mut *mut u8;
        if own {
            *s = std::ptr::null_mut();
        } else if !(*s).is_null() {
            *d = cb_fn_copy(*s, file, line);
        }
    }
}

// D-0089: a plain value holding `fn` values ends (a discarded or
// dropped temporary with no object of its own): its closures end.
#[no_mangle]
pub extern "C" fn cb_drop_value(addr: *mut u8, ty: u32) {
    let _e = enter();
    drop_in_place(addr as usize, ty, 0);
}

// D-0089: a call through an emptied `fn` slot.
#[no_mangle]
pub extern "C" fn cb_fn_moved() -> ! {
    fault("diag.stale-binding", NOLOC, 0)
}

#[no_mangle]
pub extern "C" fn cb_copy_datum(dst: *mut u8, src: *const u8, ty: u32) {
    let _e = enter();
    let r = rt();
    let mut offs = Vec::new();
    ref_offsets(r, src as usize, ty, 0, &mut offs);
    // `dst`'s bytes are now the new value's: a reference slot the old
    // value had where the new one has none (`Some(r)` overwritten by
    // `None`) holds nothing any more, and its path one holder fewer.
    let d = dst as usize;
    let size = r.types[ty as usize].size as usize;
    let stale: Vec<(usize, u64)> = r.slots.range(d, d + size).into_iter().filter(|(a, _)| !offs.iter().any(|&o| d + o as usize == *a)).collect();
    for (a, t) in stale {
        r.slots.remove(&a);
        drop_occ(r, t);
    }
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
        // The flight holds it until `cb_recv_ref` (`cb_send_datum`'s rule).
        r.paths.get_mut(&tok).unwrap().occ += 1;
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
        // The flight's hold ends; the statement's scope has it now.
        let p = r.paths.get_mut(&tok).unwrap();
        p.occ = p.occ.saturating_sub(1);
    }
}

// A reference that is a statement's result: re-stamped outward. One that
// no statement owns (formed long before, and held by some slot, such as
// a reference read out of a buffer's cells into a block's local) is
// stamped to the statement around the current one: the local it was
// copied from may end with the block, before anything holds the result,
// and a path nothing holds and no statement owns ends at once.
#[no_mangle]
pub extern "C" fn cb_result_ref(tok: u64) {
    let _e = enter();
    let r = rt();
    let cur = match r.paths.get(&tok) {
        Some(p) => p.scope,
        None => return,
    };
    let i = match cur {
        Some(i) => {
            detach_tok(r, tok);
            i
        }
        None => match top_scope(r) {
            Some(t) => t,
            None => return,
        },
    };
    let parent = scope_below(i);
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
    // statement (`return Some(&m.v);`) must not end with that statement,
    // and the sender's copy no longer holds it -- a slot left in a dead
    // stack temporary counted it as held until something happened to
    // reuse the address. `cb_recv_datum` stores it, held again, at the
    // receiver. Until then the flight itself holds it: a spawned
    // thread's arguments travel while the spawning statement may be
    // ending, and that statement's scope must not find the path unheld
    // and retire it before the callee receives it.
    for (off, t) in offs.iter().zip(toks.iter()) {
        if let Some(t) = t {
            detach_tok(r, *t);
            if r.slots.remove(&(src as usize + *off as usize)).is_none() {
                if let Some(p) = r.paths.get_mut(t) {
                    p.occ += 1;
                }
            }
        }
    }
    ts().inflight.push_back(Inflight::Datum(toks));
}

// A native body returning `Some(r)` for an element reference `r`
// (`cobc`'s native `HashMap::get`/`get_mut`): the element's `[Reclaim]`
// and `[Borrow]` as `cb_elem_borrow` makes them, the path sent as the
// datum's one reference, as `cb_send_datum` sends an `Option<ref<T>>`
// built in the body. No temporary object holds it on the way.
#[no_mangle]
pub extern "C" fn cb_elem_borrow_datum(addr: *mut u8, ty: u32, mode: u32) -> u64 {
    let _e = enter();
    let tok = elem_borrow(addr, ty, mode, false);
    if let Some(p) = rt().paths.get_mut(&tok) {
        p.occ += 1;
    }
    ts().inflight.push_back(Inflight::Datum(vec![Some(tok)]));
    tok
}

// The same body returning `None`: a datum with no reference in flight.
#[no_mangle]
pub extern "C" fn cb_send_datum_none() {
    let _e = enter();
    ts().inflight.push_back(Inflight::Datum(Vec::new()));
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
    let scope = top_scope(r);
    for (off, tok) in offs.into_iter().zip(toks) {
        if let Some(t) = tok {
            // Back in a statement's scope, as `cb_recv_ref` puts one: it
            // ends with that statement unless something holds it then.
            if rt().paths.contains_key(&t) {
                stamp_tok(rt(), t, scope);
            }
            cb_store_ref(unsafe { dst.add(off as usize) }, t);
            // The flight's hold (`cb_send_datum`) ends: the slot holds it.
            drop_occ(rt(), t);
        }
    }
}

// ---- threads (spec/19 §1) ----

extern "C" {
    fn free(p: *mut u8);
    fn calloc(n: usize, size: usize) -> *mut u8;
}

// D-0077: the lowest address this thread's stack may reach, less a
// margin (1: unknown, so unchecked).

#[cfg(target_os = "linux")]
fn find_stack_floor() -> usize {
    extern "C" {
        fn pthread_self() -> usize;
        fn pthread_getattr_np(t: usize, attr: *mut u8) -> i32;
        fn pthread_attr_getstack(attr: *const u8, addr: *mut *mut u8, size: *mut usize) -> i32;
        fn pthread_attr_destroy(attr: *mut u8) -> i32;
    }
    // Room for a `pthread_attr_t` of any glibc or musl layout.
    let mut attr = [0u64; 16];
    let a = attr.as_mut_ptr() as *mut u8;
    let mut addr: *mut u8 = std::ptr::null_mut();
    let mut size: usize = 0;
    unsafe {
        if pthread_getattr_np(pthread_self(), a) != 0 {
            return 1;
        }
        let ok = pthread_attr_getstack(a, &mut addr, &mut size) == 0;
        pthread_attr_destroy(a);
        if !ok || addr.is_null() {
            return 1;
        }
    }
    let margin = (size / 16).clamp(256 << 10, 16 << 20);
    addr as usize + margin
}

#[cfg(not(target_os = "linux"))]
fn find_stack_floor() -> usize {
    1
}

extern "C" {
    fn cb_set_stack_floor(floor: usize);
}

// Gives the generated program this thread's floor (`cb_stack_check`).
fn set_stack_floor() {
    let f = find_stack_floor();
    if f > 1 {
        unsafe { cb_set_stack_floor(f) };
    }
}

// D-0077: the whole program runs on a thread with a large stack, reserved
// rather than committed, so that it recurses as deeply as under `coby`.
// `start` ends the process itself (`cb_terminate_ok`, or a fault).
#[no_mangle]
pub extern "C" fn cb_run_main(start: unsafe extern "C" fn()) -> ! {
    let job = start as usize;
    let spawned = std::thread::Builder::new()
        .stack_size(if cfg!(target_pointer_width = "64") { 256 << 20 } else { 64 << 20 })
        .spawn(move || {
            let f: unsafe extern "C" fn() = unsafe { std::mem::transmute(job) };
            unsafe { f() }
        });
    match spawned {
        Ok(h) => {
            let _ = h.join();
            std::process::exit(101)
        }
        // No thread could be made: run where we are.
        Err(_) => {
            unsafe { start() };
            std::process::exit(0)
        }
    }
}

// D-0077: `cb_stack_check` found the stack exhausted: the fault is at the
// statement making the call.
#[no_mangle]
pub extern "C" fn cb_stack_exhausted() -> ! {
    let (f, l) = get_at();
    fault("diag.stack-exhausted", f, l)
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
    // A program thread recurses as the main one does: a generous stack,
    // reserved rather than committed (D-0077 checks its floor).
    let _ = std::thread::Builder::new().stack_size(if cfg!(target_pointer_width = "64") { 64 << 20 } else { 16 << 20 }).spawn(move || {
        let job = job;
        TID.with(|t| t.set(tid));
        set_stack_floor();
        let own = Box::into_raw(Box::new(TState { stack: Vec::new(), scopes: Vec::new(), inflight: VecDeque::new(), frame_locs: Vec::new(), dloc: (NOLOC, 0), box_draining: false, box_next: None }));
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

// ---- channels' event counts (spec/19 §3, D-0063) ----

fn event_key(id: u64) -> u64 {
    id | 1 << 62
}

// `std::event_op`: 0 makes a count (its id), 1 reads one, 2 waits until
// one differs from `seen`, 3 adds one to it and wakes its waiters, 4
// frees it. Counts change only under the runtime lock, and a waiter
// registers before letting it go (`wait_until`), so no change is missed.
#[no_mangle]
pub extern "C" fn cb_event_op(op: u64, id: u64, seen: u64) -> u64 {
    let _e = enter();
    let r = rt();
    match op {
        0 => {
            r.last_event += 1;
            let id = r.last_event;
            r.events.insert(id, 0);
            id
        }
        1 => r.events.get(&id).copied().unwrap_or(0),
        2 => {
            // `[Channel-Deadlock]`: the main thread waits on a count no
            // other thread is running to change -- checked when the wait
            // starts and whenever it wakes (the last other thread ending
            // during it is seen at the next wake-up, within 50 ms).
            let is_main = me() == 1;
            wait_until(event_key(id), &mut |r: &Rt| r.events.get(&id) != Some(&seen) || (is_main && !r.threads.values().any(|t| t.status == Status::Running)));
            let r = rt();
            if r.events.get(&id) == Some(&seen) {
                fault("diag.channel-deadlock", NOLOC, 0);
            }
            r.events.get(&id).copied().unwrap_or(0)
        }
        3 => {
            let c = r.events.entry(id).or_insert(0);
            *c = c.wrapping_add(1);
            let c = *c;
            signal(event_key(id));
            c
        }
        _ => {
            r.events.remove(&id);
            0
        }
    }
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
