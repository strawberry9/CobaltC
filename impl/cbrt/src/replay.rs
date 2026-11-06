// D-0299 step 4: a run's steps written in the replay's action format
// (`proofs/Replay.lean`, D-0270), so that a compiled program's run is
// checked step by step against the proven machine of `proofs/`.
//
// On only when `COBALTC_REPLAY_TRACE` names a file; the trace is written
// there when the program ends. A build meant for it keeps every check
// (`cobc --all-checks`, `COBALTC_PATHLESS=0`) and records every reference
// (`COBALTC_RECORD=full`): `impl/tools/replay_runtime.py` does all of it.
// With the variable unset, every hook is one relaxed load.
//
// What the trace says, against the fragment's rules:
//
// - frames and statement scopes: `ben`, `bex`, `sen`, `sex`, as the runtime
//   pushes and pops them (a statement scope the runtime makes lazily is
//   written when it is pushed);
// - objects: `est` (`cb_new`), `bnd` (`cb_bind`), `dst`/`epl` (a `drop`, a
//   binding's early end, a temporary consumed), and `xfr` for a resource
//   taken out of one binding (`cb_take`) and bound in another;
// - paths: a borrow is `prj` for each projection step (a field, an index or
//   a payload is a field of the fragment), then `brw`, then `set` for the
//   steps; a read is `prj`…`rd`…`set`; a write `prj`…`wr`…`set`, the write
//   carrying the reference it stores when one is stored (`cb_store_ref`);
// - holding: a borrow is written pending (D-0107): the runtime cannot tell a
//   call's argument from any other borrow when it forms it, and pending is
//   what an argument is until bound. It stops being pending when something
//   holds it: a frame (`cb_frame_hold`, a parameter's `cb_rec_ref`) is a
//   holder object of the frame (`bpa`, or `est`/`bnd`/`wr` for a path already
//   bound), and a slot is a write into its object;
// - the two readings the fragment does not take yet (D-0270) as the
//   replay's gaps: `inv` when a path ends because its last holder let go,
//   `rsc` when a path leaves its statement (a result, a flight);
// - a fault of the alias and temporal rules as `flt DIAG` before the action
//   the runtime refused (the replay expects the machine to refuse it too);
//   any other fault ends the trace;
// - `out REASON` where the run leaves the fragment (threads, raw storage,
//   vectors, closures, slices, reference data, destructors): what came
//   before was checked, and nothing after it is written.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

// 0: not yet read; 1: off; 2: on; 3: on and stopped (an `out`, a fault).
static STATE: AtomicU8 = AtomicU8::new(0);
static WRITTEN: AtomicBool = AtomicBool::new(false);

struct Trace {
    path: String,
    lines: Vec<String>,
    // Identities of the trace's own making (holder objects, projection
    // paths, a borrow's path before the runtime names it): above every
    // identity the runtime gives.
    next: u64,
    frames: u64,
    scopes: u64,
    // The runtime's token, as the trace names it (a borrow is written
    // before its path is minted, so that a refused one can be written).
    alias: HashMap<u64, u64>,
    // Paths written pending and not yet bound.
    pending: HashSet<u64>,
    // The holder objects the trace made for a frame's hold of a path,
    // innermost last, per path.
    holders: HashMap<u64, Vec<Option<u64>>>,
    // Per thread: the statement scopes open (the trace's numbers), the
    // projection paths still to settle, the last write's path (for a
    // reference stored by the write), where the current entry's lines
    // began (for a fault).
    threads: HashMap<u64, Thr>,
    // A resource taken out of its binding (`cb_take`), not yet bound
    // again: the object and the root path it was taken through.
    taken: HashMap<u64, u64>,
    // Each object's cells (`cb_new`, `cb_move_to`), to find the object a
    // slot is in; its root path once bound; the references stored into it
    // before it was bound (a parameter's object, D-0292), written after.
    cells: HashMap<u64, (usize, u64)>,
    roots: HashMap<u64, u64>,
    early: HashMap<u64, Vec<(u64, u64)>>,
    // References stored into a slot before any object is established over
    // it (`cobc` stores a reference binding's value, then establishes its
    // object): by the slot's address, until that object is.
    orphans: Vec<(usize, u64, u64)>,
}

