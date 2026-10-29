// The dynamic half of the model: an abstract machine over Σ (`spec/04`),
// written from the rules of `spec/05`, `08`, `09`, `14`, `15`, `16`, `19`
// and `20` §2 for the fragment's constructs.
//
// Each function is compiled to a list of instructions; one instruction is
// one step of one thread (`[Thread-Step]`), at about the granularity of a
// rule: a binding lookup, a projection, a read, a write, a borrow, a
// statement's exit, a block's exit, a call, a spawn, a join. `Vec`'s
// operations are one step each, with the effects their `std` bodies have on
// Σ (`spec/21` §1: reclaim, release, the buffer's growth).
//
// `held-by` is not stored: it is computed from what live objects hold, as
// `spec/04` defines it (D-0215).

use crate::ast::*;
use crate::statics::{Ends, Proven};
use std::collections::{BTreeMap, BTreeSet};

pub type ObjId = u32;
pub type Tok = u32;
pub type Tid = u32;
pub type FrameId = u32;
pub type ScopeId = u32;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Val {
    Unit,
    Int(i64),
    Bool(bool),
    Ref(Tok),
    Hold(Tok),
    Vec { buf: u32, len: u32, cap: u32 },
    Handle(Tid),
    Opt(Option<Box<Val>>),
    Mutex(i64),
    Guard(Tok),
    Chan { q: Vec<Val>, closed: bool, cap: u32 },
    Res(Option<Box<Val>>), // Result<void, T>: Err(v) as Some(v)
    // A closure (`rule.fn.closure`): its body's function, its captures
    // (borrows, or the values moved in), whether it is a `move` closure
    // and whether it writes itself (D-0239).
    Clo { func: usize, caps: Vec<Val>, mv: bool, ws: bool },
    // A slice: its borrow, and the run's start and length in its source.
    Slice { tok: Tok, start: u32, len: u32 },
    Job(Tid), // struct Job { handle<void> h; }
    Arr(Vec<i64>), // array<i32, 2>
    // An `Atomic<i32>`: its value, and its wake count (`state.sync`'s
    // `wake-count`, D-0253).
    Atomic(i64, u64),
}

impl Val {
    // A resource, or a closure that owns one (`rule.type.is-resource`: a
    // closure whose captures include a resource is one by derivation).
    fn owns_resource(&self) -> bool {
        match self {
            Val::Vec { .. } | Val::Handle(_) | Val::Job(_) | Val::Mutex(_) | Val::Guard(_) | Val::Chan { .. } | Val::Atomic(..) => true,
            Val::Clo { caps, .. } => caps.iter().any(|c| c.owns_resource()),
            _ => false,
        }
    }

    fn refs(&self) -> Vec<Tok> {
        match self {
            Val::Ref(t) | Val::Hold(t) | Val::Guard(t) => vec![*t],
            Val::Slice { tok, .. } => vec![*tok],
            Val::Opt(Some(v)) | Val::Res(Some(v)) => v.refs(),
            Val::Chan { q, .. } => q.iter().flat_map(|v| v.refs()).collect(),
            Val::Clo { caps, .. } => caps.iter().flat_map(|v| v.refs()).collect(),
            _ => vec![],
        }
    }
}

