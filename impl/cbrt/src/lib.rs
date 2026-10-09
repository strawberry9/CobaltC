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
// OS thread, and they run in parallel. The records of objects and paths,
// and the reference slots and reclaimed cells, are kept in heaps, one per
// running thread (D-0202): an id carries its heap, and an address lookup
// visits the heaps whose page buckets name that page. Each heap has its
// own lock. A runtime call that stays within its operand's heap and its
// own thread's takes only those heaps' locks (a local entry); one that
// may reach further, or runs code of the program (a destructor), takes
// the runtime lock exclusively and every heap's lock -- reentrant,
// because a destructor the runtime calls re-enters it -- released in
// full while a thread waits (`join`, `lock`, a handle's destructor). The
// threads, mutex owners and channel counts are under a small lock of
// their own (`book`). No lock is taken while only one thread exists, so
// a single-threaded program pays nothing. Generated code between runtime
// calls runs unlocked: any memory two threads can both reach is reached
// through access paths the runtime checks, under the locks, before each
// access. The frame/scope stack, the in-flight queue and the current
// location are the thread's own (`TState`, thread-local).

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
pub const LINK_LIBS: &[&str] = &["kernel32", "ntdll", "userenv", "ws2_32", "dbghelp", "bcrypt"];

use std::cell::{Cell, UnsafeCell};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;
use std::ffi::{c_char, c_int, CStr};

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
type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<IdHasher>>;

// The reference slots by address (`Rt::slots`): a hash of 4 KiB pages,
// each a direct array of its 512 pointer-sized positions. Point
// operations (`cb_store_ref`, `cb_load_ref`) are a hash probe and an
// index; a range is visited page by page, in address order. An ordered
// tree of every slot was a deep descent per operation once a program
// held many references (a `Vec<StringView>` of a long text); sorted
// lists per page shifted on every slot a recursion's frames took.
struct SlotMap {
    // D-0202: the heap this table belongs to (its page buckets count the
    // pages it holds slots on).
    heap: u8,
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
        SlotMap { heap: 0, pages: HashMap::default(), odd: BTreeMap::new(), spare: Vec::new(), last: std::cell::Cell::new((usize::MAX, std::ptr::null_mut())) }
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
            heap_page_count(self.heap as usize, p << SLOT_PAGE, 1, false);
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
            let old = self.odd.insert(a, t);
            if old.is_none() {
                heap_page_count(self.heap as usize, a, 1, true);
            }
            return old;
        }
        let (p, i) = Self::split(a);
        let mut pg = self.page(p);
        if pg.is_null() {
            let b = self.spare.pop().unwrap_or_else(|| Box::new(SlotPage { s: [0; 512], n: 0 }));
            self.pages.insert(p, b);
            heap_page_count(self.heap as usize, p << SLOT_PAGE, 1, true);
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
            let old = self.odd.remove(a);
            if old.is_some() {
                heap_page_count(self.heap as usize, *a, 1, false);
            }
            return old;
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
    // Each slot's path in `[lo, hi)`, unchanged.
    fn each_in(&self, lo: usize, hi: usize, f: &mut impl FnMut(u64)) {
        if lo >= hi || (self.pages.is_empty() && self.odd.is_empty()) {
            return;
        }
        let (p0, p1) = (lo >> SLOT_PAGE, (hi - 1) >> SLOT_PAGE);
        let mut visit = |p: usize, pg: &SlotPage| {
            let base = p << SLOT_PAGE;
            let to = hi.min(base + (1 << SLOT_PAGE));
            let mut a = (lo.max(base) + 7) & !7;
            while a < to {
                let t = pg.s[(a >> 3) & 511];
                if t != 0 {
                    f(t);
                }
                a += 8;
            }
        };
        if p1 - p0 >= self.pages.len() {
            for (&p, pg) in self.pages.iter() {
                if p >= p0 && p <= p1 {
                    visit(p, pg);
                }
            }
        } else {
            for p in p0..=p1 {
                if let Some(pg) = self.pages.get(&p) {
                    visit(p, pg);
                }
            }
        }
        for (_, &t) in self.odd.range(lo..hi) {
            f(t);
        }
    }
    // Whether any slot lies in `[lo, hi)`.
    fn any_in(&self, lo: usize, hi: usize) -> bool {
        if lo >= hi || (self.pages.is_empty() && self.odd.is_empty()) {
            return false;
        }
        let (p0, p1) = (lo >> SLOT_PAGE, (hi - 1) >> SLOT_PAGE);
        let hit = |p: usize, pg: &SlotPage| {
            let base = p << SLOT_PAGE;
            let to = hi.min(base + (1 << SLOT_PAGE));
            let mut a = (lo.max(base) + 7) & !7;
            while a < to {
                if pg.s[(a >> 3) & 511] != 0 {
                    return true;
                }
                a += 8;
            }
            false
        };
        let found = if p1 - p0 >= self.pages.len() {
            // Fewer pages held than the range spans: look at those.
            self.pages.iter().any(|(&p, pg)| p >= p0 && p <= p1 && hit(p, pg))
        } else {
            (p0..=p1).any(|p| self.pages.get(&p).map_or(false, |pg| hit(p, pg)))
        };
        found || self.odd.range(lo..hi).next().is_some()
    }
    // The slots in `[lo, hi)`, in address order; `take`: removed as well.
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
                    heap_page_count(self.heap as usize, *a, 1, false);
                }
            }
            out.extend(odd);
            out[start..].sort_unstable_by_key(|e| e.0);
        }
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
// D-0202: an id also names the heap its entry lives in, in bits 55 to 62
// (`HEAP_SHIFT`; at most `MAX_HEAPS`); the generation has the 23 bits
// below them. The top bit stays clear: a frame marks a held path's token
// with it (`HOLD`), which a heap index reaching into it once made an
// object's id read as. D-0204: 256 heaps rather than 128, so a server's
// ten thousand threads share each heap with half as many others.
const HEAP_SHIFT: u32 = 55;
const MAX_HEAPS: usize = 256;
const GEN_MASK: u32 = 0x007f_ffff;

struct Arena<T> {
    slots: Vec<(u32, Option<T>)>,
    free: Vec<u32>,
    // This arena's heap index (D-0202), part of every id it gives.
    heap: u8,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena { slots: Vec::new(), free: Vec::new(), heap: 0 }
    }
}

impl<T> Arena<T> {
    #[inline]
    fn split(k: u64) -> (usize, u32) {
        (((k & 0xffff_ffff) as usize).wrapping_sub(1), ((k >> 32) as u32) & GEN_MASK)
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
        ((self.heap as u64) << HEAP_SHIFT) | ((self.slots[i].0 as u64) << 32) | (i as u64 + 1)
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
    fn remove(&mut self, k: &u64) -> Option<T> {
        let (i, g) = Self::split(*k);
        match self.slots.get_mut(i) {
            Some((sg, v @ Some(_))) if *sg == g => {
                let out = v.take();
                *sg = (sg.wrapping_add(1) & GEN_MASK).max(1);
                self.free.push(i as u32);
                out
            }
            _ => None,
        }
    }
}

// D-0202: one value per heap, heap 0's held inline: a program with one
// thread only ever reaches that one, as directly as before heaps existed.
// The rest sit in a vector, room for every heap reserved so it never
// moves while threads use their parts of it.
struct Heaps<T> {
    h0: T,
    rest: Vec<T>,
}

impl<T> Heaps<T> {
    fn new(h0: T) -> Self {
        Heaps { h0, rest: Vec::with_capacity(MAX_HEAPS - 1) }
    }
    #[inline(always)]
    fn len(&self) -> usize {
        1 + self.rest.len()
    }
    // Only heap 0 exists: no thread was ever spawned.
    #[inline(always)]
    fn one(&self) -> bool {
        self.rest.is_empty()
    }
    #[inline(always)]
    fn get(&self, h: usize) -> Option<&T> {
        if h == 0 {
            Some(&self.h0)
        } else {
            self.rest.get(h - 1)
        }
    }
    #[inline(always)]
    fn get_mut(&mut self, h: usize) -> Option<&mut T> {
        if h == 0 {
            Some(&mut self.h0)
        } else {
            self.rest.get_mut(h - 1)
        }
    }
    fn push(&mut self, v: T) {
        self.rest.push(v);
    }
    fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.h0).chain(self.rest.iter())
    }
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        std::iter::once(&mut self.h0).chain(self.rest.iter_mut())
    }
}

impl<T> std::ops::Index<usize> for Heaps<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, h: usize) -> &T {
        if h == 0 {
            &self.h0
        } else {
            &self.rest[h - 1]
        }
    }
}

impl<T> std::ops::IndexMut<usize> for Heaps<T> {
    #[inline(always)]
    fn index_mut(&mut self, h: usize) -> &mut T {
        if h == 0 {
            &mut self.h0
        } else {
            &mut self.rest[h - 1]
        }
    }
}

// D-0202: the arenas of every heap, one table to the rest of the runtime:
// an id's top bits pick the heap (`HEAP_SHIFT`). `reserve` makes an id in
// the current heap (`cur`); every other operation goes by the id.
struct HeapArena<T> {
    heaps: Heaps<Arena<T>>,
}

impl<T> Default for HeapArena<T> {
    fn default() -> Self {
        HeapArena { heaps: Heaps::new(Arena::default()) }
    }
}

impl<T> HeapArena<T> {
    #[inline]
    fn heap_of(k: u64) -> usize {
        (k >> HEAP_SHIFT) as usize
    }
    #[inline]
    fn reserve(&mut self) -> u64 {
        self.heaps[cur_heap()].reserve()
    }
    // An id in heap `h` (a path in its object's heap, D-0202).
    #[inline]
    fn reserve_in(&mut self, h: usize) -> u64 {
        debug_assert!(holds_heap(h), "cbrt: heap {} used without its lock", h);
        self.heaps[h].reserve()
    }
    // Heap `h` exists (`Rt::add_heap`).
    fn ensure(&mut self, h: usize) {
        while self.heaps.len() <= h {
            let n = self.heaps.len() as u8;
            self.heaps.push(Arena { slots: Vec::new(), free: Vec::new(), heap: n });
        }
    }
    // Whether heap `h` holds no entry.
    fn heap_empty(&self, h: usize) -> bool {
        self.heaps.get(h).map_or(true, |a| a.slots.iter().all(|(_, v)| v.is_none()))
    }
    #[inline]
    fn insert(&mut self, k: u64, v: T) {
        debug_assert!(holds_heap(Self::heap_of(k)), "cbrt: heap {} used without its lock", Self::heap_of(k));
        self.heaps[Self::heap_of(k)].insert(k, v)
    }
    #[inline]
    fn get(&self, k: &u64) -> Option<&T> {
        // No id (0) names no entry, in no heap: nothing to lock or touch.
        if *k == 0 {
            return None;
        }
        debug_assert!(holds_heap(Self::heap_of(*k)), "cbrt: heap {} used without its lock", Self::heap_of(*k));
        self.heaps.get(Self::heap_of(*k))?.get(k)
    }
    #[inline]
    fn get_mut(&mut self, k: &u64) -> Option<&mut T> {
        // No id (0) names no entry, in no heap: nothing to lock or touch.
        if *k == 0 {
            return None;
        }
        debug_assert!(holds_heap(Self::heap_of(*k)), "cbrt: heap {} used without its lock", Self::heap_of(*k));
        self.heaps.get_mut(Self::heap_of(*k))?.get_mut(k)
    }
    #[inline]
    fn contains_key(&self, k: &u64) -> bool {
        self.get(k).is_some()
    }
    #[inline]
    fn remove(&mut self, k: &u64) -> Option<T> {
        // No id (0) names no entry, in no heap: nothing to lock or touch.
        if *k == 0 {
            return None;
        }
        debug_assert!(holds_heap(Self::heap_of(*k)), "cbrt: heap {} used without its lock", Self::heap_of(*k));
        self.heaps.get_mut(Self::heap_of(*k))?.remove(k)
    }
    // Every live entry, in every heap.
    fn values(&self) -> impl Iterator<Item = &T> {
        self.heaps.iter().flat_map(|a| a.slots.iter().filter_map(|(_, v)| v.as_ref()))
    }
}

impl<T> std::ops::Index<&u64> for HeapArena<T> {
    type Output = T;
    fn index(&self, k: &u64) -> &T {
        self.get(k).expect("cbrt: no entry under this id")
    }
}

impl<T> std::ops::IndexMut<&u64> for HeapArena<T> {
    fn index_mut(&mut self, k: &u64) -> &mut T {
        self.get_mut(k).expect("cbrt: no entry under this id")
    }
}

// D-0202: this thread's heap (`HEAP`): where new records go.
#[inline(always)]
fn cur_heap() -> usize {
    HEAP.with(|c| c.get()) as usize
}

// D-0202: one value per heap, the current thread's reached through the
// wrapper (the runtime's reusable lists, so threads never share one).
struct PerHeap<T: Default> {
    v: Heaps<T>,
}

impl<T: Default> Default for PerHeap<T> {
    fn default() -> Self {
        PerHeap { v: Heaps::new(T::default()) }
    }
}

impl<T: Default> PerHeap<T> {
    fn ensure(&mut self, h: usize) {
        while self.v.len() <= h {
            self.v.push(T::default());
        }
    }
}

impl<T: Default> std::ops::Deref for PerHeap<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        &self.v[cur_heap()]
    }
}

impl<T: Default> std::ops::DerefMut for PerHeap<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.v[cur_heap()]
    }
}

// D-0202: the reference slots of every heap. A slot is registered in the
// heap of the thread that stores it (`cur`) and found in whichever heap
// holds it: the current one first, then any other with a page there.
struct HeapSlots {
    heaps: Heaps<SlotMap>,
}

impl Default for HeapSlots {
    fn default() -> Self {
        HeapSlots { heaps: Heaps::new(SlotMap::default()) }
    }
}

impl HeapSlots {
    fn ensure(&mut self, h: usize) {
        while self.heaps.len() <= h {
            let n = self.heaps.len() as u8;
            self.heaps.push(SlotMap { heap: n, ..SlotMap::default() });
        }
    }
    fn heap_empty(&self, h: usize) -> bool {
        self.heaps.get(h).map_or(true, |m| m.pages.is_empty() && m.odd.is_empty())
    }
    // The heap holding a slot at `a`, if any.
    #[inline]
    fn holder(&self, a: usize) -> Option<usize> {
        let c = cur_heap();
        if self.heaps[c].get(&a).is_some() {
            return Some(c);
        }
        if self.heaps.one() {
            return None;
        }
        (0..self.heaps.len()).find(|&h| h != c && holds_heap(h) && self.heaps[h].get(&a).is_some())
    }
    #[inline]
    fn get(&self, a: &usize) -> Option<&u64> {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].get(a);
        }
        let h = self.holder(*a)?;
        self.heaps[h].get(a)
    }
    #[inline]
    fn insert(&mut self, a: usize, t: u64) -> Option<u64> {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].insert(a, t);
        }
        let c = cur_heap();
        match self.holder(a) {
            Some(h) if h != c => {
                let old = self.heaps[h].remove(&a);
                self.heaps[c].insert(a, t);
                old
            }
            _ => self.heaps[c].insert(a, t),
        }
    }
    #[inline]
    fn remove(&mut self, a: &usize) -> Option<u64> {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].remove(a);
        }
        let h = self.holder(*a)?;
        self.heaps[h].remove(a)
    }
    // Each slot's path in `[lo, hi)` of the heaps this entry holds,
    // unchanged (`Plan::forget`).
    fn each_in(&self, lo: usize, hi: usize, mut f: impl FnMut(u64)) {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].each_in(lo, hi, &mut f);
        }
        for h in 0..self.heaps.len() {
            if holds_heap(h) {
                self.heaps[h].each_in(lo, hi, &mut f);
            }
        }
    }
    // Whether a heap this entry holds has a slot in `[lo, hi)`.
    fn any_in(&self, lo: usize, hi: usize) -> bool {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].any_in(lo, hi);
        }
        self.heaps.iter().enumerate().any(|(h, m)| holds_heap(h) && m.any_in(lo, hi))
    }
    // The slots in `[lo, hi)` of every heap, in address order.
    #[inline]
    fn scan_into(&mut self, lo: usize, hi: usize, take: bool, out: &mut Vec<(usize, u64)>) {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].scan_into(lo, hi, take, out);
        }
        let start = out.len();
        let mut parts = 0;
        for (h, m) in self.heaps.iter_mut().enumerate() {
            if !holds_heap(h) {
                continue;
            }
            let before = out.len();
            m.scan_into(lo, hi, take, out);
            if out.len() > before {
                parts += 1;
            }
        }
        if parts > 1 {
            out[start..].sort_unstable_by_key(|e| e.0);
        }
    }
    fn range(&mut self, lo: usize, hi: usize) -> Vec<(usize, u64)> {
        let mut out = Vec::new();
        self.scan_into(lo, hi, false, &mut out);
        out
    }
}

// D-0202: the reclaimed objects of every heap, registered in the current
// heap and found in any.
struct HeapReclaimed {
    heaps: Heaps<Reclaimed>,
}

impl Default for HeapReclaimed {
    fn default() -> Self {
        HeapReclaimed { heaps: Heaps::new(Reclaimed::default()) }
    }
}