#[derive(Default)]
struct Thr {
    scopes: Vec<u64>,
    settle: Vec<u64>,
    last_write: Option<u64>, // the path the last write went through
    group: usize,
    // A statement scope the trace opened around an object established
    // outside every statement (a parameter's object, at a call `cobc` makes
    // with no statement scope of its own, D-0196), closed once it is bound.
    own_scope: bool,
}

static TRACE: Mutex<Option<Trace>> = Mutex::new(None);

#[inline(always)]
pub(crate) fn on() -> bool {
    match STATE.load(Ordering::Relaxed) {
        0 => init(),
        2 => true,
        _ => false,
    }
}

#[cold]
fn init() -> bool {
    let path = std::env::var("COBALTC_REPLAY_TRACE").unwrap_or_default();
    if path.is_empty() {
        STATE.store(1, Ordering::Relaxed);
        return false;
    }
    let mut g = TRACE.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(Trace { path, lines: vec!["# cbrt replay trace (D-0299 step 4)".to_string()], next: 1 << 60, frames: 1, scopes: 1, alias: HashMap::new(), pending: HashSet::new(), holders: HashMap::new(), threads: HashMap::new(), taken: HashMap::new(), cells: HashMap::new(), roots: HashMap::new(), early: HashMap::new(), orphans: Vec::new() });
    }
    STATE.store(2, Ordering::Relaxed);
    true
}

fn with<R>(f: impl FnOnce(&mut Trace) -> R) -> Option<R> {
    let mut g = TRACE.lock().unwrap_or_else(|e| e.into_inner());
    g.as_mut().map(f)
}