// A step of a path (`spec/04` `steps`): a field (`Hold`'s `r` is 0, a
// mutex's interior 1, a closure's captures 0, 1, …), or a storage step
// over a vector's elements: a slice's range, an element's index (D-0238).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PStep {
    F(u8),
    R(u32, u32),
    I(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Obj {
    pub resource: bool,
    pub val: Option<Val>,              // None: uninitialized
    pub elem: Option<(u32, u32)>,      // a reclaimed element: (buffer, index)
    pub scope: Option<ScopeId>,
    pub origin: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    T(Tid),
    Transit,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path {
    pub of: ObjId,
    pub steps: Vec<PStep>,
    pub mode: Mode,
    pub thread: Owner,
    pub valid: bool,
    pub frame: FrameId,
    pub scope: Option<ScopeId>,
    pub base: Option<Tok>,
    pub pending: bool,
    // Formed by `[Lock]`: lock-derived (`spec/19` §2).
    pub lockp: bool,
    // A projection (`[Field-Access]`): it exists only for the one access
    // made through it (`spec/08` §4's reading; `[Field-Access]` itself does
    // not say when it ends — D-0219 finding).
    pub transient: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fault {
    pub diag: &'static str,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Running,
    Done(Val),
    Failed(Fault),
    Taken,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SVal {
    V(Val),
    Place(Tok),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Act {
    pub func: usize,
    pub pc: usize,
    pub stack: Vec<SVal>,
    pub slots: BTreeMap<String, Vec<Tok>>, // innermost binding last
    pub frames: Vec<FrameId>, // this call's open block frames, innermost last
    pub param_frame: FrameId,
    pub exiting: Option<Vec<ObjId>>, // a block exit in progress (it may wait on a handle)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Thread {
    pub acts: Vec<Act>,
    pub scopes: Vec<(ScopeId, FrameId)>,
    pub status: Status,
    // The thread has begun a wait (a join, a lock, a handle's end) that did
    // not close a cycle when it began (`[Deadlock]` (a)).
    pub waiting: bool,
    // Not `contained` (`spec/18`): lent an exclusive reference at its spawn,
    // or it has locked a mutex whose interior may hold one (a channel of
    // exclusive references): its fault ends the program.
    pub lent: bool,
    // D-0241: the root paths of the bindings `Ins::Licensed` named for the
    // current statement.
    pub licensed: BTreeSet<Tok>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    Thread(Tid),
    Chan,
    Atomic,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sync {
    Unlocked,
    LockedBy(Tid),
    Poisoned(Tid),
}

// D-0270: the trace of one run, as the mechanised fragment's actions
// (`proofs/Replay.lean` reads it): on only under `cobmodel trace`.
thread_local! {
    static TRACE: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

pub fn trace_start() {
    TRACE.with(|t| *t.borrow_mut() = Some(Vec::new()));
}

pub fn trace_take() -> Vec<String> {
    TRACE.with(|t| t.borrow_mut().take().unwrap_or_default())
}

pub fn tracing() -> bool {
    TRACE.with(|t| t.borrow().is_some())
}

pub fn tr(line: String) {
    TRACE.with(|t| {
        if let Some(v) = t.borrow_mut().as_mut() {
            v.push(line);
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct State {
    // D-0270, trace only (empty and false otherwise): the next borrow is a
    // pending argument's, or a lend; the next store is of what the trace
    // already wrote; object ends are a frame's (not written one by one);
    // an object moved whole and not yet re-bound; the handle and guard
    // objects the fragment's `spawn` and `lock` establish.
    pub pend_next: bool,
    pub lend_next: bool,
    pub quiet_store: bool,
    pub quiet_end: u32,
    pub emit_end: bool,
    pub moving: Option<(ObjId, Tok, Tid, Val)>,
    pub handle_obj: BTreeMap<Tid, ObjId>,
    pub guard_obj: BTreeMap<Tok, ObjId>,
    pub objs: BTreeMap<ObjId, Obj>,
    pub paths: BTreeMap<Tok, Path>,
    pub holder: BTreeMap<ObjId, Tok>,
    pub bufs: BTreeMap<u32, Vec<Val>>, // a Vec's buffer: its cells' values
    pub elems: BTreeMap<(u32, u32), ObjId>,
    pub threads: BTreeMap<Tid, Thread>,
    pub out: Vec<i64>,
    pub next: u32,
    pub clock: u64,
    pub sync: BTreeMap<ObjId, Sync>,
    // D-0258: each static's root path (a shared path to its object, which
    // never ends), in the program's order.
    pub statics: Vec<Tok>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ins {
    StmtEnter,
    // D-0241, `--as-licensed`: the bindings every access to which in this
    // statement the discharge table proves clash-free.
    Licensed(Vec<String>),
    StmtExit { keep: bool },
    BlockEnter,
    BlockExit(Vec<String>), // the names the block declares
    EndBinding(String),
    Int(i64),
    Var(String),
    Read,
    Deref,
    Field(u8),
    // In a closure body: a borrow capture's place, `*a_i` for the
    // reference `a_i` the field on the stack holds, used as stored
    // (`[Closure-Call]`, D-0239).
    CapRef,
    // An array from its elements on the stack.
    MkArr(usize),
    // A closure value from its captures on the stack.
    MkClo { func: usize, n: usize, mv: bool, ws: bool },
    // `[Closure-Call]` of the closure at the place on the stack.
    CallClo,
    // `spawn` of the closure value on the stack.
    SpawnClo,
    JobLit,
    // An argument read by value, or moved when it is a resource.
    ArgVal,
    // `[Slice-Form]` from the place on the stack (a `Vec`, or a slice binding).
    SliceForm { mode: Mode, lo: u32, hi: u32, arg: bool },
    // `[Index-Vec]` (D-0238) / `[Slice-Index]`: an element place.
    IndexVec(Mode),
    Borrow { mode: Mode, arg: bool },
    Add,
    Gt,
    Let(String),
    Assign,
    Call(usize, usize),
    VecNew,
    VecPush,
    VecPop,
    Swap,
    Replace,
    VecIndex(Mode),
    Spawn(usize, usize),
    Join,
    HoldLit,
    MutexNew,
    Lock,
    // D-0249, D-0253: `Atomic::new`; an operation on the atomic a shared
    // reference reaches (0 load, 1 store, 4 checked add), one step
    // (`[Atomic-Op]`); `notify_all` (`[Atomic-Notify]`); `Atomic::wait` as
    // `std` writes it: a check (read the wake count and the value, and
    // leave for the given address when the value differs) and a sleep
    // until the count moves (`[Atomic-Wait]`), back to the check.
    AtomicNew,
    // D-0258: the place of the k-th static (`[Static-Use]`), its root path.
    Static(usize),
    AtomicOp(u8),
    AtomicNotify,
    AtomicWaitCheck(usize),
    AtomicWaitSleep(usize),
    ChanNew,
    ChanLend,
    ChanSend,
    ChanRecv,
    ChanClose,
    MatchSome(usize), // an Option: Some(v) leaves v, None jumps
    Move,             // a resource place passed by value: `[Store-Binding-Place-Transfer]`
    Print,
    Pop,
    JumpIfFalse(usize),
    Jump(usize),
    Ret,
}

pub struct Code {
    pub funcs: Vec<(Func, Vec<(Ins, usize)>)>,
    // D-0258: the program's statics (name, first value), in order.
    pub statics: Vec<(String, i64)>,
}

impl Code {
    pub fn index(&self, name: &str) -> usize {
        self.funcs.iter().position(|f| f.0.name == name).expect("function")
    }
}

// ---------------------------------------------------------------- compiling

struct Comp<'a> {
    prog: &'a Program,
    ends: &'a Ends,
    licensed: Option<&'a Proven>,
    fname: String,
    out: Vec<(Ins, usize)>,
    block_no: usize,
    names: Vec<String>, // function names, for indices
    // In a closure body: each capture's field and whether it is a borrow.
    caps: BTreeMap<String, (u8, bool)>,
    // What a `break` or `continue` leaves, innermost last (`[Break]`: every
    // scope and frame up to and including the loop body).
    unw: Vec<Unw>,
}

enum Unw {
    Stmt,
    Block(Vec<String>),
    // A loop: its test's address and the `break` jumps to patch.
    Loop(usize, Vec<usize>),
}

impl<'a> Comp<'a> {
    // `[Break]` / `[Continue]`: exit each statement scope and block up to
    // the loop's body, then jump to the loop's exit or its test.
    fn leave_loop(&mut self, brk: bool, line: usize) {
        let mut seq = Vec::new();
        let mut at = None;
        for (k, u) in self.unw.iter().enumerate().rev() {
            match u {
                Unw::Stmt => seq.push(Ins::StmtExit { keep: false }),
                Unw::Block(names) => seq.push(Ins::BlockExit(names.clone())),
                Unw::Loop(..) => {
                    at = Some(k);
                    break;
                }
            }
        }
        // The loop's own statement scope lies below its entry and stays:
        // the test runs in it, and its exit follows the loop's end.
        let k = at.expect("break outside a loop (rejected statically)");
        for i in seq {
            self.emit(i, line);
        }
        let j = self.out.len();
        if brk {
            self.emit(Ins::Jump(0), line);
            if let Unw::Loop(_, bs) = &mut self.unw[k] {
                bs.push(j);
            }
        } else {
            let top = match &self.unw[k] {
                Unw::Loop(t, _) => *t,
                _ => unreachable!(),
            };
            self.emit(Ins::Jump(top), line);
        }
    }

    fn emit(&mut self, i: Ins, line: usize) {
        self.out.push((i, line));
    }
    fn fidx(&self, f: &str) -> usize {
        self.names.iter().position(|n| n == f).expect("helper")
    }

    fn expr(&mut self, e: &Expr, line: usize, arg: bool) {
        match e {
            Expr::Int(k) => self.emit(Ins::Int(*k), line),
            Expr::Var(n) => match self.caps.get(n).copied() {
                // A capture: the closure's field, reached through `self`;
                // a borrow capture is the place its reference reaches.
                Some((i, borrow)) => {
                    self.emit(Ins::Var("$self".into()), line);
                    self.emit(Ins::Field(i), line);
                    if borrow {
                        self.emit(Ins::CapRef, line);
                    }
                }
                None if self.prog.is_static(n) => {
                    let k = self.prog.statics.iter().position(|(s, _)| s == n).unwrap();
                    self.emit(Ins::Static(k), line);
                }
                None => self.emit(Ins::Var(n.clone()), line),
            },
            Expr::Deref(a) => {
                self.expr(a, line, false);
                self.emit(Ins::Deref, line);
            }
            Expr::Field(a) => {
                self.expr(a, line, false);
                self.emit(Ins::Field(0), line);
            }
            Expr::Index(v, k) => {
                // `v[k]`: the element place (`[Index-Vec]`, D-0238, or
                // `[Slice-Index]`); the mode is fixed by the context: see
                // `place`.
                self.index(v, *k, Mode::Shared, line);
            }
            Expr::Slice(m, v, lo, hi) => {
                self.expr(&Expr::Var(v.clone()), line, false);
                self.emit(Ins::SliceForm { mode: *m, lo: *lo as u32, hi: *hi as u32, arg }, line);
            }
            Expr::Closure(c) => {
                for x in &c.caps {
                    let xe = Expr::Var(x.clone());
                    if c.mv {
                        self.expr(&xe, line, false);
                        self.emit(Ins::Read, line);
                    } else {
                        let m = if writes_root(&c.body, x) { Mode::Excl } else { Mode::Shared };
                        self.place(&xe, m, line);
                        self.emit(Ins::Borrow { mode: m, arg: false }, line);
                    }
                }
                let ws = c.mv && c.caps.iter().any(|x| writes_root(&c.body, x));
                self.emit(Ins::MkClo { func: self.prog.funcs.len() + c.id, n: c.caps.len(), mv: c.mv, ws }, line);
            }
            Expr::CallVal(a) => {
                self.expr(a, line, false);
                self.emit(Ins::CallClo, line);
            }
            Expr::SpawnClo(a) => {
                self.expr(a, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::SpawnClo, line);
            }
            Expr::JobLit(a) => {
                self.expr(a, line, false);
                self.emit(Ins::ArgVal, line);
                self.emit(Ins::JobLit, line);
            }
            Expr::ArrLit(xs) => {
                for x in xs {
                    self.expr(x, line, false);
                    self.emit(Ins::Read, line);
                }
                self.emit(Ins::MkArr(xs.len()), line);
            }
            Expr::Borrow(m, a) => {
                self.place(a, *m, line);
                self.emit(Ins::Borrow { mode: *m, arg }, line);
            }
            Expr::Add(a, b) => {
                self.expr(a, line, false);
                self.emit(Ins::Read, line);
                self.expr(b, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Add, line);
            }
            Expr::Gt(a, b) => {
                self.expr(a, line, false);
                self.emit(Ins::Read, line);
                self.expr(b, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Gt, line);
            }
            Expr::Call(f, args) => match f.as_str() {
                "Vec::new" => self.emit(Ins::VecNew, line),
                "Mutex::new" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::MutexNew, line);
                }
                "lock" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::Lock, line);
                }
                "Channel::new" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::ChanNew, line);
                }
                "Atomic::new" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::AtomicNew, line);
                }
                "Atomic::load" | "Atomic::notify_all" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(if f == "Atomic::load" { Ins::AtomicOp(0) } else { Ins::AtomicNotify }, line);
                }
                "Atomic::store" | "Atomic::fetch_add" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.expr(&args[1], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::AtomicOp(if f == "Atomic::store" { 1 } else { 4 }), line);
                }
                "Atomic::wait" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.expr(&args[1], line, true);
                    self.emit(Ins::Read, line);
                    let check = self.out.len();
                    self.emit(Ins::AtomicWaitCheck(0), line);
                    self.emit(Ins::AtomicWaitSleep(check), line);
                    let end = self.out.len();
                    self.out[check].0 = Ins::AtomicWaitCheck(end);
                }
                "Channel::send" => {
                    // `lend_places(&mut v)` is the body's first line, before
                    // any wait.
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.expr(&args[1], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(Ins::ChanLend, line);
                    self.emit(Ins::ChanSend, line);
                }
                "Channel::recv" | "Channel::close" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::Read, line);
                    self.emit(if f == "Channel::recv" { Ins::ChanRecv } else { Ins::ChanClose }, line);
                }
                "Vec::push" => {
                    self.expr(&args[0], line, true);
                    self.expr(&args[1], line, true);
                    self.emit(Ins::ArgVal, line);
                    self.emit(Ins::VecPush, line);
                }
                "Vec::pop" => {
                    self.expr(&args[0], line, true);
                    self.emit(Ins::VecPop, line);
                }
                // `swap(a, b)`, `replace(r, v)` (`spec/21` §3b): the
                // references passed on, `[Swap-Places]` one step (D-0235).
                "swap" | "replace" => {
                    self.arg(&args[0], None, line);
                    self.arg(&args[1], None, line);
                    self.emit(if f == "swap" { Ins::Swap } else { Ins::Replace }, line);
                }
                // `Vec::index_exclusive(v, k)` called as written: the
                // reference passed on whole (D-0222's case).
                "Vec::index_exclusive" => {
                    self.arg(&args[0], None, line);
                    self.arg(&args[1], None, line);
                    self.emit(Ins::VecIndex(Mode::Excl), line);
                }
                _ => {
                    let params: Vec<Ty> = self.prog.func(f).map(|x| x.params.iter().map(|p| p.0.clone()).collect()).unwrap_or_default();
                    for (k, a) in args.iter().enumerate() {
                        self.arg(a, params.get(k), line);
                    }
                    let i = self.fidx(f);
                    self.emit(Ins::Call(i, args.len()), line);
                }
            },
            Expr::Spawn(f, args) => {
                let params: Vec<Ty> = self.prog.func(f).map(|x| x.params.iter().map(|p| p.0.clone()).collect()).unwrap_or_default();
                for (k, a) in args.iter().enumerate() {
                    self.arg(a, params.get(k), line);
                }
                let i = self.fidx(f);
                self.emit(Ins::Spawn(i, args.len()), line);
            }
            Expr::Join(t) => {
                self.emit(Ins::Var(t.clone()), line);
                self.emit(Ins::Join, line);
            }
            Expr::HoldLit(a) => {
                self.expr(a, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::HoldLit, line);
            }
        }
    }

    // An argument: a resource is moved from its place, anything else read.
    fn arg(&mut self, a: &Expr, pt: Option<&Ty>, line: usize) {
        if pt.map_or(false, |t| t.is_resource()) && matches!(a, Expr::Var(_)) {
            self.expr(a, line, false);
            self.emit(Ins::Move, line);
        } else {
            self.expr(a, line, true);
            self.emit(Ins::Read, line);
        }
    }

    fn index(&mut self, v: &str, k: i64, m: Mode, line: usize) {
        self.expr(&Expr::Var(v.to_string()), line, false);
        self.emit(Ins::Int(k), line);
        self.emit(Ins::IndexVec(m), line);
    }

    // A place expression evaluated as a place, for a write or a borrow of
    // mode `m`.
    fn place(&mut self, e: &Expr, m: Mode, line: usize) {
        match e {
            Expr::Index(v, k) => self.index(v, *k, m, line),
            // `*v[k]` written: the element by index_exclusive (D-0219 finding 6)
            Expr::Deref(a) if m == Mode::Excl && matches!(&**a, Expr::Index(..)) => {
                let Expr::Index(v, k) = &**a else { unreachable!() };
                self.index(v, *k, Mode::Excl, line);
                self.emit(Ins::Deref, line);
            }
            _ => self.expr(e, line, false),
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        let line = s.line;
        self.emit(Ins::StmtEnter, line);
        self.unw.push(Unw::Stmt);
        if let Some(p) = self.licensed {
            let mut names: Vec<String> = p.iter().filter(|((f, l, _), ok)| **ok && *f == self.fname && *l == line).map(|((_, _, x), _)| x.clone()).collect();
            names.sort();
            if !names.is_empty() {
                self.emit(Ins::Licensed(names), line);
            }
        }
        match &s.kind {
            StmtKind::Let { name, init, .. } => {
                self.expr(init, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Let(name.clone()), line);
            }
            StmtKind::Assign { place, value } => {
                self.place(place, Mode::Excl, line);
                self.expr(value, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Assign, line);
            }
            StmtKind::Expr(e) => {
                self.expr(e, line, false);
                self.emit(Ins::Pop, line);
            }
            StmtKind::Print(e) => {
                self.expr(e, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Print, line);
            }
            StmtKind::Block(b) => self.block(b, None),
            StmtKind::Discard(e) => {
                self.expr(e, line, false);
                self.emit(Ins::Read, line);
                self.emit(Ins::Pop, line);
            }
            StmtKind::Match { scrut, binder, some, none } => {
                self.expr(scrut, line, false);
                self.emit(Ins::Read, line);
                let m = self.out.len();
                self.emit(Ins::MatchSome(0), line);
                self.block(some, Some(binder.clone()));
                let j = self.out.len();
                self.emit(Ins::Jump(0), line);
                let l1 = self.out.len();
                self.block(none, None);
                let l2 = self.out.len();
                self.out[m].0 = Ins::MatchSome(l1);
                self.out[j].0 = Ins::Jump(l2);
            }
            StmtKind::While { cond, body } => {
                let top = self.out.len();
                self.expr(cond, line, false);
                self.emit(Ins::Read, line);
                let jf = self.out.len();
                self.emit(Ins::JumpIfFalse(0), line);
                self.unw.push(Unw::Loop(top, Vec::new()));
                self.block(body, None);
                let Some(Unw::Loop(_, breaks)) = self.unw.pop() else { unreachable!() };
                self.emit(Ins::Jump(top), line);
                let end = self.out.len();
                self.out[jf].0 = Ins::JumpIfFalse(end);
                for j in breaks {
                    self.out[j].0 = Ins::Jump(end);
                }
            }
            StmtKind::Break => self.leave_loop(true, line),
            StmtKind::Continue => self.leave_loop(false, line),
            StmtKind::If { cond, then, els } => {
                self.expr(cond, line, false);
                self.emit(Ins::Read, line);
                let jf = self.out.len();
                self.emit(Ins::JumpIfFalse(0), line);
                self.block(then, None);
                let j = self.out.len();
                self.emit(Ins::Jump(0), line);
                let l1 = self.out.len();
                if let Some(e) = els {
                    self.block(e, None);
                }
                let l2 = self.out.len();
                self.out[jf].0 = Ins::JumpIfFalse(l1);
                self.out[j].0 = Ins::Jump(l2);
            }
        }
        self.unw.pop();
        self.emit(Ins::StmtExit { keep: false }, line);
    }

    // A block; `binder` is a pattern binder its region declares, bound from
    // the value on the operand stack (a `match` arm's).
    fn block(&mut self, b: &Block, binder: Option<String>) {
        let me = self.block_no;
        self.block_no += 1;
        let first = b.stmts.first().map_or(b.end_line, |s| s.line);
        self.emit(Ins::BlockEnter, first);
        if let Some(n) = &binder {
            self.emit(Ins::Let(n.clone()), first);
        }
        let mut names: Vec<String> = b.stmts.iter().filter_map(|s| match &s.kind {
            StmtKind::Let { name, .. } => Some(name.clone()),
            _ => None,
        }).collect();
        if let Some(n) = binder {
            names.push(n);
        }
        self.unw.push(Unw::Block(names.clone()));
        for (i, s) in b.stmts.iter().enumerate() {
            self.stmt(s);
            if let Some(ns) = self.ends.get(&(self.fname.clone(), me, i)) {
                for n in ns {
                    self.emit(Ins::EndBinding(n.clone()), s.line);
                }
            }
        }
        if let Some(t) = &b.tail {
            self.emit(Ins::StmtEnter, b.tail_line);
            self.expr(t, b.tail_line, false);
            self.emit(Ins::StmtExit { keep: true }, b.tail_line);
        } else {
            self.emit(Ins::Int(0), b.end_line);
            self.emit(Ins::Pop, b.end_line);
        }
        self.unw.pop();
        // A fault at the exit (a destructor's wait, a handle's failure) is
        // reported at the block's last statement, as both tools report it
        // (the reporting form of an outcome is the implementation's,
        // `spec/22` §5).
        let exit_line = b.tail.as_ref().map(|_| b.tail_line).or_else(|| b.stmts.last().map(|s| last_line(s))).unwrap_or(b.end_line);
        self.emit(Ins::BlockExit(names), exit_line);
    }
}

// The line of a statement's last line of code (a nested block's last
// statement, recursively).
fn last_line(s: &Stmt) -> usize {
    match &s.kind {
        StmtKind::Block(b) => b.stmts.last().map(last_line).unwrap_or(s.line),
        _ => s.line,
    }
}

pub fn compile(prog: &Program, ends: &Ends) -> Code {
    compile_with(prog, ends, None)
}

// D-0241: `licensed` given, each statement first names the bindings whose
// accesses there the discharge table proves (`Ins::Licensed`).
pub fn compile_with(prog: &Program, ends: &Ends, licensed: Option<&Proven>) -> Code {
    let names: Vec<String> = prog.funcs.iter().map(|f| f.name.clone()).collect();
    let mut funcs = Vec::new();
    for f in &prog.funcs {
        let mut c = Comp { prog, ends, licensed, fname: f.name.clone(), out: Vec::new(), block_no: 0, names: names.clone(), caps: BTreeMap::new(), unw: Vec::new() };
        c.block(&f.body, None);
        c.emit(Ins::Ret, f.body.end_line);
        let _ = c.prog;
        funcs.push((f.clone(), c.out));
    }
    // Each closure's body, a function of its own after the program's,
    // numbered by the closure's id; `self` is its one parameter.
    let mut clos = Vec::new();
    for f in &prog.funcs {
        block_closures(&f.body, &mut clos);
    }
    clos.sort_by_key(|c| c.id);
    for cl in clos {
        let fname = format!("$clo{}", cl.id);
        let caps = cl.caps.iter().enumerate().map(|(i, x)| (x.clone(), (i as u8, !cl.mv))).collect();
        let mut c = Comp { prog, ends, licensed, fname: fname.clone(), out: Vec::new(), block_no: 0, names: names.clone(), caps, unw: Vec::new() };
        c.block(&cl.body, None);
        c.emit(Ins::Ret, cl.body.end_line);
        let line = cl.body.stmts.first().map_or(cl.body.end_line, |s| s.line);
        funcs.push((Func { name: fname, params: vec![], ret: cl.ret.clone(), body: cl.body.clone(), line }, c.out));
    }
    Code { funcs, statics: prog.statics.clone() }
}