impl HeapReclaimed {
    fn ensure(&mut self, h: usize) {
        while self.heaps.len() <= h {
            let n = self.heaps.len() as u8;
            self.heaps.push(Reclaimed { heap: n, ..Reclaimed::default() });
        }
    }
    fn heap_empty(&self, h: usize) -> bool {
        self.heaps.get(h).map_or(true, |m| m.entries() == 0)
    }
    // `cb_reclaimed_entries` is kept by `Reclaimed::pc`, entry by entry
    // (summing every heap's count would read heaps this entry may not hold).
    #[inline(always)]
    fn publish(&self) {}
    fn holder(&self, a: usize) -> Option<usize> {
        let c = cur_heap();
        if self.heaps[c].get(&a).is_some() {
            return Some(c);
        }
        (0..self.heaps.len()).find(|&h| h != c && holds_heap(h) && self.heaps[h].get(&a).is_some())
    }
    #[inline]
    fn get(&self, a: &usize) -> Option<&u64> {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].get(a);
        }
        let h = self.holder(*a)?;
        self.heaps[h].get(a)
    }
    // Registered in the current heap, any entry at `a` elsewhere removed
    // first (as `Reclaimed::insert` replaces one).
    fn insert(&mut self, a: usize, obj: u64, size: u64) {
        if let Some(h) = sole_heap(self.heaps.one()) {
            self.heaps[h].insert(a, obj, size);
            return self.publish();
        }
        let c = cur_heap();
        if let Some(h) = self.holder(a).filter(|&h| h != c) {
            self.heaps[h].remove(&a);
        }
        self.heaps[c].insert(a, obj, size);
        self.publish();
    }
    fn insert_ephemeral(&mut self, a: usize, obj: u64, size: u64) {
        let c = cur_heap();
        self.heaps[c].insert_ephemeral(a, obj, size);
        self.publish();
    }
    fn remove_if(&mut self, a: usize, obj: u64, ephemeral: bool) {
        if let Some(h) = sole_heap(self.heaps.one()) {
            self.heaps[h].remove_if(a, obj, ephemeral);
        } else {
            for (h, m) in self.heaps.iter_mut().enumerate() {
                if holds_heap(h) {
                    m.remove_if(a, obj, ephemeral);
                }
            }
        }
        self.publish();
    }
    #[inline]
    fn any_in(&self, lo: usize, hi: usize) -> bool {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].any_in(lo, hi);
        }
        self.heaps.iter().enumerate().any(|(h, m)| holds_heap(h) && m.any_in(lo, hi))
    }
    fn range(&self, lo: usize, hi: usize) -> Vec<(usize, u64)> {
        if let Some(h) = sole_heap(self.heaps.one()) {
            return self.heaps[h].range(lo, hi);
        }
        let mut v: Vec<(usize, u64)> = self.heaps.iter().enumerate().filter(|(h, _)| holds_heap(*h)).flat_map(|(_, m)| m.range(lo, hi)).collect();
        v.sort_unstable();
        v
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
    // D-0202: the heap this table belongs to (its page buckets).
    heap: u8,
    ordered: BTreeMap<usize, u64>,
    // `ordered` again, by hash: every element or `Box` access looks its
    // address up, and a tree of every reclaimed object (a tree of
    // `Box`es) was a deep descent each time.
    ordered_ix: HashMap<usize, u64>,
    ephemeral: BTreeMap<usize, u64>,
    // Each entry's extent in bytes (its object's size when entered), by
    // address, one map per kind: `cb_reclaimed_pages` counts entries by
    // the pages their extents overlap.
    ordered_sz: HashMap<usize, u64>,
    ephemeral_sz: HashMap<usize, u64>,
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

// D-0202: what `cb_reclaimed_entries` reads while threads run. Kept entry
// by entry, its one cache line moved between cores at every element
// borrow of every thread; held at a constant, it sends the generated code
// on to the page buckets, which are exact and spread by page (a heap
// that took a share of the count only at its first entry did worse: the
// runtime's own early exits then searched). The runtime's checks ask
// `quiet_cells`. `go_multi` sets it, and `go_solo` counts the entries
// again.
const THREADS_ENTRIES: usize = 1 << 40;

// Whether no object lies over `[a, a + n)` by the global counts: no entry
// at all, or none on the buckets of its pages (a long run is not asked).
#[inline(always)]
fn quiet_cells(a: usize, n: usize) -> bool {
    cb_reclaimed_entries.load(Ordering::Relaxed) == 0 || quiet_pages(a, n)
}

#[inline(never)]
fn quiet_pages(a: usize, n: usize) -> bool {
    let first = a >> 12;
    let last = (a + n.max(1) - 1) >> 12;
    if last - first >= 64 {
        return false;
    }
    (first..=last).all(|p| cb_reclaimed_pages[p & (RECLAIMED_PAGE_BUCKETS - 1)].load(Ordering::Relaxed) == 0)
}

// The same count by page: bucket `(a >> 12) & 4095` counts the entries
// whose extents overlap a page that maps to it. An element whose cells
// lie on pages whose buckets are 0 has no object over it, so the
// generated code's `cb_elem_access` (`cbrt.h`) need not ask, wherever
// else in the program objects lie over raw cells (every element of a
// `Vec` of views, an element a held reference was taken to). A bucket
// shared by several pages only sends an access to the slow path, which
// answers exactly. Read without the lock, as `cb_reclaimed_entries` is.
pub const RECLAIMED_PAGE_BUCKETS: usize = 4096;
#[allow(clippy::declare_interior_mutable_const)]
const ZERO_BUCKET: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[no_mangle]
#[allow(non_upper_case_globals)]
pub static cb_reclaimed_pages: [std::sync::atomic::AtomicU32; RECLAIMED_PAGE_BUCKETS] = [ZERO_BUCKET; RECLAIMED_PAGE_BUCKETS];

// D-0202: each heap's own page buckets, as `cb_reclaimed_pages` but
// counting both its reclaimed entries and its pages of reference slots.
// An entry that works by address (releasing cells, an element's object)
// locks only the heaps whose buckets for those pages are not 0. Read
// without their locks, which is enough for the same reason as
// `cb_reclaimed_pages`: an entry registered by another thread over cells
// this thread is using would be a race the program's checks exclude.
#[allow(clippy::declare_interior_mutable_const)]
const NO_PAGES: std::sync::atomic::AtomicPtr<std::sync::atomic::AtomicU32> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
static HEAP_PAGES: [std::sync::atomic::AtomicPtr<std::sync::atomic::AtomicU32>; 256] = [NO_PAGES; 256];

fn heap_pages(h: usize) -> &'static [std::sync::atomic::AtomicU32] {
    let p = HEAP_PAGES[h].load(Ordering::Acquire);
    if !p.is_null() {
        return unsafe { std::slice::from_raw_parts(p, RECLAIMED_PAGE_BUCKETS) };
    }
    let v: Vec<std::sync::atomic::AtomicU32> = (0..RECLAIMED_PAGE_BUCKETS).map(|_| std::sync::atomic::AtomicU32::new(0)).collect();
    let fresh = Box::leak(v.into_boxed_slice()).as_mut_ptr();
    match HEAP_PAGES[h].compare_exchange(std::ptr::null_mut(), fresh, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => unsafe { std::slice::from_raw_parts(fresh, RECLAIMED_PAGE_BUCKETS) },
        Err(p) => unsafe { std::slice::from_raw_parts(p, RECLAIMED_PAGE_BUCKETS) },
    }
}

// How many extents each heap has registered in its buckets: 0 means none
// of its buckets is counting anything (`addr_heaps`).
#[allow(clippy::declare_interior_mutable_const)]
const ZERO_TOTAL: AtomicU64 = AtomicU64::new(0);
static HEAP_TOTAL: [AtomicU64; MAX_HEAPS] = [ZERO_TOTAL; MAX_HEAPS];