impl Trace {
    fn fresh(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn name(&self, tok: u64) -> u64 {
        self.alias.get(&tok).copied().unwrap_or(tok)
    }

    fn thr(&mut self, t: u64) -> &mut Thr {
        self.threads.entry(t).or_default()
    }

    // The projection paths a read or a borrow formed, settled; done before
    // any other line of the thread, except the reference a write stores.
    fn settle(&mut self, t: u64) {
        let s = std::mem::take(&mut self.thr(t).settle);
        for a in s.into_iter().rev() {
            self.lines.push(format!("set {} {}", t, a));
        }
        self.thr(t).last_write = None;
    }

    fn close_own(&mut self, t: u64) {
        if self.thr(t).own_scope {
            self.thr(t).own_scope = false;
            self.thr(t).scopes.pop();
            self.lines.push(format!("sex {}", t));
        }
    }

    // The start of an entry's lines.
    fn begin(&mut self, t: u64, keep: bool) {
        self.settle(t);
        if !keep {
            self.close_own(t);
        }
        let n = self.lines.len();
        self.thr(t).group = n;
    }

    // `prj` for each step from `base`; the path the last step forms.
    fn project(&mut self, t: u64, base: u64, steps: &[u64]) -> u64 {
        let mut cur = self.name(base);
        for &f in steps {
            let a = self.fresh();
            self.lines.push(format!("prj {} {} {} {}", t, cur, a, f));
            self.thr(t).settle.push(a);
            cur = a;
        }
        cur
    }

    // A path is held by a new object of the thread's current frame: a pending
    // one is bound so (`[Call]`'s parameter binding, `bpa`); one already bound
    // gains a holder.
    fn hold(&mut self, t: u64, tok: u64) -> Option<u64> {
        let a = self.name(tok);
        let o = self.fresh();
        let root = self.fresh();
        if self.pending.remove(&a) {
            self.lines.push(format!("bpa {} {} {} {}", t, a, o, root));
            return Some(o);
        }
        // Bound already: a further holder, a temporary of the current
        // statement adopted by the frame (`[Binding-Form]`) that stores it;
        // outside any statement (a parameter at a call made as a whole
        // statement's own frame) the path's holders are left as they are.
        if self.thr(t).scopes.is_empty() {
            return None;
        }
        self.lines.push(format!("est {} {} 0", t, o));
        self.lines.push(format!("bnd {} {} {}", t, o, root));
        self.lines.push(format!("wr {} {} 0 1 {}", t, root, a));
        Some(o)
    }
}

// The thread as the trace numbers it: the main thread is 0.
fn tid(me: u64) -> u64 {
    me.saturating_sub(1)
}

#[inline(always)]
fn emit(me: u64, f: impl FnOnce(&mut Trace, u64)) {
    emit_in(me, false, f)
}

// `keep`: an object's establishment and binding keep the scope the trace
// opened for them (`own_scope`); every other line closes it first.
#[inline(always)]
fn emit_in(me: u64, keep: bool, f: impl FnOnce(&mut Trace, u64)) {
    if on() {
        emit_on(me, keep, f);
    }
}

#[cold]
#[inline(never)]
fn emit_on(me: u64, keep: bool, f: impl FnOnce(&mut Trace, u64)) {
    with(|tr| {
        let t = tid(me);
        tr.begin(t, keep);
        f(tr, t)
    });
}

// The run leaves the fragment: the trace ends here.
#[inline(always)]
pub(crate) fn out(me: u64, why: &str) {
    if on() {
        out_on(me, why);
    }
}

#[cold]
#[inline(never)]
fn out_on(me: u64, why: &str) {
    with(|tr| {
        let t = tid(me);
        tr.settle(t);
        tr.lines.push(format!("out {}", why));
    });
    STATE.store(3, Ordering::Relaxed);
}

#[inline]
pub(crate) fn frame_push(me: u64) {
    emit(me, |tr, t| {
        let f = tr.frames;
        tr.frames += 1;
        tr.lines.push(format!("ben {} {}", t, f));
    });
}

#[inline]
pub(crate) fn frame_pop(me: u64) {
    emit(me, |tr, t| tr.lines.push(format!("bex {}", t)));
}

#[inline]
pub(crate) fn stmt_push(me: u64) {
    emit(me, |tr, t| {
        let s = tr.scopes;
        tr.scopes += 1;
        tr.thr(t).scopes.push(s);
        tr.lines.push(format!("sen {} {}", t, s));
    });
}

#[inline]
pub(crate) fn stmt_pop(me: u64) {
    emit(me, |tr, t| {
        tr.thr(t).scopes.pop();
        tr.lines.push(format!("sex {}", t));
    });
}

#[inline]
pub(crate) fn establish(me: u64, obj: u64, resource: bool, addr: usize, size: u64) {
    emit_in(me, true, |tr, t| {
        if tr.thr(t).scopes.is_empty() && !tr.thr(t).own_scope {
            let s = tr.scopes;
            tr.scopes += 1;
            tr.thr(t).own_scope = true;
            tr.thr(t).scopes.push(s);
            tr.lines.push(format!("sen {} {}", t, s));
        }
        tr.cells.insert(obj, (addr, size));
        tr.roots.remove(&obj);
        tr.early.remove(&obj);
        tr.lines.push(format!("est {} {} {}", t, obj, if resource { 1 } else { 0 }));
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut tr.orphans).into_iter().partition(|&(slot, _, _)| addr <= slot && slot + 8 <= addr + size as usize);
        tr.orphans = rest;
        if !mine.is_empty() {
            tr.early.insert(obj, mine.into_iter().map(|(_, old, tok)| (old, tok)).collect());
        }
    });
}

#[inline]
pub(crate) fn moved(obj: u64, addr: usize) {
    if !on() {
        return;
    }
    with(|tr| {
        if let Some(c) = tr.cells.get_mut(&obj) {
            c.0 = addr;
        }
    });
}