// ---------------------------------------------------------------- running

fn replace_ref(v: &mut Val, from: Tok, to: Tok) {
    match v {
        Val::Ref(t) | Val::Hold(t) | Val::Guard(t) if *t == from => *t = to,
        Val::Slice { tok, .. } if *tok == from => *tok = to,
        Val::Opt(Some(b)) | Val::Res(Some(b)) => replace_ref(b, from, to),
        Val::Chan { q, .. } => q.iter_mut().for_each(|x| replace_ref(x, from, to)),
        Val::Clo { caps, .. } => caps.iter_mut().for_each(|x| replace_ref(x, from, to)),
        _ => {}
    }
}

pub enum Step {
    Next(State),
    Blocked,
    Fault(State, Fault),
    Exit(State), // the program ended normally
}

type Res<T> = Result<T, Fault>;

pub struct Machine<'a> {
    pub code: &'a Code,
    // The reading of `[Object-End]`'s Dead set at a block's exit: literally
    // (false), or sparing the references in the block's own result (true),
    // as `unwind-to` and `[Stmt-Exit]` spare the result `r` (D-0219,
    // finding 2: `fn pass(ref<i32, exclusive> p) : ref<i32, exclusive> { p }`).
    pub spare_result: bool,
}

fn fault<T>(diag: &'static str, line: usize) -> Res<T> {
    Err(Fault { diag, line })
}

impl State {
    pub fn new(code: &Code) -> State {
        let main = code.index("main");
        let mut s = State {
            pend_next: false,
            lend_next: false,
            quiet_store: false,
            quiet_end: 0,
            emit_end: false,
            moving: None,
            handle_obj: BTreeMap::new(),
            guard_obj: BTreeMap::new(),
            objs: BTreeMap::new(),
            paths: BTreeMap::new(),
            holder: BTreeMap::new(),
            bufs: BTreeMap::new(),
            elems: BTreeMap::new(),
            threads: BTreeMap::new(),
            out: Vec::new(),
            next: 1,
            clock: 0,
            sync: BTreeMap::new(),
            statics: Vec::new(),
        };
        let pf = s.fresh();
        s.threads.insert(0, Thread { acts: vec![Act { func: main, pc: 0, stack: vec![], slots: BTreeMap::new(), frames: vec![], param_frame: pf, exiting: None }], scopes: vec![], status: Status::Running, waiting: false, lent: false, licensed: BTreeSet::new() });
        // D-0258: the statics' objects, established in Σ_0 (`[Program]`):
        // resources that no frame owns and no scope ends, each with a
        // shared root path (the main thread's) every thread borrows from.
        for (_, k) in &code.statics {
            let o = s.fresh();
            s.objs.insert(o, Obj { resource: true, val: Some(Val::Atomic(*k, 0)), elem: None, scope: None, origin: 0 });
            let root = s.new_path(o, vec![], Mode::Shared, 0, None);
            s.statics.push(root);
        }
        s
    }

    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    fn th(&mut self, t: Tid) -> &mut Thread {
        self.threads.get_mut(&t).unwrap()
    }

    fn act(&mut self, t: Tid) -> &mut Act {
        self.threads.get_mut(&t).unwrap().acts.last_mut().unwrap()
    }

    fn cur_frame(&self, t: Tid) -> FrameId {
        let a = self.threads[&t].acts.last().unwrap();
        *a.frames.last().unwrap_or(&a.param_frame)
    }

    fn cur_scope(&self, t: Tid) -> Option<ScopeId> {
        self.threads[&t].scopes.last().map(|s| s.0)
    }

    fn alive(&self, o: ObjId) -> bool {
        self.objs.contains_key(&o)
    }

    pub fn temporally_valid(&self, a: Tok) -> bool {
        self.paths.get(&a).map_or(false, |p| p.valid && self.alive(p.of))
    }

    fn ancestors(&self, a: Tok) -> BTreeSet<Tok> {
        let mut s = BTreeSet::new();
        let mut cur = Some(a);
        while let Some(x) = cur {
            s.insert(x);
            cur = self.paths.get(&x).and_then(|p| p.base);
        }
        s
    }