// Adds one to (or takes one from) heap `h`'s bucket of every page
// `[a, a + size)` overlaps (each bucket once).
// A page's bucket in a heap's table: hashed, not its low bits. Every
// thread's stack sits at the same offset in an equally sized region, so by
// low bits the stacks of all threads shared their buckets, and every
// operation on a slot on the stack reckoned every heap.
#[inline]
fn heap_bucket(page: usize) -> usize {
    ((page as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 52) as usize
}

// Kept only while threads run: `go_multi` rebuilds every heap's from the
// tables when locking turns on. (Inline, a load and a branch; the work
// out of line, off the one-thread paths.)
#[inline(always)]
fn heap_page_count(h: usize, a: usize, size: u64, up: bool) {
    if multi() {
        heap_page_count_multi(h, a, size, up);
    }
}

#[cold]
#[inline(never)]
fn heap_page_count_multi(h: usize, a: usize, size: u64, up: bool) {
    if up {
        HEAP_TOTAL[h].fetch_add(1, Ordering::Relaxed);
    } else {
        HEAP_TOTAL[h].fetch_sub(1, Ordering::Relaxed);
    }
    let t = heap_pages(h);
    let first = a >> 12;
    let last = (a + (size.max(1) as usize) - 1) >> 12;
    let n = (last - first + 1).min(RECLAIMED_PAGE_BUCKETS);
    let (w, bit) = (h >> 6, 1u64 << (h & 63));
    for p in first..first + n {
        let i = heap_bucket(p);
        // A bucket's count changes only under its heap's lock, so its
        // passing 0 to 1 and back is this heap's bit in `PAGE_HEAPS`.
        if up {
            if t[i].fetch_add(1, Ordering::Relaxed) == 0 {
                PAGE_HEAPS[i][w].fetch_or(bit, Ordering::Release);
            }
        } else if t[i].fetch_sub(1, Ordering::Relaxed) == 1 {
            PAGE_HEAPS[i][w].fetch_and(!bit, Ordering::Release);
        }
    }
}

// For each bucket, the heaps whose buckets count something there: what
// `addr_heaps` reads, two words a page, however many heaps there are.
#[allow(clippy::declare_interior_mutable_const)]
const NO_HEAPS: [AtomicU64; 4] = [ZERO_TOTAL, ZERO_TOTAL, ZERO_TOTAL, ZERO_TOTAL];
static PAGE_HEAPS: [[AtomicU64; 4]; RECLAIMED_PAGE_BUCKETS] = [NO_HEAPS; RECLAIMED_PAGE_BUCKETS];

// Every heap's page buckets, counted afresh from its tables.
fn rebuild_heap_pages(r: &Rt) {
    for b in PAGE_HEAPS.iter() {
        for w in b.iter() {
            w.store(0, Ordering::Relaxed);
        }
    }
    for h in 0..r.slots.heaps.len().max(r.reclaimed.heaps.len()) {
        let t = heap_pages(h);
        for b in t {
            b.store(0, Ordering::Relaxed);
        }
        HEAP_TOTAL[h].store(0, Ordering::Relaxed);
        let add = |a: usize, size: u64| {
            HEAP_TOTAL[h].fetch_add(1, Ordering::Relaxed);
            let first = a >> 12;
            let last = (a + (size.max(1) as usize) - 1) >> 12;
            for p in first..first + (last - first + 1).min(RECLAIMED_PAGE_BUCKETS) {
                let i = heap_bucket(p);
                t[i].fetch_add(1, Ordering::Relaxed);
                PAGE_HEAPS[i][h >> 6].fetch_or(1u64 << (h & 63), Ordering::Relaxed);
            }
        };
        if let Some(m) = r.slots.heaps.get(h) {
            for &p in m.pages.keys() {
                add(p << SLOT_PAGE, 1);
            }
            for &a in m.odd.keys() {
                add(a, 1);
            }
        }
        if let Some(m) = r.reclaimed.heaps.get(h) {
            for (&a, &size) in m.ordered_sz.iter().chain(m.ephemeral_sz.iter()) {
                add(a, size);
            }
        }
    }
}

// The heaps other than this thread's whose buckets say they may hold a
// slot or a reclaimed entry on a page of `[lo, hi)`, with this thread's.
fn addr_heaps(lo: usize, hi: usize) -> HeapSet {
    let mut set = HeapSet::EMPTY;
    let c = cur_heap();
    set.add(c);
    let first = lo >> 12;
    let last = (hi.max(lo + 1) - 1) >> 12;
    let n = (last - first + 1).min(RECLAIMED_PAGE_BUCKETS);
    // A long run of pages: every heap with anything registered, without
    // asking each page (more heaps locked, never too few).
    if n > 64 {
        for h in 0..HEAPS_N.load(Ordering::Acquire) {
            if HEAP_TOTAL[h].load(Ordering::Relaxed) != 0 {
                set.add(h);
            }
        }
        return set;
    }
    let words = (HEAPS_N.load(Ordering::Acquire) + 63) / 64;
    for q in first..first + n {
        let b = &PAGE_HEAPS[heap_bucket(q)];
        for w in 0..words {
            set.0[w] |= b[w].load(Ordering::Acquire);
        }
    }
    set
}

// Adds `d` (+1 or -1) to the bucket of every page `[a, a + size)` overlaps
// (each bucket once, however many of its pages the extent covers).
fn page_count(a: usize, size: u64, up: bool) {
    let first = a >> 12;
    let last = (a + (size.max(1) as usize) - 1) >> 12;
    let n = (last - first + 1).min(RECLAIMED_PAGE_BUCKETS);
    for p in first..first + n {
        let b = &cb_reclaimed_pages[p & (RECLAIMED_PAGE_BUCKETS - 1)];
        if up {
            b.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            b.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl Reclaimed {
    // The global page buckets and this heap's, and the entry count: one
    // call per entry added or removed. While threads run the count is not
    // kept (`THREADS_ENTRIES`).
    fn pc(heap: u8, a: usize, size: u64, up: bool) {
        page_count(a, size, up);
        if multi() {
            heap_page_count(heap as usize, a, size, up);
            return;
        }
        if up {
            cb_reclaimed_entries.fetch_add(1, Ordering::Relaxed);
        } else {
            cb_reclaimed_entries.fetch_sub(1, Ordering::Relaxed);
        }
    }
    // D-0202: the entries this heap holds; `HeapReclaimed` publishes the
    // sum over heaps as `cb_reclaimed_entries`.
    fn count(&self) {}
    fn entries(&self) -> usize {
        self.ordered.len() + self.ephemeral.len()
    }
    fn get(&self, a: &usize) -> Option<&u64> {
        self.ephemeral.get(a).or_else(|| self.ordered_ix.get(a))
    }
    // `size`: the object's, in bytes.
    fn insert(&mut self, a: usize, obj: u64, size: u64) {
        if self.ephemeral.remove(&a).is_some() {
            Self::pc(self.heap, a, self.ephemeral_sz.remove(&a).unwrap_or(0), false);
        }
        if self.ordered.insert(a, obj).is_some() {
            Self::pc(self.heap, a, self.ordered_sz.remove(&a).unwrap_or(0), false);
        }
        self.ordered_ix.insert(a, obj);
        self.ordered_sz.insert(a, size);
        Self::pc(self.heap, a, size, true);
        self.count();
    }
    // Only where no live object lies at `a` (`cb_elem_borrow`); an ordered
    // entry left there names an ended object, which every reader skips.
    fn insert_ephemeral(&mut self, a: usize, obj: u64, size: u64) {
        if self.ephemeral.insert(a, obj).is_some() {
            Self::pc(self.heap, a, self.ephemeral_sz.remove(&a).unwrap_or(0), false);
        }
        self.ephemeral_sz.insert(a, size);
        Self::pc(self.heap, a, size, true);
        self.count();
    }
    fn remove(&mut self, a: &usize) {
        if self.ephemeral.remove(a).is_some() {
            Self::pc(self.heap, *a, self.ephemeral_sz.remove(a).unwrap_or(0), false);
        } else if self.ordered.remove(a).is_some() {
            self.ordered_ix.remove(a);
            Self::pc(self.heap, *a, self.ordered_sz.remove(a).unwrap_or(0), false);
        }
        self.count();
    }
    // Removes the entry at `a` if it is `obj`.
    fn remove_if(&mut self, a: usize, obj: u64, ephemeral: bool) {
        if ephemeral {
            if let std::collections::btree_map::Entry::Occupied(e) = self.ephemeral.entry(a) {
                if *e.get() == obj {
                    e.remove();
                    Self::pc(self.heap, a, self.ephemeral_sz.remove(&a).unwrap_or(0), false);
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
// made absolute (`[…, Range(l, h), Index(r)]` is `[…, Index(l + r)]`),
// into a stack buffer: a path of a few steps (every path a
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

// `Norm`'s folding as an iterator, for comparing a stored path without
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

// Whether two paths overlap, `a` normalized, `b` as stored: one a prefix
// of the other, step by step, a range meeting an index inside it or a
// range it intersects. Without allocating.
fn ranged_overlap_with(a: &[Proj], b: &[Proj]) -> bool {
    a.iter().copied().zip(NormIter { s: b, i: 0 }).all(|(x, y)| match (x, y) {
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
    // A datum's reference tokens, one per reference slot of its value, in
    // order; 0 where a slot held none. From the runtime's pool.
    Datum(Vec<u64>),
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
    // D-0196: statement scopes pushed and not yet made (their locations),
    // above every entry of `stack`. Most statements record nothing in
    // theirs, so a scope is made only when the runtime next looks at this
    // thread's state (`ts`), and a pending one is popped for nothing.
    pend: Vec<(u32, u32)>,
    // D-0204: a contained fault is unwinding this thread; the jump buffer
    // of its trampoline (`cb_thread_jmp`), where the unwind ends; and the
    // failure it recorded, for `cb_thread_failed`.
    cunwind: bool,
    // The buffer's address (an address, not a pointer: `TState` derives
    // `Default`, which Rust 1.77 has no raw pointer for); 0 for none.
    jmp: usize,
    failed: Option<u64>,
}

// `state.threads`: a spawned thread's argument block (its result at
// offset 0), and whether the result is still there to be claimed.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Status {
    Running,
    Done,
    // D-0204: the thread's fault was contained; its failure's index in
    // `Book::failures`.
    Failed(u64),
    Taken,
}

// D-0204: what a waiting thread waits for (`wait_until`), as much as it
// takes to tell whether the wait still holds it.
#[derive(Clone, PartialEq, Eq)]
enum Wait {
    Join(u64),
    Lock(usize),
    Event(u64, u64),
    Timer,
}

// A contained failure: the fault as raised (id and message, location),
// and its report.
struct Failure {
    raw: String,
    file: u32,
    line: u32,
    text: String,
}

struct ThreadRec {
    status: Status,
    env: usize,
    env_size: u64,
    res_ty: u32,
    res_obj: u64,
}

// D-0202: the threads, the mutexes' holders and the channels' event
// counts, under a lock of their own (`book`): a channel's operations
// touch nothing else, so they no longer stop every thread. An exclusive
// entry takes it inside its own hold; a holder of it takes no other lock.
#[derive(Default)]
struct Book {
    threads: HashMap<u64, ThreadRec>,
    locks: HashMap<usize, u64>, // a mutex's address -> the thread holding it (`state.sync`)
    events: HashMap<u64, u64>,
    last_event: u64,
    // D-0204: threads asked to stop; threads lent an exclusive reference
    // (never cancelled, never contained); contained failures; poisoned
    // mutexes; what each waiting thread waits for; threads in a wait
    // outside the runtime (a socket, input, a child); threads found
    // deadlocked, to fault as they wake.
    cancel: HashSet<u64>,
    lent: HashSet<u64>,
    failures: Vec<Failure>,
    poisoned: HashSet<usize>,
    waits: HashMap<u64, Wait>,
    deadlocked: HashSet<u64>,
    // Each running program thread's OS thread (`netio::os_thread`), for
    // `cancel` to interrupt a socket wait; removed as the thread ends.
    os: HashMap<u64, u64>,
    // The key each waiting thread waits on, for `cancel` to wake it.
    wake: HashMap<u64, u64>,
}

impl Book {
    // Whether a waiting thread's wait still holds it (one woken but not
    // yet run is not stuck).
    fn still_waits(&self, w: &Wait) -> bool {
        match w {
            Wait::Join(t) => matches!(self.threads.get(t), Some(t) if t.status == Status::Running),
            Wait::Lock(key) => self.locks.contains_key(key),
            Wait::Event(id, seen) => self.events.get(id) == Some(seen),
            Wait::Timer => false,
        }
    }

    // Whether `me` waiting for `kind` would close a cycle of joins and
    // locks.
    fn closes_cycle(&self, me: u64, kind: &Wait) -> bool {
        let next = |k: &Wait| -> Option<u64> {
            match k {
                Wait::Join(t) => Some(*t),
                Wait::Lock(key) => self.locks.get(key).copied(),
                _ => None,
            }
        };
        let mut at = next(kind);
        let mut steps = 0;
        while let Some(t) = at {
            if t == me {
                return true;
            }
            // A thread asked to stop ends at its next look: no cycle
            // through it.
            if self.cancel.contains(&t) && !self.lent.contains(&t) {
                return false;
            }
            steps += 1;
            if steps > self.waits.len() + 1 {
                return false;
            }
            at = self.waits.get(&t).and_then(|k| next(k));
        }
        false
    }

    // When every live thread waits and no wait can end: one member of a
    // cycle of joins and locks is deadlocked, if there is a cycle; else
    // every one of them is (`coby`'s `find_deadlock`).
    fn find_deadlock(&mut self) {
        if IN_IO.load(Ordering::Acquire) != 0 {
            return;
        }
        let mut live: Vec<u64> = vec![1];
        live.extend(self.threads.iter().filter(|(_, t)| t.status == Status::Running).map(|(k, _)| *k));
        if !live.iter().all(|t| self.waits.get(t).map_or(false, |w| self.still_waits(w)) && !(self.cancel.contains(t) && !self.lent.contains(t))) {
            return;
        }
        let mut waiting: Vec<u64> = self.waits.keys().copied().collect();
        waiting.sort_unstable();
        for &t in waiting.iter().rev() {
            if let Some(k) = self.waits.get(&t).cloned() {
                if self.closes_cycle(t, &k) {
                    self.deadlocked.insert(t);
                    return;
                }
            }
        }
        // No cycle: the ends of the chains of waits -- the threads waiting
        // on a channel, not on another thread -- fault; the threads waiting
        // on them then learn of it as their joins and locks end (a failure
        // raised again, a mutex poisoned). One outcome, whatever the timing.
        let ends: Vec<u64> = live.iter().copied().filter(|t| !matches!(self.waits.get(t), Some(Wait::Join(_)) | Some(Wait::Lock(_)))).collect();
        for t in if ends.is_empty() { live } else { ends } {
            self.deadlocked.insert(t);
        }
    }
}

static BOOK: Mutex<Option<Book>> = Mutex::new(None);

struct BookGuard(std::sync::MutexGuard<'static, Option<Book>>);

impl std::ops::Deref for BookGuard {
    type Target = Book;
    fn deref(&self) -> &Book {
        self.0.as_ref().expect("cbrt: book used before cb_init")
    }
}

impl std::ops::DerefMut for BookGuard {
    fn deref_mut(&mut self) -> &mut Book {
        self.0.as_mut().expect("cbrt: book used before cb_init")
    }
}

fn book() -> BookGuard {
    let mut g = BOOK.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(Book::default());
    }
    BookGuard(g)
}

// Type ids are stable names, not positions: the compiler gives each type
// a 32-bit hash of its mangled name (the same in every program, so a
// compiled function's object code can be cached across programs), and
// `cb_init` gets the ids beside the descriptors. This table maps an id to
// the descriptor's position: open addressing, a power-of-two size, the
// empty slot marked NOTYPE (never a valid id). Each slot holds the id and
// the position together: a lookup is on every destruction's path.
struct TypeMap {
    slots: Vec<(u32, u32)>,
    mask: usize,
}

impl TypeMap {
    fn build(ids: &[u32]) -> TypeMap {
        let mut cap = 16;
        while cap < ids.len() * 2 {
            cap *= 2;
        }
        let mut m = TypeMap { slots: vec![(NOTYPE, 0); cap], mask: cap - 1 };
        for (i, &id) in ids.iter().enumerate() {
            let mut at = (id.wrapping_mul(0x9E37_79B1) as usize) & m.mask;
            loop {
                if m.slots[at].0 == NOTYPE {
                    m.slots[at] = (id, i as u32);
                    break;
                }
                if m.slots[at].0 == id {
                    panic!("cbrt: two type descriptors share the id {}", id);
                }
                at = (at + 1) & m.mask;
            }
        }
        m
    }
    #[inline]
    fn get(&self, id: u32) -> Option<usize> {
        let mut at = (id.wrapping_mul(0x9E37_79B1) as usize) & self.mask;
        loop {
            // In bounds: `slots.len()` is `mask + 1`, a power of two.
            let (k, v) = unsafe { *self.slots.get_unchecked(at) };
            if k == id {
                return Some(v as usize);
            }
            if k == NOTYPE {
                return None;
            }
            at = (at + 1) & self.mask;
        }
    }
}

struct Rt {
    files: Vec<String>,
    types: Vec<TypeInfo>,
    type_map: TypeMap,
    objs: HeapArena<Obj>,
    paths: HeapArena<PathRec>,
    slots: HeapSlots,
    // `forget_slots`'s reusable buffer.
    slot_buf: PerHeap<Vec<(usize, u64)>>,
    reclaimed: HeapReclaimed,
    fn_items: HashMap<usize, usize>,  // code address -> its interned box
    // The fn items whose reference parameters are all pure direct (`cobc`'s
    // `Gen::pure_direct`): called with bare addresses, they read their
    // arguments and keep nothing (`cb_fn_pure`, D-0191).
    pure_items: std::collections::HashSet<usize>,
    fn_boxes: HashMap<usize, u64>,    // closure box handle -> the closure object it owns
    unwinding: bool,
    // The first fault's rendered report, printed once the unwind is done
    // (`[Fault-Unwind]`: unwind, then terminate reporting it).
    fault_report: Option<String>,

    // `std::event_op`'s counts (D-0063), by id, and the last id made.

    // Emptied id lists, reused: a frame's, a scope's and an object's
    // path list are created and dropped at every block, statement and
    // borrow, and allocating each was a tenth of a program's run.
    pool: PerHeap<Vec<Vec<u64>>>,
    // D-0202: which heaps a running thread owns (index 0, the main
    // thread's, always).
    // D-0202: how many running threads use each heap.
    heap_users: Vec<u32>,
    // The same for a path's projection: every borrow makes one, and every
    // path's end drops it.
    proj_pool: PerHeap<Vec<Vec<Proj>>>,
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

// The state read without changing it, as the read-only entries
// (`enter_read`) do, several threads at once: a shared reference only.
fn rt_ro() -> &'static Rt {
    unsafe { (*RT.0.get()).as_ref().expect("cb_init not called") }
}

// ---- the runtime lock ----

// Set when a thread is spawned; cleared again (D-0192) once every spawned
// thread has ended and left the runtime (`SOLO_PENDING`, `enter`).
static MULTI: AtomicBool = AtomicBool::new(false);

// Set by the last running spawned thread as it leaves the runtime for good
// (its final entry, `cb_spawn`'s thread body): the main thread's next entry,
// once it holds the lock (so that thread has let it go), switches locking
// off if no spawned thread is running.
static SOLO_PENDING: AtomicBool = AtomicBool::new(false);

// The runtime lock: a standard mutex (it spins briefly before sleeping,
// which a runtime taken and released at every call needs — a condition
// variable hand-off convoyed), made reentrant by recording its owner
// and this thread's depth; the owning thread keeps the guard.
static RTLOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());
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
    // D-0202: this thread's heap.
    static HEAP: Cell<u8> = const { Cell::new(0) };
    // A spawned thread's own state; null on the main thread.
    static TSP: Cell<*mut TState> = const { Cell::new(std::ptr::null_mut()) };
    static HELD: Cell<u32> = const { Cell::new(0) };
    static GUARD: std::cell::RefCell<Option<std::sync::RwLockWriteGuard<'static, ()>>> = const { std::cell::RefCell::new(None) };
    // D-0192: a read-only entry's shared hold on the lock (`enter_read`).
    static RGUARD: std::cell::RefCell<Option<std::sync::RwLockReadGuard<'static, ()>>> = const { std::cell::RefCell::new(None) };
    static RHELD: Cell<u32> = const { Cell::new(0) };
}

// Until the first spawn only the main thread exists, so its state and
// its runtime-call depth are plain statics and no thread-local is read.
static mut MAIN_TS: TState = TState { stack: Vec::new(), scopes: Vec::new(), inflight: VecDeque::new(), frame_locs: Vec::new(), dloc: (NOLOC, 0), box_draining: false, box_next: None, pend: Vec::new(), cunwind: false, jmp: 0, failed: None };
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
fn ts_raw() -> &'static mut TState {
    let p = if multi() { TSP.with(|c| c.get()) } else { std::ptr::null_mut() };
    if p.is_null() {
        unsafe { &mut *std::ptr::addr_of_mut!(MAIN_TS) }
    } else {
        unsafe { &mut *p }
    }
}

// This thread's state, its pending statement scopes made first (D-0196):
// whatever the caller does sees the stack as if each had been pushed.
#[inline]
fn ts() -> &'static mut TState {
    let t = ts_raw();
    if !t.pend.is_empty() {
        make_pending(t);
    }
    t
}

#[cold]
fn make_pending(t: &mut TState) {
    for (file, line) in std::mem::take(&mut t.pend) {
        t.scopes.push(t.stack.len());
        t.stack.push(Entry::Scope(Scope { temps: Vec::new(), toks: Vec::new(), loc: (file, line) }));
    }
}

// The runtime's critical sections are short (a check, a table update), so
// a thread that finds the lock taken tries again for a while before it
// sleeps: sleeping and being woken in the kernel for each of them cost
// more than the work itself once several threads use the runtime.
fn lock_write_spinning() -> std::sync::RwLockWriteGuard<'static, ()> {
    for i in 0..SPIN_TRIES {
        match RTLOCK.try_write() {
            Ok(g) => return g,
            Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        if i % 256 == 255 {
            std::thread::yield_now();
        } else {
            std::hint::spin_loop();
        }
    }
    RTLOCK.write().unwrap_or_else(|e| e.into_inner())
}

const SPIN_TRIES: u32 = 1024;

// How many times `wait_until` asks before it sleeps.
const SPIN_WAITS: u32 = 200;

fn lock_acquire(count: u32) {
    let me = TID.with(|t| t.get());
    if OWNER.load(Ordering::Relaxed) == me {
        HELD.with(|h| h.set(h.get() + count));
        return;
    }
    let g = lock_write_spinning();
    // D-0202: and every heap's lock, so no local entry runs meanwhile.
    let n = HEAPS_N.load(Ordering::Acquire);
    for h in 0..n {
        hlock(h);
    }
    XN.with(|c| c.set(n));
    OWNER.store(me, Ordering::Relaxed);
    HELD.with(|h| h.set(count));
    GUARD.with(|c| *c.borrow_mut() = Some(g));
}

// The heap locks an exclusive hold took, given back.
fn unlock_heaps() {
    let n = XN.with(|c| c.replace(0));
    for h in (0..n).rev() {
        hunlock(h);
    }
}

fn lock_release() {
    let left = HELD.with(|h| {
        let n = h.get() - 1;
        h.set(n);
        n
    });
    if left == 0 {
        OWNER.store(0, Ordering::Relaxed);
        unlock_heaps();
        GUARD.with(|c| drop(c.borrow_mut().take()));
    }
}

// Gives the lock up entirely (a blocking wait); returns the depth to restore.
fn lock_release_all() -> u32 {
    let n = HELD.with(|h| h.replace(0));
    OWNER.store(0, Ordering::Relaxed);
    unlock_heaps();
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
        enter_multi();
    }
    Entered(m)
}

// `enter` while threads run, out of line (as `enter_on_multi`).
#[cold]
#[inline(never)]
fn enter_multi() {
    if sched_on() {
        sched_entry();
    }
    // D-0202: an exclusive entry inside a local one would wait for its
    // own shared hold: a bug, reported rather than a hang.
    if LHELD.with(|c| c.get()) > 0 && OWNER.load(Ordering::Relaxed) != TID.with(|t| t.get()) {
        panic!("cbrt: an exclusive entry inside a local one");
    }
    if STATS_ON.load(Ordering::Relaxed) {
        N_EXCL.fetch_add(1, Ordering::Relaxed);
    }
    lock_acquire(1);
    if SOLO_PENDING.load(Ordering::Relaxed) {
        go_solo();
    }
}

// Back to a single thread (D-0192): on the main thread, holding the lock,
// with no spawned thread running, locking stops. The entries that took the
// lock still release it as they end (`Entered(true)`); later entries take
// none. Every spawned thread is joined (a handle joins as it ends), and the
// last one to end set `SOLO_PENDING` within its final entry, which it has
// left now that this thread holds the lock: nothing else is in the runtime.
// A later spawn switches locking on again (`go_multi`).
#[cold]
fn go_solo() {
    if TID.with(|t| t.get()) != 1 {
        return;
    }
    if LIVE.load(Ordering::Acquire) == 1 {
        SOLO_PENDING.store(false, Ordering::Relaxed);
        let n: usize = rt().reclaimed.heaps.iter().map(|m| m.entries()).sum();
        cb_reclaimed_entries.store(n, Ordering::Relaxed);
        MULTI.store(false, Ordering::Release);
    }
}

// D-0192: entering the runtime for a check that only reads its state
// (`cb_read`, `cb_borrow_check`, `cb_live_in`, …): with other threads
// running, the lock is held shared, so such checks in several threads run
// at once; a thread holding it exclusively passes straight through. A
// failing check's `fault` trades the shared hold for the exclusive one
// before anything else (`fault_lock`).
struct ReadEntered(u8);

#[inline(always)]
fn enter_read() -> ReadEntered {
    if !multi() {
        return ReadEntered(0);
    }
    let me = TID.with(|t| t.get());
    if OWNER.load(Ordering::Relaxed) == me {
        return ReadEntered(0);
    }
    if RHELD.with(|h| h.get()) > 0 {
        RHELD.with(|h| h.set(h.get() + 1));
        return ReadEntered(1);
    }
    let g = RTLOCK.read().unwrap_or_else(|e| e.into_inner());
    RGUARD.with(|c| *c.borrow_mut() = Some(g));
    RHELD.with(|h| h.set(1));
    ReadEntered(1)
}

impl Drop for ReadEntered {
    #[inline(always)]
    fn drop(&mut self) {
        if self.0 == 1 {
            let left = RHELD.with(|h| {
                let n = h.get() - 1;
                h.set(n);
                n
            });
            if left == 0 {
                RGUARD.with(|c| drop(c.borrow_mut().take()));
            }
        }
    }
}

// A fault reached from a read-only entry: the shared hold given up, the
// lock taken exclusively, as every other fault holds it. D-0202: and from
// a local entry, its heap's lock given up as well.
fn fault_lock() {
    if LHELD.with(|h| h.get()) > 0 {
        LHELD.with(|h| h.set(0));
        unlock_local();
        lock_acquire(1);
        return;
    }
    if RHELD.with(|h| h.get()) > 0 {
        RHELD.with(|h| h.set(0));
        RGUARD.with(|c| drop(c.borrow_mut().take()));
        lock_acquire(1);
    }
}

// ---- D-0202: local entries ----
//
// An entry whose work stays within a few heaps -- its operand's, the
// heaps whose page buckets name the cells it works on, and the entering
// thread's own (its stack, in-flight queue and pools are the thread's) --
// holds those heaps' locks only, so threads working in their own heaps
// run at once. Each entry reckons its heaps from the ids and addresses it
// is given, then, holding them, checks that its work reaches no further
// (no object of another heap, no reference slot whose forgetting could
// retire a path anywhere); if it would, it gives them back and enters
// exclusively before changing anything. An entry that may run code of
// the program (a destructor) is exclusive too: the runtime lock and then
// every heap's lock. Heap locks are always taken in index order (some by
// a local entry, all by an exclusive one), so no two entries wait for
// each other in a cycle. While a local entry holds its heaps, the address
// tables consult only those (`holds_heap`); a debug build asserts that no
// record of another heap is touched.
// Each heap's lock: a spin lock alone on its cache line (a local entry
// mostly takes only its own, so threads in their own heaps share no line).
#[repr(align(64))]
struct HLock(std::sync::atomic::AtomicU32);
const HEAP_LOCK: HLock = HLock(std::sync::atomic::AtomicU32::new(0));
static HEAP_LOCKS: [HLock; 256] = [HEAP_LOCK; 256];
// How many heaps exist (`claim_heap`): an exclusive entry locks them all.
static HEAPS_N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

#[inline]
fn hlock(h: usize) {
    let l = &HEAP_LOCKS[h].0;
    let mut i = 0u32;
    while l.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
        i += 1;
        if i < 128 {
            std::hint::spin_loop();
        } else {
            std::thread::yield_now();
        }
    }
}

#[inline]
fn hunlock(h: usize) {
    HEAP_LOCKS[h].0.store(0, Ordering::Release);
}
// D-0202: more threads run at once than there are heaps, so some heaps
// (and their pools) serve several (`claim_heap`): their users lock them
// even where a thread's own heap needs no lock (`own_heap`).
#[allow(clippy::declare_interior_mutable_const)]
const NOT_SHARED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static HEAP_SHARED: [std::sync::atomic::AtomicBool; MAX_HEAPS] = [NOT_SHARED; MAX_HEAPS];

// The hold a lock-free step on this thread's own stack and pools needs:
// none, unless this thread's heap is shared.
#[inline(always)]
fn own_heap() -> Option<Held> {
    if multi() && HEAP_SHARED[cur_heap()].load(Ordering::Relaxed) {
        own_heap_shared()
    } else {
        None
    }
}

#[cold]
#[inline(never)]
fn own_heap_shared() -> Option<Held> {
    Some(enter_set_multi(HeapSet::cur_and(cur_heap())))
}

// D-0202 measurement (`COBALTC_RT_STATS`): entries taken locally, and
// exclusively, while threads run.
static N_LOCAL: AtomicU64 = AtomicU64::new(0);
static N_EXCL: AtomicU64 = AtomicU64::new(0);
static N_MIGRATED: AtomicU64 = AtomicU64::new(0);
static STATS_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static LHELD: Cell<u32> = const { Cell::new(0) };
    // The heap a local hold has locked, and how many heaps an exclusive
    // hold locked (`lock_acquire`).
    // The heaps a local hold has locked.
    static LSET: Cell<HeapSet> = const { Cell::new(HeapSet::EMPTY) };
    // The one heap a local hold has locked, when it is one; else NO_SOLE.
    static LSOLE: Cell<usize> = const { Cell::new(NO_SOLE) };
    static XN: Cell<usize> = const { Cell::new(0) };
}

// How an entry holds the runtime (`enter_on`), as a tag so that its
// release inlines at every entry:
// - SOLO: one thread. As `Entered(false)`: if a destructor callback inside
//   the entry spawns, `go_multi` took a count for this entry, which its
//   end gives back.
// - LOCAL: a local hold on this thread's heap.
// - EXCL: an exclusive hold (`Entered(true)`).
// - NESTED: inside this thread's own exclusive hold; nothing to release.
struct Held(u8);

const SOLO: u8 = 0;
const LOCAL: u8 = 1;
const EXCL: u8 = 2;
const NESTED: u8 = 3;

impl Held {
    #[inline(always)]
    fn is_local(&self) -> bool {
        self.0 == LOCAL
    }

    // An exclusive entry, as `enter()` made it.
    #[cold]
    #[inline(never)]
    fn excl() -> Held {
        let e = enter();
        let t = if e.0 { EXCL } else { SOLO };
        std::mem::forget(e);
        Held(t)
    }
}

impl Drop for Held {
    // One thread: a load and a branch, inlined at every entry.
    #[inline(always)]
    fn drop(&mut self) {
        if self.0 == SOLO && !multi() {
            return;
        }
        held_release(self.0);
    }
}

#[cold]
#[inline(never)]
fn held_release(tag: u8) {
    match tag {
        SOLO | EXCL => lock_release(),
        LOCAL => {
            let left = LHELD.with(|h| {
                let n = h.get() - 1;
                h.set(n);
                n
            });
            if left == 0 {
                unlock_local();
            }
        }
        _ => {}
    }
}

// A set of heaps, by index.
#[derive(Clone, Copy, PartialEq, Eq)]
struct HeapSet([u64; 4]);

impl HeapSet {
    const EMPTY: HeapSet = HeapSet([0; 4]);

    #[inline]
    fn add(&mut self, h: usize) {
        if h < 256 {
            self.0[h >> 6] |= 1 << (h & 63);
        }
    }
    #[inline]
    fn has(&self, h: usize) -> bool {
        h < 256 && self.0[h >> 6] & (1 << (h & 63)) != 0
    }
    fn covers(&self, o: &HeapSet) -> bool {
        (0..4).all(|i| o.0[i] & !self.0[i] == 0)
    }
    // Its heaps, in ascending order.
    fn each(&self, mut f: impl FnMut(usize)) {
        for i in 0..4 {
            let mut w = self.0[i];
            while w != 0 {
                let b = w.trailing_zeros() as usize;
                f(i * 64 + b);
                w &= w - 1;
            }
        }
    }
    // This thread's heap and `h`.
    #[inline]
    fn cur_and(h: usize) -> HeapSet {
        let mut s = HeapSet::EMPTY;
        s.add(cur_heap());
        if h < HEAPS_N.load(Ordering::Acquire) {
            s.add(h);
        }
        s
    }
}

// Takes the heap locks of `set`, in ascending order.
fn lock_set(set: &HeapSet) {
    let mut n = 0;
    let mut one = NO_SOLE;
    set.each(|h| {
        hlock(h);
        n += 1;
        one = h;
    });
    LSET.with(|c| c.set(*set));
    LSOLE.with(|c| c.set(if n == 1 { one } else { NO_SOLE }));
}

// A local hold's heap locks, given back.
fn unlock_local() {
    let set = LSET.with(|c| c.replace(HeapSet::EMPTY));
    LSOLE.with(|c| c.set(NO_SOLE));
    set.each(hunlock);
}

const NO_SOLE: usize = usize::MAX;

// The one heap the address tables need consult, when there is one: heap 0
// while it is the only heap, or the heap a local hold has locked alone.
// (Each method otherwise asks `holds_heap` of every heap.)
#[inline(always)]
fn sole_heap(one: bool) -> Option<usize> {
    if one {
        return Some(0);
    }
    if LHELD.with(|c| c.get()) == 0 {
        return None;
    }
    let h = LSOLE.with(|c| c.get());
    if h == NO_SOLE {
        None
    } else {
        Some(h)
    }
}

// Whether the entry being run may touch heap `h`'s records: any heap
// under an exclusive hold or with one thread, else the heaps its local
// hold locked.
#[inline]
fn holds_heap(h: usize) -> bool {
    LHELD.with(|c| c.get()) == 0 || LSET.with(|c| c.get()).has(h)
}

// Enters for work on heap `h`'s records and this thread's own (its new
// objects, its pools): a local hold on the two.
#[inline(always)]
fn enter_on(h: usize) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_set_multi(HeapSet::cur_and(h))
}

// A local hold on the heaps of `set` (which names this thread's own), the
// form every entry's choice takes while threads run: out of line, so the
// one-thread path at every entry stays a load and a branch.
#[cold]
#[inline(never)]
fn enter_set_multi(set: HeapSet) -> Held {
    if sched_on() {
        sched_entry();
    }
    let me = TID.with(|t| t.get());
    if OWNER.load(Ordering::Relaxed) == me {
        return Held(NESTED);
    }
    if LHELD.with(|c| c.get()) > 0 {
        // A local entry reached from inside another (a frame's end freeing
        // a vector's buffer): the same hold. The outer entry planned what
        // the inner one touches; the heaps the inner one reckons may name
        // more, through page buckets other threads are changing, which it
        // then does not consult (`holds_heap`). A debug build asserts every
        // record touched is held.
        let _ = set;
        LHELD.with(|c| c.set(c.get() + 1));
        return Held(LOCAL);
    }
    if RHELD.with(|c| c.get()) > 0 {
        return Held::excl();
    }
    lock_set(&set);
    LHELD.with(|c| c.set(1));
    if STATS_ON.load(Ordering::Relaxed) {
        N_LOCAL.fetch_add(1, Ordering::Relaxed);
    }
    Held(LOCAL)
}


// D-0202: what an entry's work will touch, reckoned under a local hold
// before anything changes: the heaps it needs (`need`), and whether its
// work can be done locally at all (`ok`: false where it would run code of
// the program, or reach what the plan does not follow). Each step mirrors
// the runtime function it plans for. A record in a heap not yet held is
// not looked at; its heap is added, and the entry plans again holding it
// (`enter_planned`), so the plan reaches as far as the work does.
struct Plan {
    need: HeapSet,
    ok: bool,
}

impl Plan {
    fn new() -> Plan {
        let mut need = HeapSet::EMPTY;
        need.add(cur_heap());
        Plan { need, ok: true }
    }

    fn heap(&mut self, h: usize) -> bool {
        self.need.add(h);
        holds_heap(h)
    }

    // The cells `[lo, hi)`: whatever is registered over them by address.
    fn cells(&mut self, lo: usize, hi: usize) -> bool {
        let more = addr_heaps(lo, hi.max(lo + 1));
        let mut all = true;
        more.each(|h| {
            self.need.add(h);
            all &= holds_heap(h);
        });
        all
    }

    // `drop_occ`: a holder fewer, and when it was the last, `retire_token`.
    fn drop_occ(&mut self, r: &Rt, tok: u64) {
        if !self.heap(heap_of(tok)) {
            return;
        }
        if let Some(p) = r.paths.get(&tok) {
            if p.occ <= 1 && (!p.valid || (p.scope.is_none() && !p.is_root)) {
                self.retire(r, tok);
            }
        }
    }

    // `retire_token`: the path's object (its heap), which ends with it if
    // it is an ephemeral element object left unobserved.
    fn retire(&mut self, r: &Rt, tok: u64) {
        let Some(p) = r.paths.get(&tok) else { return };
        if p.is_root {
            return;
        }
        if let Some(o) = r.objs.get(&p.of) {
            if o.ephemeral {
                self.end_obj(r, p.of);
            }
        }
    }

    // `invalidate`: retired at once when unheld.
    fn invalidate(&mut self, r: &Rt, tok: u64) {
        if !self.heap(heap_of(tok)) {
            return;
        }
        if let Some(p) = r.paths.get(&tok) {
            if p.occ == 0 {
                self.retire(r, tok);
            }
        }
    }

    // `forget_slots`: every slot in the range, and its path's holder.
    fn forget(&mut self, r: &Rt, lo: usize, hi: usize) {
        if lo >= hi || !self.cells(lo, hi) {
            return;
        }
        r.slots.each_in(lo, hi, |t| self.drop_occ(r, t));
    }

    // `end_identity`: the object's paths (its heap), the slots in its
    // storage, its reclaimed entry.
    fn end_obj(&mut self, r: &Rt, obj: u64) {
        if !self.heap(heap_of(obj)) {
            return;
        }
        let Some(o) = r.objs.get(&obj) else { return };
        let t = r.typ(o.ty);
        let hi = o.addr + t.size.max(1) as usize;
        if !o.ephemeral && t.has_refs {
            self.forget(r, o.addr, hi);
        }
        if o.reclaimed || o.ephemeral {
            self.cells(o.addr, hi);
        }
    }

    // `cb_move_to`: the slots in the object's storage moved to `to`
    // (`rekey_slots`), a slot already there let go.
    fn move_obj(&mut self, r: &Rt, obj: u64, to: usize) {
        if obj == 0 || !self.heap(heap_of(obj)) {
            return;
        }
        let Some(o) = r.objs.get(&obj) else { return };
        let t = r.typ(o.ty);
        if !t.has_refs || o.addr == to {
            return;
        }
        let size = t.size.max(1) as usize;
        self.cells(o.addr, o.addr + size);
        self.forget(r, to, to + size);
    }

    // `cb_lock`: the reference to the mutex, and every path it was formed
    // through (the guard's path lists them, and reads theirs).
    fn lock_path(&mut self, r: &Rt, tok: u64) {
        if !self.heap(heap_of(tok)) {
            return;
        }
        let Some(rec) = r.paths.get(&tok) else { return };
        for &a in &rec.ancestors {
            self.heap(heap_of(a));
        }
    }

    // `cb_store_ref`: the slot (its address), the path stored, and the
    // path it held before let go.
    fn store(&mut self, r: &Rt, slot: usize, tok: u64) {
        if tok != 0 {
            self.heap(heap_of(tok));
        }
        self.forget(r, slot, slot + 8);
    }

    // A value of type `ty` at `addr` with its slots: their cells, their
    // paths.
    fn slots_of(&mut self, r: &Rt, addr: usize, ty: u32) {
        let size = r.typ(ty).size.max(1) as usize;
        if !self.cells(addr, addr + size) {
            return;
        }
        r.slots.each_in(addr, addr + size, |t| {
            self.heap(heap_of(t));
        });
    }

    // `destroy_obj`: the value's destruction, then its identity's end.
    fn destroy(&mut self, r: &Rt, obj: u64) {
        if !self.heap(heap_of(obj)) {
            return;
        }
        let Some(o) = r.objs.get(&obj) else { return };
        if !o.alive {
            return;
        }
        if o.is_resource || r.typ(o.ty).holds_fn {
            self.drop_value(r, o.addr, o.ty);
        }
        self.end_obj(r, obj);
    }

    // `drop_in_place`: local only for what the runtime itself does -- a
    // guard's release, a plain vector's buffer, and the owning parts of a
    // struct, enum or array of those. A destructor of the program, a box,
    // a handle or a `fn` value: not local.
    fn drop_value(&mut self, r: &Rt, addr: usize, ty: u32) {
        if !self.ok {
            return;
        }
        let info = r.typ(ty);
        if info.box_of.is_some() || info.kind == K_FN || info.kind == K_HANDLE {
            self.ok = false;
            return;
        }
        if !info.is_resource && !info.holds_fn {
            return;
        }
        if info.kind == K_GUARD {
            // `guard_drop`: the guard's slot, and the lock path it holds.
            if !self.cells(addr, addr + 8) {
                return;
            }
            if let Some(&tok) = r.slots.get(&addr) {
                self.invalidate(r, tok);
            }
            return;
        }
        if let (Some(esz), true) = (info.vec_plain, info.drop.is_some()) {
            // The buffer's release (`cb_vec_drop_plain`, `cb_deallocate`):
            // local while no object lies over its cells.
            let (po, co) = (info.fields[0].0 as usize, info.fields[2].0 as usize);
            let (ptr, cap) = unsafe { (*((addr + po) as *const usize), *((addr + co) as *const u64)) };
            if cap > 0 {
                let hi = ptr + (cap * esz) as usize;
                if !self.cells(ptr, hi) {
                    return;
                }
                if !quiet_cells(ptr, hi - ptr) && r.reclaimed.any_in(ptr, hi) {
                    self.ok = false;
                }
            }
            return;
        }
        if info.drop.is_some() {
            self.ok = false;
            return;
        }
        match info.kind {
            K_STRUCT => {
                for &(off, fty) in &info.owning {
                    self.drop_value(r, addr + off as usize, fty);
                }
            }
            K_ENUM => {
                let tag = unsafe { *(addr as *const u32) } as usize;
                if let Some(&pty) = info.variants.get(tag) {
                    if pty != NOTYPE {
                        self.drop_value(r, addr + info.payload_off as usize, pty);
                    }
                }
            }
            K_ARRAY => {
                let esize = r.typ(info.elem).size as usize;
                for i in 0..info.count as usize {
                    self.drop_value(r, addr + i * esize, info.elem);
                }
            }
            _ => self.ok = false,
        }
    }
}

// Enters for work `plan` reckons: a local hold on the heaps it needs,
// widened (and the work planned again) until it covers them; exclusive
// when the work cannot be local, or the plan does not settle.
#[inline(always)]
fn enter_planned(plan: impl FnMut(&mut Plan, &Rt)) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_planned_multi(plan)
}

#[cold]
#[inline(never)]
fn enter_planned_multi(mut plan: impl FnMut(&mut Plan, &Rt)) -> Held {
    let mut set = HeapSet::EMPTY;
    set.add(cur_heap());
    for _ in 0..4 {
        let h = enter_set_multi(set);
        if !h.is_local() || LHELD.with(|c| c.get()) > 1 {
            return h;
        }
        let mut p = Plan::new();
        plan(&mut p, rt_ro());
        if !p.ok {
            break;
        }
        if set.covers(&p.need) {
            return h;
        }
        drop(h);
        p.need.each(|x| set.add(x));
    }
    Held::excl()
}

// D-0202: entering to end the statement scope on top of this thread's
// stack (`end_scope`): its temporaries destroyed, its unheld paths retired.
#[inline(always)]
fn enter_scope_end() -> Held {
    enter_planned(|p, r| {
        let Some(Entry::Scope(s)) = ts().stack.last() else { return };
        for &obj in &s.temps {
            if !p.heap(heap_of(obj)) {
                continue;
            }
            if matches!(r.objs.get(&obj), Some(o) if o.alive && o.frame.is_none() && o.root.is_none()) {
                p.destroy(r, obj);
            }
        }
        for &t in &s.toks {
            if !p.heap(heap_of(t)) {
                continue;
            }
            if matches!(r.paths.get(&t), Some(q) if q.occ == 0) {
                p.retire(r, t);
            }
        }
    })
}

// D-0202: entering to end the frame on top of this thread's stack
// (`end_owned`): its objects destroyed, its held paths let go.
#[inline(always)]
fn enter_frame_end() -> Held {
    enter_planned(|p, r| {
        let Some(Entry::Frame(owned)) = ts().stack.last() else { return };
        for &e in owned.iter() {
            if e & HOLD != 0 {
                p.drop_occ(r, e & !HOLD);
            } else {
                p.destroy(r, e);
            }
        }
    })
}

// The heap an object's or path's id names.
#[inline(always)]
fn heap_of(id: u64) -> usize {
    // No id (0, a path that does not exist): nothing to touch, so the
    // entering thread's own heap -- not heap 0, which every thread shares.
    if id == 0 {
        return cur_heap();
    }
    (id >> HEAP_SHIFT) as usize
}

// Whether every path `tok` was formed through is in a heap this entry
// holds.
fn ancestors_local(tok: u64) -> bool {
    rt().paths.get(&tok).map_or(true, |p| p.ancestors.iter().all(|&a| holds_heap(heap_of(a))))
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
        rebuild_heap_pages(rt());
        cb_reclaimed_entries.store(THREADS_ENTRIES, Ordering::Relaxed);
        STATS_ON.store(std::env::var_os("COBALTC_RT_STATS").is_some(), Ordering::Relaxed);
        MULTI.store(true, Ordering::Release);
    }
}