#[inline]
pub(crate) fn bind(me: u64, obj: u64, root: u64) {
    emit_in(me, true, |tr, t| {
        tr.alias.remove(&root);
        match tr.taken.remove(&obj) {
            // `[Authority-Transfer]`: the resource taken out of one binding is
            // bound here, by the thread that took it.
            Some(old) => {
                let o = tr.name(old);
                tr.lines.push(format!("xfr {} {} {} {} {} {}", t, t, o, obj, obj, root));
            }
            None => tr.lines.push(format!("bnd {} {} {}", t, obj, root)),
        }
        tr.roots.insert(obj, root);
        // References stored into it before it was bound, through its root.
        for (old, tok) in tr.early.remove(&obj).unwrap_or_default() {
            tr.store_via(t, root, old, tok);
        }
        tr.close_own(t);
    });
}

#[inline]
pub(crate) fn take(me: u64, obj: u64, root: u64) {
    emit(me, |tr, _| {
        tr.taken.insert(obj, root);
    });
}

// An object ends by itself: `drop(x)`, a binding ended at its last use, a
// temporary consumed. A taken resource still names its old root.
#[inline]
pub(crate) fn end(me: u64, obj: u64, resource: bool) {
    emit(me, |tr, t| {
        tr.taken.remove(&obj);
        tr.lines.push(format!("{} {} {}", if resource { "dst" } else { "epl" }, t, obj));
    });
}

// `[Borrow]` from `base` through `steps`, in `mode`; `slot` is where the
// trace names the new path, which the runtime gives once it is minted.
#[inline]
pub(crate) fn borrow(me: u64, base: u64, steps: &[u64], exclusive: bool) -> u64 {
    let mut a = 0;
    emit(me, |tr, t| {
        let from = tr.project(t, base, steps);
        a = tr.fresh();
        tr.pending.insert(a);
        tr.lines.push(format!("brw {} {} {} {} 1", t, from, a, if exclusive { 1 } else { 0 }));
    });
    a
}

#[inline]
pub(crate) fn minted(named: u64, tok: u64) {
    if named == 0 || !on() {
        return;
    }
    with(|tr| {
        tr.alias.insert(tok, named);
    });
}

#[inline]
pub(crate) fn read(me: u64, base: u64, steps: &[u64]) {
    emit(me, |tr, t| {
        let a = tr.project(t, base, steps);
        tr.lines.push(format!("rd {} {}", t, a));
    });
}

#[inline]
pub(crate) fn write(me: u64, base: u64, steps: &[u64]) {
    emit(me, |tr, t| {
        let a = tr.project(t, base, steps);
        tr.lines.push(format!("wr {} {} 0 0", t, a));
        tr.thr(t).last_write = Some(a);
    });
}

// A reference stored into a slot (`old`: what the slot held). By the write
// the thread made just before (`cobc` checks the place, `cb_write`, then
// stores the pointer and records it, `cb_store_ref`); or into an object
// established and not yet bound (a parameter's object, D-0292), written
// through its root once bound; or into a bound object with no write before
// it, through its root.
#[inline]
pub(crate) fn store(me: u64, slot: usize, old: u64, tok: u64) {
    if !on() {
        return;
    }
    let ok = with(|tr| {
        let t = tid(me);
        if let Some(a) = tr.thr(t).last_write {
            tr.store_via(t, a, old, tok);
            return true;
        }
        // The object the slot is in: the latest established that contains it.
        let o = tr.cells.iter().filter(|(_, &(a, n))| a <= slot && slot + 8 <= a + n as usize).map(|(o, _)| *o).max();
        let Some(o) = o else {
            tr.orphans.push((slot, old, tok));
            return true;
        };
        match tr.roots.get(&o).copied() {
            Some(root) => {
                tr.store_via(t, root, old, tok);
            }
            None => tr.early.entry(o).or_default().push((old, tok)),
        }
        true
    });
    if ok != Some(true) {
        out(me, "a reference stored into data the trace cannot place");
    }
}