    // `overlap` by steps: `meet` (D-0238) -- the lists agree step by step
    // over the shorter one's length; a range and an index inside it, two
    // intersecting ranges (an empty one meets nothing), equal indices and
    // equal fields agree.
    fn overlap(a: &[PStep], b: &[PStep]) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
            (PStep::F(i), PStep::F(j)) => i == j,
            (PStep::I(i), PStep::I(j)) => i == j,
            (PStep::R(i, j), PStep::I(k)) | (PStep::I(k), PStep::R(i, j)) => i <= k && k < j,
            (PStep::R(i, j), PStep::R(i2, j2)) => i.max(i2) < j.min(j2),
            _ => false,
        })
    }

    // `clash(a, m, Σ)` (`spec/04` §2).
    pub fn clash(&self, a: Tok, m: Mode) -> bool {
        let p = &self.paths[&a];
        let anc = self.ancestors(a);
        let ld = self.lock_derived(a);
        self.paths.iter().any(|(t2, q)| {
            q.valid
                && self.alive(q.of)
                && !q.pending
                && q.of == p.of
                && !anc.contains(t2)
                && Self::overlap(&p.steps, &q.steps)
                && !(m == Mode::Shared && q.mode == Mode::Shared)
                && !self.sync_exempt(ld, m, *t2)
        })
    }

    fn lock_derived(&self, a: Tok) -> bool {
        self.ancestors(a).iter().any(|t| self.paths[t].lockp)
    }

    // `sync-exempt(a, m, a', Σ)` (`spec/19` §2).
    fn sync_exempt(&self, a_ld: bool, m: Mode, a2: Tok) -> bool {
        let b_ld = self.lock_derived(a2);
        (a_ld && !b_ld && self.paths[&a2].mode == Mode::Shared) || (b_ld && !a_ld && m == Mode::Shared)
    }

    fn running(&self, t: Tid) -> bool {
        self.threads.get(&t).map_or(false, |x| x.status == Status::Running)
    }

    // D-0241: the diagnostic for a failed `¬clash` at an access through
    // `a`: under `--as-licensed`, an access the discharge table proved
    // (its root path named by `Ins::Licensed`) is the distinct outcome
    // `model.proven-but-false` -- an implementation the text licenses to
    // omit the check would have gone on.
    fn unlicensed(&self, a: Tok, me: Tid) -> &'static str {
        // Up through projections only (each adds a step); a borrow keeps its
        // base's steps and is another place of the table (`*q`).
        let mut r = a;
        while let Some(b) = self.paths[&r].base {
            if self.paths[&r].steps.len() <= self.paths[&b].steps.len() {
                return "diag.aliasing-conflict";
            }
            r = b;
        }
        if self.threads.get(&me).map_or(false, |x| x.licensed.contains(&r)) {
            "model.proven-but-false"
        } else {
            "diag.aliasing-conflict"
        }
    }

    // `[Thread-Affine]` (D-0213), with the take-over it allows; D-0237: a
    // spawned thread that takes a path over is no longer contained.
    fn affine(&mut self, a: Tok, me: Tid, line: usize) -> Res<()> {
        let p = &self.paths[&a];
        if p.mode != Mode::Excl {
            return Ok(());
        }
        match p.thread {
            Owner::T(t) if t == me => Ok(()),
            Owner::T(t) if self.running(t) => fault("diag.aliasing-conflict", line),
            _ => {
                if tracing() {
                    tr(format!("tak {} {}", me, a));
                }
                self.paths.get_mut(&a).unwrap().thread = Owner::T(me);
                if me != 0 {
                    self.th(me).lent = true;
                }
                Ok(())
            }
        }
    }

    // The live objects whose stored value holds path `a` (`held-by`).
    fn held_by(&self, a: Tok) -> BTreeSet<ObjId> {
        self.objs
            .iter()
            .filter(|(_, o)| match o.elem {
                Some((b, i)) => self.bufs.get(&b).and_then(|c| c.get(i as usize)).map_or(false, |v| v.refs().contains(&a)),
                None => o.val.as_ref().map_or(false, |v| v.refs().contains(&a)),
            })
            .map(|(i, _)| *i)
            .collect()
    }

    fn new_path(&mut self, of: ObjId, steps: Vec<PStep>, mode: Mode, me: Tid, base: Option<Tok>) -> Tok {
        let t = self.fresh();
        let frame = self.cur_frame(me);
        let scope = self.cur_scope(me);
        self.paths.insert(t, Path { of, steps, mode, thread: Owner::T(me), valid: true, frame, scope, base, pending: false, transient: false, lockp: false });
        t
    }

    // `[Borrow]`.
    fn borrow(&mut self, a0: Tok, m: Mode, me: Tid, line: usize) -> Res<Tok> {
        if !self.temporally_valid(a0) {
            return fault("diag.stale-binding", line);
        }
        if m == Mode::Excl && self.paths[&a0].mode != Mode::Excl {
            return fault("diag.borrow-exceeds-source", line);
        }
        self.affine(a0, me, line)?;
        if self.clash(a0, m) {
            return fault(self.unlicensed(a0, me), line);
        }
        let (of, steps) = (self.paths[&a0].of, self.paths[&a0].steps.clone());
        let a = self.new_path(of, steps, m, me, Some(a0));
        if tracing() {
            if self.lend_next {
                tr(format!("lnd {} {} {}", me, a0, a));
            } else {
                tr(format!("brw {} {} {} {} {}", me, a0, a, if m == Mode::Excl { 1 } else { 0 }, if self.pend_next { 1 } else { 0 }));
            }
            self.lend_next = false;
            self.pend_next = false;
        }
        self.consume(a0);
        Ok(a)
    }

    // A projection's one access is done: it ends, and so do the
    // projections and element borrows it was formed through.
    fn consume(&mut self, a: Tok) {
        let mut cur = Some(a);
        while let Some(x) = cur {
            match self.paths.get_mut(&x) {
                Some(p) if p.transient => {
                    p.valid = false;
                    cur = p.base;
                    if tracing() {
                        tr(format!("set 0 {}", x));
                    }
                }
                _ => break,
            }
        }
    }

    // D-0242: a path formed for one access -- a projection, `[Index-Vec]`'s
    // element borrow, the element place reached through it or through a
    // slice -- is pending until that access: no other access meets it, and
    // the access ends it (`consume`).
    fn one_access(&mut self, a: Tok) {
        let p = self.paths.get_mut(&a).unwrap();
        p.transient = true;
        p.pending = true;
    }

    fn value_at(&self, a: Tok) -> Option<Val> {
        let p = &self.paths[&a];
        let o = &self.objs[&p.of];
        if let Some((b, i)) = o.elem {
            return Some(self.bufs[&b][i as usize].clone());
        }
        let v = o.val.clone()?;
        if p.steps.is_empty() {
            return Some(v);
        }
        match (v, p.steps.last()) {
            (Val::Hold(t), _) => Some(Val::Ref(t)),
            (Val::Mutex(k), _) => Some(Val::Int(k)),
            (Val::Clo { caps, .. }, Some(PStep::F(i))) => caps.get(*i as usize).cloned(),
            (Val::Arr(xs), Some(PStep::I(k))) => xs.get(*k as usize).map(|v| Val::Int(*v)),
            _ => None,
        }
    }

    // `[Read]`, and D-0218's `[Read-Ref-Through-Shared]`: an exclusive
    // reference read through a shared path is a shared borrow of it.
    fn read(&mut self, a: Tok, me: Tid, line: usize) -> Res<Val> {
        if !self.temporally_valid(a) {
            return fault("diag.stale-binding", line);
        }
        if self.objs[&self.paths[&a].of].val.is_none() && self.objs[&self.paths[&a].of].elem.is_none() {
            return fault("diag.use-of-uninitialized", line);
        }
        self.affine(a, me, line)?;
        if self.clash(a, Mode::Shared) {
            return fault(self.unlicensed(a, me), line);
        }
        let v = self.value_at(a).unwrap();
        if tracing() {
            tr(format!("rd {} {}", me, a));
        }
        // `[Read-Fn-Through-Shared]` (D-0218, D-0239): a closure read
        // through a shared path that holds an exclusive capture, or writes
        // itself.
        if self.paths[&a].mode == Mode::Shared && self.fn_writes(&v) {
            return fault("diag.borrow-exceeds-source", line);
        }
        self.consume(a);
        // `[Fn-Value-Read]` (D-0089): a closure that owns a resource moves
        // to the result, and the place's `fn` value is left empty.
        if matches!(v, Val::Clo { .. }) && v.owns_resource() {
            let o = self.paths[&a].of;
            self.store_raw(o, &self.paths[&a].steps.clone(), Val::Unit);
        }
        if self.paths[&a].mode == Mode::Shared {
            if let Val::Ref(t) = v {
                if self.paths.get(&t).map_or(false, |p| p.mode == Mode::Excl) {
                    let b = self.borrow(t, Mode::Shared, me, line)?;
                    return Ok(Val::Ref(b));
                }
            }
        }
        Ok(v)
    }

    // A closure with a capture that is an exclusive reference (a borrow
    // capture its body writes, or a moved exclusive one), or one that writes
    // itself (D-0239).
    fn fn_writes(&self, v: &Val) -> bool {
        match v {
            Val::Clo { caps, ws, .. } => *ws || caps.iter().any(|c| matches!(c, Val::Ref(t) if self.paths.get(t).map_or(false, |p| p.mode == Mode::Excl))),
            _ => false,
        }
    }

    // The threads the destructor of what `o` holds waits for, in its
    // order (`[Handle-Destructor]`; a `Vec`'s elements first to last).
    fn handles_in(&self, o: ObjId) -> Vec<Tid> {
        match self.objs.get(&o).and_then(|x| x.val.as_ref()) {
            Some(Val::Handle(h)) | Some(Val::Job(h)) => vec![*h],
            Some(Val::Vec { buf, len, .. }) => self.bufs.get(buf).map_or(vec![], |c| c.iter().take(*len as usize).filter_map(|v| if let Val::Handle(h) = v { Some(*h) } else { None }).collect()),
            _ => vec![],
        }
    }

    // The thread the next step of `o`'s destruction waits for: its first
    // handle not yet taken, while that thread runs.
    pub fn first_wait(&self, o: ObjId) -> Option<Tid> {
        let h = self.handles_in(o).into_iter().find(|h| self.threads[h].status != Status::Taken)?;
        (self.threads[&h].status == Status::Running).then_some(h)
    }

    // A place's value replaced with no access and no bookkeeping: the empty
    // `fn` value a move out of it leaves.
    fn store_raw(&mut self, o: ObjId, steps: &[PStep], v: Val) {
        let cur = self.objs[&o].val.clone();
        let nv = match (cur, steps.last()) {
            (Some(Val::Clo { func, mut caps, mv, ws }), Some(PStep::F(i))) => {
                caps[*i as usize] = v;
                Val::Clo { func, caps, mv, ws }
            }
            _ => v,
        };
        self.objs.get_mut(&o).unwrap().val = Some(nv);
    }

    // `[Write]`, with D-0215's held-by bookkeeping.
    fn write(&mut self, a: Tok, v: Val, me: Tid, line: usize) -> Res<()> {
        if !self.temporally_valid(a) {
            return fault("diag.stale-binding", line);
        }
        if self.paths[&a].mode != Mode::Excl {
            return fault("diag.write-through-shared", line);
        }
        self.affine(a, me, line)?;
        if self.clash(a, Mode::Excl) {
            return fault(self.unlicensed(a, me), line);
        }
        self.store(a, v, me);
        self.consume(a);
        Ok(())
    }

    // The cells written, and the references the write drops released.
    fn store(&mut self, a: Tok, v: Val, me: Tid) {
        let p = self.paths[&a].clone();
        let o = p.of;
        if let Some((b, i)) = self.objs[&o].elem {
            let old: BTreeSet<Tok> = self.bufs[&b][i as usize].refs().into_iter().collect();
            let before: BTreeMap<Tok, BTreeSet<ObjId>> = old.iter().map(|t| (*t, self.held_by(*t))).collect();
            self.bufs.get_mut(&b).unwrap()[i as usize] = v;
            let kept: BTreeSet<Tok> = self.bufs[&b][i as usize].refs().into_iter().collect();
            self.release_gone(&old, &kept, &before, o, me);
            return;
        }
        let old_refs: BTreeSet<Tok> = self.objs[&o].val.as_ref().map(|x| x.refs().into_iter().collect()).unwrap_or_default();
        let before: BTreeMap<Tok, BTreeSet<ObjId>> = old_refs.iter().map(|t| (*t, self.held_by(*t))).collect();
        let newval = if p.steps.is_empty() {
            v
        } else {
            match (v, self.objs[&o].val.clone(), p.steps.last()) {
                (x, Some(Val::Clo { func, mut caps, mv, ws }), Some(PStep::F(i))) => {
                    caps[*i as usize] = x;
                    Val::Clo { func, caps, mv, ws }
                }
                (Val::Int(v), Some(Val::Arr(mut xs)), Some(PStep::I(k))) => {
                    xs[*k as usize] = v;
                    Val::Arr(xs)
                }
                (Val::Int(k), Some(Val::Mutex(_)), _) => Val::Mutex(k),
                (Val::Ref(t), _, _) => Val::Hold(t),
                (x, _, _) => x,
            }
        };
        self.objs.get_mut(&o).unwrap().val = Some(newval);
        let kept: BTreeSet<Tok> = self.objs[&o].val.as_ref().unwrap().refs().into_iter().collect();
        if tracing() {
            if self.quiet_store {
                self.quiet_store = false;
            } else {
                let olds: Vec<String> = old_refs.iter().map(|t| t.to_string()).collect();
                let news: Vec<String> = kept.iter().map(|t| t.to_string()).collect();
                tr(format!("wr {} {} {} {} {} {}", me, a, olds.len(), olds.join(" "), news.len(), news.join(" ")));
            }
        }
        self.release_gone(&old_refs, &kept, &before, o, me);
    }

    // `[Write]`: a reference the write drops whose only holder was o, and
    // whose statement has ended, is invalid.
    fn release_gone(&mut self, old_refs: &BTreeSet<Tok>, kept: &BTreeSet<Tok>, before: &BTreeMap<Tok, BTreeSet<ObjId>>, o: ObjId, me: Tid) {
        let open: BTreeSet<ScopeId> = self.threads[&me].scopes.iter().map(|s| s.0).collect();
        for g in old_refs.difference(kept) {
            let hb = &before[g];
            if hb.len() == 1 && hb.contains(&o) {
                let sc = self.paths[g].scope;
                if sc.map_or(true, |s| !open.contains(&s)) {
                    if tracing() && self.paths[g].valid {
                        tr(format!("inv {}", g));
                    }
                    self.paths.get_mut(g).unwrap().valid = false;
                }
            }
        }
    }

    // `[Object-Establish-*]` and `[Binding-Form]`, then the first write.
    fn bind(&mut self, name: &str, v: Val, me: Tid) -> Tok {
        let resource = v.owns_resource();
        let is_mutex = matches!(v, Val::Mutex(_));
        let o = self.fresh();
        let origin = self.tick();
        let scope = self.cur_scope(me);
        self.objs.insert(o, Obj { resource, val: None, elem: None, scope, origin });
        let a = self.new_path(o, vec![], Mode::Excl, me, None);
        self.holder.insert(o, a);
        self.act(me).slots.entry(name.to_string()).or_default().push(a);
        if tracing() {
            let moved = matches!(&self.moving, Some((_, _, _, mv)) if *mv == v);
            if moved {
                let (mo, ma, mt, _) = self.moving.take().unwrap();
                tr(format!("xfr {} {} {} {} {} {}", mt, me, ma, mo, o, a));
                self.quiet_store = true;
            } else {
                match &v {
                    Val::Handle(h) if self.handle_obj.contains_key(h) => {
                        tr(format!("bnh {} {} {} {}", me, o, a, self.handle_obj[h]));
                        self.quiet_store = true;
                    }
                    Val::Guard(g) if self.guard_obj.contains_key(g) => {
                        tr(format!("bnh {} {} {} {}", me, o, a, self.guard_obj[g]));
                        self.quiet_store = true;
                    }
                    Val::Handle(_) | Val::Guard(_) => tr("out a handle or guard the trace did not see made".to_string()),
                    Val::Job(_) => tr("out job".to_string()),
                    Val::Vec { .. } => tr("out vec".to_string()),
                    Val::Chan { .. } => tr("out channel".to_string()),
                    Val::Clo { .. } => tr("out closure".to_string()),
                    Val::Slice { .. } => tr("out slice".to_string()),
                    Val::Arr(_) => tr("out array".to_string()),
                    _ => {
                        tr(format!("est {} {} {}", me, o, if resource { 1 } else { 0 }));
                        tr(format!("bnd {} {} {}", me, o, a));
                    }
                }
            }
        }
        self.store(a, v, me);
        if is_mutex {
            self.sync.insert(o, Sync::Unlocked);
        }
        a
    }

    // `[Object-End]`.
    fn end_object(&mut self, o: ObjId, exempt: &BTreeSet<Tok>) {
        if tracing() && self.emit_end && self.quiet_end == 0 {
            tr(format!("{} 0 {}", if self.objs[&o].resource { "dst" } else { "epl" }, o));
        }
        let hold: BTreeMap<Tok, BTreeSet<ObjId>> = self.paths.keys().map(|t| (*t, self.held_by(*t))).filter(|(_, h)| h.contains(&o)).collect();
        for (t, p) in self.paths.iter_mut() {
            if p.of == o {
                p.valid = false;
            }
            if let Some(h) = hold.get(t) {
                if h.len() == 1 {
                    let spare = exempt.contains(t);
                    if !spare {
                        if p.valid && tracing() {
                            tr(format!("inv {}", t));
                        }
                        p.valid = false;
                    }
                }
            }
        }
        if let Some((b, i)) = self.objs[&o].elem {
            self.elems.remove(&(b, i));
        }
        self.objs.remove(&o);
        self.holder.remove(&o);
        self.sync.remove(&o);
    }

    // The reclaimed object over a buffer slot: re-attached, or established
    // (`[Reclaim]`; `[Rawptr-Write]` when a reference is written there).
    fn elem_obj(&mut self, buf: u32, i: u32, me: Tid) -> ObjId {
        if let Some(o) = self.elems.get(&(buf, i)) {
            return *o;
        }
        let o = self.fresh();
        let origin = self.tick();
        let scope = self.cur_scope(me);
        self.objs.insert(o, Obj { resource: false, val: None, elem: Some((buf, i)), scope, origin });
        self.elems.insert((buf, i), o);
        o
    }

    // `foreign-held(o, ℓ, Σ)` (`spec/20` §2, D-0214).
    fn foreign_held(&self, o: ObjId, me: Tid) -> bool {
        self.paths.iter().any(|(t, p)| {
            p.valid
                && p.of == o
                && p.base.is_some()
                && p.mode == Mode::Excl
                && !self.held_by(*t).is_empty()
                && match p.thread {
                    Owner::Transit => true,
                    Owner::T(x) => x != me && self.running(x),
                }
        })
    }

    // `[Release]` over every element object of buffer `b` at index ≥ from.
    fn release_elems(&mut self, b: u32, from: u32, to: u32, me: Tid, line: usize) -> Res<()> {
        let objs: Vec<ObjId> = self.elems.iter().filter(|((bb, i), _)| *bb == b && *i >= from && *i < to).map(|(_, o)| *o).collect();
        for o in &objs {
            if self.foreign_held(*o, me) {
                return fault("diag.aliasing-conflict", line);
            }
        }
        for o in objs {
            self.end_object(o, &BTreeSet::new());
        }
        Ok(())
    }
}

impl<'a> Machine<'a> {
    pub fn line(&self, s: &State, t: Tid) -> usize {
        let a = s.threads[&t].acts.last().unwrap();
        self.code.funcs[a.func].1.get(a.pc).map_or(0, |x| x.1)
    }

    // What thread t's next instruction waits on, if it cannot go on now:
    // the thread it joins (or whose handle it ends), or the holder of the
    // mutex it locks.
    fn wait_target(&self, s: &State, t: Tid) -> Option<Tid> {
        match self.wait_on(s, t)? {
            Wait::Thread(h) => Some(h),
            Wait::Chan | Wait::Atomic => None,
        }
    }

    // D-0253: a thread at the end of a chain of waits (`[Deadlock]` (b)):
    // one waiting on a channel or on an atomic, not on a thread or a mutex.
    pub fn waits_at_chain_end(&self, s: &State, t: Tid) -> bool {
        matches!(self.wait_on(s, t), Some(Wait::Chan) | Some(Wait::Atomic))
    }

    // Is thread t's next instruction a wait that cannot end now?
    pub fn waits(&self, s: &State, t: Tid) -> bool {
        self.wait_on(s, t).is_some()
    }

    pub fn waits_on_channel(&self, s: &State, t: Tid) -> bool {
        matches!(self.wait_on(s, t), Some(Wait::Chan))
    }

    fn chan_of(s: &State, v: Option<&SVal>) -> Option<(bool, usize, u32)> {
        let Some(SVal::V(Val::Ref(r))) = v else { return None };
        match s.objs.get(&s.paths.get(r)?.of)?.val.as_ref()? {
            Val::Chan { q, closed, cap } => Some((*closed, q.len(), *cap)),
            _ => None,
        }
    }

    fn wait_on(&self, s: &State, t: Tid) -> Option<Wait> {
        let th = &s.threads[&t];
        let a = th.acts.last()?;
        let (ins, _) = &self.code.funcs[a.func].1[a.pc];
        match ins {
            Ins::ChanSend => {
                // stack: …, Ref(channel), value
                let (closed, n, cap) = Self::chan_of(s, a.stack.get(a.stack.len().wrapping_sub(2)))?;
                (!closed && n as u32 >= cap).then_some(Wait::Chan)
            }
            Ins::ChanRecv => {
                let (closed, n, _) = Self::chan_of(s, a.stack.last())?;
                (!closed && n == 0).then_some(Wait::Chan)
            }
            Ins::Join => match a.stack.last() {
                Some(SVal::Place(p)) => match s.objs.get(&s.paths[p].of).and_then(|o| o.val.clone()) {
                    Some(Val::Handle(h)) if s.threads[&h].status == Status::Running => Some(Wait::Thread(h)),
                    _ => None,
                },
                _ => None,
            },
            Ins::AtomicWaitSleep(_) => {
                // stack: …, Ref(atomic), expected, seen
                let n = a.stack.len();
                let (Some(SVal::V(Val::Ref(r))), Some(SVal::V(Val::Int(seen)))) = (a.stack.get(n.wrapping_sub(3)), a.stack.get(n.wrapping_sub(1))) else { return None };
                match s.objs.get(&s.paths.get(r)?.of)?.val.as_ref()? {
                    Val::Atomic(_, w) if *w as i64 == *seen => Some(Wait::Atomic),
                    _ => None,
                }
            }
            Ins::Lock => match a.stack.last() {
                Some(SVal::V(Val::Ref(r))) => match s.paths.get(r).and_then(|p| s.sync.get(&p.of)) {
                    Some(Sync::LockedBy(h)) if *h != t => Some(Wait::Thread(*h)),
                    _ => None,
                },
                _ => None,
            },
            Ins::BlockExit(_) => {
                // Only once the exit has ended what comes before the handle
                // (`[Block-Exit]`'s order) does it wait on the handle -- a
                // handle held by a binding, a `Job` or a `Vec`.
                let first = a.exiting.as_ref().and_then(|l| l.first().copied())?;
                s.first_wait(first).map(Wait::Thread)
            }
            _ => None,
        }
    }