// D-0202: the calling thread's heap (`cur_heap`): its new objects, slots
// and reclaimed entries are recorded there.
#[inline]
fn set_heap(_r: &mut Rt, h: u8) {
    HEAP.with(|c| c.set(h));
}

// D-0202: a heap for a new thread: one no running thread uses and that
// holds nothing, else a new one; when `MAX_HEAPS` are in use, the one with
// fewest users but the main thread's, shared from then on.
fn claim_heap(r: &mut Rt) -> u8 {
    let free = (1..r.heap_users.len()).find(|&h| {
        r.heap_users[h] == 0 && r.objs.heap_empty(h) && r.paths.heap_empty(h) && r.slots.heap_empty(h) && r.reclaimed.heap_empty(h)
    });
    let h = match free {
        Some(h) => h,
        None if r.heap_users.len() < MAX_HEAPS => {
            r.heap_users.push(0);
            HEAPS_N.store(r.heap_users.len(), Ordering::Release);
            r.heap_users.len() - 1
        }
        None => {
            let h = (1..r.heap_users.len()).min_by_key(|&h| r.heap_users[h]).unwrap_or(0);
            HEAP_SHARED[h].store(true, Ordering::Release);
            h
        }
    };
    r.heap_users[h] += 1;
    r.objs.ensure(h);
    r.paths.ensure(h);
    r.slots.ensure(h);
    r.reclaimed.ensure(h);
    r.pool.ensure(h);
    r.proj_pool.ensure(h);
    r.slot_buf.ensure(h);
    h as u8
}