impl Trace {
    // `[Write]` through `a` of a slot that held `old` and now holds `tok`.
    fn store_via(&mut self, t: u64, a: u64, old: u64, tok: u64) {
        let new = self.name(tok);
        if tok != 0 && self.pending.contains(&new) {
            // A stored reference is bound (D-0107: no pending path is stored):
            // the trace's holder object binds it first.
            self.hold(t, tok);
        }
        let removed = if old != 0 { format!("1 {}", self.name(old)) } else { "0".to_string() };
        let added = if tok != 0 { format!("1 {}", new) } else { "0".to_string() };
        self.lines.push(format!("wr {} {} {} {}", t, self.name(a), removed, added));
    }
}

#[inline]
pub(crate) fn frame_hold(me: u64, tok: u64) {
    emit(me, |tr, t| {
        let a = tr.name(tok);
        let o = tr.hold(t, tok);
        tr.holders.entry(a).or_default().push(o);
    });
}

// A parameter's reference: held by a parameter object of the callee's
// frame, which ends with the frame.
#[inline]
pub(crate) fn param(me: u64, tok: u64) {
    emit(me, |tr, t| {
        tr.hold(t, tok);
    });
}

#[inline]
pub(crate) fn frame_unhold(me: u64, tok: u64) {
    emit(me, |tr, t| {
        let a = tr.name(tok);
        if let Some(Some(o)) = tr.holders.get_mut(&a).and_then(|v| v.pop()) {
            tr.lines.push(format!("epl {} {}", t, o));
        }
    });
}

// A scope no thread has open: a path re-stamped to it outlives every
// statement until something holds it.
const NONE: u64 = u32::MAX as u64;

// A path that leaves its statement: a result for the parent statement, or
// one in flight (held by no scope).
#[inline]
pub(crate) fn restamp(me: u64, tok: u64, parent: bool) {
    emit(me, |tr, t| {
        let a = tr.name(tok);
        let sc = tr.thr(t).scopes.clone();
        let s = if parent { sc.len().checked_sub(2).map(|i| sc[i]).unwrap_or(NONE) } else { NONE };
        tr.lines.push(format!("rsc {} {}", a, s));
    });
}

#[inline]
pub(crate) fn flight_in(me: u64, tok: u64) {
    emit(me, |tr, t| {
        let a = tr.name(tok);
        let s = tr.thr(t).scopes.last().copied().unwrap_or(NONE);
        tr.lines.push(format!("rsc {} {}", a, s));
    });
}

// A path ended because its last holder let go (`[Object-End]`'s `Dead`).
#[inline]
pub(crate) fn dead(tok: u64) {
    if !on() {
        return;
    }
    with(|tr| {
        let a = tr.name(tok);
        tr.lines.push(format!("inv {}", a));
    });
}

// A fault: one of the alias and temporal rules' is written before the lines
// of the entry that raised it, which the machine must refuse too; any
// other ends the trace.
const COMPARED: [&str; 4] = ["diag.aliasing-conflict", "diag.stale-binding", "diag.destroy-while-aliased", "diag.move-while-aliased"];

#[inline]
pub(crate) fn fault(me: u64, id: &str) {
    if !on() {
        return;
    }
    let diag = id.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-')).next().unwrap_or(id).to_string();
    with(|tr| {
        let t = tid(me);
        let g = tr.thr(t).group.min(tr.lines.len());
        if COMPARED.contains(&diag.as_str()) && g < tr.lines.len() {
            tr.lines.insert(g, format!("flt {}", diag));
        } else {
            tr.lines.push(format!("out a fault the trace does not compare: {}", diag));
        }
    });
    STATE.store(3, Ordering::Relaxed);
}

// The program ends: the trace is written.
pub(crate) fn finish() {
    if STATE.load(Ordering::Relaxed) < 2 || WRITTEN.swap(true, Ordering::Relaxed) {
        return;
    }
    with(|tr| {
        let mut text = tr.lines.join("\n");
        text.push('\n');
        if let Err(e) = std::fs::write(&tr.path, text) {
            eprintln!("cbrt: cannot write the replay trace {}: {}", tr.path, e);
        }
    });
}