    // Does thread t's wait close a cycle of waits (`[Deadlock]` (a))?
    fn closes_cycle(&self, s: &State, t: Tid) -> bool {
        let mut cur = self.wait_target(s, t);
        let mut seen = BTreeSet::new();
        while let Some(u) = cur {
            if u == t {
                return true;
            }
            if !seen.insert(u) || !s.threads[&u].waiting {
                return false;
            }
            cur = self.wait_target(s, u);
        }
        false
    }

    // Can thread t take a step now? A thread at a wait steps to begin it
    // (or to fault, when it closes a cycle), and once waiting steps again
    // only when the wait can end.
    pub fn enabled(&self, s: &State, t: Tid) -> bool {
        let th = &s.threads[&t];
        if th.status != Status::Running {
            return false;
        }
        if self.waits(s, t) {
            !th.waiting
        } else {
            true
        }
    }

    pub fn step(&self, mut s: State, t: Tid) -> Step {
        let line = self.line(&s, t);
        match self.exec(&mut s, t) {
            Ok(Some(true)) => Step::Exit(s),
            Ok(Some(false)) => Step::Blocked,
            Ok(None) => Step::Next(s),
            Err(f) => {
                let f = if f.line == 0 { Fault { diag: f.diag, line } } else { f };
                if t == 0 || s.threads[&t].lent {
                    Step::Fault(s, f)
                } else {
                    self.contain(&mut s, t, f);
                    Step::Next(s)
                }
            }
        }
    }

    // `[Fault-Contain]`: every mutex the thread holds is poisoned, then its
    // frames unwind and it has failed.
    pub fn contain(&self, s: &mut State, t: Tid, f: Fault) {
        if tracing() {
            tr(format!("fct {}", t));
            s.quiet_end += 1;
        }
        for (_, st) in s.sync.iter_mut() {
            if *st == Sync::LockedBy(t) {
                *st = Sync::Poisoned(t);
            }
        }
        s.th(t).waiting = false;
        self.unwind(s, t);
        s.th(t).status = Status::Failed(f);
    }

    fn unwind(&self, s: &mut State, t: Tid) {
        // End every object the thread's frames own, innermost first.
        let frames: BTreeSet<FrameId> = s.threads[&t].acts.iter().flat_map(|a| a.frames.iter().copied().chain(std::iter::once(a.param_frame))).collect();
        // Last bound first (`[Block-Exit]`): by when each object's holder was
        // formed, so a moved object goes where it was moved to (D-0233).
        let mut owned: Vec<(u64, ObjId)> = s.holder.iter().filter(|(_, a)| frames.contains(&s.paths[*a].frame)).map(|(o, a)| (u64::from(*a), *o)).collect();
        owned.sort();
        owned.reverse();
        for (_, o) in owned {
            if s.alive(o) {
                // The unwind runs every destructor D-0008 would run (`spec/18`
                // §1): a vector releases its elements, a guard unlocks (a
                // poisoned mutex stays poisoned).
                match s.objs[&o].val.clone() {
                    Some(Val::Vec { buf, .. }) => {
                        let objs: Vec<ObjId> = s.elems.iter().filter(|((b, _), _)| *b == buf).map(|(_, x)| *x).collect();
                        for e in objs {
                            s.end_object(e, &BTreeSet::new());
                        }
                        s.bufs.remove(&buf);
                    }
                    Some(Val::Guard(g)) => {
                        let m = s.paths[&g].of;
                        if let Some(Sync::LockedBy(_)) = s.sync.get(&m) {
                            s.sync.insert(m, Sync::Unlocked);
                        }
                        s.paths.get_mut(&g).unwrap().valid = false;
                    }
                    _ => {}
                }
                s.end_object(o, &BTreeSet::new());
            }
        }
        let scopes: Vec<ScopeId> = s.threads[&t].scopes.iter().map(|x| x.0).collect();
        for (_, p) in s.paths.iter_mut() {
            if p.scope.map_or(false, |x| scopes.contains(&x)) && p.valid {
                p.valid = false;
            }
        }
        s.th(t).scopes.clear();
        if tracing() {
            s.quiet_end -= 1;
        }
    }

    fn pop(s: &mut State, t: Tid) -> SVal {
        s.act(t).stack.pop().expect("stack")
    }

    // `[Atomic-Op]`'s checks on `a`: temporally valid, `[Thread-Affine]`,
    // and a shared access that clashes with nothing (its ancestors
    // excluded); the operation forms no path. The atomic's object.
    fn atomic_access(s: &mut State, t: Tid, am: Tok, line: usize) -> Res<ObjId> {
        if let Some(p) = s.paths.get_mut(&am) {
            p.pending = false;
        }
        if !s.temporally_valid(am) {
            return fault("diag.stale-binding", line);
        }
        s.affine(am, t, line)?;
        if s.clash(am, Mode::Shared) {
            return fault("diag.aliasing-conflict", line);
        }
        Ok(s.paths[&am].of)
    }

    fn pop_val(s: &mut State, t: Tid) -> Val {
        match Self::pop(s, t) {
            SVal::V(v) => v,
            SVal::Place(_) => panic!("a place where a value was compiled"),
        }
    }

    fn push(s: &mut State, t: Tid, v: SVal) {
        s.act(t).stack.push(v);
    }