fn signal(key: u64) {
    // A release or a signal: another thread may go on now (a schedule's
    // switch point, made at this thread's next entry holding nothing).
    if sched_on() {
        SCHED_PENDING.with(|c| c.set(true));
    }
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
// D-0202: `ready` is asked under the book's lock, and may change the book
// (claim a mutex) when it answers yes; the waiter registers before that
// lock is let go, so no change made under it is missed. An exclusive hold
// on the runtime is given up for the wait and taken again after.
fn wait_until(key: u64, kind: Wait, ready: &mut dyn FnMut(&mut Book) -> bool) {
    // A timer's wait ends at its deadline (`cb_sleep_ns`).
    let deadline = if kind == Wait::Timer { TIMER_DEADLINE.with(|c| c.get()) } else { None };
    // A short wait first, asking again now and then, before sleeping: most
    // waits for a mutex are for another thread's few steps, and sleeping
    // and being woken by the kernel cost far more than those (D-0202). Not
    // while this thread holds the runtime exclusively: its owner may need
    // it to finish.
    if OWNER.load(Ordering::Relaxed) != TID.with(|t| t.get()) && !sched_on() {
        for i in 0..SPIN_WAITS {
            if ready(&mut book()) {
                return;
            }
            for _ in 0..64 {
                std::hint::spin_loop();
            }
            if i % 16 == 15 {
                std::thread::yield_now();
            }
        }
    }
    // D-0204: a wait ends in a fault, contained like any other, when this
    // thread is asked to stop (`cancel`), when waiting would close a cycle
    // of joins and locks, or when it is found deadlocked.
    let me = TID.with(|t| t.get());
    let timer = kind == Wait::Timer;
    {
        let mut b = book();
        if ready(&mut b) {
            return;
        }
        if let Some(d) = stop_reason(&mut b, me) {
            drop(b);
            fault(d, NOLOC, 0);
        }
        if b.closes_cycle(me, &kind) {
            drop(b);
            fault("diag.deadlock", NOLOC, 0);
        }
        b.waits.insert(me, kind);
        b.wake.insert(me, key);
        b.find_deadlock();
    }
    // A schedule (`COBALTC_SCHEDULE_SEED`): ask, and if not yet, pass the
    // turn. A timer's wait leaves the schedule until its deadline instead,
    // as a call outside the runtime does.
    let sched = sched_on() && !timer;
    let asleep = sched_on() && timer;
    if asleep {
        sched_io(me, true);
    }
    loop {
        if sched {
            let mut bk = book();
            if ready(&mut bk) {
                bk.waits.remove(&me);
                bk.wake.remove(&me);
                return;
            }
            if let Some(d) = stop_reason(&mut bk, me) {
                bk.waits.remove(&me);
                bk.wake.remove(&me);
                drop(bk);
                fault(d, NOLOC, 0);
            }
            drop(bk);
            let held = if OWNER.load(Ordering::Relaxed) == me { lock_release_all() } else { 0 };
            if LHELD.with(|c| c.get()) != 0 {
                panic!("cbrt: a scheduled wait inside a local hold");
            }
            sched_yield();
            // Every other thread outside the program (a socket, input,
            // asleep): only their return can end this wait.
            if sched_alone(me) {
                std::thread::sleep(Duration::from_micros(200));
            }
            if held > 0 {
                lock_acquire(held);
            }
            continue;
        }
        let mut bk = book();
        if ready(&mut bk) {
            bk.waits.remove(&me);
            bk.wake.remove(&me);
            if asleep {
                drop(bk);
                sched_io(me, false);
            }
            return;
        }
        if let Some(d) = stop_reason(&mut bk, me) {
            bk.waits.remove(&me);
            bk.wake.remove(&me);
            drop(bk);
            if asleep {
                sched_io(me, false);
            }
            fault(d, NOLOC, 0);
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
        drop(bk);
        let me = TID.with(|t| t.get());
        let held = if OWNER.load(Ordering::Relaxed) == me { lock_release_all() } else { 0 };
        {
            let mut g = EVENTS.lock().unwrap();
            loop {
                let now = g.iter().find(|ev| ev.key == key).map_or(e, |ev| ev.signalled);
                if now != e {
                    break;
                }
                let mut wait = Duration::from_millis(50);
                if let Some(d) = deadline {
                    let now = std::time::Instant::now();
                    if now >= d {
                        break;
                    }
                    wait = wait.min(d - now);
                }
                let (g2, to) = cv.wait_timeout(g, wait).unwrap();
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
        if held > 0 {
            lock_acquire(held);
        }
    }
}

// Why this thread's wait must end in a fault (D-0204), if it must:
// found deadlocked, or asked to stop -- unless it was lent an exclusive
// reference, or is already unwinding a contained fault.
fn stop_reason(b: &mut Book, me: u64) -> Option<&'static str> {
    if b.deadlocked.remove(&me) {
        return Some("diag.deadlock");
    }
    if b.cancel.contains(&me) && !b.lent.contains(&me) && !ts_raw().cunwind {
        return Some("diag.thread-cancelled");
    }
    None
}

thread_local! {
    // The deadline of a timer's wait (`cb_sleep_ns`).
    static TIMER_DEADLINE: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
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

// D-0178: standard output's buffer, written out before a fault's report,
// a read of standard input and the program's end (src/outbuf.rs).
#[path = "../../src/outbuf.rs"]
mod outbuf;

fn flush_stdout() {
    let _ = outbuf::flush();
}

// The program's end, however it ends: standard output's buffer (D-0178)
// and every file's (D-0201) written out.
fn flush_at_end() {
    let _ = outbuf::flush();
    fileio::flush_all();
}

// ---- program ----

impl Rt {
    #[inline]
    fn typ(&self, id: u32) -> &TypeInfo {
        match self.type_map.get(id) {
            Some(i) => &self.types[i],
            None => panic!("cbrt: no type descriptor has the id {}", id),
        }
    }
    // The descriptor's position, for a caller that reads it more than once.
    #[inline]
    fn typ_pos(&self, id: u32) -> usize {
        match self.type_map.get(id) {
            Some(i) => i,
            None => panic!("cbrt: no type descriptor has the id {}", id),
        }
    }
    #[inline]
    fn typ_opt(&self, id: u32) -> Option<&TypeInfo> {
        self.type_map.get(id).map(|i| &self.types[i])
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_init(files: *const *const c_char, nfiles: usize, types: *const CbType, ntypes: usize, ids: *const u32) {
    netio::set_cancel_probe(cancel_probe);
    netio::wake_init();
    sched_init();
    set_stack_floor();
    let id_list: Vec<u32> = (0..ntypes).map(|i| *ids.add(i)).collect();
    let map = TypeMap::build(&id_list);
    let at = |id: u32| map.get(id);
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
            let inner = t.fields.iter().any(|&(_, f)| at(f).and_then(|i| ts.get(i)).map_or(false, |x| x.holds_fn))
                || t.variants.iter().any(|&v| v != NOTYPE && at(v).and_then(|i| ts.get(i)).map_or(false, |x| x.holds_fn))
                || (t.kind == K_ARRAY && at(t.elem).and_then(|i| ts.get(i)).map_or(false, |x| x.holds_fn));
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
            let owning: Vec<(u64, u32)> = ts[i].fields.iter().copied().filter(|&(_, f)| at(f).and_then(|i| ts.get(i)).map_or(false, |x| x.is_resource || x.holds_fn)).collect();
            ts[i].owning = owning;
        }
    }
    *RT.0.get() = Some(Rt {
        files: fs,
        types: ts,
        type_map: map,
        objs: HeapArena::default(),
        paths: HeapArena::default(),
        slots: HeapSlots::default(),
        slot_buf: PerHeap::default(),
        reclaimed: HeapReclaimed::default(),
        fn_items: HashMap::default(),
        pure_items: std::collections::HashSet::default(),
        fn_boxes: HashMap::default(),
        unwinding: false,
        fault_report: None,


        pool: PerHeap::default(),
        heap_users: vec![1],
        proj_pool: PerHeap::default(),
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

// `std::exit(status)` (D-0150, `[Terminate-Exit]`): the calling thread's
// scopes and frames are folded exactly as a fault folds them, every
// destructor running, and the program ends with `status`, reporting
// nothing. A destructor that faults during this unwind makes that fault
// the outcome (`fault` below, with `unwinding` set and no report kept).
#[no_mangle]
pub extern "C" fn cb_exit(status: u8) -> ! {
    let _e = enter();
    let r = rt();
    flush_stdout();
    if r.unwinding {
        // `exit` from a destructor already unwinding: the outcome is the
        // ending in progress.
        if let Some(rep) = r.fault_report.take() {
            eprint!("{}", diagnostics::for_stderr(&rep));
            std::process::exit(1);
        }
        std::process::exit(status as i32);
    }
    r.unwinding = true;
    while let Some(entry) = ts().stack.pop() {
        match entry {
            Entry::Frame(owned) => end_owned(owned),
            Entry::Scope(s) => {
                ts().scopes.pop();
                end_scope(s)
            }
        }
    }
    flush_at_end();
    std::process::exit(status as i32);
}

fn fault(id: &str, file: u32, line: u32) -> ! {
    fault_lock();
    let (file, line) = if file == NOLOC && line == 0 { get_at() } else { (file, line) };
    // D-0204 `[Fault-Contain]`: a spawned thread's fault ends that thread
    // alone, unless it was lent an exclusive reference, its trampoline did
    // not register (a fault before it ran), or the program is ending.
    let me = me();
    if me != 1 && !rt().unwinding && !ts_raw().cunwind && ts_raw().jmp != 0 && !book().lent.contains(&me) {
        contained_fault(id, file, line, me);
    }
    // The GIL is never released again: `[Fault-Unwind]` — other threads
    // take no further steps and their frames are not unwound.
    let r = rt();
    flush_stdout();
    if r.unwinding {
        // A destructor faulting during the unwind: the program's outcome
        // is the first fault; during an `exit`'s unwind (no report kept),
        // this fault.
        match r.fault_report.take() {
            Some(rep) => eprint!("{}", diagnostics::for_stderr(&rep)),
            None => {
                let at = if file == NOLOC { None } else { r.files.get(file as usize).map(|f| (f.as_str(), line as usize)) };
                let (id, msg) = match id.split_once(diagnostics::DETAIL_SEP) {
                    Some((i, m)) => (i, Some(m)),
                    None => (id, None),
                };
                let out = diagnostics::render(id, "dynamic", at);
                let out = match msg {
                    Some(m) => diagnostics::with_message(out, m),
                    None => out,
                };
                eprint!("{}", diagnostics::for_stderr(&out));
            }
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
    flush_at_end();
    if let Some(rep) = rt().fault_report.take() {
        eprint!("{}", diagnostics::for_stderr(&rep));
    }
    std::process::exit(1);
}

// A fault's report, as the program prints it.
fn render_fault(id: &str, file: u32, line: u32) -> String {
    let r = rt();
    let at = if file == NOLOC { None } else { r.files.get(file as usize).map(|f| (f.as_str(), line as usize)) };
    let (id, msg) = match id.split_once(diagnostics::DETAIL_SEP) {
        Some((i, m)) => (i, Some(m)),
        None => (id, None),
    };
    let out = diagnostics::render(id, "dynamic", at);
    match msg {
        Some(m) => diagnostics::with_message(out, m),
        None => out,
    }
}

// D-0204 `[Fault-Contain]`: the thread's mutexes released and poisoned,
// its scopes and frames folded as `[Fault-Unwind]` folds them (its own
// threads cancelled and waited for, `handle_destructor`), the failure
// recorded, every hold on the runtime given up, and control back to the
// trampoline's `setjmp` (`cb_thread_jmp`), which ends the thread
// (`cb_thread_failed`). The Rust frames this jump leaves own nothing that
// needs dropping: the holds are given up here.
fn contained_fault(id: &str, file: u32, line: u32, me: u64) -> ! {
    let text = render_fault(id, file, line);
    ts().cunwind = true;
    let keys: Vec<usize> = {
        let mut b = book();
        let ks: Vec<usize> = b.locks.iter().filter(|(_, t)| **t == me).map(|(k, _)| *k).collect();
        for k in &ks {
            b.locks.remove(k);
            b.poisoned.insert(*k);
        }
        ks
    };
    for k in keys {
        signal(mutex_key(k));
    }
    while let Some(entry) = ts().stack.pop() {
        match entry {
            Entry::Frame(owned) => end_owned(owned),
            Entry::Scope(s) => {
                ts().scopes.pop();
                end_scope(s)
            }
        }
    }
    let t = ts();
    t.scopes.clear();
    t.pend.clear();
    t.inflight.clear();
    t.frame_locs.clear();
    let fid = {
        let mut b = book();
        b.failures.push(Failure { raw: id.to_string(), file, line, text });
        (b.failures.len() - 1) as u64
    };
    t.failed = Some(fid);
    t.cunwind = false;
    if OWNER.load(Ordering::Relaxed) == me {
        let _ = lock_release_all();
    }
    let jb = t.jmp as *mut u8;
    unsafe { longjmp(jb, 1) }
}

extern "C" {
    fn longjmp(env: *mut u8, val: c_int) -> !;
}

// `[Terminate-Ok]`: `ok(s)`, reported as exit status `s` (`rule.fn.program`).
#[no_mangle]
pub extern "C" fn cb_terminate_ok(status: u8) -> ! {
    flush_at_end();
    // A test hook (CHG-0093): what the runtime still records once the
    // program has ended normally, which a program's own bookkeeping
    // must not make grow with its running time.
    if std::env::var_os("COBALTC_RT_STATS").is_some() {
        let r = rt();
        let objects = r.objs.values().filter(|o| o.alive).count();
        let paths = r.paths.values().count();
        eprintln!("cbrt: live at exit: {} objects, {} paths", objects, paths);
        eprintln!("cbrt: entries while threads ran: {} local, {} exclusive; {} objects migrated", N_LOCAL.load(Ordering::Relaxed), N_EXCL.load(Ordering::Relaxed), N_MIGRATED.load(Ordering::Relaxed));
    }
    std::process::exit(status as i32);
}

// The program's arguments (`rule.stdlib.args`): `argv` without the
// program's own name, copied once at startup, before `main` begins.
static ARGS: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

// The program's arguments, and the runtime's start-up: a write to a pipe
// whose reader is gone (a child that ended without reading its input,
// `Child::write_input` after the child's end, standard output into
// `head`) must fail with `Err(Io)` as it does under `coby`, not end the
// program by SIGPIPE (`procio::ignore_sigpipe`, D-0143).
#[no_mangle]
pub unsafe extern "C" fn cb_set_args(argc: i32, argv: *const *const c_char) {
    procio::ignore_sigpipe();
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
    match outbuf::write(bytes) {
        Ok(()) => n as i64,
        Err(_) => -1,
    }
}

// `std::stdout_flush` (D-0178): standard output's buffer written out.
#[no_mangle]
pub extern "C" fn cb_flush_out() -> i64 {
    match outbuf::flush() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

// `std::stderr_write` (`eprintf`, D-0040): as `cb_write_out`, to standard
// error, unbuffered. Standard output's buffer is written out first, so the
// two interleave in program order (D-0178).
#[no_mangle]
pub unsafe extern "C" fn cb_write_err(p: *const u8, n: u64) -> i64 {
    let bytes = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n as usize) };
    match outbuf::write_err(bytes) {
        Ok(()) => n as i64,
        Err(_) => -1,
    }
}

// `std::stdin_read`: up to `n` bytes of standard input into `p`; 0 at its end,
// -1 on an error. Rust's stdin is buffered, so `read_line`'s byte at a
// time costs no system call per byte. Standard output's buffer is written
// out first, so a prompt is on the screen before the program waits.
#[no_mangle]
pub unsafe extern "C" fn cb_read_in(p: *mut u8, n: u64) -> i64 {
    flush_stdout();
    if n == 0 {
        return 0;
    }
    let buf = std::slice::from_raw_parts_mut(p, n as usize);
    outside(|| read_stdin(buf))
}

fn read_stdin(buf: &mut [u8]) -> i64 {
    let mut input = std::io::stdin().lock();
    loop {
        match std::io::Read::read(&mut input, buf) {
            Ok(k) => return k as i64,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return -1,
        }
    }
}

// `std::stdin_read_line` (`read_line`'s primitive, src/fileio.rs): up to
// `n` bytes of standard input into `p`, through the next `\n`.
#[no_mangle]
pub unsafe extern "C" fn cb_read_line_in(p: *mut u8, n: u64) -> i64 {
    if n == 0 {
        return 0;
    }
    outside(|| fileio::read_line_chunk(std::slice::from_raw_parts_mut(p, n as usize)))
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
    let _ = outbuf::write(text.as_bytes());
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

// `std::tz_offset` (D-0147): the machine's offset from UTC at a moment.
#[no_mangle]
pub extern "C" fn cb_tz_offset(unix: i64) -> i64 {
    fileio::tz_offset(unix)
}

// `std::sleep_ns` (D-0124).
#[no_mangle]
pub extern "C" fn cb_sleep_ns(ns: u64) -> i64 {
    // D-0204: while threads run, a wait on the clock, which `cancel`
    // ends; it never counts as stuck.
    if !multi() {
        fileio::sleep_ns(ns);
        return 0;
    }
    let deadline = std::time::Instant::now() + Duration::from_nanos(ns);
    let me = me();
    TIMER_DEADLINE.with(|c| c.set(Some(deadline)));
    wait_until(me | 3 << 62, Wait::Timer, &mut |_| std::time::Instant::now() >= deadline);
    TIMER_DEADLINE.with(|c| c.set(None));
    0
}

// `std::path_op` (D-0141), `std::proc_op` (D-0143), `std::net_op`
// (D-0144): `(op, h, a, an, b, bn, out, cap)`, the number back and up to
// `cap` bytes of the answer at `out`, as `coby` has them.
#[path = "../../src/procio.rs"]
mod procio;
#[path = "../../src/netio.rs"]
mod netio;
#[path = "../../src/osrand.rs"]
mod osrand;

#[allow(clippy::too_many_arguments)]
unsafe fn byte_call(f: fileio::ByteCall, op: u64, h: u64, a: *const u8, an: u64, b: *const u8, bn: u64, out: *mut u8, cap: u64) -> i64 {
    let (r, v) = f(op, h, bytes(a, an), bytes(b, bn), cap);
    let k = v.len().min(cap as usize);
    if k > 0 {
        std::ptr::copy_nonoverlapping(v.as_ptr(), out, k);
    }
    r
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn cb_path_op(op: u64, h: u64, a: *const u8, an: u64, b: *const u8, bn: u64, out: *mut u8, cap: u64) -> i64 {
    byte_call(fileio::path_call, op, h, a, an, b, bn, out, cap)
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn cb_proc_op(op: u64, h: u64, a: *const u8, an: u64, b: *const u8, bn: u64, out: *mut u8, cap: u64) -> i64 {
    outside(|| byte_call(procio::call, op, h, a, an, b, bn, out, cap))
}

// D-0204: a call that may wait outside the runtime (a socket, a child,
// input), which may end by itself: while it lasts, no deadlock is
// declared.
// ---- schedules (D-0204: `cobc --explore`, `--schedule-seed`) ----
//
// With `COBALTC_SCHEDULE_SEED` set, one program thread runs at a time: the
// one holding the turn. The turn passes, to a thread the seed's sequence
// chooses, wherever a thread could be preempted without holding the
// runtime: a `spawn`, every wait, a thread's end, a call outside the
// runtime, after a mutex is released (at the thread's next entry holding
// nothing), and now and then at any such entry. One seed gives one order;
// different seeds explore different ones.
struct SchedSt {
    turn: u64,
    members: Vec<u64>,
    io: Vec<u64>,
    rng: u64,
    // Each waiting thread's OS thread, to wake the one whose turn it is
    // and no other (with hundreds of threads, waking all at every switch
    // was most of a run's time).
    parked: Vec<(u64, std::thread::Thread)>,
}

static SCHED_ON: AtomicBool = AtomicBool::new(false);
static SCHED: Mutex<SchedSt> = Mutex::new(SchedSt { turn: 1, members: Vec::new(), io: Vec::new(), rng: 0, parked: Vec::new() });

thread_local! {
    // A switch asked for where none could be made (a mutex released under
    // a runtime hold): made at the thread's next entry holding nothing.
    static SCHED_PENDING: Cell<bool> = const { Cell::new(false) };
}

fn sched_on() -> bool {
    SCHED_ON.load(Ordering::Relaxed)
}

fn sched_lock() -> std::sync::MutexGuard<'static, SchedSt> {
    SCHED.lock().unwrap_or_else(|e| e.into_inner())
}

impl SchedSt {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    // The next turn: a member not outside the runtime, or none (0) when
    // every one is; the first back then takes it.
    fn pick(&mut self) {
        let n = self.members.iter().filter(|m| !self.io.contains(m)).count();
        if n == 0 {
            self.turn = 0;
            return;
        }
        let k = (self.next() % n as u64) as usize;
        let io = &self.io;
        self.turn = *self.members.iter().filter(|m| !io.contains(m)).nth(k).unwrap();
        self.wake();
    }
    fn wake(&self) {
        if let Some((_, t)) = self.parked.iter().find(|(id, _)| *id == self.turn) {
            t.unpark();
        }
    }
}

fn sched_init() {
    let seed = std::env::var("COBALTC_SCHEDULE_SEED").ok().and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0);
    if seed != 0 {
        let mut g = sched_lock();
        g.rng = seed;
        g.members = vec![1];
        g.turn = 1;
        SCHED_ON.store(true, Ordering::Relaxed);
    }
}

// Waits for this thread's turn (taking it when no one has it).
fn sched_wait(me: u64, mut g: std::sync::MutexGuard<'static, SchedSt>) {
    if g.turn != me && g.turn != 0 {
        g.parked.push((me, std::thread::current()));
        while g.turn != me && g.turn != 0 {
            drop(g);
            std::thread::park();
            g = sched_lock();
        }
        g.parked.retain(|(id, _)| *id != me);
    }
    g.turn = me;
}

// Gives the turn to the seed's choice (maybe this thread again) and waits
// for it to come back. Only where this thread holds nothing of the runtime.
fn sched_yield() {
    SCHED_PENDING.with(|c| c.set(false));
    let me = TID.with(|t| t.get());
    let mut g = sched_lock();
    g.pick();
    sched_wait(me, g);
}

// At an entry: the switch a release asked for, or one in 32 by the seed,
// when this thread holds nothing.
#[cold]
fn sched_entry() {
    let me = TID.with(|t| t.get());
    if HELD.with(|h| h.get()) != 0 || LHELD.with(|c| c.get()) != 0 || OWNER.load(Ordering::Relaxed) == me {
        return;
    }
    let pending = SCHED_PENDING.with(|c| c.get());
    if pending || sched_lock().next() % 32 == 0 {
        sched_yield();
    }
}

fn sched_alone(me: u64) -> bool {
    let g = sched_lock();
    !g.members.iter().any(|m| *m != me && !g.io.contains(m))
}

fn sched_add(t: u64) {
    sched_lock().members.push(t);
}

fn sched_leave(me: u64) {
    let mut g = sched_lock();
    g.members.retain(|m| *m != me);
    if g.turn == me {
        g.pick();
    }
}

fn sched_io(me: u64, on: bool) {
    let mut g = sched_lock();
    if on {
        g.io.push(me);
        if g.turn == me {
            g.pick();
        }
    } else {
        g.io.retain(|m| *m != me);
        sched_wait(me, g);
    }
}

fn outside<T>(f: impl FnOnce() -> T) -> T {
    if !multi() {
        return f();
    }
    // Under a schedule the call is made out of turn -- unless this thread
    // holds the runtime (a destructor's call), which no other thread could
    // then enter anyway: it keeps its turn.
    let me = TID.with(|t| t.get());
    if sched_on() && HELD.with(|h| h.get()) == 0 && LHELD.with(|c| c.get()) == 0 && OWNER.load(Ordering::Relaxed) != me {
        sched_io(me, true);
        IN_IO.fetch_add(1, Ordering::AcqRel);
        let r = f();
        IN_IO.fetch_sub(1, Ordering::AcqRel);
        sched_io(me, false);
        return r;
    }
    // A count, not the book: every socket operation of every thread
    // passes here, and the book's one lock became the server's bottleneck.
    IN_IO.fetch_add(1, Ordering::AcqRel);
    let r = f();
    IN_IO.fetch_sub(1, Ordering::AcqRel);
    r
}

// How many threads are in a call outside the runtime (`outside`), and how
// many cancellations were ever asked (`cancel_probe` needs no lock while
// there are none).
static IN_IO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CANCELS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn cb_net_op(op: u64, h: u64, a: *const u8, an: u64, b: *const u8, bn: u64, out: *mut u8, cap: u64) -> i64 {
    let r = outside(|| byte_call(netio::call, op, h, a, an, b, bn, out, cap));
    // D-0204: a thread asked to stop leaves its socket's wait.
    if r == netio::CANCELLED {
        fault("diag.thread-cancelled", NOLOC, 0);
    }
    r
}

// D-0204: `netio`'s question while a socket waits: has this thread been
// asked to stop (and can it be)?
fn cancel_probe() -> bool {
    if CANCELS.load(Ordering::Acquire) == 0 || !multi() || ts_raw().cunwind {
        return false;
    }
    let me = me();
    let b = book();
    b.cancel.contains(&me) && !b.lent.contains(&me)
}

// `std::os_random` (D-0142).
#[no_mangle]
pub unsafe extern "C" fn cb_os_random(out: *mut u8, n: u64) -> i64 {
    if n == 0 {
        return 0;
    }
    osrand::fill(std::slice::from_raw_parts_mut(out, n as usize))
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

// D-0181: `parse_check`/`parse_value` for `bool`.
#[no_mangle]
pub unsafe extern "C" fn cb_parse_bool(p: *const u8, n: u64, out: *mut u8) -> i64 {
    match numtext::parse_bool(bytes(p, n)) {
        Ok(v) => {
            if !out.is_null() {
                *out = v as u8;
            }
            -1
        }
        Err(code) => code,
    }
}

// ---- frames and scopes ----

#[no_mangle]
pub extern "C" fn cb_frame_push() {
    // D-0202: this thread's own stack and its heap's pool: no lock.
    let _h = own_heap();
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
    // D-0202: a frame that owns nothing ends nothing: no lock.
    if matches!(ts().stack.last(), Some(Entry::Frame(v)) if v.is_empty()) {
        let _h = own_heap();
        ts().frame_locs.pop();
        if let Some(Entry::Frame(v)) = ts().stack.pop() {
            give_vec(&mut rt().pool, v);
        }
        return;
    }
    let _e = enter_frame_end();
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
    // No path (a pathless reference, CHG-0237): nothing to hold.
    if tok == 0 {
        return;
    }
    // D-0204: the path's record and this thread's own frame: local (a
    // match arm binding a map's value did this in every loop iteration,
    // stopping every other thread).
    let _e = enter_on(heap_of(tok));
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
    let _e = enter_planned(|p, r| p.drop_occ(r, tok));
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
    // Pending until the runtime next looks (`ts`): this thread's own state
    // only, so no lock (D-0196).
    ts_raw().pend.push((NOLOC, 0));
}

// A statement scope whose end reports `file:line` for a destruction
// fault, as `cb_frame_push_at` does for a frame: a temporary that ends
// with the statement while a kept reference holds it.
#[no_mangle]
pub extern "C" fn cb_stmt_push_at(file: u32, line: u32) {
    ts_raw().pend.push((file, line));
}

#[no_mangle]
pub extern "C" fn cb_stmt_pop() {
    // A scope never made held nothing: popping it ends nothing.
    if ts_raw().pend.pop().is_some() {
        return;
    }
    // D-0202: a made scope that holds nothing ends nothing: no lock.
    if matches!(ts().stack.last(), Some(Entry::Scope(s)) if s.temps.is_empty() && s.toks.is_empty()) {
        let _h = own_heap();
        ts().scopes.pop();
        if let Some(Entry::Scope(s)) = ts().stack.pop() {
            let r = rt();
            give_vec(&mut r.pool, s.temps);
            give_vec(&mut r.pool, s.toks);
        }
        return;
    }
    let _e = enter_scope_end();
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
    let is_resource = r.typ_opt(ty).map_or(false, |t| t.is_resource);
    r.objs.insert(id, Obj { addr, ty, alive: true, init, is_resource, reclaimed: false, root: None, frame: None, scope: None, tid: 0, ephemeral: false, toks: TokList::default(), xtoks: TokList::default() });
    let scope = top_scope(r);
    stamp_temp(r, id, scope);
    id
}

#[no_mangle]
pub extern "C" fn cb_new(addr: *mut u8, ty: u32, _file: u32, _line: u32) -> u64 {
    // D-0202: a new object in this thread's heap, in its own scope.
    let _h = enter_on(cur_heap());
    new_obj(addr as usize, ty, true)
}

#[no_mangle]
pub extern "C" fn cb_new_uninit(addr: *mut u8, ty: u32, _file: u32, _line: u32) -> u64 {
    let _h = enter_on(cur_heap());
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
    // D-0202: a path lives in its object's heap.
    let tok = r.paths.reserve_in(HeapArena::<Obj>::heap_of(of));
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
    // D-0202: the object's records and its root path (in its heap), and
    // this thread's frame.
    let _h = enter_on(heap_of(obj));
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
    // D-0204: the common case -- the root still reaches its live object --
    // asked under a local hold of the root's heap (and the object's, when
    // it is the same); only a new object's binding enters exclusively.
    // (An X25519 ladder rebinds in its inner loop: every TLS handshake
    // stopped every other thread thousands of times.)
    if multi() {
        let _h = enter_on(heap_of(root));
        let r = rt();
        if let Some(p) = r.paths.get(&root) {
            if p.valid && holds_heap(heap_of(p.of)) && r.objs.get(&p.of).map_or(false, |o| o.alive) {
                return root;
            }
        }
    }
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
    // Into the buffer `forget_slots` keeps (a move of a value holding
    // references is as common as an object's end); a nested use, through
    // `drop_occ`, takes a new one.
    let mut moved = std::mem::take(&mut *r.slot_buf);
    r.slots.scan_into(old, old + size as usize, true, &mut moved);
    for &(a, t) in &moved {
        // A slot already at the destination (a copy made before the move
        // re-homed the object) stops holding what it held: overwriting it
        // silently left that path counted as held forever.
        if let Some(prev) = r.slots.insert(new + (a - old), t) {
            drop_occ(r, prev);
        }
    }
    moved.clear();
    let r = rt();
    if r.slot_buf.capacity() < moved.capacity() {
        *r.slot_buf = moved;
    }
}

#[no_mangle]
pub extern "C" fn cb_move_to(obj: u64, addr: *mut u8) {
    let _e = enter_planned(|p, r| p.move_obj(r, obj, addr as usize));
    let r = rt();
    let (old, size, refs) = match r.objs.get(&obj) {
        Some(o) => {
            let t = r.typ(o.ty);
            (o.addr, t.size, t.has_refs)
        }
        None => return,
    };
    // Only a reference slot is stored (`end_identity`'s note): a type that
    // holds none has none to move.
    if refs {
        rekey_slots(r, old, addr as usize, size);
    }
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
    // D-0204: the object's records (its heap): local.
    let _e = enter_on(heap_of(obj));
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
    let _e = enter_planned(|p, r| p.end_obj(r, obj));
    let r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

// `cb_absorb` for plain data holding references that has just moved into
// the container being built (`cb_move_to` to the part's address): the
// temporary's record ends, and the references it held stay in their slots,
// now the container's -- held as before, no occurrence lost or added --
// rather than being copied there while the temporary ended with its
// statement (`cobc`'s `store_part`). The temporary is fresh: no path
// reaches it.
#[no_mangle]
pub extern "C" fn cb_absorb_refs(obj: u64) {
    let _e = enter();
    let r = rt();
    if !r.objs.contains_key(&obj) {
        return;
    }
    detach(r, obj);
    end_identity_in(r, obj, false);
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
    let _e = enter_planned(|p, r| {
        p.end_obj(r, obj);
    });
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
    } else if r.typ(ty).holds_fn {
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
    let pos = r.typ_pos(ty);
    let info = &r.types[pos];
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
    let info = &r.types[pos];
    match info.kind {
        K_STRUCT => {
            // The owning fields (`TypeInfo::owning`; a plain one's
            // destruction does nothing), last declared first, the first
            // declared last; read by index each time, not copied out (one
            // list per struct destroyed).
            let n = info.owning.len();
            for i in (0..n).rev() {
                let (off, fty) = rt().types[pos].owning[i];
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
            let esize = r.typ(elem).size as usize;
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
    let cells = rt().typ(t).size.max(1);
    if !full {
        cb_deallocate(p, cells, 0);
        return;
    }
    // With no live object over the contents, `[Reclaim]` would establish
    // a fresh one, no path but its root, which the destruction ends at
    // once: unobservable, so the contents are destroyed in place (root
    // and object 0) -- a tree's teardown makes no object per node.
    let none_over = quiet_cells(p as usize, cells as usize) || live_reclaimed(rt(), p as usize).is_none();
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
            std::alloc::dealloc((*b).data, std::alloc::Layout::from_size_align_unchecked(rt().typ((*b).ty).size.max(1) as usize, 16));
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
    end_identity_in(r, obj, true)
}

// `forget`: the references stored in the object's storage stop being
// occurrences (every end but `cb_absorb_refs`, whose storage now belongs
// to the container it moved into).
fn end_identity_in(r: &mut Rt, obj: u64, forget: bool) {
    let ty = match r.objs.get(&obj) {
        Some(o) => o.ty,
        None => return,
    };
    let size = r.typ(ty).size;
    let (addr, ephemeral, toks) = match r.objs.get_mut(&obj) {
        Some(o) => {
            o.xtoks = TokList::default();
            (o.addr, o.ephemeral, std::mem::take(&mut o.toks).into_vec())
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
    // its storage; nor does any other object's whose type holds none (a
    // `String`, a `Vec` of plain data): only a reference slot is stored.
    if forget && !ephemeral && r.typ(ty).has_refs {
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
    let mut held = std::mem::take(&mut *r.slot_buf);
    r.slots.scan_into(addr, addr + size as usize, true, &mut held);
    for &(_, t) in &held {
        drop_occ(r, t);
    }
    held.clear();
    let r = rt();
    if r.slot_buf.capacity() < held.capacity() {
        *r.slot_buf = held;
    }
}

// ---- raw storage ----

#[no_mangle]
pub extern "C" fn cb_reclaim(addr: *mut u8, ty: u32) -> u64 {
    // D-0204: as the element entries: the heaps of the cells' pages, and
    // an object found there in one of them.
    let _e = enter_elem(addr as usize, ty);
    let r = rt();
    let a = addr as usize;
    if let Some(obj) = live_reclaimed(r, a) {
        // Its root token is about to be in the program's hands.
        let o = r.objs.get_mut(&obj).unwrap();
        if o.ephemeral {
            o.ephemeral = false;
            let size = r.typ(r.objs[&obj].ty).size;
            r.reclaimed.insert(a, obj, size);
        }
        let root = match r.objs[&obj].root {
            Some(root) => root,
            None => {
                let root = mint(r, obj, Vec::new(), true, true, Vec::new());
                r.objs.get_mut(&obj).unwrap().root = Some(root);
                root
            }
        };
        return root;
    }
    establish_reclaimed(r, a, ty, false).0
}

// `[Reclaim]`'s fresh object over the cells at `a`: its root token (0 for
// an ephemeral object, which has none) and its id.
fn establish_reclaimed(r: &mut Rt, a: usize, ty: u32, ephemeral: bool) -> (u64, u64) {
    let id = r.objs.reserve();
    let is_resource = r.typ_opt(ty).map_or(false, |t| t.is_resource);
    r.objs.insert(id, Obj { addr: a, ty, alive: true, init: true, is_resource, reclaimed: true, root: None, frame: None, scope: None, tid: 0, ephemeral, toks: TokList::default(), xtoks: TokList::default() });
    if ephemeral {
        // No root path: nothing outside the runtime ever holds one, and
        // `cb_reclaim` mints it if the program re-attaches the object.
        r.reclaimed.insert_ephemeral(a, id, r.typ(ty).size);
        return (0, id);
    }
    r.reclaimed.insert(a, id, r.typ(ty).size);
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
    let _e = enter_on(heap_of(base));
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    check_access(rt_ro(), base, extra, mode == 1, file, line);
}

#[no_mangle]
pub unsafe extern "C" fn cb_borrow_check(base: u64, p: *const CbProj, n: usize, mode: u32, file: u32, line: u32) {
    if base == 0 {
        return;
    }
    let _e = enter_on(heap_of(base));
    let r = rt_ro();
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
    let _e = enter_elem(addr as usize, ty);
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
    let _e = enter_elem(addr as usize, ty);
    elem_borrow(addr, ty, mode, true)
}

// D-0202: entering for the object over an element's cells at `a`: a local
// hold on the heaps whose page buckets name its pages (where an object
// over them is registered) and this thread's (where a new one is made),
// when the object found there, if any, is in one of them.
#[inline(always)]
fn enter_elem(a: usize, ty: u32) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_elem_multi(a, ty)
}

#[cold]
#[inline(never)]
fn enter_elem_multi(a: usize, ty: u32) -> Held {
    let size = rt_ro().typ(ty).size;
    let h = enter_set_multi(addr_heaps(a, a + size.max(1) as usize));
    if !h.is_local() || rt_ro().reclaimed.get(&a).map_or(true, |&o| holds_heap(heap_of(o))) {
        return h;
    }
    drop(h);
    Held::excl()
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
    let _e = enter_elem(addr as usize, ty);
    let r = rt();
    let a = addr as usize;
    if live_reclaimed(r, a).is_none() {
        return;
    }
    // A clash faults in `elem_borrow`, which takes the current location:
    // the caller's, when it gives one (a native loop calling the program's
    // functions, whose own lines would otherwise be the last recorded).
    if !(file == NOLOC && line == 0) {
        set_at(file, line);
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
            let t = &r.typ(ty);
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
    let a = addr as usize;
    let n = (len * size) as usize;
    // D-0202: local while no object lies over the elements (the common
    // case: a check that finds nothing); `cb_reclaim` below is exclusive.
    let _e = enter_live_empty(a, a + n.max(1));
    let r = rt();
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

// Whether a live object starts over any of the `n` cells from `addr`
// (`live_reclaimed`): a native body over a run of elements takes its
// general path, the one that forms each element's borrow, only when one
// of them has an object a path may hold.
#[no_mangle]
pub extern "C" fn cb_live_in(addr: *mut u8, n: u64) -> u8 {
    if n == 0 {
        return 0;
    }
    // D-0202: a local hold on the heaps whose page buckets name these
    // cells, when every object registered over them is in one of those.
    let a = addr as usize;
    let _e = enter_live(a, a + n as usize);
    if quiet_cells(a, n as usize) {
        return 0;
    }
    let r = rt_ro();
    if !r.reclaimed.any_in(a, a + n as usize) {
        return 0;
    }
    r.reclaimed.range(a, a + n as usize).into_iter().any(|(at, _)| live_reclaimed(r, at).is_some()) as u8
}

// `cb_live_in` of two runs of `n` cells, in one call (`Vec::swap`'s two
// elements).
#[no_mangle]
pub extern "C" fn cb_live_in2(a: *mut u8, b: *mut u8, n: u64) -> u8 {
    // Each run under its own hold: the answer is two reads, as before.
    (cb_live_in(a, n) != 0 || cb_live_in(b, n) != 0) as u8
}

#[inline(always)]
fn enter_live(lo: usize, hi: usize) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_live_multi(lo, hi)
}

// As `enter_live`, but local only when no reclaimed entry lies over the
// cells at all.
#[inline(always)]
fn enter_live_empty(lo: usize, hi: usize) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_live_empty_multi(lo, hi)
}

#[cold]
#[inline(never)]
fn enter_live_empty_multi(lo: usize, hi: usize) -> Held {
    let h = enter_set_multi(addr_heaps(lo, hi));
    if !h.is_local() || quiet_cells(lo, hi - lo) || !rt_ro().reclaimed.any_in(lo, hi) {
        return h;
    }
    drop(h);
    Held::excl()
}

#[cold]
#[inline(never)]
fn enter_live_multi(lo: usize, hi: usize) -> Held {
    let h = enter_set_multi(addr_heaps(lo, hi));
    if !h.is_local() {
        return h;
    }
    let r = rt_ro();
    if quiet_cells(lo, hi - lo) || !r.reclaimed.any_in(lo, hi) {
        return h;
    }
    if r.reclaimed.range(lo, hi).iter().all(|&(_, o)| holds_heap(heap_of(o))) {
        return h;
    }
    drop(h);
    Held::excl()
}

// Whether a live object lies over the cells at `addr` (`live_reclaimed`):
// for a native body that must take the general path only for such an
// element (`cobc`'s native `Vec<String>::drop`).
#[no_mangle]
pub extern "C" fn cb_live_at(addr: *mut u8) -> u8 {
    let _e = enter_live(addr as usize, addr as usize + 1);
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
    let t = &r.typ(ty);
    let size = t.size;
    if !t.has_refs && t.kind != K_FN && r.objs.get(&obj).map_or(false, |o| o.toks.is_empty()) {
        end_identity(r, obj);
        return;
    }
    r.reclaimed.insert(a, obj, size);
}

#[no_mangle]
pub extern "C" fn cb_raw_move_out(addr: *mut u8, dst: *mut u8, ty: u32) {
    let _e = enter();
    let r = rt();
    // `[Rawptr-Move-Out]` re-keys held-by to the new object: the
    // references the value holds move with its bytes (already copied to
    // `dst`), and only then does the reclaimed identity at `addr` end.
    let size = r.typ(ty).size;
    rekey_slots(r, addr as usize, dst as usize, size);
    if let Some(&obj) = r.reclaimed.get(&(addr as usize)) {
        end_identity(r, obj);
    }
}

#[no_mangle]
pub extern "C" fn cb_release(addr: *mut u8, n: u64) {
    let _e = enter_cells(addr as usize, n);
    release_cells(addr, n);
}

// D-0202: entering to end what lies over the cells `[a, a + n)`: a local
// hold on the heaps whose page buckets name those pages, when nothing
// there reaches further -- no reference slot (forgetting one may retire a
// path in any heap), and only reclaimed objects of held heaps that hold
// no references. Otherwise exclusive, decided before anything changes.
#[inline(always)]
fn enter_cells(a: usize, n: u64) -> Held {
    if !multi() {
        return Held(SOLO);
    }
    enter_cells_multi(a, n)
}

#[cold]
#[inline(never)]
fn enter_cells_multi(a: usize, n: u64) -> Held {
    let hi = a + n.max(1) as usize;
    let h = enter_set_multi(addr_heaps(a, hi));
    if !h.is_local() || cells_local(a, hi) {
        return h;
    }
    drop(h);
    Held::excl()
}

fn cells_local(a: usize, hi: usize) -> bool {
    let r = rt_ro();
    if r.slots.any_in(a, hi) {
        return false;
    }
    if quiet_cells(a, hi - a) || !r.reclaimed.any_in(a, hi) {
        return true;
    }
    r.reclaimed.range(a, hi).iter().all(|&(_, o)| holds_heap(heap_of(o)) && r.objs.get(&o).map_or(true, |x| !r.typ(x.ty).has_refs))
}

// `cb_release`'s work, inside an entry.
fn release_cells(addr: *mut u8, n: u64) {
    let r = rt();
    let a = addr as usize;
    // No entry on these pages (`quiet_cells`): no object over these cells.
    if !quiet_cells(a, n as usize) && r.reclaimed.any_in(a, a + n as usize) {
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

// `cb_release` for cells that never hold a reference: a `Vec` of plain
// elements (no reference, resource or `fn` value), whose cells are only
// ever written with plain values, in a buffer whose cells were all
// released when any earlier buffer there was freed. The objects over the
// cells end as `cb_release` ends them; its scan for references held by
// the cells, which could find none, is not made.
#[no_mangle]
pub extern "C" fn cb_release_plain(addr: *mut u8, n: u64) {
    let _e = enter_cells(addr as usize, n);
    release_plain_cells(addr, n);
}

// `cb_release_plain`'s work, inside an entry.
fn release_plain_cells(addr: *mut u8, n: u64) {
    let r = rt();
    let a = addr as usize;
    debug_assert!(r.slots.range(a, a + n as usize).is_empty(), "cbrt: a plain release over cells holding references");
    if !quiet_cells(a, n as usize) && r.reclaimed.any_in(a, a + n as usize) {
        let stale: Vec<u64> = r.reclaimed.range(a, a + n as usize).into_iter().map(|(_, o)| o).collect();
        for obj in stale {
            end_identity(r, obj);
        }
    }
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
    fn realloc(p: *mut u8, n: usize) -> *mut u8;
}

#[no_mangle]
pub extern "C" fn cb_allocate(size: u64, align: u64) -> *mut u8 {
    // No runtime state but `ALLOCS`, which has its own lock: `malloc` is
    // thread-safe, so no hold on the runtime lock (D-0192).
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
    let _e = enter_cells(addr as usize, size);
    release_cells(addr, size);
    free_block(addr, size);
}

// The block at `addr` handed back to the allocator (its cells released).
fn free_block(addr: *mut u8, size: u64) {
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

// `allocate(new)`, `copy_raw` of the old contents and `deallocate(old)`,
// as `Vec`'s growth makes them, in one step for a buffer of plain
// elements (`cb_release_plain`'s condition): `realloc`, which may grow the
// block where it is or move it without copying page by page, then the
// old cells released. None if the allocation fails, the old block then
// untouched, as `allocate`'s failure leaves it. An over-aligned buffer
// takes the three steps.
#[no_mangle]
pub unsafe extern "C" fn cb_reallocate_plain(addr: *mut u8, old: u64, new: u64, align: u64) -> *mut u8 {
    if old == 0 {
        return cb_allocate(new, align);
    }
    // D-0202: one entry for the old cells (the helpers below enter none:
    // a second reckoning of their heaps could differ from this one).
    let _e = enter_cells(addr as usize, old);
    if new == 0 {
        release_cells(addr, old);
        free_block(addr, old);
        return 1 as *mut u8;
    }
    if (align.max(1) as usize).next_power_of_two() > MALLOC_ALIGN {
        let p = cb_allocate(new, align);
        if p.is_null() {
            return p;
        }
        std::ptr::copy_nonoverlapping(addr, p, old.min(new) as usize);
        release_cells(addr, old);
        free_block(addr, old);
        return p;
    }
    let p = realloc(addr, new as usize);
    if !p.is_null() {
        // The old cells' objects end, whether the block moved or grew in place.
        release_plain_cells(addr, old);
    }
    p
}

// `cb_deallocate` for cells that never held a reference (a `String`'s
// bytes, a buffer of elements with none in them): no slot to forget, so
// only an object over the cells, if any could be there, ends first.
#[no_mangle]
pub extern "C" fn cb_deallocate_plain(addr: *mut u8, size: u64) {
    if size == 0 {
        return;
    }
    if !quiet_cells(addr as usize, size as usize) || OVER_ALIGNED.load(std::sync::atomic::Ordering::Relaxed) != 0 {
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

// `cb_fn_item` for a function whose reference parameters are all pure
// direct: the handle is the same, and the code is recorded as one that
// keeps nothing of its arguments (`cb_fn_pure`).
#[no_mangle]
pub extern "C" fn cb_fn_item_pure(code: *mut u8) -> *mut u8 {
    let h = cb_fn_item(code);
    let _e = enter();
    rt().pure_items.insert(code as usize);
    h
}

// Whether the `fn` value `h` is such a fn item (D-0191): a `Vec` call
// handed it may pass it bare element addresses, as a direct call would.
#[no_mangle]
pub unsafe extern "C" fn cb_fn_pure(h: *mut u8) -> u8 {
    if h.is_null() {
        return 0;
    }
    let _e = enter_read();
    let b = &*(h as *const CbFnBox);
    rt_ro().pure_items.contains(&(b.code as usize)) as u8
}

// D-0192: a closure's code whose reference parameters are all direct and
// never handed on (`cobc`'s pure closures): recorded as `cb_fn_item_pure`
// records a function, for `cb_fn_pure`.
#[no_mangle]
pub extern "C" fn cb_fn_mark_pure(code: *mut u8) {
    let _e = enter();
    rt().pure_items.insert(code as usize);
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
    let size = r.typ(ty).size;
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
    if r.typ(ty).is_resource {
        fault("diag.read-of-resource", file, line);
    }
    let size = r.typ(ty).size;
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
    // The `fn` values among the captures are copied in turn: each copy owns
    // its own closures (a shared box would be dropped by whichever copy
    // ends first, leaving the other's slot empty).
    let mut offs = Vec::new();
    if size > 0 {
        fn_offsets(rt(), data as usize, ty, 0, &mut offs);
    }
    for o in offs {
        let slot = data.add(o as usize) as *mut *mut u8;
        if !(*slot).is_null() {
            *slot = cb_fn_copy(*slot, file, line);
        }
    }
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
    // D-0202: local when the base's object, and every path it was formed
    // through (whose ancestors the new path's list closes over), are in
    // this thread's heap.
    let h = enter_on(heap_of(base));
    let _e = if h.is_local() && !ancestors_local(base) {
        drop(h);
        Held::excl()
    } else {
        h
    };
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
    // No path (0): a range of a reference whose checks were proved (an
    // objectless `String` lent to a `std` builder, D-0196) has none either.
    if base == 0 {
        return 0;
    }
    // D-0204: as `cb_borrow`: local when the base's heap, and every path it
    // was formed through, are this thread's to hold.
    let h = enter_on(heap_of(base));
    let _e = if h.is_local() && !ancestors_local(base) {
        drop(h);
        Held::excl()
    } else {
        h
    };
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    let r = rt();
    let rec = match r.paths.get(&base) {
        Some(p) if p.valid => p,
        _ => fault("diag.stale-binding", file, line),
    };
    // The new path's projection and ancestors come from the runtime's
    // pools, and the check compares through a stack buffer (`Norm`,
    // `ranged_overlap_with`), as `borrow_in`'s do: every slice and view a
    // program forms passes through here.
    let mut path = r.proj_pool.pop().unwrap_or_default();
    path.extend_from_slice(&rec.proj);
    path.extend(extra.iter().map(|q| conv(*q)));
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
    let brec = &r.paths[&base];
    let rec_of = brec.of;
    let rec_root = brec.is_root;
    let exclusive = mode == 1;
    // A shared borrow can clash only with an exclusive path.
    let toks: &[u64] = match r.objs.get(&rec_of) {
        Some(o) if o.alive => if exclusive { &o.toks } else { &o.xtoks },
        _ => fault("diag.stale-binding", file, line),
    };
    let mine = Norm::of(path.iter().copied());
    for &t in toks {
        let q = &r.paths[&t];
        if !q.valid || q.is_root || q.occ == 0 {
            continue;
        }
        debug_assert!(
            rec_root || brec.ancestors.contains(&t) || !brec.ancestors.iter().any(|e| r.paths.get(e).map_or(false, |pe| pe.ancestors.contains(&t))),
            "cbrt: an ancestors list is not closed"
        );
        if !rec_root && (t == base || brec.ancestors.contains(&t)) {
            continue;
        }
        if !ranged_overlap_with(mine.get(), &q.proj) {
            continue;
        }
        if q.lock_of.is_some() != brec.lock_of.is_some() && !(if brec.lock_of.is_some() { q.exclusive } else { exclusive }) {
            continue;
        }
        if exclusive || q.exclusive {
            fault("diag.aliasing-conflict", file, line);
        }
    }
    if !minted {
        give_proj(&mut r.proj_pool, path);
        return 0;
    }
    let mut anc2 = if rec_root { Vec::new() } else { take_vec(&mut r.pool) };
    let brec = &r.paths[&base];
    if !rec_root {
        anc2.push(base);
        anc2.extend(brec.ancestors.iter().copied());
        for e in std::iter::once(&base).chain(brec.ancestors.iter()) {
            if let Some(pe) = r.paths.get(e) {
                for a in &pe.ancestors {
                    if !anc2.contains(a) {
                        anc2.push(*a);
                    }
                }
            }
        }
    }
    let lock_of = brec.lock_of.clone();
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
    // No path (0): a reference whose checks the compiler proved (an
    // objectless local lent to a `std` builder, D-0196) lends none on.
    if base == 0 {
        return 0;
    }
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

// D-0205 / CHG-0237: whether a frozen parameter's body may run (`cobc`'s
// `NAME__frz`, chosen at entry by its dispatcher): reads through `tok`
// cannot clash with anything now, and nothing the call does can change
// that. `tok` is valid, over a live, initialized object, not formed under
// a lock, and no valid exclusive path to that object overlaps it (held or
// not -- conservative); for a vector parameter (`n > 0`), no object lies
// over its buffer `[lo, lo + n)` (an element held by reference). 0, a
// path the caller already vouched for (pathless, CHG-0237, D-0196): yes.
// While the call runs no exclusive path to what `tok` reaches can be
// formed: not by the function (its parameters are shared or hold no
// exclusive reference), and not by another thread (it would clash with the
// held paths `tok` was formed through).
#[no_mangle]
pub unsafe extern "C" fn cb_frozen_ok(tok: u64, lo: *const u8, n: usize) -> u8 {
    if tok == 0 {
        return 1;
    }
    if n > 0 && cb_live_in(lo as *mut u8, n as u64) != 0 {
        return 0;
    }
    let _e = enter_on(heap_of(tok));
    let r = rt_ro();
    let rec = match r.paths.get(&tok) {
        Some(p) if p.valid && p.lock_of.is_none() => p,
        _ => return 0,
    };
    let o = match r.objs.get(&rec.of) {
        Some(o) if o.alive && o.init => o,
        _ => return 0,
    };
    let mine = Norm::of(rec.proj.iter().copied());
    for &t in o.xtoks.iter() {
        if t == tok || rec.ancestors.contains(&t) {
            continue;
        }
        let Some(q) = r.paths.get(&t) else { continue };
        if !q.valid || q.is_root {
            continue;
        }
        let overlap = if has_range(&rec.proj) || has_range(&q.proj) {
            ranged_overlap_with(mine.get(), &q.proj)
        } else {
            let m = rec.proj.len().min(q.proj.len());
            (0..m).all(|i| rec.proj[i] == q.proj[i])
        };
        if overlap {
            return 0;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn cb_read(base: u64, p: *const CbProj, n: usize, file: u32, line: u32) {
    if base == 0 {
        return;
    }
    let _e = enter_on(heap_of(base));
    let r = rt_ro();
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
    let info = &r.typ(ty);
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
            let esize = r.typ(info.elem).size;
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

// D-0194: `[Write]` to a place of a quiet type (`quiet_ty`): `cb_write`'s
// check, then, where the target holds a value (its object initialized)
// that owns something, that value is destroyed in place
// (`[Write-Quiet-Replace]`) instead of faulting; the caller stores the
// new value after. A place with no path (0) always holds its value.
#[no_mangle]
pub unsafe extern "C" fn cb_write_quiet(base: u64, p: *const CbProj, n: usize, addr: *mut u8, ty: u32, file: u32, line: u32) {
    let _e = enter();
    let init = if base == 0 {
        true
    } else {
        let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
        let r = rt();
        let of = check_access(r, base, extra, true, file, line);
        let o = r.objs.get_mut(&of).unwrap();
        let was = o.init;
        o.init = true;
        was
    };
    if init && owns(rt(), addr as usize, ty) {
        drop_in_place(addr as usize, ty, 0);
    }
}

#[no_mangle]
pub unsafe extern "C" fn cb_write(base: u64, p: *const CbProj, n: usize, target_resource: u32, file: u32, line: u32) {
    // No path (0), as for `cb_read`: a confined vector's unchecked body
    // (a field confined for a dual body's fast half, D-0189) passes none.
    if base == 0 {
        return;
    }
    let extra = if n == 0 { &[][..] } else { std::slice::from_raw_parts(p, n) };
    // D-0192: the check, and the target already initialized (nearly every
    // write), change nothing: under the shared hold. Otherwise the whole
    // write again, exclusively (the state may have moved in between).
    // D-0202: all of it within the base's heap: local when that is this
    // thread's.
    let _e = enter_on(heap_of(base));
    let r = rt();
    let of = check_access(r, base, extra, true, file, line);
    let o = r.objs.get_mut(&of).unwrap();
    if target_resource != 0 && o.init {
        fault("diag.overwrite-of-live-resource", file, line);
    }
    o.init = true;
}

// ---- reference data ----

#[no_mangle]
pub extern "C" fn cb_store_ref(slot: *mut u8, tok: u64) {
    let _e = enter_planned(|p, r| p.store(r, slot as usize, tok));
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
    // No path (0): a place whose checks were proven (an objectless local
    // lent to a writer, D-0200) is there for the whole call.
    if tok == 0 {
        return;
    }
    let _e = enter();
    path_target(rt(), tok, file, line);
}

#[no_mangle]
pub extern "C" fn cb_load_ref(slot: *const u8) -> u64 {
    let _e = enter_planned(|p, _| {
        p.cells(slot as usize, slot as usize + 8);
    });
    rt().slots.get(&(slot as usize)).copied().unwrap_or(0)
}

// The offsets of every reference slot inside a value of type `ty`
// (enums by the active variant, read from the value itself).
fn ref_offsets(r: &Rt, addr: usize, ty: u32, base: u64, out: &mut Vec<u64>) {
    let info = &r.typ(ty);
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
            let esize = r.typ(info.elem).size;
            for i in 0..info.count {
                ref_offsets(r, addr + (i * esize) as usize, info.elem, base + i * esize, out);
            }
        }
        _ => {}
    }
}

// D-0089: the offsets of the `fn` slots in the value at `addr`.
fn fn_offsets(r: &Rt, addr: usize, ty: u32, base: u64, out: &mut Vec<u64>) {
    let info = &r.typ(ty);
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
            let esize = r.typ(info.elem).size;
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
    if r.typ(b.ty).is_resource {
        return true;
    }
    // A closure holding a `fn` value whose closure owns a resource owns
    // it too (its capture's type, `fn(…)`, cannot say so).
    let mut offs = Vec::new();
    fn_offsets(r, b.data as usize, b.ty, 0, &mut offs);
    offs.iter().any(|&o| fn_owns_resource(r, unsafe { *(b.data.add(o as usize) as *mut *mut u8) }))
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

// An object whose value now lives in a C local that no path can ever
// reach but through checks the compiler proved (`cobc`'s objectless
// `String` locals): its record ends, the value is not dropped -- the
// local owns it, and `cb_drop_local` drops it where the binding ends. It
// has no path: it was just received, and nothing has borrowed it.
#[no_mangle]
pub extern "C" fn cb_forget_obj(obj: u64) {
    let _e = enter_planned(|p, r| {
        p.end_obj(r, obj);
    });
    let r = rt();
    if !r.objs.contains_key(&obj) {
        return;
    }
    detach(r, obj);
    end_identity(r, obj);
}

// The value at `addr` of a reference-free resource type with no object
// (`cb_forget_obj`) ends: `[Destroy]` as an owned object's end would make
// it, with no record to end.
#[no_mangle]
pub extern "C" fn cb_drop_local(addr: *mut u8, ty: u32) {
    let _e = enter();
    drop_in_place(addr as usize, ty, 0);
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
    let _e = enter_planned(|p, r| {
        let size = r.typ(ty).size.max(1) as usize;
        p.forget(r, dst as usize, dst as usize + size);
        p.slots_of(r, src as usize, ty);
    });
    let r = rt();
    // The offsets and the slots found are kept in the runtime's reusable
    // lists (`pool`, `slot_buf`): a copy of a value holding references is
    // one of a program's commonest runtime events.
    let mut offs = take_vec(&mut r.pool);
    ref_offsets(r, src as usize, ty, 0, &mut offs);
    // `dst`'s bytes are now the new value's: a reference slot the old
    // value had where the new one has none (`Some(r)` overwritten by
    // `None`) holds nothing any more, and its path one holder fewer.
    let d = dst as usize;
    let size = r.typ(ty).size as usize;
    let mut held = std::mem::take(&mut *r.slot_buf);
    r.slots.scan_into(d, d + size, false, &mut held);
    for &(a, t) in &held {
        if !offs.iter().any(|&o| d + o as usize == a) {
            let r = rt();
            r.slots.remove(&a);
            drop_occ(r, t);
        }
    }
    held.clear();
    let r = rt();
    if r.slot_buf.capacity() < held.capacity() {
        *r.slot_buf = held;
    }
    for &off in &offs {
        if let Some(&t) = rt().slots.get(&(src as usize + off as usize)) {
            cb_store_ref(unsafe { dst.add(off as usize) }, t);
        }
    }
    give_vec(&mut rt().pool, offs);
}

// ---- values in flight ----

#[no_mangle]
pub extern "C" fn cb_send(obj: u64) {
    let _h = enter_on(heap_of(obj));
    let r = rt();
    detach(r, obj);
    ts().inflight.push_back(Inflight::Obj(obj));
}

#[no_mangle]
pub extern "C" fn cb_recv(addr: *mut u8) -> u64 {
    // D-0202: local for an object of this thread's heap whose type holds
    // no reference (its move rekeys no slot).
    let _e = if !multi() {
        Held(SOLO)
    } else {
        let front = match ts().inflight.front() {
            Some(Inflight::Obj(o)) => *o,
            _ => 0,
        };
        enter_planned(|p, r| p.move_obj(r, front, addr as usize))
    };
    let obj = match ts().inflight.pop_front() {
        Some(Inflight::Obj(o)) => o,
        _ => panic!("cbrt: receive of an object that was not sent"),
    };
    let obj = migrate(rt(), obj);
    cb_move_to(obj, addr);
    let r = rt();
    let scope = top_scope(r);
    stamp_temp(r, obj, scope);
    obj
}

// D-0202: an object received from another heap (a spawn's argument, a
// join's result) moves into this thread's heap under a new id, so its
// new owner's work on it stays local. Only a value nothing else names:
// detached, with no path at all (the sender's root ended when it moved),
// not over raw cells, and holding no reference. The caller holds both
// heaps. The old id, wherever the sender's code still keeps it, is stale
// (its generation moved on).
fn migrate(r: &mut Rt, obj: u64) -> u64 {
    if !multi() || heap_of(obj) == cur_heap() {
        return obj;
    }
    let movable = match r.objs.get(&obj) {
        Some(o) => o.alive && o.toks.is_empty() && o.root.is_none() && !o.reclaimed && !o.ephemeral && o.frame.is_none() && o.scope.is_none() && !r.typ(o.ty).has_refs,
        None => false,
    };
    if !movable {
        return obj;
    }
    if STATS_ON.load(Ordering::Relaxed) {
        N_MIGRATED.fetch_add(1, Ordering::Relaxed);
    }
    let o = r.objs.remove(&obj).unwrap();
    let id = r.objs.reserve();
    r.objs.insert(id, o);
    id
}

// A reference leaving a function: out of the callee's scopes, into the
// caller's current statement — the same journey an object makes.
#[no_mangle]
pub extern "C" fn cb_send_ref(tok: u64) {
    // D-0202: the path's record and this thread's stack: local.
    let _e = enter_on(heap_of(tok));
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
    let _e = if !multi() {
        Held(SOLO)
    } else {
        let front = match ts().inflight.front() {
            Some(Inflight::Ref(t)) => *t,
            _ => 0,
        };
        enter_on(heap_of(front))
    };
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
    let _e = enter_on(heap_of(tok));
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
    let _e = enter_planned(|p, r| p.slots_of(r, src as usize, ty));
    let r = rt();
    let mut offs = take_vec(&mut r.pool);
    ref_offsets(r, src as usize, ty, 0, &mut offs);
    let mut toks = take_vec(&mut r.pool);
    toks.extend(offs.iter().map(|off| r.slots.get(&(src as usize + *off as usize)).copied().unwrap_or(0)));
    // In flight, as `cb_send_ref`'s: a path formed in the sending
    // statement (`return Some(&m.v);`) must not end with that statement,
    // and the sender's copy no longer holds it -- a slot left in a dead
    // stack temporary counted it as held until something happened to
    // reuse the address. `cb_recv_datum` stores it, held again, at the
    // receiver. Until then the flight itself holds it: a spawned
    // thread's arguments travel while the spawning statement may be
    // ending, and that statement's scope must not find the path unheld
    // and retire it before the callee receives it.
    for (off, &t) in offs.iter().zip(toks.iter()) {
        if t != 0 {
            detach_tok(r, t);
            if r.slots.remove(&(src as usize + *off as usize)).is_none() {
                if let Some(p) = r.paths.get_mut(&t) {
                    p.occ += 1;
                }
            }
        }
    }
    give_vec(&mut r.pool, offs);
    ts().inflight.push_back(Inflight::Datum(toks));
}

// A native body returning `Some(r)` for an element reference `r`
// (`cobc`'s native `HashMap::get`/`get_mut`): the element's `[Reclaim]`
// and `[Borrow]` as `cb_elem_borrow` makes them, the path sent as the
// datum's one reference, as `cb_send_datum` sends an `Option<ref<T>>`
// built in the body. No temporary object holds it on the way.
#[no_mangle]
pub extern "C" fn cb_elem_borrow_datum(addr: *mut u8, ty: u32, mode: u32) -> u64 {
    let _e = enter_elem(addr as usize, ty);
    let tok = elem_borrow(addr, ty, mode, false);
    let r = rt();
    if let Some(p) = r.paths.get_mut(&tok) {
        p.occ += 1;
    }
    let mut toks = take_vec(&mut r.pool);
    toks.push(tok);
    ts().inflight.push_back(Inflight::Datum(toks));
    tok
}

// A native body returning reference data with one reference, `tok`, a
// path it formed just now and stored in no slot (`cobc`'s native
// `StringView::sub`): sent as `cb_send_datum` sends such a datum built
// in a local -- out of the forming statement's scope, held by the flight.
#[no_mangle]
pub extern "C" fn cb_send_tok_datum(tok: u64) {
    // No path (a `str`'s view): nothing of the runtime's to touch.
    if tok == 0 && ts_raw().pend.is_empty() {
        ts_raw().inflight.push_back(Inflight::Datum(vec![0]));
        return;
    }
    let _e = enter_on(heap_of(tok));
    let r = rt();
    if r.paths.contains_key(&tok) {
        detach_tok(r, tok);
        if let Some(p) = r.paths.get_mut(&tok) {
            p.occ += 1;
        }
    }
    let mut toks = take_vec(&mut r.pool);
    toks.push(tok);
    ts().inflight.push_back(Inflight::Datum(toks));
}

// The token of the `k`-th of the `n` data in flight to this thread (0:
// none), left in flight: `cobc`'s dual-body dispatcher reads a slice
// argument's path before the body it chooses receives it. Exactly the
// callee's `n` slice arguments must be in flight, each one reference: a
// queue of any other shape would pair a parameter with another's path,
// and is the compiler's error, not the program's.
#[no_mangle]
pub extern "C" fn cb_peek_datum_tok(k: usize, n: usize) -> u64 {
    let _e = enter();
    let q = &ts().inflight;
    if q.len() != n || k >= n {
        panic!("cbrt: dual-body dispatch found {} data in flight, expected {}", q.len(), n);
    }
    match q.get(k) {
        Some(Inflight::Datum(t)) => match t.as_slice() {
            [tok] => *tok,
            [] => 0,
            _ => panic!("cbrt: dual-body dispatch found reference data with more than one reference"),
        },
        _ => panic!("cbrt: dual-body dispatch found something other than reference data in flight"),
    }
}

// Whether the `n` paths `toks` (each exclusive where `excl` says so) are
// all valid, over live objects that are not reclaimed cells, not
// lock-derived, and pairwise disjoint
// wherever one of a pair is exclusive: then no access through one of them
// can clash with another, and `cobc`'s dual-body dispatcher may run the
// body that reads them unchecked. Anything it cannot vouch for (a 0
// token, a lock-derived path) answers no: the checked body runs.
#[no_mangle]
pub unsafe extern "C" fn cb_paths_disjoint(n: usize, toks: *const u64, excl: *const u8) -> u8 {
    let toks = std::slice::from_raw_parts(toks, n);
    // D-0202: local when every path is in this thread's heap.
    let c = cur_heap();
    let _e = if toks.iter().all(|&t| heap_of(t) == c) { enter_on(c) } else { Held::excl() };
    let r = rt_ro();
    let excl = std::slice::from_raw_parts(excl, n);
    let mut recs = Vec::with_capacity(n);
    for &t in toks {
        let rec = match r.paths.get(&t) {
            // Not over a reclaimed object (an element, a box's cells) when
            // there are pairs to compare: a clash between it and a range of
            // its container is found by other means than the two paths'
            // projections. A single path (a field dual body's, D-0189) has
            // nothing to be compared with: its validity is the test, and a
            // range of its container that overlaps it could not have been
            // live when it was formed (`[Borrow-Denied]`).
            Some(p) if p.valid && p.lock_of.is_none() && r.objs.get(&p.of).map_or(false, |o| o.alive && o.init && (n == 1 || !o.reclaimed)) => p,
            _ => return 0,
        };
        recs.push(rec);
    }
    for i in 0..n {
        for j in i + 1..n {
            if !(excl[i] != 0 || excl[j] != 0) || recs[i].of != recs[j].of {
                continue;
            }
            let (a, b) = (&recs[i].proj, &recs[j].proj);
            let overlap = if has_range(a) || has_range(b) {
                let na = Norm::of(a.iter().copied());
                ranged_overlap_with(na.get(), b)
            } else {
                a.iter().zip(b.iter()).all(|(x, y)| x == y)
            };
            if overlap {
                return 0;
            }
        }
    }
    1
}

// The same body returning `None`: a datum with no reference in flight.
#[no_mangle]
pub extern "C" fn cb_send_datum_none() {
    let _e = enter();
    ts().inflight.push_back(Inflight::Datum(Vec::new()));
}

// Reference data with at most one reference (`Option<ref<T>>`) received
// with no slot to hold it (`cobc`'s match on a call's result): the path
// back in the statement's scope, as `cb_recv_ref` puts one, and its token
// returned (0: none sent). Nothing holds it until a binder does
// (`cb_frame_hold`); it ends with the statement, as the temporary that
// would have held it ends then.
#[no_mangle]
pub extern "C" fn cb_recv_datum_tok() -> u64 {
    // D-0204: the one path's records (its heap, peeked at in this thread's
    // own queue) and this thread's scopes: local.
    let front = match ts_raw().inflight.front() {
        Some(Inflight::Datum(t)) if t.len() == 1 => t[0],
        _ => 0,
    };
    let _e = enter_on(heap_of(front));
    let r = rt();
    let toks = match ts().inflight.pop_front() {
        Some(Inflight::Datum(t)) => t,
        _ => panic!("cbrt: receive of reference data that was not sent"),
    };
    let tok = match toks.as_slice() {
        [] => 0,
        [t] => *t,
        _ => panic!("cbrt: reference data with more than one reference received as one"),
    };
    give_vec(&mut r.pool, toks);
    if tok == 0 {
        return 0;
    }
    if r.paths.contains_key(&tok) {
        let scope = top_scope(r);
        stamp_tok(r, tok, scope);
        // The flight's hold ends; the statement's scope has it now.
        let p = r.paths.get_mut(&tok).unwrap();
        p.occ = p.occ.saturating_sub(1);
    }
    tok
}

#[no_mangle]
pub extern "C" fn cb_recv_datum(dst: *mut u8, ty: u32) {
    let _e = enter_planned(|p, r| {
        if let Some(Inflight::Datum(toks)) = ts().inflight.front() {
            for &t in toks.iter() {
                if t != 0 {
                    p.drop_occ(r, t);
                }
            }
        }
        let size = r.typ(ty).size.max(1) as usize;
        p.forget(r, dst as usize, dst as usize + size);
    });
    let r = rt();
    let toks = match ts().inflight.pop_front() {
        Some(Inflight::Datum(t)) => t,
        _ => panic!("cbrt: receive of reference data that was not sent"),
    };
    let mut offs = take_vec(&mut r.pool);
    ref_offsets(r, dst as usize, ty, 0, &mut offs);
    let scope = top_scope(r);
    for (&off, &t) in offs.iter().zip(toks.iter()) {
        if t != 0 {
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
    let r = rt();
    give_vec(&mut r.pool, offs);
    give_vec(&mut r.pool, toks);
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
            flush_at_end();
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

// `[Spawn]`: the arguments are already in the block; the new thread
// runs `body(env)`, which makes the call and reports its result (on an
// OS thread that finished an earlier job, if one waits: `worker`).
#[no_mangle]
pub extern "C" fn cb_spawn(body: unsafe extern "C" fn(*mut u8), env: *mut u8, env_size: u64, res_ty: u32) -> u64 {
    // D-0204: the thread's id, its record in the book and the count of
    // running threads need no hold on the runtime; only the first spawn,
    // which turns locking on (`go_multi`), enters it.
    let _e = if multi() { None } else { Some(enter()) };
    let tid = NEXT_TID.fetch_add(1, Ordering::AcqRel);
    if sched_on() {
        sched_add(tid);
        SCHED_PENDING.with(|c| c.set(true));
    }
    book().threads.insert(tid, ThreadRec { status: Status::Running, env: env as usize, env_size, res_ty, res_obj: 0 });
    LIVE.fetch_add(1, Ordering::AcqRel);
    go_multi();
    let job = Job { tid, body: body as usize, env: env as usize, at: get_at() };
    // D-0202 phase 4: an OS thread that finished a job takes this one, if
    // one waits (`worker`); otherwise a new thread. Creating a thread, and
    // reserving and releasing its stack, cost far more than most threads'
    // work.
    let idle = IDLE.lock().unwrap_or_else(|e| e.into_inner()).pop();
    match idle {
        Some(w) => {
            *w.job.lock().unwrap_or_else(|e| e.into_inner()) = Some(job);
            w.cv.notify_one();
        }
        None => {
            let w = std::sync::Arc::new(Idle { job: Mutex::new(None), cv: Condvar::new() });
            // A program thread recurses as the main one does: a generous
            // stack, reserved rather than committed (D-0077 checks its floor).
            let _ = std::thread::Builder::new().stack_size(if cfg!(target_pointer_width = "64") { 64 << 20 } else { 16 << 20 }).spawn(move || worker(job, w));
        }
    }
    tid
}

// The threads running, the main one included, and the next thread's id
// (the main thread's is 1).
static LIVE: AtomicU64 = AtomicU64::new(1);
static NEXT_TID: AtomicU64 = AtomicU64::new(2);

// A program thread's job: what `spawn` hands an OS thread (`worker`).
struct Job {
    tid: u64,
    body: usize,
    env: usize,
    at: (u32, u32),
}

// An OS thread waiting for its next job.
struct Idle {
    job: Mutex<Option<Job>>,
    cv: Condvar,
}

static IDLE: Mutex<Vec<std::sync::Arc<Idle>>> = Mutex::new(Vec::new());

// How many finished OS threads wait for more work at most; more end.
const IDLE_MAX: usize = 64;

// An OS thread: its first job, then each one handed to it while it waits
// in `IDLE`. Every job starts as a new thread did (its thread id, a heap,
// its own stack of frames, the spawning statement's location) and leaves
// nothing behind.
fn worker(first: Job, me: std::sync::Arc<Idle>) {
    set_stack_floor();
    let mut job = first;
    loop {
        run_job(job);
        {
            let mut idle = IDLE.lock().unwrap_or_else(|e| e.into_inner());
            if idle.len() >= IDLE_MAX {
                return;
            }
            idle.push(me.clone());
        }
        let mut g = me.job.lock().unwrap_or_else(|e| e.into_inner());
        while g.is_none() {
            g = me.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        job = g.take().unwrap();
    }
}

fn run_job(job: Job) {
    TID.with(|t| t.set(job.tid));
    book().os.insert(job.tid, netio::os_thread());
    if sched_on() {
        sched_wait(job.tid, sched_lock());
    }
    // D-0202: the thread's own heap.
    {
        let _e = enter();
        let h = claim_heap(rt());
        HEAP.with(|c| c.set(h));
        set_heap(rt(), h);
    }
    let own = Box::into_raw(Box::new(TState { stack: Vec::new(), scopes: Vec::new(), inflight: VecDeque::new(), frame_locs: Vec::new(), dloc: (NOLOC, 0), box_draining: false, box_next: None, pend: Vec::new(), cunwind: false, jmp: 0, failed: None }));
    set_at(job.at.0, job.at.1);
    TSP.with(|c| c.set(own));
    let body: unsafe extern "C" fn(*mut u8) = unsafe { std::mem::transmute(job.body) };
    unsafe { body(job.env as *mut u8) };
    let _e = enter();
    LIVE.fetch_sub(1, Ordering::AcqRel);
    // D-0202: its heap is free for another thread once empty; used by one
    // thread again, it is no longer shared.
    let h = HEAP.with(|c| c.get()) as usize;
    if h != 0 {
        let r = rt();
        r.heap_users[h] = r.heap_users[h].saturating_sub(1);
        if r.heap_users[h] <= 1 {
            HEAP_SHARED[h].store(false, Ordering::Release);
        }
    }
    // `live` counts the main thread: 1 is none spawned still running.
    if LIVE.load(Ordering::Acquire) == 1 {
        SOLO_PENDING.store(true, Ordering::Relaxed);
    }
    TSP.with(|c| c.set(std::ptr::null_mut()));
    drop(unsafe { Box::from_raw(own) });
    drop(_e);
    book().os.remove(&job.tid);
    if sched_on() {
        sched_leave(job.tid);
    }
}

// `[Thread-Body-Done]`: the result (and its object, for a resource) is
// in the block, waiting to be claimed.
#[no_mangle]
pub extern "C" fn cb_thread_done(res_obj: u64) {
    if let Some(t) = book().threads.get_mut(&me()) {
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
    wait_until(tid, Wait::Join(tid), &mut |b: &mut Book| b.threads.get(&tid).map_or(true, |t| t.status != Status::Running));
}

// The block's remaining references stop being occurrences, the block
// is freed, and the result counts as taken.
fn free_env(tid: u64) {
    let r = rt();
    let Some((env, size)) = book().threads.get(&tid).map(|t| (t.env, t.env_size)) else { return };
    forget_slots(r, env, size);
    if let Some(t) = book().threads.get_mut(&tid) {
        t.status = Status::Taken;
        t.env = 0;
    }
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
    // D-0204: `[Join-Failed]`: the thread's contained fault, raised again
    // here.
    let failed = book().threads.get(&tid).and_then(|t| match t.status {
        Status::Failed(fid) => Some(fid),
        _ => None,
    });
    if let Some(fid) = failed {
        free_env(tid);
        reraise(fid);
    }
    let r = rt();
    let found = book().threads.get(&tid).filter(|t| t.status == Status::Done).map(|t| (t.env, t.res_ty, t.res_obj));
    let (env, res_ty, res_obj) = match found {
        Some(x) => x,
        None => panic!("cbrt: join of a thread whose result was already taken"),
    };
    let size = r.typ(res_ty).size;
    if size > 0 {
        unsafe { std::ptr::copy_nonoverlapping(env as *const u8, dst, size as usize) };
    }
    let res_obj = if res_obj != 0 { migrate(r, res_obj) } else { 0 };
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

// `[Handle-Destructor]`: waits; an unclaimed result is discarded. During
// a fault's (or `exit`'s) unwind nothing waits: the other threads take no
// further steps (`[Fault-Unwind]`), so a wait for one could never end;
// the thread is abandoned with the process.
fn handle_destructor(tid: u64) {
    if rt().unwinding {
        book().threads.remove(&tid);
        return;
    }
    // D-0204: while a contained fault unwinds this thread, its threads are
    // cancelled first, then waited for: none outlives its handle.
    let unwinding = ts_raw().cunwind;
    if unwinding {
        request_cancel(tid);
    }
    wait_done(tid);
    let r = rt();
    let (status, res_obj) = match book().threads.get(&tid).map(|t| (t.status, t.res_obj)) {
        Some(x) => x,
        None => return,
    };
    if status == Status::Done && res_obj != 0 && r.objs.contains_key(&res_obj) {
        destroy_obj(res_obj, None);
    }
    if matches!(status, Status::Done | Status::Failed(_)) {
        free_env(tid);
    }
    let asked = book().cancel.contains(&tid);
    book().threads.remove(&tid);
    // A failure nobody took is the destroyer's, raised again here -- unless
    // this thread is unwinding already, or asked for the stop.
    if let Status::Failed(fid) = status {
        let cancelled = book().failures[fid as usize].raw.starts_with("diag.thread-cancelled");
        if !unwinding && !(asked && cancelled) {
            reraise(fid);
        }
    }
}

// A contained failure raised again in this thread (`[Join-Failed]`): the
// same fault, at its own location.
fn reraise(fid: u64) -> ! {
    let (raw, file, line) = {
        let b = book();
        let f = &b.failures[fid as usize];
        (f.raw.clone(), f.file, f.line)
    };
    fault(&raw, file, line)
}

// D-0204: `std::try_join`'s and `std::is_finished`'s primitive
// (`join_status`): 0 while the thread runs, 1 when it finished (its
// result left for `join`), its failure's id plus 2 when it failed. Mode 0
// waits for the end and takes a failure (its block freed); mode 1 only
// looks.
#[no_mangle]
pub extern "C" fn cb_join_status(tid: u64, mode: u8) -> u64 {
    // Only looking (mode 1): the book alone.
    if mode == 1 {
        return match book().threads.get(&tid).map(|t| t.status) {
            Some(Status::Failed(fid)) => fid + 2,
            Some(Status::Running) => 0,
            _ => 1,
        };
    }
    let _e = enter();
    if mode == 0 {
        wait_done(tid);
    }
    let status = book().threads.get(&tid).map(|t| t.status);
    match status {
        Some(Status::Failed(fid)) => {
            if mode == 0 {
                free_env(tid);
            }
            fid + 2
        }
        Some(Status::Running) => 0,
        _ => 1,
    }
}

// D-0204: `cancel(&h)`: the thread's current or next wait faults
// (`diag.thread-cancelled`), unless it was lent an exclusive reference.
#[no_mangle]
pub extern "C" fn cb_cancel(tid: u64) {
    request_cancel(tid);
}

// Marks `tid` cancelled and wakes it: a runtime wait through its event
// key, a socket wait through its OS thread (under the book's lock, so the
// thread cannot have ended in between).
fn request_cancel(tid: u64) {
    CANCELS.fetch_add(1, Ordering::AcqRel);
    let key = {
        let mut b = book();
        b.cancel.insert(tid);
        if let Some(&t) = b.os.get(&tid) {
            netio::wake_thread(t);
        }
        b.wake.get(&tid).copied()
    };
    if let Some(k) = key {
        signal(k);
    }
}

// D-0204: a spawned thread's trampoline (`cobc`) gives the runtime the
// buffer its `setjmp` filled: a contained fault's unwind ends there.
#[no_mangle]
pub extern "C" fn cb_thread_jmp(jb: *mut u8) {
    ts().jmp = jb as usize;
}

// D-0204: as the thread starts, whether it was lent an exclusive
// reference: one held in its argument block, or in the captures of the
// closure it runs.
#[no_mangle]
pub extern "C" fn cb_thread_start(env: *mut u8, size: u64, h: *mut u8) {
    let _e = enter();
    let r = rt();
    let mut lent = false;
    let mut look = |lo: usize, n: usize| {
        r.slots.each_in(lo, lo + n, |t| {
            if r.paths.get(&t).map_or(false, |p| p.exclusive) {
                lent = true;
            }
        });
    };
    look(env as usize, size as usize);
    if !h.is_null() && r.fn_boxes.contains_key(&(h as usize)) {
        let b = unsafe { &*(h as *const CbFnBox) };
        look(b.data as usize, r.typ(b.ty).size as usize);
    }
    if lent {
        book().lent.insert(me());
    }
}

// D-0204: this thread locked a mutex whose interior may hold an exclusive
// reference (`cobc` decides by its type): it may now change data that is
// not its own, so it is never cancelled and its fault is not contained.
#[no_mangle]
pub extern "C" fn cb_thread_lent() {
    let me = me();
    if me != 1 {
        book().lent.insert(me);
    }
}

// D-0204: the trampoline's last step after a contained fault, its block's
// references forgotten and its callee dropped: the thread's result is the
// failure.
#[no_mangle]
pub extern "C" fn cb_thread_failed() {
    let fid = ts().failed.take().unwrap_or(0);
    if let Some(t) = book().threads.get_mut(&me()) {
        t.status = Status::Failed(fid);
    }
    signal(me());
}

// D-0204: `std::task_op`: a contained failure's report (op 0) or its
// diagnostic's name (op 1), at most `cap` bytes at `out`; the whole length
// back.
#[no_mangle]
pub unsafe extern "C" fn cb_task_op(op: u64, id: u64, out: *mut u8, cap: u64) -> i64 {
    let text: Vec<u8> = {
        let b = book();
        match b.failures.get(id as usize) {
            Some(f) if op == 0 => f.text.clone().into_bytes(),
            Some(f) => f.raw.split(diagnostics::DETAIL_SEP).next().unwrap_or("").as_bytes().to_vec(),
            None => Vec::new(),
        }
    };
    if text.len() as u64 <= cap {
        std::ptr::copy_nonoverlapping(text.as_ptr(), out, text.len());
    }
    text.len() as i64
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
    // D-0202: the book alone (the counts and the threads' states).
    match op {
        0 => {
            let mut b = book();
            b.last_event += 1;
            let id = b.last_event;
            b.events.insert(id, 0);
            id
        }
        1 => book().events.get(&id).copied().unwrap_or(0),
        2 => {
            // `[Channel-Deadlock]`: the main thread waits on a count no
            // other thread is running to change -- checked when the wait
            // starts and whenever it wakes (the last other thread ending
            // during it is seen at the next wake-up, within 50 ms).
            let is_main = me() == 1;
            wait_until(event_key(id), Wait::Event(id, seen), &mut |b: &mut Book| b.events.get(&id) != Some(&seen) || (is_main && !b.threads.values().any(|t| t.status == Status::Running)));
            let now = book().events.get(&id).copied();
            if now == Some(seen) {
                fault("diag.channel-deadlock", NOLOC, 0);
            }
            now.unwrap_or(0)
        }
        3 => {
            let c = {
                let mut b = book();
                let c = b.events.entry(id).or_insert(0);
                *c = c.wrapping_add(1);
                *c
            };
            signal(event_key(id));
            c
        }
        _ => {
            book().events.remove(&id);
            0
        }
    }
}

// ---- mutexes (spec/19 §2) ----

// `[Lock]`: `tok` is the shared reference to the mutex at `m`. Returns
// the lock path's token, which the guard holds.
#[no_mangle]
pub extern "C" fn cb_lock(tok: u64, m: *mut u8, file: u32, line: u32) -> u64 {
    let key = m as usize;
    let me = me();
    // D-0202: the checks and the guard's path under a local hold on the
    // heaps of the reference to the mutex and the paths it was formed
    // through; the wait under none (only the book's lock). Nothing between
    // can end or change that reference but this thread's own steps.
    {
        let _e = enter_planned(|p, r| p.lock_path(r, tok));
        // [Lock-Reentrant]: a fault, not a deadlock.
        if book().locks.get(&key) == Some(&me) {
            fault("diag.mutex-reentrant-lock", file, line);
        }
        let _ = path_target(rt(), tok, file, line);
    }
    // Claimed in the same hold of the book that finds it free.
    claim_mutex(key, me, file, line);
    let _e = enter_planned(|p, r| p.lock_path(r, tok));
    let r = rt();
    let (of, proj, excludes) = path_target(r, tok, file, line);
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

// The mutex at `key` claimed by `me`, in the same hold of the book that
// finds it free. D-0204: a poisoned mutex (`[Lock-Poisoned]`) is not
// claimed; locking it faults.
fn claim_mutex(key: usize, me: u64, file: u32, line: u32) {
    let mut poisoned = false;
    wait_until(mutex_key(key), Wait::Lock(key), &mut |b: &mut Book| {
        if b.poisoned.contains(&key) {
            poisoned = true;
            return true;
        }
        if b.locks.contains_key(&key) {
            return false;
        }
        b.locks.insert(key, me);
        true
    });
    if poisoned {
        fault("diag.mutex-poisoned", file, line);
    }
}

// D-0200: `[Lock]` for compiled code that reaches the mutex's interior
// only between this call and `cb_unlock_bare`, with nothing else (a
// native `Channel::send`/`recv`): the reentrancy fault and the wait as
// `cb_lock`'s, but no lock path (no guard is made; the caller checked its
// reference to the mutex).
#[no_mangle]
pub extern "C" fn cb_lock_bare(m: *mut u8, file: u32, line: u32) {
    // D-0202: the book alone.
    let key = m as usize;
    let me = me();
    let reentrant = book().locks.get(&key) == Some(&me);
    if reentrant {
        fault("diag.mutex-reentrant-lock", file, line);
    }
    claim_mutex(key, me, file, line);
}

// D-0200: `event_op(3, id, 0)` (a waiter's event signalled) and the mutex
// at `m` released, in one entry: a native `Channel::send`/`recv`'s last
// steps under the lock, in the body's order.
#[no_mangle]
pub extern "C" fn cb_unlock_signal(m: *mut u8, id: u64) {
    let key = m as usize;
    {
        let mut b = book();
        let c = b.events.entry(id).or_insert(0);
        *c = c.wrapping_add(1);
    }
    signal(event_key(id));
    book().locks.remove(&key);
    signal(mutex_key(key));
}

// D-0200: the mutex at `m` released, as a guard's drop releases it.
#[no_mangle]
pub extern "C" fn cb_unlock_bare(m: *mut u8) {
    let key = m as usize;
    book().locks.remove(&key);
    signal(mutex_key(key));
}

// `[Guard-Drop]`: the guard at `addr` holds the address of the mutex's
// interior (its first field, so the mutex's own address) and, in the
// slot table, the lock path.
fn guard_drop(addr: usize) {
    let r = rt();
    let key = unsafe { *(addr as *const usize) };
    book().locks.remove(&key);
    if let Some(&tok) = r.slots.get(&addr) {
        invalidate(r, tok);
    }
    signal(mutex_key(key));
}