    // One instruction. Ok(None): a step was taken; Ok(Some(false)): blocked
    // (no step); Ok(Some(true)): the program ended.
    fn exec(&self, s: &mut State, t: Tid) -> Res<Option<bool>> {
        let (func, pc) = {
            let a = s.threads[&t].acts.last().unwrap();
            (a.func, a.pc)
        };
        let (ins, line) = self.code.funcs[func].1[pc].clone();
        if self.waits(s, t) {
            // The instruction cannot go on: its wait closes a cycle and
            // faults, or the thread begins to wait (a step that changes
            // nothing else).
            if self.closes_cycle(s, t) {
                return fault("diag.deadlock", line);
            }
            if t == 0 && self.waits_on_channel(s, t) && !s.threads.iter().any(|(k, x)| *k != 0 && x.status == Status::Running) {
                return fault("diag.channel-deadlock", line); // `[Channel-Deadlock]`
            }
            // `recv` and `send` lock the channel's state before they wait.
            if matches!(ins, Ins::ChanSend | Ins::ChanRecv) {
                s.th(t).lent = true;
            }
            s.th(t).waiting = true;
            return Ok(None);
        }
        s.th(t).waiting = false;
        s.act(t).pc += 1;
        match ins {
            Ins::StmtEnter => {
                let sc = s.fresh();
                let f = s.cur_frame(t);
                s.th(t).scopes.push((sc, f));
                s.th(t).licensed.clear();
                if tracing() {
                    tr(format!("sen {} {}", t, sc));
                }
            }
            Ins::Licensed(names) => {
                let toks: Vec<Tok> = names.iter().filter_map(|n| s.threads[&t].acts.last().unwrap().slots.get(n).and_then(|v| v.last()).copied()).collect();
                s.th(t).licensed.extend(toks);
            }
            Ins::StmtExit { keep } => {
                // `[Stmt-Exit]`
                let r = if keep {
                    let v = match Self::pop(s, t) {
                        SVal::V(v) => v,
                        SVal::Place(a) => s.read(a, t, line)?,
                    };
                    Some(v)
                } else {
                    None
                };
                let (sc, _) = *s.threads[&t].scopes.last().unwrap();
                let rrefs: BTreeSet<Tok> = r.as_ref().map(|v| v.refs().into_iter().collect()).unwrap_or_default();
                let owners: BTreeSet<Tok> = s.holder.values().copied().collect();
                let unheld: Vec<Tok> = s
                    .paths
                    .iter()
                    .filter(|(k, p)| p.scope == Some(sc) && p.valid && !owners.contains(k) && !rrefs.contains(k))
                    .map(|(k, _)| *k)
                    .filter(|k| s.held_by(*k).is_empty())
                    .collect();
                for k in unheld {
                    s.paths.get_mut(&k).unwrap().valid = false;
                }
                s.th(t).scopes.pop();
                let parent = s.cur_scope(t);
                for k in &rrefs {
                    if s.held_by(*k).is_empty() {
                        if let Some(p) = s.paths.get_mut(k) {
                            p.scope = parent;
                            if tracing() {
                                tr(format!("rsc {} {}", k, parent.map_or(0, |x| x + 1)));
                            }
                        }
                    }
                }
                if tracing() {
                    tr(format!("sex {}", t));
                }
                if let Some(v) = r {
                    Self::push(s, t, SVal::V(v));
                }
            }
            Ins::BlockEnter => {
                let f = s.fresh();
                s.act(t).frames.push(f);
                if tracing() {
                    tr(format!("ben {} {}", t, f));
                }
            }
            Ins::BlockExit(names) => {
                // `[Block-Exit]`: end what the frame owns, last bound first (the
                // holder formed last; D-0233); a handle waits for its thread.
                let f = *s.threads[&t].acts.last().unwrap().frames.last().unwrap();
                if s.act(t).exiting.is_none() {
                    let mut owned: Vec<(u64, ObjId)> = s.holder.iter().filter(|(_, a)| s.paths[*a].frame == f).map(|(o, a)| (u64::from(*a), *o)).collect();
                    owned.sort();
                    owned.reverse();
                    s.act(t).exiting = Some(owned.into_iter().map(|x| x.1).collect());
                }
                loop {
                    let next = s.threads[&t].acts.last().unwrap().exiting.as_ref().unwrap().first().copied();
                    let Some(o) = next else { break };
                    if s.alive(o) {
                        if s.first_wait(o).is_some() {
                            // Wait here: the next step begins the wait.
                            s.act(t).pc -= 1;
                            return Ok(None);
                        }
                        let exempt: BTreeSet<Tok> = match (self.spare_result, s.threads[&t].acts.last().unwrap().stack.last()) {
                            (true, Some(SVal::V(v))) => v.refs().into_iter().collect(),
                            _ => BTreeSet::new(),
                        };
                        s.quiet_end += 1;
                        let done = self.destroy_owned(s, o, t, line, &exempt);
                        s.quiet_end -= 1;
                        if !done? {
                            // A later handle's thread still runs: wait for it.
                            s.act(t).pc -= 1;
                            return Ok(None);
                        }
                    }
                    s.act(t).exiting.as_mut().unwrap().remove(0);
                }
                s.act(t).exiting = None;
                s.act(t).frames.pop();
                if tracing() {
                    tr(format!("bex {}", t));
                }
                // Unbind the block's names; one already ended at its last use
                // (`EndBinding`) is unbound already.
                for n in names {
                    let top = s.threads[&t].acts.last().unwrap().slots.get(&n).and_then(|v| v.last()).copied();
                    if let Some(a) = top {
                        if s.paths[&a].frame == f {
                            let act = s.act(t);
                            let v = act.slots.get_mut(&n).unwrap();
                            v.pop();
                            if v.is_empty() {
                                act.slots.remove(&n);
                            }
                        }
                    }
                }
            }
            Ins::EndBinding(n) => {
                // `[Ref-Binding-Last-Use]`: as at the block's exit, for one
                // binding that owns no resource.
                let a = s.act(t).slots.get_mut(&n).and_then(|v| v.pop());
                if let Some(a) = a {
                    let o = s.paths[&a].of;
                    if s.alive(o) && s.holder.get(&o) == Some(&a) {
                        s.emit_end = true;
                        s.end_object(o, &BTreeSet::new());
                        s.emit_end = false;
                    }
                }
            }
            Ins::Int(k) => Self::push(s, t, SVal::V(Val::Int(k))),
            Ins::Static(k) => {
                if tracing() {
                    tr("out static".to_string());
                }
                let a = s.statics[k];
                Self::push(s, t, SVal::Place(a));
            }
            Ins::Var(n) => {
                // `[Binding-Lookup]`
                let Some(a) = s.threads[&t].acts.last().unwrap().slots.get(&n).and_then(|v| v.last()).copied() else { return fault("diag.stale-binding", line) };
                if !s.temporally_valid(a) {
                    return fault("diag.stale-binding", line);
                }
                Self::push(s, t, SVal::Place(a));
            }
            Ins::Read => {
                // `[LValue-To-RValue]`: a place in value position is read; a
                // value stays as it is.
                let v = match Self::pop(s, t) {
                    SVal::Place(a) => s.read(a, t, line)?,
                    SVal::V(v) => v,
                };
                Self::push(s, t, SVal::V(v));
            }
            Ins::Deref => {
                // `[Ref-Deref-Place]`: the operand is read to its reference;
                // `[Guard-Deref]`: a guard is not read, its lock path is the place.
                let v = match Self::pop(s, t) {
                    SVal::V(v) => v,
                    SVal::Place(a) => match s.value_at(a) {
                        Some(Val::Guard(g)) => {
                            if !s.temporally_valid(a) {
                                return fault("diag.stale-binding", line);
                            }
                            Self::push(s, t, SVal::Place(g));
                            return Ok(None);
                        }
                        _ => s.read(a, t, line)?,
                    },
                };
                let Val::Ref(r) = v else { panic!("deref of a non-reference") };
                Self::push(s, t, SVal::Place(r));
            }
            Ins::Field(fi) => {
                // `[Field-Access]`, through a reference by
                // `[Field-Access-Auto-Deref]`.
                let base = match Self::pop(s, t) {
                    SVal::Place(a) => match s.value_at(a) {
                        Some(Val::Ref(_)) if s.paths[&a].steps.is_empty() && !matches!(s.objs[&s.paths[&a].of].val, Some(Val::Hold(_))) => {
                            let Val::Ref(r) = s.read(a, t, line)? else { unreachable!() };
                            r
                        }
                        _ => a,
                    },
                    SVal::V(Val::Ref(r)) => r,
                    SVal::V(_) => panic!("field of a value"),
                };
                if !s.temporally_valid(base) {
                    return fault("diag.stale-binding", line);
                }
                s.affine(base, t, line)?;
                let (of, mode) = (s.paths[&base].of, s.paths[&base].mode);
                let a = s.new_path(of, vec![PStep::F(fi)], mode, t, Some(base));
                s.one_access(a);
                if tracing() {
                    tr(format!("prj {} {} {} {}", t, base, a, fi));
                }
                Self::push(s, t, SVal::Place(a));
            }
            Ins::CapRef => {
                if tracing() {
                    tr("out closure".to_string());
                }
                // The field holding a borrow capture is read as stored: no
                // `[Read-Ref-Through-Shared]` borrow (D-0239); the place is
                // `*a_i`.
                let SVal::Place(a) = Self::pop(s, t) else { panic!("capture of a value") };
                if !s.temporally_valid(a) {
                    return fault("diag.stale-binding", line);
                }
                s.affine(a, t, line)?;
                if s.clash(a, Mode::Shared) {
                    return fault(s.unlicensed(a, t), line);
                }
                let Some(Val::Ref(r)) = s.value_at(a) else { panic!("a borrow capture holds a reference") };
                s.consume(a);
                Self::push(s, t, SVal::Place(r));
            }
            Ins::MkArr(n) => {
                if tracing() {
                    tr("out array".to_string());
                }
                let mut xs: Vec<i64> = (0..n)
                    .map(|_| match Self::pop_val(s, t) {
                        Val::Int(k) => k,
                        _ => panic!("array element"),
                    })
                    .collect();
                xs.reverse();
                Self::push(s, t, SVal::V(Val::Arr(xs)));
            }
            Ins::MkClo { func, n, mv, ws } => {
                if tracing() {
                    tr("out closure".to_string());
                }
                let mut caps: Vec<Val> = (0..n).map(|_| Self::pop_val(s, t)).collect();
                caps.reverse();
                Self::push(s, t, SVal::V(Val::Clo { func, caps, mv, ws }));
            }
            Ins::CallClo => {
                if tracing() {
                    tr("out closure".to_string());
                }
                // `[Closure-Call]`: `self` is a borrow of the closure's place,
                // shared, or exclusive for one that writes itself (D-0239).
                let a_c = match Self::pop(s, t) {
                    SVal::Place(a) => a,
                    SVal::V(Val::Ref(r)) => r,
                    SVal::V(_) => panic!("call of a value"),
                };
                if !s.temporally_valid(a_c) {
                    return fault("diag.stale-binding", line);
                }
                let Some(v) = s.value_at(a_c) else { return fault("diag.use-of-uninitialized", line) };
                // `[Fn-Value-Empty-Call]`: its closure was moved out.
                if v == Val::Unit {
                    return fault("diag.stale-binding", line);
                }
                let Val::Clo { func, ws, .. } = v.clone() else { panic!("call of a non-closure") };
                if s.paths[&a_c].mode == Mode::Shared && s.fn_writes(&v) {
                    return fault("diag.borrow-exceeds-source", line); // `[Read-Fn-Through-Shared]`
                }
                let m = if ws { Mode::Excl } else { Mode::Shared };
                let a_self = s.borrow(a_c, m, t, line)?;
                let pf = s.fresh();
                s.th(t).acts.push(Act { func, pc: 0, stack: vec![], slots: BTreeMap::new(), frames: vec![], param_frame: pf, exiting: None });
                s.bind("$self", Val::Ref(a_self), t);
            }
            Ins::SpawnClo => {
                if tracing() {
                    tr("out closure".to_string());
                }
                // `[Spawn]` of a `move` closure: its captures are lent
                // (`[Thread-Lend]`); the thread holds the closure in its
                // parameter frame and calls it.
                let mut v = Self::pop_val(s, t);
                let lent = self.lend(s, t, &mut v, line)?;
                let Val::Clo { func, ws, .. } = v.clone() else { panic!("spawn of a non-closure") };
                let nt = s.fresh();
                let pf = s.fresh();
                s.threads.insert(nt, Thread { acts: vec![Act { func, pc: 0, stack: vec![], slots: BTreeMap::new(), frames: vec![], param_frame: pf, exiting: None }], scopes: vec![], status: Status::Running, waiting: false, lent: false, licensed: BTreeSet::new() });
                let a_c = s.bind("$clo", v, nt);
                let a_self = s.borrow(a_c, if ws { Mode::Excl } else { Mode::Shared }, nt, line)?;
                s.bind("$self", Val::Ref(a_self), nt);
                if !lent.is_empty() {
                    s.th(nt).lent = true;
                }
                for a2 in lent {
                    s.paths.get_mut(&a2).unwrap().thread = Owner::Transit;
                }
                Self::push(s, t, SVal::V(Val::Handle(nt)));
            }
            Ins::JobLit => {
                if tracing() {
                    tr("out job".to_string());
                }
                let Val::Handle(h) = Self::pop_val(s, t) else { panic!("job") };
                Self::push(s, t, SVal::V(Val::Job(h)));
            }
            Ins::ArgVal => {
                // A resource is moved from its place (`[Store-Binding-Place-
                // Transfer]`), anything else read.
                let v = match Self::pop(s, t) {
                    SVal::V(v) => v,
                    SVal::Place(a) => {
                        if s.temporally_valid(a) && s.objs[&s.paths[&a].of].resource && s.paths[&a].steps.is_empty() {
                            Self::push(s, t, SVal::Place(a));
                            return self.exec_move(s, t, line).map(|_| None);
                        }
                        s.read(a, t, line)?
                    }
                };
                Self::push(s, t, SVal::V(v));
            }
            Ins::SliceForm { mode, lo, hi, arg } => {
                if tracing() {
                    tr("out slice".to_string());
                }
                // `[Slice-Form]`: the range [lo, hi) of a `Vec`, or of a
                // slice's source, offset by its start.
                let SVal::Place(a0) = Self::pop(s, t) else { panic!("slice of a value") };
                if !s.temporally_valid(a0) {
                    return fault("diag.stale-binding", line);
                }
                let (a_s, start, len) = match s.value_at(a0) {
                    Some(Val::Vec { len, .. }) => (a0, 0, len),
                    Some(Val::Arr(xs)) => (a0, 0, xs.len() as u32),
                    Some(Val::Slice { .. }) => {
                        let Val::Slice { tok, start, len } = s.read(a0, t, line)? else { unreachable!() };
                        if !s.temporally_valid(tok) {
                            return fault("diag.stale-binding", line);
                        }
                        (tok, start, len)
                    }
                    _ => panic!("slice source"),
                };
                if lo > hi || hi > len {
                    return fault("diag.index-out-of-bounds", line);
                }
                if mode == Mode::Excl && s.paths[&a_s].mode != Mode::Excl {
                    return fault("diag.borrow-exceeds-source", line);
                }
                s.affine(a_s, t, line)?;
                let mut steps = s.paths[&a_s].steps.clone();
                steps.push(PStep::R(start + lo, start + hi));
                let of = s.paths[&a_s].of;
                let a_r = s.new_path(of, steps, mode, t, Some(a_s));
                if s.clash(a_r, mode) {
                    return fault(s.unlicensed(a_r, t), line);
                }
                if arg {
                    s.paths.get_mut(&a_r).unwrap().pending = true; // D-0107
                }
                Self::push(s, t, SVal::V(Val::Slice { tok: a_r, start: start + lo, len: hi - lo }));
            }
            Ins::IndexVec(m) => {
                if tracing() {
                    tr("out vec".to_string());
                }
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("index") };
                let SVal::Place(a0) = Self::pop(s, t) else { panic!("index of a value") };
                if !s.temporally_valid(a0) {
                    return fault("diag.stale-binding", line);
                }
                let a_e = match s.value_at(a0) {
                    Some(Val::Vec { buf, len, .. }) => {
                        // `[Index-Vec]` (D-0238): the element borrow a_k, its
                        // last step index(k); the element object reached
                        // through a path whose base is a_k.
                        s.affine(a0, t, line)?;
                        if k < 0 || k as u32 >= len {
                            return fault("diag.index-out-of-bounds", line);
                        }
                        let mut steps = s.paths[&a0].steps.clone();
                        steps.push(PStep::I(k as u32));
                        let of = s.paths[&a0].of;
                        let a_k = s.new_path(of, steps, m, t, Some(a0));
                        if s.clash(a_k, m) {
                            return fault("diag.aliasing-conflict", line);
                        }
                        s.one_access(a_k);
                        let o = s.elem_obj(buf, k as u32, t);
                        let a_e = s.new_path(o, vec![], m, t, Some(a_k));
                        s.one_access(a_e);
                        a_e
                    }
                    Some(Val::Arr(xs)) => {
                        // `[Index-Checked]`: a projection of the array's path.
                        if k < 0 || k as usize >= xs.len() {
                            return fault("diag.index-out-of-bounds", line);
                        }
                        let mut steps = s.paths[&a0].steps.clone();
                        steps.push(PStep::I(k as u32));
                        let (of, mode) = (s.paths[&a0].of, s.paths[&a0].mode);
                        let a_e = s.new_path(of, steps, mode, t, Some(a0));
                        s.one_access(a_e);
                        a_e
                    }
                    Some(Val::Slice { .. }) => {
                        // `[Slice-Index]`: element start + k of the source,
                        // reached through the slice's borrow.
                        let Val::Slice { tok, start, len } = s.read(a0, t, line)? else { unreachable!() };
                        if !s.temporally_valid(tok) {
                            return fault("diag.stale-binding", line);
                        }
                        s.affine(tok, t, line)?;
                        // D-0248: the slice's own path is checked too, against
                        // what lives on the vector -- a lent reborrow of it, a
                        // borrow of the whole vector.
                        let am = if m == Mode::Excl { Mode::Excl } else { Mode::Shared };
                        if s.clash(tok, am) {
                            return fault("diag.aliasing-conflict", line);
                        }
                        if k < 0 || k as u32 >= len {
                            return fault("diag.index-out-of-bounds", line);
                        }
                        let mode = s.paths[&tok].mode;
                        let a_e = match s.objs[&s.paths[&tok].of].val.clone() {
                            Some(Val::Vec { buf, .. }) => {
                                let o = s.elem_obj(buf, start + k as u32, t);
                                s.new_path(o, vec![], mode, t, Some(tok))
                            }
                            // An array source: a projection of the slice's path
                            // onto the element's cells.
                            Some(Val::Arr(_)) => {
                                let mut steps = s.paths[&tok].steps.clone();
                                steps.push(PStep::I(start + k as u32));
                                let of = s.paths[&tok].of;
                                s.new_path(of, steps, mode, t, Some(tok))
                            }
                            _ => panic!("slice of a non-vector"),
                        };
                        s.one_access(a_e);
                        a_e
                    }
                    _ => panic!("index of a non-vector"),
                };
                Self::push(s, t, SVal::Place(a_e));
            }
            Ins::Borrow { mode, arg } => {
                let SVal::Place(a0) = Self::pop(s, t) else { panic!("borrow of a value") };
                if tracing() {
                    s.pend_next = arg;
                }
                let a = s.borrow(a0, mode, t, line)?;
                if arg {
                    s.paths.get_mut(&a).unwrap().pending = true; // D-0107
                }
                Self::push(s, t, SVal::V(Val::Ref(a)));
            }
            Ins::Add => {
                let (Val::Int(b), Val::Int(a)) = (Self::pop_val(s, t), Self::pop_val(s, t)) else { panic!("add") };
                let r = a + b;
                if r > i32::MAX as i64 || r < i32::MIN as i64 {
                    return fault("diag.arith-overflow", line);
                }
                Self::push(s, t, SVal::V(Val::Int(r)));
            }
            Ins::Gt => {
                let (Val::Int(b), Val::Int(a)) = (Self::pop_val(s, t), Self::pop_val(s, t)) else { panic!("gt") };
                Self::push(s, t, SVal::V(Val::Bool(a > b)));
            }
            Ins::Let(n) => {
                let v = Self::pop_val(s, t);
                s.bind(&n, v, t);
            }
            Ins::Assign => {
                let v = Self::pop_val(s, t);
                let SVal::Place(a) = Self::pop(s, t) else { panic!("assign to a value") };
                s.write(a, v, t, line)?;
            }
            Ins::Call(fi, n) => {
                let mut args: Vec<Val> = (0..n).map(|_| Self::pop_val(s, t)).collect();
                args.reverse();
                self.call(s, t, fi, args, line)?;
            }
            Ins::Ret => {
                // The body's value; `[Call]` exits the parameter frame.
                let v = match s.threads[&t].acts.last().unwrap().stack.last().cloned() {
                    Some(SVal::V(v)) => v,
                    _ => Val::Unit,
                };
                let pf = s.threads[&t].acts.last().unwrap().param_frame;
                let owned: Vec<(u64, ObjId)> = {
                    // Last bound first, as `[Block-Exit]` (D-0233).
                    let mut o: Vec<(u64, ObjId)> = s.holder.iter().filter(|(_, a)| s.paths[*a].frame == pf).map(|(o, a)| (u64::from(*a), *o)).collect();
                    o.sort();
                    o.reverse();
                    o
                };
                // The parameter frame's exit spares the result's references.
                let exempt: BTreeSet<Tok> = if self.spare_result { v.refs().into_iter().collect() } else { BTreeSet::new() };
                let last = s.threads[&t].acts.len() == 1;
                if tracing() && last && t != 0 {
                    // `[Thread-Body-Done]`: the result's objects stay; the rest of the parameter frame ends.
                    let res: Vec<String> = match &v {
                        Val::Handle(h) => s.handle_obj.get(h).map(|o| o.to_string()).into_iter().collect(),
                        Val::Guard(g) => s.guard_obj.get(g).map(|o| o.to_string()).into_iter().collect(),
                        _ => vec![],
                    };
                    tr(format!("tdn {} {} {}", t, res.len(), res.join(" ")));
                }
                s.quiet_end += 1;
                for (_, o) in owned {
                    s.end_object(o, &exempt);
                }
                s.quiet_end -= 1;
                s.th(t).acts.pop();
                if s.threads[&t].acts.is_empty() {
                    if t == 0 {
                        if tracing() {
                            tr("end".to_string());
                        }
                        return Ok(Some(true));
                    }
                    s.th(t).status = Status::Done(v); // `[Thread-Body-Done]`
                } else {
                    if tracing() {
                        tr(format!("bex {}", t));
                    }
                    // The result flows into the caller's statement.
                    let sc = s.cur_scope(t);
                    for r in v.refs() {
                        if s.held_by(r).is_empty() {
                            if let Some(p) = s.paths.get_mut(&r) {
                                p.scope = sc;
                                if tracing() {
                                    tr(format!("rsc {} {}", r, sc.map_or(0, |x| x + 1)));
                                }
                            }
                        }
                    }
                    Self::push(s, t, SVal::V(v));
                }
            }
            Ins::VecNew => {
                if tracing() {
                    tr("out vec".to_string());
                }
                let b = s.fresh();
                s.bufs.insert(b, vec![]);
                Self::push(s, t, SVal::V(Val::Vec { buf: b, len: 0, cap: 0 }));
            }
            Ins::VecPush => {
                if tracing() {
                    tr("out vec".to_string());
                }
                let x = Self::pop_val(s, t);
                let Val::Ref(av) = Self::pop_val(s, t) else { panic!("push") };
                for r in x.refs() {
                    if let Some(p) = s.paths.get_mut(&r) {
                        p.pending = false;
                    }
                }
                let pf = self.enter_std(s, t, av)?;
                let hdr = s.read(av, t, line)?;
                let Val::Vec { mut buf, len, mut cap } = hdr else { panic!("vec") };
                if len == cap {
                    // `Vec::grow`: a new buffer, the cells copied (`copy_raw`),
                    // the old one deallocated, ending every element object
                    // over it (`[Release]`). A cell holding a reference keeps
                    // a holder: an element object over its new slot. Read
                    // literally, `[Copy-Raw]` gives the copy none, and the
                    // release would end every reference the vector holds
                    // (D-0219, finding 5); the tools keep them.
                    let nc = if cap == 0 { 4 } else { cap * 2 };
                    let nb = s.fresh();
                    let mut cells = s.bufs[&buf].clone();
                    cells.resize(nc as usize, Val::Int(0));
                    s.bufs.insert(nb, cells.clone());
                    for (i, c) in cells.iter().enumerate() {
                        if !c.refs().is_empty() {
                            s.elem_obj(nb, i as u32, t);
                        }
                    }
                    s.release_elems(buf, 0, u32::MAX, t, line)?;
                    s.bufs.remove(&buf);
                    buf = nb;
                    cap = nc;
                }
                let has_refs = !x.refs().is_empty();
                s.bufs.get_mut(&buf).unwrap()[len as usize] = x;
                if has_refs {
                    s.elem_obj(buf, len, t); // `[Rawptr-Write]`: the holder
                }
                s.write(av, Val::Vec { buf, len: len + 1, cap }, t, line)?;
                self.leave_std(s, t, pf);
                Self::push(s, t, SVal::V(Val::Unit));
            }
            Ins::Swap | Ins::Replace => {
                if tracing() {
                    tr("out swap".to_string());
                }
                // `[Swap-Places]` (D-0235): for distinct places, `[Write]`'s
                // premises of each reference, all checked before either
                // place changes; the same place twice is no access.
                let second = Self::pop_val(s, t);
                let Val::Ref(a) = Self::pop_val(s, t) else { panic!("swap") };
                let b = match &second {
                    Val::Ref(b) if ins == Ins::Swap => Some(*b),
                    _ => None,
                };
                let same = b.map_or(false, |b| s.paths[&a].of == s.paths[&b].of && s.paths[&a].steps == s.paths[&b].steps);
                let mut result = Val::Unit;
                if !same {
                    let toks: Vec<Tok> = std::iter::once(a).chain(b).collect();
                    for &x in &toks {
                        if !s.temporally_valid(x) {
                            return fault("diag.stale-binding", line);
                        }
                        s.affine(x, t, line)?;
                        if s.clash(x, Mode::Excl) {
                            return fault(s.unlicensed(x, t), line);
                        }
                    }
                    let va = s.value_at(a).unwrap();
                    match b {
                        Some(b) => {
                            let vb = s.value_at(b).unwrap();
                            s.store(a, vb, t);
                            s.store(b, va, t);
                        }
                        None => {
                            s.store(a, second, t);
                            result = va;
                        }
                    }
                }
                Self::push(s, t, SVal::V(result));
            }
            Ins::VecPop => {
                if tracing() {
                    tr("out vec".to_string());
                }
                let Val::Ref(av) = Self::pop_val(s, t) else { panic!("pop") };
                let pf = self.enter_std(s, t, av)?;
                let Val::Vec { buf, len, cap } = s.read(av, t, line)? else { panic!("vec") };
                let r = if len == 0 {
                    Val::Opt(None)
                } else {
                    s.write(av, Val::Vec { buf, len: len - 1, cap }, t, line)?;
                    let x = s.bufs[&buf][(len - 1) as usize].clone();
                    s.release_elems(buf, len - 1, len, t, line)?;
                    Val::Opt(Some(Box::new(x)))
                };
                self.leave_std(s, t, pf);
                Self::push(s, t, SVal::V(r));
            }
            Ins::VecIndex(m) => {
                if tracing() {
                    tr("out vec".to_string());
                }
                // `Vec::index_shared` / `index_exclusive`: bounds, then
                // `&_m reclaim<T>(ptr + i)`.
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("index") };
                let Val::Ref(av) = Self::pop_val(s, t) else { panic!("index vec") };
                let pf = self.enter_std(s, t, av)?;
                let Val::Vec { buf, len, .. } = s.read(av, t, line)? else { panic!("vec") };
                if k < 0 || k as u32 >= len {
                    return fault("diag.index-out-of-bounds", line);
                }
                if m == Mode::Excl {
                    // D-0222: `_ = &mut (*v)[i .. i + 1];`, element i
                    // exclusively through `v`, a slice borrow ended with
                    // its statement.
                    s.affine(av, t, line)?;
                    let mut steps = s.paths[&av].steps.clone();
                    steps.push(PStep::R(k as u32, k as u32 + 1));
                    let of = s.paths[&av].of;
                    let a = s.new_path(of, steps, Mode::Excl, t, Some(av));
                    if s.clash(a, Mode::Excl) {
                        return fault("diag.aliasing-conflict", line);
                    }
                    s.paths.get_mut(&a).unwrap().valid = false;
                }
                let o = s.elem_obj(buf, k as u32, t);
                // The callee's statement: the reclaim's root path, the borrow
                // from it; the root ends with that statement.
                let sc = s.fresh();
                let f = s.cur_frame(t);
                s.th(t).scopes.push((sc, f));
                let a0 = s.new_path(o, vec![], Mode::Excl, t, None);
                let a1 = s.borrow(a0, m, t, line)?;
                s.paths.get_mut(&a0).unwrap().valid = false;
                s.th(t).scopes.pop();
                self.leave_std(s, t, pf);
                let sc = s.cur_scope(t);
                s.paths.get_mut(&a1).unwrap().scope = sc;
                Self::push(s, t, SVal::V(Val::Ref(a1)));
            }
            Ins::Spawn(fi, n) => {
                let mut args: Vec<Val> = (0..n).map(|_| Self::pop_val(s, t)).collect();
                args.reverse();
                // `[Thread-Lend]`: each exclusive path in the arguments is
                // replaced by a path borrowed from it, in transit.
                let mut lent = Vec::new();
                for v in args.iter_mut() {
                    lent.extend(self.lend(s, t, v, line)?);
                }
                let pending_before: BTreeSet<Tok> = args.iter().flat_map(|v| v.refs()).filter(|r| s.paths.get(r).map_or(false, |p| p.pending)).collect();
                for v in &args {
                    for r in v.refs() {
                        if let Some(p) = s.paths.get_mut(&r) {
                            p.pending = false;
                        }
                    }
                }
                let nt = s.fresh();
                let pf = s.fresh();
                s.threads.insert(nt, Thread { acts: vec![Act { func: fi, pc: 0, stack: vec![], slots: BTreeMap::new(), frames: vec![], param_frame: pf, exiting: None }], scopes: vec![], status: Status::Running, waiting: false, lent: false, licensed: BTreeSet::new() });
                let pseudo = if tracing() {
                    // `[Spawn]`: the handle object the fragment establishes, and
                    // a statement scope of the new thread for its parameters' binding.
                    let h = s.fresh();
                    s.handle_obj.insert(nt, h);
                    tr(format!("spn {} {} {} {}", t, nt, pf, h));
                    let sc = s.fresh();
                    tr(format!("sen {} {}", nt, sc));
                    Some(sc)
                } else {
                    None
                };
                // The parameters are bound by the spawn step itself, in the
                // new thread (`rule.fn.bind-param`).
                let params = self.code.funcs[fi].0.params.clone();
                for ((pt, pn), v) in params.iter().zip(args.into_iter()) {
                    let v = self.param_value(s, nt, pt, v, line)?;
                    if tracing() {
                        if let Val::Ref(a) = &v {
                            if pending_before.contains(a) && matches!(pt, Ty::Ref(..)) {
                                self.bind_param_traced(s, nt, pn, *a, v.clone());
                                continue;
                            }
                        }
                    }
                    s.bind(pn, v, nt);
                }
                if pseudo.is_some() {
                    tr(format!("sex {}", nt));
                }
                if !lent.is_empty() {
                    s.th(nt).lent = true;
                }
                for a2 in lent {
                    // The lent path stays in transit until the thread uses it.
                    s.paths.get_mut(&a2).unwrap().thread = Owner::Transit;
                }
                Self::push(s, t, SVal::V(Val::Handle(nt)));
            }
            Ins::Join => {
                let SVal::Place(a) = Self::pop(s, t) else { panic!("join") };
                let o = s.paths[&a].of;
                let Some(Val::Handle(h)) = s.objs[&o].val.clone() else { panic!("join of a non-handle") };
                match s.threads[&h].status.clone() {
                    Status::Running => unreachable!("a join that must wait is handled before"),
                    Status::Failed(f) => {
                        s.th(h).status = Status::Taken;
                        return Err(f); // `[Join-Failed]`: raised again, at its own location
                    }
                    Status::Done(v) => {
                        s.th(h).status = Status::Taken;
                        if tracing() {
                            tr(format!("jon {} {}", t, h));
                        }
                        s.quiet_end += 1;
                        let r = self.destroy_owned(s, o, t, line, &BTreeSet::new());
                        s.quiet_end -= 1;
                        let _ = r?;
                        Self::push(s, t, SVal::V(v));
                    }
                    Status::Taken => return fault("diag.stale-binding", line),
                }
            }
            Ins::MutexNew => {
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("mutex") };
                Self::push(s, t, SVal::V(Val::Mutex(k)));
            }
            Ins::Lock => {
                // `[Lock]`, `[Lock-Reentrant]`, `[Lock-Poisoned]`.
                let Val::Ref(am) = Self::pop_val(s, t) else { panic!("lock") };
                if let Some(p) = s.paths.get_mut(&am) {
                    p.pending = false;
                }
                if !s.temporally_valid(am) {
                    return fault("diag.stale-binding", line);
                }
                let o = s.paths[&am].of;
                match s.sync.get(&o).cloned() {
                    Some(Sync::LockedBy(h)) if h == t => return fault("diag.mutex-reentrant-lock", line),
                    Some(Sync::Poisoned(_)) => return fault("diag.mutex-poisoned", line),
                    Some(Sync::LockedBy(_)) => unreachable!("a lock that must wait is handled before"),
                    _ => {}
                }
                s.sync.insert(o, Sync::LockedBy(t));
                let ag = s.new_path(o, vec![PStep::F(1)], Mode::Excl, t, Some(am));
                s.paths.get_mut(&ag).unwrap().lockp = true;
                if tracing() {
                    let g = s.fresh();
                    s.guard_obj.insert(ag, g);
                    tr(format!("lck {} {} {} {}", t, am, ag, g));
                }
                Self::push(s, t, SVal::V(Val::Guard(ag)));
            }
            Ins::AtomicNew => {
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("atomic") };
                Self::push(s, t, SVal::V(Val::Atomic(k, 0)));
            }
            Ins::AtomicOp(op) => {
                if tracing() {
                    tr("out atomic".to_string());
                }
                let x = if op == 0 {
                    0
                } else {
                    let Val::Int(x) = Self::pop_val(s, t) else { panic!("atomic operand") };
                    x
                };
                let Val::Ref(am) = Self::pop_val(s, t) else { panic!("atomic") };
                let o = Self::atomic_access(s, t, am, line)?;
                let Some(Val::Atomic(old, w)) = s.objs[&o].val.clone() else { panic!("not an atomic") };
                let new = match op {
                    0 => old,
                    1 => x,
                    _ => match old.checked_add(x).filter(|v| i32::try_from(*v).is_ok()) {
                        Some(v) => v,
                        None => return fault("diag.arith-overflow", line),
                    },
                };
                s.objs.get_mut(&o).unwrap().val = Some(Val::Atomic(new, w));
                Self::push(s, t, SVal::V(if op == 1 { Val::Unit } else { Val::Int(old) }));
            }
            Ins::AtomicNotify => {
                if tracing() {
                    tr("out atomic".to_string());
                }
                let Val::Ref(am) = Self::pop_val(s, t) else { panic!("atomic") };
                let o = Self::atomic_access(s, t, am, line)?;
                let Some(Val::Atomic(v, w)) = s.objs[&o].val.clone() else { panic!("not an atomic") };
                s.objs.get_mut(&o).unwrap().val = Some(Val::Atomic(v, w + 1));
                Self::push(s, t, SVal::V(Val::Unit));
            }
            Ins::AtomicWaitCheck(end) => {
                if tracing() {
                    tr("out atomic".to_string());
                }
                // stack: …, Ref(atomic), expected -- kept for the sleep.
                let n = s.threads[&t].acts.last().unwrap().stack.len();
                let (SVal::V(Val::Ref(am)), SVal::V(Val::Int(want))) = (s.threads[&t].acts.last().unwrap().stack[n - 2].clone(), s.threads[&t].acts.last().unwrap().stack[n - 1].clone()) else { panic!("atomic wait") };
                let o = Self::atomic_access(s, t, am, line)?;
                let Some(Val::Atomic(v, w)) = s.objs[&o].val.clone() else { panic!("not an atomic") };
                if v != want {
                    Self::pop_val(s, t);
                    Self::pop_val(s, t);
                    Self::push(s, t, SVal::V(Val::Unit));
                    s.act(t).pc = end;
                } else {
                    Self::push(s, t, SVal::V(Val::Int(w as i64)));
                }
            }
            Ins::AtomicWaitSleep(check) => {
                // Woken: the count moved. Check the value again.
                Self::pop_val(s, t);
                s.act(t).pc = check;
            }
            Ins::ChanNew => {
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("channel") };
                if k == 0 {
                    return fault("diag.channel-zero-capacity", line);
                }
                Self::push(s, t, SVal::V(Val::Chan { q: vec![], closed: false, cap: k as u32 }));
            }
            Ins::ChanLend => {
                if tracing() {
                    tr("out channel".to_string());
                }
                // `[Send]`: v := lend(v) (`[Thread-Lend]`).
                let mut v = Self::pop_val(s, t);
                self.lend(s, t, &mut v, line)?;
                for r in v.refs() {
                    if let Some(p) = s.paths.get_mut(&r) {
                        p.pending = false;
                    }
                }
                Self::push(s, t, SVal::V(v));
            }
            Ins::ChanSend => {
                if tracing() {
                    tr("out channel".to_string());
                }
                let v = Self::pop_val(s, t);
                // The channel's state is a mutex whose interior may hold an
                // exclusive reference: the thread is lent (`spec/18`).
                s.th(t).lent = true;
                let Val::Ref(ac) = Self::pop_val(s, t) else { panic!("send") };
                let o = self.chan_obj(s, ac, line)?;
                let Some(Val::Chan { mut q, closed, cap }) = s.objs[&o].val.clone() else { panic!("chan") };
                let r = if closed {
                    Val::Res(Some(Box::new(v))) // given back
                } else {
                    q.push(v);
                    Val::Res(None)
                };
                s.objs.get_mut(&o).unwrap().val = Some(Val::Chan { q, closed, cap });
                Self::push(s, t, SVal::V(r));
            }
            Ins::ChanRecv => {
                if tracing() {
                    tr("out channel".to_string());
                }
                let Val::Ref(ac) = Self::pop_val(s, t) else { panic!("recv") };
                // The channel's state is a mutex whose interior may hold an
                // exclusive reference: the thread is lent (`spec/18`).
                s.th(t).lent = true;
                let o = self.chan_obj(s, ac, line)?;
                let Some(Val::Chan { mut q, closed, cap }) = s.objs[&o].val.clone() else { panic!("chan") };
                let r = if q.is_empty() { Val::Opt(None) } else { Val::Opt(Some(Box::new(q.remove(0)))) };
                s.objs.get_mut(&o).unwrap().val = Some(Val::Chan { q, closed, cap });
                // The value moves into the receiver's statement: its unheld
                // paths are re-stamped to that statement's scope, as `[Join]`
                // does for a result. `[Recv]` does not say so; read literally,
                // the sender's statement exit could end a path the receiver
                // has taken (D-0219, finding 4).
                let sc = s.cur_scope(t);
                for a in r.refs() {
                    if s.held_by(a).is_empty() {
                        if let Some(p) = s.paths.get_mut(&a) {
                            p.scope = sc;
                        }
                    }
                }
                Self::push(s, t, SVal::V(r));
            }
            Ins::ChanClose => {
                if tracing() {
                    tr("out channel".to_string());
                }
                let Val::Ref(ac) = Self::pop_val(s, t) else { panic!("close") };
                // The channel's state is a mutex whose interior may hold an
                // exclusive reference: the thread is lent (`spec/18`).
                s.th(t).lent = true;
                let o = self.chan_obj(s, ac, line)?;
                if let Some(Val::Chan { closed, .. }) = s.objs.get_mut(&o).unwrap().val.as_mut() {
                    *closed = true;
                }
                Self::push(s, t, SVal::V(Val::Unit));
            }
            Ins::Move => {
                self.exec_move(s, t, line)?;
            }
            Ins::MatchSome(l) => match Self::pop_val(s, t) {
                Val::Opt(Some(v)) => Self::push(s, t, SVal::V(*v)),
                Val::Opt(None) => s.act(t).pc = l,
                _ => panic!("match on a non-Option"),
            },
            Ins::HoldLit => {
                let Val::Ref(r) = Self::pop_val(s, t) else { panic!("hold") };
                Self::push(s, t, SVal::V(Val::Hold(r)));
            }
            Ins::Print => {
                let Val::Int(k) = Self::pop_val(s, t) else { panic!("print") };
                s.out.push(k);
            }
            Ins::Pop => {
                Self::pop(s, t);
            }
            Ins::JumpIfFalse(l) => {
                let Val::Bool(b) = Self::pop_val(s, t) else { panic!("if") };
                if !b {
                    s.act(t).pc = l;
                }
            }
            Ins::Jump(l) => s.act(t).pc = l,
        }
        Ok(None)
    }

    // The whole object at the place on the stack moves; every other path to
    // it would be left dangling (`solitary`, `diag.move-while-aliased`).
    fn exec_move(&self, s: &mut State, t: Tid, line: usize) -> Res<()> {
        let SVal::Place(a) = Self::pop(s, t) else { panic!("move of a value") };
        if !s.temporally_valid(a) {
            return fault("diag.stale-binding", line);
        }
        let o = s.paths[&a].of;
        if s.paths.iter().any(|(k, p)| *k != a && p.of == o && p.valid) {
            return fault("diag.move-while-aliased", line);
        }
        let v = s.objs[&o].val.clone().unwrap();
        let exempt: BTreeSet<Tok> = v.refs().into_iter().collect();
        if tracing() {
            s.moving = Some((o, a, t, v.clone()));
            s.quiet_end += 1;
        }
        s.end_object(o, &exempt);
        if tracing() {
            s.quiet_end -= 1;
        }
        Self::push(s, t, SVal::V(v));
        Ok(())
    }

    // `[Thread-Lend]` of one value: every exclusive path in it is replaced
    // by an exclusive path borrowed from it, in transit.
    fn lend(&self, s: &mut State, t: Tid, v: &mut Val, line: usize) -> Res<Vec<Tok>> {
        let mut lent = Vec::new();
        let rs = v.refs();
        for r in rs {
            // A lock path is not lent, and stays its locking thread's (D-0240).
            if s.paths.get(&r).map_or(false, |p| p.mode == Mode::Excl && !p.lockp) {
                if tracing() {
                    s.lend_next = true;
                }
                let a2 = s.borrow(r, Mode::Excl, t, line)?;
                s.paths.get_mut(&a2).unwrap().thread = Owner::Transit;
                replace_ref(v, r, a2);
                lent.push(a2);
            }
        }
        Ok(lent)
    }

    // The channel object a shared reference reaches.
    fn chan_obj(&self, s: &mut State, ac: Tok, line: usize) -> Res<ObjId> {
        if let Some(p) = s.paths.get_mut(&ac) {
            p.pending = false;
        }
        if !s.temporally_valid(ac) {
            return fault("diag.stale-binding", line);
        }
        Ok(s.paths[&ac].of)
    }

    // `rule.fn.bind-param`: a shared reference parameter given an exclusive
    // reference borrows shared through it first (D-0049).
    fn param_value(&self, s: &mut State, t: Tid, pt: &Ty, v: Val, line: usize) -> Res<Val> {
        if let (Ty::Ref(_, Mode::Shared), Val::Ref(a)) = (pt, &v) {
            if s.paths[a].mode == Mode::Excl {
                let b = s.borrow(*a, Mode::Shared, t, line)?;
                return Ok(Val::Ref(b));
            }
        }
        Ok(v)
    }

    fn call(&self, s: &mut State, t: Tid, fi: usize, args: Vec<Val>, line: usize) -> Res<()> {
        let pf = s.fresh();
        s.th(t).acts.push(Act { func: fi, pc: 0, stack: vec![], slots: BTreeMap::new(), frames: vec![], param_frame: pf, exiting: None });
        if tracing() {
            tr(format!("ben {} {}", t, pf));
        }
        // D-0270: an argument's pending path is bound by `[Call]` (`bindParam`);
        // the trace keeps the pending ones apart before the flags are cleared.
        let pending_before: BTreeSet<Tok> = args.iter().flat_map(|v| v.refs()).filter(|r| s.paths.get(r).map_or(false, |p| p.pending)).collect();
        let params = self.code.funcs[fi].0.params.clone();
        for v in &args {
            for r in v.refs() {
                if let Some(p) = s.paths.get_mut(&r) {
                    p.pending = false;
                }
            }
        }
        for ((pt, pn), v) in params.iter().zip(args.into_iter()) {
            let v = self.param_value(s, t, pt, v, line)?;
            if tracing() {
                if let Val::Ref(a) = &v {
                    if pending_before.contains(a) && matches!(pt, Ty::Ref(..)) {
                        self.bind_param_traced(s, t, pn, *a, v.clone());
                        continue;
                    }
                }
            }
            s.bind(pn, v, t);
        }
        Ok(())
    }

    // D-0270: the binding as `[Call]` and `[Spawn]` make it of a pending
    // argument path: the parameter object and its root, the argument's
    // path pending no longer (the fragment's `bindParam`).
    fn bind_param_traced(&self, s: &mut State, t: Tid, pn: &str, a: Tok, v: Val) {
        let o = s.fresh();
        let origin = s.tick();
        let scope = s.cur_scope(t);
        let resource = v.owns_resource();
        s.objs.insert(o, Obj { resource, val: None, elem: None, scope, origin });
        let ar = s.new_path(o, vec![], Mode::Excl, t, None);
        s.holder.insert(o, ar);
        s.act(t).slots.entry(pn.to_string()).or_default().push(ar);
        tr(format!("bpa {} {} {} {}", t, a, o, ar));
        s.quiet_store = true;
        s.store(ar, v, t);
    }

    // A `std` call's parameter frame holding the vector reference `av`.
    fn enter_std(&self, s: &mut State, t: Tid, av: Tok) -> Res<FrameId> {
        if let Some(p) = s.paths.get_mut(&av) {
            p.pending = false;
        }
        let pf = s.fresh();
        s.act(t).frames.push(pf);
        if tracing() {
            tr(format!("ben {} {}", t, pf));
        }
        s.bind("$v", Val::Ref(av), t);
        Ok(pf)
    }

    fn leave_std(&self, s: &mut State, t: Tid, pf: FrameId) {
        let owned: Vec<ObjId> = s.holder.iter().filter(|(_, a)| s.paths[*a].frame == pf).map(|(o, _)| *o).collect();
        s.quiet_end += 1;
        for o in owned {
            s.end_object(o, &BTreeSet::new());
        }
        s.quiet_end -= 1;
        if let Some(v) = s.act(t).slots.get_mut("$v") {
            v.pop();
        }
        s.act(t).frames.pop();
        if tracing() {
            tr(format!("bex {}", t));
        }
    }

    // `rule.resauth.destroy` of an object a frame owns: `solitary`, then
    // the destructor (a Vec's releases its buffer), then the object ends.
    // Ok(false): a handle it holds names a thread that still runs; the
    // destruction goes on once it is done (the handles before it taken).
    fn destroy_owned(&self, s: &mut State, o: ObjId, t: Tid, line: usize, exempt: &BTreeSet<Tok>) -> Res<bool> {
        if s.objs[&o].resource {
            let a = s.holder[&o];
            let other = s.paths.iter().any(|(k, p)| *k != a && p.of == o && p.valid);
            if other {
                return fault("diag.destroy-while-aliased", line);
            }
            // `[Handle-Destructor]` of each handle it holds, in order: a
            // finished thread's result is discarded, a failed one's
            // failure raised again (`[Join-Failed]`).
            for h in s.handles_in(o) {
                match s.threads[&h].status.clone() {
                    Status::Taken => {}
                    Status::Running => return Ok(false),
                    Status::Failed(f) => {
                        s.th(h).status = Status::Taken;
                        return Err(f);
                    }
                    Status::Done(_) => {
                        s.th(h).status = Status::Taken;
                        if tracing() {
                            tr(format!("hds {} {}", t, h));
                        }
                    }
                }
            }
            // A closure that owns a resource: its captures' destructors
            // (`spec/16` §1, the struct rules).
            if let Some(Val::Clo { caps, .. }) = s.objs[&o].val.clone() {
                for c in caps {
                    if let Val::Vec { buf, .. } = c {
                        s.release_elems(buf, 0, u32::MAX, t, line)?;
                        s.bufs.remove(&buf);
                    }
                }
            }
            match s.objs[&o].val.clone() {
                Some(Val::Vec { buf, .. }) => {
                    if tracing() {
                        tr("out vec".to_string());
                    }
                    s.release_elems(buf, 0, u32::MAX, t, line)?;
                    s.bufs.remove(&buf);
                }
                Some(Val::Guard(g)) => {
                    if tracing() {
                        tr(format!("gdp {} {}", t, o));
                    }
                    // `[Guard-Drop]`: unlock a mutex some thread holds; a
                    // poisoned one stays poisoned.
                    let m = s.paths[&g].of;
                    if let Some(Sync::LockedBy(_)) = s.sync.get(&m) {
                        s.sync.insert(m, Sync::Unlocked);
                    }
                    s.paths.get_mut(&g).unwrap().valid = false;
                }
                _ => {}
            }
        }
        s.end_object(o, exempt);
        Ok(true)
    }
}
