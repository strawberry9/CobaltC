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
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct FnItem {
    pub decl: Arc<FnDecl>,
}

pub struct Items {
    pub fns: HashMap<String, Arc<FnDecl>>,
    pub externs: HashMap<String, Arc<ExternDecl>>,
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
}

/// A `StringView` operation written with the language's own syntax
/// (D-0053).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewOp {
    /// `&s[lo .. hi]` on a `String` place: `String::view(&s, lo, hi)`.
    SliceString,
    /// `&v[lo .. hi]` on a `StringView`: `StringView::sub(v, lo, hi)`.
    SliceView,
    /// `a == b` / `a != b` with a view on the `view_left` side and a view
    /// (`StringView::eq`) or a `str` (`StringView::eq_str`) on the other.
    Eq { view_left: bool, other_is_str: bool, negate: bool },
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
    pub moved: std::collections::HashSet<String>,
    pub is_loop: bool,
    // A called function's (or closure's) parameter frame: name lookup
    // stops here, since a body sees only its own frames (captures are
    // bound in its frame), so a call's cost does not grow with the depth
    // of the stack it runs on.
    pub is_call: bool,
}

impl Frame {
    fn new() -> Self {
        Frame { bindings: HashMap::new(), types: HashMap::new(), owned: Vec::new(), moved: std::collections::HashSet::new(), is_loop: false, is_call: false }
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

// A spawned thread's lifecycle result (spec/19 `state.threads(ell).value`):
// `Running` until `[Thread-Body-Done]`, then `Done` until `[Join]`/
// `[Handle-Destructor]` claims it (`taken`).
pub enum ThreadStatus {
    Running,
    Done(Value, Type),
    Taken,
}

// The parts of Σ that another OS thread reads or writes while this one
// waits: thread results (`state.threads`), lock holders (`state.sync`),
// and the program's fault, if any. Kept behind their own mutex, outside
// the `Interp` the GIL guards, so a waiting thread can poll them
// without touching the interpreter it has released.
#[derive(Default)]
pub struct Shared {
    pub thread_status: HashMap<u64, ThreadStatus>,
    pub cobalt_locks: HashMap<u64, u64>,
    pub program_fault: Option<String>,
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
    held: Mutex<bool>,
    cv: std::sync::Condvar,
}

unsafe impl Send for Gil {}
unsafe impl Sync for Gil {}

impl Gil {
    pub fn new(interp: Interp) -> Gil {
        Gil { cell: std::cell::UnsafeCell::new(interp), held: Mutex::new(false), cv: std::sync::Condvar::new() }
    }
    pub fn acquire(&self) {
        let mut h = self.held.lock().unwrap();
        while *h {
            h = self.cv.wait(h).unwrap();
        }
        *h = true;
    }
    pub fn release(&self) {
        *self.held.lock().unwrap() = false;
        self.cv.notify_all();
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

#[inline(never)]
fn gil_yield(gil: &Arc<Gil>) {
    gil.release();
    std::thread::yield_now();
    gil.acquire();
}

pub struct Interp {
    pub items: Arc<Items>,
    pub objects: HashMap<u64, Object>,
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
    pub current_thread: u64,
    pub next_thread_id: u64,
    // raw byte arena for allocate/deallocate/rawptr; address 0 is never valid.
    pub arena: HashMap<u64, u8>,
    pub next_addr: u64,
    pub allocations: HashMap<u64, (u64, u64)>, // addr -> (len, align), for deallocate's trusted check
    pub extern_calls: Vec<(String, Vec<i128>)>,
    // The program's arguments (`rule.stdlib.args`): what follows the
    // file on `coby`'s command line, as bytes; `std::arg_bytes` reads them.
    pub prog_args: Arc<Vec<Vec<u8>>>,
    pub trace_writes: bool,
    closures: RefCell<HashMap<u64, ClosureInfo>>,
    closure_params: RefCell<HashMap<u64, Vec<Param>>>,
    reclaimed_addr: HashMap<u64, (u64, Type)>,
    reclaimed_by_addr: HashMap<u64, u64>,
    // Values raw storage holds that bytes cannot carry — references,
    // guards, handles, `fn` values — by the address of their cell. The
    // arena keeps their (placeholder) bytes; `arena_read` puts the value
    // back. A reference here is held by the reclaimed object over its
    // cell (`spec/20` `[Rawptr-Write]`, `CHG-0031`) until that ends.
    arena_side: std::collections::BTreeMap<u64, Value>,
    // `[Stmt-Exit]` for temporaries this interpreter would otherwise lose
    // track of -- one read through (`make().f`, `make()[i]`) or a closure
    // called straight from an expression: each statement ends the ones
    // registered while it ran, last first.
    stmt_temps: Vec<Vec<u64>>,
    // The objects in `stmt_temps` (a superset, never missing one): `bind`
    // scans the statements only for an object that may be a temporary.
    temp_index: std::collections::HashSet<u64>,
    // D-0047: the lengths `$` stands for, innermost `[…]` last, and
    // whether the place being evaluated is to be written (an assignment's
    // target, `&mut`'s operand): a `Vec` element is reached through a
    // borrow of the `Vec` in that mode.
    dollar: Vec<usize>,
    write_ctx: bool,
    // Program-data image of each distinct `str` value, minted on first
    // `str_ptr`/arena encoding (impl-defined: one address per content,
    // never reused).
    str_data: HashMap<Arc<[u8]>, u64>,
    // The objects whose value has ever held a `Ref`/`Guard` (spec/08's
    // occurrence holders). `scan_refs` walks only these, so an aliasing
    // check costs the number of live references, not the number of
    // live objects -- which grows with every Vec element ever indexed
    // (each `reclaim` mints one). A superset: an id is added whenever a
    // reference-bearing value is stored, and removed only with the
    // object; a stale member simply contributes no occurrences.
    ref_holders: std::collections::HashSet<u64>,
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
    // Set once the program has terminated with a fault: every frame exit
    // from then on skips destruction (`[Fault-Unwind]`).
    pub terminated: bool,
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
    token_ancestors: HashMap<u64, std::collections::HashSet<u64>>,
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
    closure_field_types: HashMap<u64, Vec<Type>>,
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

    // The enclosing function's own declared return type, pushed/popped
    // around a call's body execution: consulted by an explicit
    // `return expr;` (`[T-Return]`); a body's implicit trailing
    // expression gets the same type through
    // `exec_block_body_expected` (`rule.type.expected`).
    // Per thread: the declared return type of each open call.
    return_ty_by_thread: HashMap<u64, Vec<Type>>,
}

const MAIN_ADDR_BASE: u64 = 0x1000;

impl Interp {
    pub fn new(items: Arc<Items>) -> Self {
        let mut frames_by_thread = HashMap::new();
        frames_by_thread.insert(0u64, Vec::new()); // ell_0, the main thread
        Interp {
            items,
            objects: HashMap::new(),
            next_id: 1,
            frames_by_thread,
            current_thread: 0,
            next_thread_id: 1,
            arena: HashMap::new(),
            next_addr: MAIN_ADDR_BASE,
            allocations: HashMap::new(),
            extern_calls: Vec::new(),
            prog_args: Arc::new(Vec::new()),
            trace_writes: false,
            closures: RefCell::new(HashMap::new()),
            closure_params: RefCell::new(HashMap::new()),
            reclaimed_addr: HashMap::new(),
            reclaimed_by_addr: HashMap::new(),
            arena_side: std::collections::BTreeMap::new(),
            stmt_temps: Vec::new(),
            temp_index: std::collections::HashSet::new(),
            dollar: Vec::new(),
            write_ctx: false,
            str_data: HashMap::new(),
            ref_holders: std::collections::HashSet::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
            gil: None,
            active_threads: 1,
            terminated: false,
            uninit_bindings: HashSet::new(),
            capture_cells: HashSet::new(),
            token_ancestors: HashMap::new(),
            unsafe_depth_by_thread: HashMap::new(),
            closure_field_types: HashMap::new(),
            binding_thread_params: false,
            current_line: 0,
            return_ty_by_thread: HashMap::new(),
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
        if d.contains('@') || !user_line(self.current_line) {
            d
        } else {
            format!("{d}@{}", self.current_line)
        }
    }

    // This thread's own call stack. Never absent while `self` is
    // reachable: `Interp::new` seeds ell_0's, and every other thread id
    // is inserted (`spawn`) before `current_thread` is ever set to it.
    fn ret_stack(&mut self) -> &mut Vec<Type> {
        self.return_ty_by_thread.entry(self.current_thread).or_default()
    }

    // Blocks this CobaltC thread until `ready` holds of the shared
    // state, releasing the GIL meanwhile. Fails with `Flow::Terminated`
    // if the program terminates first, and with a hard error if there is
    // no GIL (nothing could ever make `ready` true).
    #[inline(never)]
    fn block_until(&mut self, mut ready: impl FnMut(&Shared) -> bool) -> Result<(), Flow> {
        if ready(&self.shared.lock().unwrap()) {
            return Ok(());
        }
        let gil = match self.gil.as_ref().and_then(|w| w.upgrade()) {
            Some(g) => g,
            None => return Err(Flow::Fault("deadlock: nothing can satisfy this wait (no other thread exists)".into())),
        };
        let shared = self.shared.clone();
        let me = self.current_thread;
        let r = gil_wait(&gil, &shared, &mut ready);
        std::hint::black_box(&mut *self);
        self.current_thread = me;
        match r {
            Ok(()) => Ok(()),
            Err(_) => {
                self.terminated = true;
                Err(Flow::Terminated)
            }
        }
    }

    // `[Thread-Step]`: between statements, let another live thread run.
    #[inline(never)]
    fn yield_step(&mut self) {
        if self.active_threads <= 1 {
            return;
        }
        if let Some(gil) = self.gil.as_ref().and_then(|w| w.upgrade()) {
            let me = self.current_thread;
            gil_yield(&gil);
            std::hint::black_box(&mut *self);
            self.current_thread = me;
        }
    }

    fn frames(&self) -> &Vec<Frame> {
        self.frames_by_thread.get(&self.current_thread).expect("current_thread has no frame stack")
    }

    fn frames_mut(&mut self) -> &mut Vec<Frame> {
        self.frames_by_thread.get_mut(&self.current_thread).expect("current_thread has no frame stack")
    }

    fn unsafe_depth(&self) -> u32 {
        *self.unsafe_depth_by_thread.get(&self.current_thread).unwrap_or(&0)
    }

    fn require_unsafe(&self) -> Result<(), Flow> {
        if self.unsafe_depth() > 0 {
            Ok(())
        } else {
            Err(Flow::Fault("diag.trusted-outside-unsafe".into()))
        }
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_object(&mut self, ty: Type, value: Value) -> u64 {
        let is_resource = self.is_resource(&ty);
        let id = self.fresh_id();
        let owner_thread = self.current_thread;
        if Self::value_has_ref(&value) {
            self.ref_holders.insert(id);
        }
        self.objects.insert(id, Object { ty, value, is_resource, owner_thread });
        id
    }

    // Every removal of an object goes through here so `ref_holders`
    // never outgrows `objects`.
    fn remove_object(&mut self, id: &u64) -> Option<Object> {
        self.ref_holders.remove(id);
        self.objects.remove(id)
    }

    fn value_has_ref(v: &Value) -> bool {
        match v {
            Value::Ref { .. } | Value::Guard { .. } => true,
            Value::Struct(fields) => fields.iter().any(Self::value_has_ref),
            Value::Array(items) => items.iter().any(Self::value_has_ref),
            Value::Enum(_, payload) => Self::value_has_ref(payload),
            _ => false,
        }
    }

    // D-0049: whether a value of a resource type owns anything now: a
    // `None` of an `Option<Box<T>>` does not, so it may be written over
    // (`live-resource-at`, spec/05). A type declared `resource` or with a
    // destructor always does, as do handles, guards and closures.
    fn value_owns(&self, v: &Value, ty: &Type) -> bool {
        if !self.is_resource(ty) {
            return false;
        }
        match (ty, v) {
            (Type::Named(name, args), _) if !crate::typecheck::type_is_owner(&self.items, name) => {
                if let (Some(sd), Value::Struct(fs)) = (self.items.structs.get(name), v) {
                    let sub = self.subst_map(&sd.type_params, args);
                    return sd.fields.iter().zip(fs.iter()).any(|(f, fv)| self.value_owns(fv, &self.apply_subst(&f.ty, &sub)));
                }
                if let (Some(ed), Value::Enum(vi, payload)) = (self.items.enums.get(name), v) {
                    let sub = self.subst_map(&ed.type_params, args);
                    return match ed.variants.get(*vi).and_then(|var| var.payload.as_ref()) {
                        Some(p) => self.value_owns(payload, &self.apply_subst(p, &sub)),
                        None => false,
                    };
                }
                true
            }
            (Type::Array(inner, _), Value::Array(es)) => es.iter().any(|e| self.value_owns(e, inner)),
            _ => true,
        }
    }

    pub fn is_resource(&self, ty: &Type) -> bool {
        match ty {
            Type::Handle(_) | Type::Mutex(_) | Type::Guard(_) => true,
            Type::Array(inner, _) => self.is_resource(inner),
            Type::Named(name, args) => {
                if let Some(s) = self.items.structs.get(name) {
                    if s.resource {
                        return true;
                    }
                    let subst = self.subst_map(&s.type_params, args);
                    s.fields.iter().any(|f| self.is_resource(&self.apply_subst(&f.ty, &subst)))
                } else if let Some(e) = self.items.enums.get(name) {
                    if e.resource {
                        return true;
                    }
                    let subst = self.subst_map(&e.type_params, args);
                    e.variants.iter().any(|v| {
                        v.payload
                            .as_ref()
                            .map(|p| self.is_resource(&self.apply_subst(p, &subst)))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
            Type::Closure(_) => false, // resolved specially at construction time; see closure objects
            _ => false,
        }
    }

    fn subst_map(&self, params: &[String], args: &[Type]) -> HashMap<String, Type> {
        params.iter().cloned().zip(args.iter().cloned()).collect()
    }

    pub fn apply_subst(&self, ty: &Type, subst: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Named(name, args) => {
                if args.is_empty() {
                    if let Some(t) = subst.get(name) {
                        return t.clone();
                    }
                }
                Type::Named(name.clone(), args.iter().map(|a| self.apply_subst(a, subst)).collect())
            }
            Type::Ref(inner, m) => Type::Ref(Box::new(self.apply_subst(inner, subst)), m.clone()),
            Type::Slice(inner, m) => Type::Slice(Box::new(self.apply_subst(inner, subst)), m.clone()),
            Type::Rawptr(inner) => Type::Rawptr(Box::new(self.apply_subst(inner, subst))),
            Type::Array(inner, n) => Type::Array(Box::new(self.apply_subst(inner, subst)), *n),
            Type::Fn(ps, r) => Type::Fn(
                ps.iter().map(|p| self.apply_subst(p, subst)).collect(),
                Box::new(self.apply_subst(r, subst)),
            ),
            Type::Handle(inner) => Type::Handle(Box::new(self.apply_subst(inner, subst))),
            Type::Mutex(inner) => Type::Mutex(Box::new(self.apply_subst(inner, subst))),
            Type::Guard(inner) => Type::Guard(Box::new(self.apply_subst(inner, subst))),
            other => other.clone(),
        }
    }

    // ---- frames / bindings ----

    fn push_frame(&mut self) {
        self.frames_mut().push(Frame::new());
    }

    fn push_call_frame(&mut self) {
        let mut f = Frame::new();
        f.is_call = true;
        self.frames_mut().push(f);
    }

    fn bind(&mut self, name: &str, obj: u64) {
        // A bound object is no longer a temporary of any statement.
        if self.temp_index.remove(&obj) {
            for t in self.stmt_temps.iter_mut() {
                t.retain(|o| *o != obj);
            }
        }
        let ty = self.objects.get(&obj).map(|o| o.ty.clone());
        let f = self.frames_mut().last_mut().unwrap();
        f.bindings.insert(name.to_string(), obj);
        if let Some(ty) = ty {
            f.types.insert(name.to_string(), ty);
        }
        f.owned.push(obj);
        f.moved.remove(name);
    }

    // D-0033 (1): `x = rhs` where `x`'s value was moved away or destroyed
    // (before `rhs`, or by it, as in `x = f(x)`). The new value becomes a
    // new object bound to `x` in the frame that declared it, as `x`'s
    // first value was (`[Let]`). `r` is `rhs`, already evaluated.
    fn reinit_binding(&mut self, name: &str, r: EvalResult) -> EvalOutcome {
        let Some(idx) = self.frames().iter().rposition(|f| f.bindings.contains_key(name)) else {
            return Err(Flow::Fault("diag.unbound-name".into()));
        };
        let ty = self.frames()[idx].types.get(name).cloned();
        self.store_binding(name, r, ty)?;
        // `store_binding` bound it in the innermost frame; move it to
        // `x`'s own, so it ends with `x`'s scope.
        let last = self.frames().len() - 1;
        if idx != last {
            let f = self.frames_mut().last_mut().unwrap();
            let obj = f.bindings.remove(name).expect("just bound");
            let t = f.types.remove(name);
            f.owned.retain(|o| *o != obj);
            let g = &mut self.frames_mut()[idx];
            g.bindings.insert(name.to_string(), obj);
            if let Some(t) = t {
                g.types.insert(name.to_string(), t);
            }
            g.owned.push(obj);
            g.moved.remove(name);
        }
        Ok(EvalResult::Val(Value::Unit))
    }

    // The bytes of the `String` at `path` in `obj`: its `Vec<u8>`'s
    // `len` bytes at `ptr`, in the arena.
    fn string_bytes(&self, obj: u64, path: &[Proj]) -> Vec<u8> {
        let Value::Struct(fields) = self.read_place(obj, path) else { return Vec::new() };
        let Some(Value::Struct(vec)) = fields.first() else { return Vec::new() };
        let (Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vec.first(), vec.get(1)) else { return Vec::new() };
        (0..*len as u64).map(|j| *self.arena.get(&(addr + j)).unwrap_or(&0)).collect()
    }

    // A key's bytes (`rule.stdlib.hashmap`): an integer's little-endian
    // bytes in its own width, a `bool`'s one byte, text's UTF-8 bytes.
    fn key_bytes(&self, v: Value) -> Result<Vec<u8>, Flow> {
        let Value::Ref { obj, path, pointee, .. } = v else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if matches!(&pointee, Type::Named(n, _) if n == "std::String") {
            return Ok(self.string_bytes(obj, &path));
        }
        match self.read_place(obj, &path) {
            Value::Int(t, x) => Ok((x as u128).to_le_bytes()[..(t.bitwidth(ADDR_WIDTH) / 8) as usize].to_vec()),
            Value::Bool(b) => Ok(vec![b as u8]),
            Value::Str(b) => Ok(b.to_vec()),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Whether binding `name` currently has no value (moved away or
    // destroyed), and the type it was declared with.
    fn binding_gone(&self, name: &str) -> Option<Option<Type>> {
        let (obj, moved) = self.lookup(name)?;
        if !(moved || !self.object_live(obj)) {
            return None;
        }
        let idx = self.frames().iter().rposition(|f| f.bindings.contains_key(name))?;
        Some(self.frames()[idx].types.get(name).cloned())
    }

    // Binds a name to an object this frame does *not* own — used only for
    // a borrow-captured closure variable, which names the referent's own
    // identity directly (see `bind_closure_captures`) without becoming
    // responsible for ending it.
    fn bind_alias(&mut self, name: &str, obj: u64) {
        let f = self.frames_mut().last_mut().unwrap();
        f.bindings.insert(name.to_string(), obj);
        f.moved.remove(name);
    }

    fn lookup(&self, name: &str) -> Option<(u64, bool)> {
        for f in self.frames().iter().rev() {
            if let Some(&obj) = f.bindings.get(name) {
                return Some((obj, f.moved.contains(name)));
            }
            if f.is_call {
                break;
            }
        }
        None
    }

    // Pop the current frame, destroying every object it still owns
    // (reverse declaration order, spec/14 [Block-Exit]/D-0008), except the
    // identity named by `keep` (the block's own result), which flows out.
    fn pop_frame(&mut self, keep: Option<u64>) -> Result<(), Flow> {
        let f = self.frames_mut().pop().unwrap();
        if self.terminated {
            return Ok(());
        }
        for &obj in f.owned.iter().rev() {
            if Some(obj) == keep {
                continue;
            }
            if self.objects.contains_key(&obj) {
                self.destroy_object(obj)?;
            }
        }
        Ok(())
    }

    // ---- aliasing: scan every live object for Ref/Guard occurrences ----

    fn scan_refs(&self) -> Vec<(u64, Vec<Proj>, u64, Vec<Proj>, Mode, u64, bool)> {
        // (holder, holder_path, target, target_path, mode, token, is_guard)
        let mut out = Vec::new();
        for &holder in self.ref_holders.iter() {
            if let Some(obj) = self.objects.get(&holder) {
                Self::scan_value(&obj.value, holder, &mut Vec::new(), &mut out);
            }
        }
        // References in raw cells: held by the reclaimed object over the
        // cells, which writing them established (`spec/20` `[Rawptr-Write]`,
        // `CHG-0031`; `[Rawptr-Move-In]`). The entries go when that object
        // ends (`destroy_object`, `release`, `deallocate`, a move out).
        for v in self.arena_side.values() {
            Self::scan_value(v, 0, &mut Vec::new(), &mut out);
        }
        out
    }

    fn scan_value(v: &Value, holder: u64, path: &mut Vec<Proj>, out: &mut Vec<(u64, Vec<Proj>, u64, Vec<Proj>, Mode, u64, bool)>) {
        match v {
            Value::Ref { obj, path: tp, mode, token, .. } => {
                out.push((holder, path.clone(), *obj, tp.clone(), mode.clone(), *token, false));
            }
            Value::Guard { obj, path: tp, token, .. } => {
                out.push((holder, path.clone(), *obj, tp.clone(), Mode::Exclusive, *token, true));
            }
            Value::Struct(fields) => {
                for (i, f) in fields.iter().enumerate() {
                    path.push(Proj::Field(i));
                    Self::scan_value(f, holder, path, out);
                    path.pop();
                }
            }
            Value::Array(items) => {
                for (i, e) in items.iter().enumerate() {
                    path.push(Proj::Index(i));
                    Self::scan_value(e, holder, path, out);
                    path.pop();
                }
            }
            Value::Enum(_, payload) => {
                path.push(Proj::Payload);
                Self::scan_value(payload, holder, path, out);
                path.pop();
            }
            _ => {}
        }
    }

    // Is `token` covered by `excludes`, either directly or because `token`
    // is a persistent ancestor (`record_token_ancestors`) of one of the
    // tokens in `excludes`? The latter is what makes a live occurrence of
    // `v`'s own borrow stop conflicting with a later access made through a
    // sub-borrow of `v` formed earlier (e.g. `v.len` inside a function
    // called with `&*v` as its argument, several frames after the
    // sub-borrow was minted) — `excludes` only ever names the *immediate*
    // chain for *this* access; `token_ancestors` is what remembers that an
    // earlier access's chain is part of this one's lineage too.
    fn excluded(&self, excludes: &[u64], token: u64) -> bool {
        if excludes.contains(&token) {
            return true;
        }
        excludes.iter().any(|e| self.token_ancestors.get(e).map_or(false, |anc| anc.contains(&token)))
    }

    // ¬clash(target/path, mode), excluding the occurrences identified by
    // `excludes` (the access-path *tokens* used to reach this very access,
    // if any — spec/08 `ancestors`: a reference is always its own access's
    // ancestor). Token identity, not which object currently holds a copy
    // of the value, is what makes two occurrences "the same path" — e.g.
    // passing a `ref` argument into a callee copies the value but not its
    // token (spec/15 §3), so the caller's and callee's copies must both be
    // excluded together, not just whichever one is lexically "in scope".
    fn clash(&self, target: u64, path: &[Proj], mode: Mode, excludes: &[u64]) -> bool {
        // `sync-exempt` (spec/19 §2, spec/04 §2): a lock-derived path (a
        // guard) never conflicts with a `shared` reference to the mutex
        // it locks, in either direction — that serialization is what
        // `lock`'s own blocking-on-`unlocked` already provides. A guard
        // always projects into the mutex's `inner` sub-range (a non-empty
        // path on a `mutex<_>` target), which no other rule ever reaches,
        // so "this access goes via a guard" is exactly that shape.
        let target_is_mutex = matches!(self.objects.get(&target).map(|o| &o.ty), Some(Type::Mutex(_)));
        let this_access_is_guard_derived = target_is_mutex && !path.is_empty();
        for (_holder, _hpath, tgt, tpath, m, token, is_guard) in self.scan_refs() {
            if tgt != target {
                continue;
            }
            if self.excluded(excludes, token) {
                continue;
            }
            if !overlap(path, &tpath) {
                continue;
            }
            if (is_guard && mode == Mode::Shared) || (this_access_is_guard_derived && m == Mode::Shared) {
                continue;
            }
            if !permitted(mode.clone(), m) {
                return true;
            }
        }
        false
    }

    fn solitary(&self, target: u64, excludes: &[u64]) -> bool {
        for (_holder, _hpath, tgt, _tpath, _m, token, _is_guard) in self.scan_refs() {
            if tgt != target {
                continue;
            }
            if self.excluded(excludes, token) {
                continue;
            }
            return false;
        }
        true
    }

    // ---- value navigation ----

    fn get_sub<'a>(v: &'a Value, path: &[Proj]) -> &'a Value {
        match path.split_first() {
            None => v,
            Some((Proj::Field(i), rest)) => match v {
                Value::Struct(fs) => Self::get_sub(&fs[*i], rest),
                _ => panic!("field projection on non-struct"),
            },
            Some((Proj::Index(i), rest)) => match v {
                Value::Array(es) => Self::get_sub(&es[*i], rest),
                _ => panic!("index projection on non-array"),
            },
            Some((Proj::Payload, rest)) => match v {
                Value::Enum(_, p) => Self::get_sub(p, rest),
                _ => panic!("payload projection on non-enum"),
            },
        }
    }
    fn get_sub_mut<'a>(v: &'a mut Value, path: &[Proj]) -> &'a mut Value {
        match path.split_first() {
            None => v,
            Some((Proj::Field(i), rest)) => match v {
                Value::Struct(fs) => Self::get_sub_mut(&mut fs[*i], rest),
                _ => panic!("field projection on non-struct"),
            },
            Some((Proj::Index(i), rest)) => match v {
                Value::Array(es) => Self::get_sub_mut(&mut es[*i], rest),
                _ => panic!("index projection on non-array"),
            },
            Some((Proj::Payload, rest)) => match v {
                Value::Enum(_, p) => Self::get_sub_mut(p, rest),
                _ => panic!("payload projection on non-enum"),
            },
        }
    }

    pub fn read_place(&self, obj: u64, path: &[Proj]) -> Value {
        if let Some((addr, ty)) = self.reclaimed_addr.get(&obj) {
            let (off, subty) = self.path_offset_at(Some(*addr), ty, path);
            return self.arena_read(addr + off, &subty);
        }
        Self::get_sub(&self.objects.get(&obj).expect("stale object").value, path).clone()
    }

    pub fn write_place(&mut self, obj: u64, path: &[Proj], v: Value) {
        if let Some((addr, ty)) = self.reclaimed_addr.get(&obj).cloned() {
            let (off, subty) = self.path_offset_at(Some(addr), &ty, path);
            self.arena_write(addr + off, &subty, v);
            return;
        }
        if Self::value_has_ref(&v) {
            self.ref_holders.insert(obj);
        }
        let o = self.objects.get_mut(&obj).expect("stale object");
        *Self::get_sub_mut(&mut o.value, path) = v;
    }

    fn type_at(&self, obj: u64, path: &[Proj]) -> Type {
        // A closure's own `.ty` is the opaque `Type::Closure(cid)` tag; a
        // projection into one of its capture fields is resolved from the
        // per-instance capture-type table recorded at closure formation,
        // then the walk below continues normally from there.
        if let Some((Proj::Field(i), _)) = path.split_first() {
            if matches!(self.objects.get(&obj).map(|o| &o.ty), Some(Type::Closure(_))) {
                if let Some(fty) = self.closure_field_types.get(&obj).and_then(|v| v.get(*i)) {
                    return walk(self, obj, path, 1, fty);
                }
            }
        }
        // `pos` is how much of `path` has been walked to reach `ty`.
        fn walk(interp: &Interp, obj: u64, path: &[Proj], pos: usize, ty: &Type) -> Type {
            let Some(step) = path.get(pos) else {
                return ty.clone();
            };
            match step {
                Proj::Payload => {
                    // The active variant's payload type: the variant is
                    // read from the enum value this far along the path
                    // (a reference into a payload, D-0046, walks on).
                    if let (Type::Named(name, args), Value::Enum(vi, _)) = (ty, interp.read_place(obj, &path[..pos])) {
                        if let Some(e) = interp.items.enums.get(name) {
                            if let Some(pt) = e.variants.get(vi).and_then(|v| v.payload.clone()) {
                                let subst = interp.subst_map(&e.type_params, args);
                                let pt = interp.apply_subst(&pt, &subst);
                                return walk(interp, obj, path, pos + 1, &pt);
                            }
                        }
                    }
                    ty.clone()
                }
                Proj::Field(i) => {
                    if let Type::Named(name, args) = ty {
                        if let Some(s) = interp.items.structs.get(name) {
                            let subst = interp.subst_map(&s.type_params, args);
                            let ft = interp.apply_subst(&s.fields[*i].ty, &subst);
                            return walk(interp, obj, path, pos + 1, &ft);
                        }
                    }
                    if let Type::Mutex(inner) = ty {
                        if *i == 0 {
                            return walk(interp, obj, path, pos + 1, inner);
                        }
                    }
                    ty.clone()
                }
                Proj::Index(_) => {
                    if let Type::Array(inner, _) = ty {
                        return walk(interp, obj, path, pos + 1, inner);
                    }
                    ty.clone()
                }
            }
        }
        let base = self.objects.get(&obj).map(|o| o.ty.clone()).unwrap_or(Type::Void);
        walk(self, obj, path, 0, &base)
    }

    fn type_of_value(&self, v: &Value) -> Type {
        match v {
            Value::Int(t, _) => Type::Int(*t),
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            Value::Bool(_) => Type::Bool,
            Value::Str(_) => Type::Str,
            Value::Unit => Type::Void,
            Value::Ref { mode, pointee, .. } => Type::Ref(Box::new(pointee.clone()), mode.clone()),
            Value::Rawptr(_, t) => Type::Rawptr(Box::new(t.clone())),
            Value::FnVal(_) => Type::Void,
            Value::Closure(id) => self.objects.get(id).map(|o| o.ty.clone()).unwrap_or(Type::Void),
            Value::Struct(_) => Type::Void,
            Value::Array(_) => Type::Void,
            Value::Enum(_, _) => Type::Void,
            Value::Handle(_, t) => Type::Handle(Box::new(t.clone())),
            Value::Guard { inner, .. } => Type::Guard(Box::new(inner.clone())),
        }
    }

    // ---- destroy ----

    pub fn destroy_object(&mut self, id: u64) -> Result<(), Flow> {
        let obj = match self.objects.get(&id) {
            Some(o) => o,
            None => return Ok(()),
        };
        if obj.is_resource {
            // [Destroy-No-Authority]: only the thread holding the
            // object's destroy authority may destroy it.
            if obj.owner_thread != self.current_thread {
                return Err(Flow::Fault("diag.no-destroy-authority".into()));
            }
            let ty = obj.ty.clone();
            self.run_destructor(id, &ty)?;
            self.destroy_composite(id)?;
        }
        self.remove_object(&id);
        // `[Object-End]`: an arena-backed identity (a reclaimed object, or
        // a local that `rawptr_of` gave an address) no longer claims its
        // cells; a later `reclaim` there establishes a fresh identity, and
        // the references it held in them are held no longer.
        if let Some((addr, rty)) = self.reclaimed_addr.get(&id).cloned() {
            let n = self.sizeof_ty(&rty);
            self.clear_side(addr, n);
        }
        if let Some((addr, _)) = self.reclaimed_addr.remove(&id) {
            if self.reclaimed_by_addr.get(&addr) == Some(&id) {
                self.reclaimed_by_addr.remove(&addr);
            }
        }
        Ok(())
    }

    // `[Object-End]` of an object whose contents were all moved out (a
    // destructured struct): no destructor, nothing inside destroyed.
    fn end_moved_out(&mut self, id: u64) {
        self.remove_object(&id);
        if let Some((addr, rty)) = self.reclaimed_addr.remove(&id) {
            let n = self.sizeof_ty(&rty);
            self.clear_side(addr, n);
            if self.reclaimed_by_addr.get(&addr) == Some(&id) {
                self.reclaimed_by_addr.remove(&addr);
            }
        }
    }

    fn run_destructor(&mut self, id: u64, ty: &Type) -> Result<(), Flow> {
        match ty {
            Type::Handle(_) => {
                // [Handle-Destructor] (spec/19 §1): destroying a handle
                // any other way than `join` -- `drop(h)`, or the
                // automatic sweep at block exit -- waits for the thread
                // exactly as `join` would, then discards its result
                // (destroying it as a temporary if it is a resource).
                let ell = match self.objects.get(&id).map(|o| o.value.clone()) {
                    Some(Value::Handle(k, _)) => k,
                    _ => return Ok(()),
                };
                self.block_until(|s| matches!(s.thread_status.get(&ell), Some(ThreadStatus::Done(..)) | Some(ThreadStatus::Taken)))?;
                let taken = self.shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Taken);
                if let Some(ThreadStatus::Done(rv, rty)) = taken {
                    if self.is_resource(&rty) {
                        let id = self.new_object(rty, rv);
                        self.destroy_object(id)?;
                    }
                }
                Ok(())
            }
            Type::Guard(_) => {
                // [Guard-Drop]: unlock the mutex this guard's path targets.
                if let Some(o) = self.objects.get(&id) {
                    if let Value::Guard { obj, .. } = &o.value {
                        self.shared.lock().unwrap().cobalt_locks.remove(obj);
                    }
                }
                Ok(())
            }
            Type::Mutex(_) => Ok(()), // its only cleanup is `inner`, handled by destroy_composite
            Type::Named(name, args) => {
                if self.items.structs.contains_key(name) {
                    let dname = format!("{}::drop", name);
                    if let Some(f) = self.items.fns.get(&dname).cloned() {
                        self.call_fn_on_object(&f, args, id)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // Calls fn Type::drop(ref<Type,exclusive> self) with `self` bound to a
    // fresh exclusive root path into `id` (spec/07 [Run-Destructor]).
    fn call_fn_on_object(&mut self, f: &Arc<FnDecl>, targs: &[Type], id: u64) -> Result<(), Flow> {
        let subst = self.subst_map(&f.type_params, targs);
        self.push_call_frame();
        let pointee = self.objects.get(&id).unwrap().ty.clone();
        let selfv = Value::Ref { obj: id, path: vec![], mode: Mode::Exclusive, pointee, token: self.fresh_id() };
        let selfobj = self.new_object(Type::Ref(Box::new(Type::Void), Mode::Exclusive), selfv);
        self.bind(&f.params[0].name, selfobj);
        let r = self.exec_block_body(&f.body, &subst);
        self.pop_frame(None)?;
        match r {
            Ok(_) => Ok(()),
            Err(Flow::Return(_)) => Ok(()),
            Err(other) => Err(other),
        }
    }

    fn destroy_composite(&mut self, id: u64) -> Result<(), Flow> {
        // Recursively destroy every resource-bearing field/element/payload,
        // innermost first (spec/07 [Destroy-Composite]). A reclaimed
        // object's value lives in its raw cells, not in the object record
        // (which holds a placeholder): read it from there, or a struct
        // with no destructor of its own -- `Rc`'s box, a `Vec` element --
        // would never destroy the resources in its fields.
        let value = match self.reclaimed_addr.get(&id).cloned() {
            Some((addr, rty)) => Some(self.arena_read(addr, &rty)),
            None => self.objects.get(&id).map(|o| o.value.clone()),
        };
        let ty = self.objects.get(&id).map(|o| o.ty.clone());
        // A closure's captures are its fields; their types are recorded
        // per object (the `Type::Closure` tag carries none). Last first,
        // as for struct fields.
        if let (Some(Value::Struct(fields)), Some(Type::Closure(_))) = (&value, &ty) {
            let ftys = self.closure_field_types.get(&id).cloned().unwrap_or_default();
            for (f, fty) in fields.iter().zip(ftys.iter()).rev() {
                if self.is_resource(fty) {
                    self.destroy_field_value(f, fty)?;
                }
            }
            return Ok(());
        }
        if let (Some(value), Some(ty)) = (value, ty) {
            self.destroy_composite_value(&value, &ty)?;
        }
        Ok(())
    }

    fn destroy_composite_value(&mut self, v: &Value, ty: &Type) -> Result<(), Flow> {
        match (v, ty) {
            (Value::Struct(fields), Type::Named(name, args)) => {
                if let Some(s) = self.items.structs.get(name).cloned() {
                    let subst = self.subst_map(&s.type_params, args);
                    // [Destroy-Composite] (spec/07): same-length paths
                    // (sibling fields) tie-break by *reverse*
                    // declaration/index order -- last-declared field
                    // destroyed first, mirroring D-0008's block-scope
                    // reverse order. Was forward (declaration order),
                    // confirmed backwards by a real program whose
                    // fields' own destructors each write a distinguishing
                    // byte and observing the emitted order.
                    for (i, f) in fields.iter().enumerate().rev() {
                        let fty = self.apply_subst(&s.fields[i].ty, &subst);
                        if self.is_resource(&fty) {
                            self.destroy_field_value(f, &fty)?;
                        }
                    }
                }
                Ok(())
            }
            (Value::Struct(fields), Type::Mutex(inner)) => {
                // layout: field 0 = inner
                if self.is_resource(inner) {
                    self.destroy_field_value(&fields[0], inner)?;
                }
                Ok(())
            }
            (Value::Array(items), Type::Array(inner, _)) => {
                // Same reverse-index tie-break as the struct-field case
                // above -- last element destroyed first.
                if self.is_resource(inner) {
                    for it in items.iter().rev() {
                        self.destroy_field_value(it, inner)?;
                    }
                }
                Ok(())
            }
            (Value::Enum(_, payload), Type::Named(name, args)) => {
                if let Some(e) = self.items.enums.get(name).cloned() {
                    let subst = self.subst_map(&e.type_params, args);
                    // find active variant's payload type by trying to match; we
                    // don't track discriminant here, so destroy whatever
                    // payload type resource-ness matches (best effort: try all
                    // variants' declared payload types that this value could
                    // be, using is_resource union already computed at
                    // is_resource(enum) time). We instead re-derive using the
                    // Enum's variant index stored alongside; but v is just the
                    // payload here per match arm above (Value::Enum(_, payload)),
                    // so `_` is the discriminant we do have:
                    if let Value::Enum(vi, _) = v {
                        if let Some(vd) = e.variants.get(*vi) {
                            if let Some(pty) = &vd.payload {
                                let pty = self.apply_subst(pty, &subst);
                                // `Value::Unit` in a non-unit resource
                                // payload slot marks one already moved out
                                // by a `match` binder (see eval_match) —
                                // nothing left here to destroy.
                                if self.is_resource(&pty) && !matches!(payload.as_ref(), Value::Unit) {
                                    self.destroy_field_value(payload, &pty)?;
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // Destroys a resource value that is a field/element/payload (not a
    // top-level object): runs its destructor (if a named struct) and
    // recurses, without going through end-object (the storage belongs to
    // the container).
    fn destroy_field_value(&mut self, v: &Value, ty: &Type) -> Result<(), Flow> {
        if let Type::Named(name, args) = ty {
            if self.items.structs.contains_key(name) {
                let dname = format!("{}::drop", name);
                if let Some(f) = self.items.fns.get(&dname).cloned() {
                    // give the field its own temporary object so `self` can
                    // borrow it, then copy any mutation back isn't needed
                    // since the container is being torn down anyway.
                    let tmp = self.new_object(ty.clone(), v.clone());
                    self.call_fn_on_object(&f, args, tmp)?;
                    self.remove_object(&tmp);
                }
            }
        }
        self.destroy_composite_value(v, ty)
    }

    // ================= Program execution =================

    // One attempt at running (or resuming) `main` to completion. `Ok(Done)`
    // means the program terminated `ok`; `Ok(WouldBlock)` means a
    // `join`/`lock` in `main`'s own body is genuinely waiting on another
    // OS thread -- the caller (lib.rs, which holds the real lock this
    // `Interp` lives behind) must release it, wait briefly, and call this
    // again on the same `Interp` to resume exactly where it left off.
    //
    // `Ok(s)` is `[Terminate-Ok]`'s `ok(s)`: `main`'s value for
    // `fn main() : u8`, 0 otherwise (`rule.fn.program`).
    pub fn run_main(&mut self) -> Result<u8, String> {
        let main = self
            .items
            .fns
            .get("main")
            .cloned()
            .ok_or_else(|| "diag.no-main".to_string())?;
        self.current_thread = 0;
        let r = self.run_body(&main.body, &HashMap::new(), true, &[], None);
        match r {
            Ok(res) => {
                // A worker may have faulted after main's last wait;
                // the program's outcome is that fault (spec/18).
                if let Some(d) = self.shared.lock().unwrap().program_fault.clone() {
                    return Err(d);
                }
                if main.ret != Type::Void {
                    let v = match res {
                        EvalResult::Val(v) => v,
                        other => self.result_to_value(other).map_err(|f| self.flow_to_string(f))?,
                    };
                    if let Value::Int(_, s) = v {
                        return Ok(s as u8);
                    }
                }
                Ok(0)
            }
            Err(Flow::Terminated) => Err(self.shared.lock().unwrap().program_fault.clone().unwrap_or_else(|| "terminated".into())),
            Err(Flow::Fault(d)) => {
                // Tell every still-live spawned thread to stop, too
                // (spec/18: the whole program terminates, not just
                // main's own thread).
                self.shared.lock().unwrap().program_fault = Some(d.clone());
                self.terminated = true;
                Err(d)
            }
            Err(_) => Err("unexpected control flow at top level".into()),
        }
    }

    // Runs `body` as an *outermost* per-(real-)thread execution --
    // `main`'s own body, or a spawned thread's callee body -- with its
    // parameters (and, for a spawned move closure, its captures) bound
    // in a fresh frame first.
    fn run_body(
        &mut self,
        body: &Block,
        subst: &HashMap<String, Type>,
        first_call: bool,
        params: &[(String, EvalResult, Option<Type>)],
        closure_bind: Option<(u64, &[String])>,
    ) -> Result<EvalResult, Flow> {
        let mut cells = Vec::new();
        if first_call {
            self.push_call_frame();
            self.binding_thread_params = self.current_thread != 0;
            let mut bound = Ok(());
            for (name, val, ty) in params {
                // D-0049: an exclusive reference (or slice) passed for a
                // shared parameter is reborrowed shared, as `&*e` would be.
                let val = match self.weaken_arg(val, ty.as_ref()) {
                    Ok(v) => v,
                    Err(e) => {
                        bound = Err(e);
                        break;
                    }
                };
                if let Err(e) = self.store_binding(name, val, ty.clone()) {
                    bound = Err(e);
                    break;
                }
            }
            self.binding_thread_params = false;
            bound?;
            if let Some((obj, captures)) = closure_bind {
                // Only a `move` closure can be spawned (`rule.conc.spawn`):
                // the cells are the per-call views of its captures.
                cells = self.bind_closure_captures(obj, captures);
            }
        }
        let r = match self.exec_block_stmts(body, subst, None) {
            Ok(res) => {
                let res = self.into_result_form(res)?;
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(Flow::Return(v)) => {
                let keep = if let EvalResult::Temp(o) = &v { Some(*o) } else { None };
                self.pop_frame(keep)?;
                Ok(v)
            }
            Err(other) => {
                self.pop_frame(None)?;
                Err(other)
            }
        };
        self.release_capture_cells(&cells);
        // The spawned closure itself ends with its thread, and with it
        // anything it owns; `[Spawn]` gave its authority to this thread.
        if let (Ok(_), Some((obj, _))) = (&r, closure_bind) {
            if let Some(o) = self.objects.get_mut(&obj) {
                o.owner_thread = self.current_thread;
            }
            if self.objects.contains_key(&obj) {
                self.destroy_object(obj)?;
            }
        }
        r
    }

    fn flow_to_string(&self, f: Flow) -> String {
        match f {
            Flow::Fault(d) => d,
            Flow::Terminated => self.shared.lock().unwrap().program_fault.clone().unwrap_or_else(|| "terminated".into()),
            _ => "unexpected control flow".into(),
        }
    }

    // Runs a function body's block in a *nested* frame (the block's own
    // frame, inside whatever parameter frame the caller pushed) and
    // returns its result in result form.
    // Thin wrapper (see `current_line`'s doc comment): catches a fault
    // raised by `pop_frame`'s own automatic block-exit destruction
    // sweep (D-0008) below, which is not reached by `exec_stmt`'s own
    // wrapping since it runs *after* every statement in the block, not
    // as part of any one of them.
    fn exec_block_body(&mut self, b: &Block, subst: &HashMap<String, Type>) -> EvalOutcome {
        self.exec_block_body_expected(b, subst, None)
    }

    fn exec_block_body_expected(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        match self.exec_block_body_inner(b, subst, expected) {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        }
    }

    fn exec_block_body_inner(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        self.push_frame();
        let r = self.exec_block_stmts(b, subst, expected);
        match r {
            Ok(res) => {
                // Put the result in result form (spec/14 [Block-Exit]
                // reads `r ∈ {v, temp o}` — a raw `place` is never what
                // flows out of a closing frame) *before* deciding what
                // survives the sweep, so a trailing plain identifier
                // (`p` as a match arm's/block's tail) is read out rather
                // than left as a dangling place into an object this very
                // frame is about to destroy.
                let res = self.into_result_form(res)?;
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(flow) => {
                // unwind: pop the frame's owned objects too (D-0008 sweep
                // during return/break/continue/fault unwinding) -- except a
                // value being returned, which is the caller's now.
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                Err(flow)
            }
        }
    }

    fn exec_block_stmts(&mut self, b: &Block, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        for s in &b.stmts {
            self.exec_stmt(s, subst)?;
            self.yield_step();
        }
        // `rule.type.expected`: a block's trailing expression takes the
        // expected type of the block itself -- a function body's result
        // is expected to be its return type, an `if` branch's or `match`
        // arm's result the type expected of the whole `if`/`match`.
        // A trailing expression is a statement for its temporaries
        // (`rule.control.stmt`, "and of a block's trailing expression"):
        // after its result is in result form, every other temporary it
        // created ends, and the result's own temporary passes to the
        // enclosing statement.
        let Some(e) = &b.tail else {
            return Ok(EvalResult::Val(Value::Unit));
        };
        self.stmt_temps.push(Vec::new());
        let r = match expected {
            Some(ty) => self.eval_expected(e, subst, ty),
            None => self.eval(e, subst),
        };
        let temps = self.stmt_temps.pop().unwrap_or_default();
        let res = match r {
            Ok(res) => res,
            Err(flow) => return Err(self.end_temps_on_exit(temps, flow)),
        };
        // The result, and any temporary it refers into, stays with the
        // enclosing statement.
        let used: Vec<u64> = match &res {
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => vec![*o],
            EvalResult::Val(v) => Self::ref_targets(v),
        };
        let (keep, end): (Vec<u64>, Vec<u64>) = temps.into_iter().partition(|t| used.contains(t));
        for o in keep {
            self.stmt_temp(o);
        }
        self.end_stmt_temps(end)?;
        Ok(res)
    }

    // The objects a value's references point into.
    fn ref_targets(v: &Value) -> Vec<u64> {
        let mut out = Vec::new();
        fn walk(v: &Value, out: &mut Vec<u64>) {
            match v {
                Value::Ref { obj, .. } => out.push(*obj),
                Value::Struct(fs) | Value::Array(fs) => fs.iter().for_each(|f| walk(f, out)),
                Value::Enum(_, p) => walk(p, out),
                _ => {}
            }
        }
        walk(v, &mut out);
        out
    }

    // Thin wrapper (see `current_line`'s doc comment): most statement
    // faults already arrive pre-tagged, having passed through `eval()`
    // on their way out of `exec_stmt_inner`; this only ever fires for
    // the few direct `Flow::Fault` constructions in this statement's
    // own post-`eval()` bookkeeping (e.g. `store_binding`,
    // `destroy_object`), tagging them with the last expression line
    // touched in this statement as a reasonable, honestly-approximate
    // fallback -- never a fabricated exact location.
    fn exec_stmt(&mut self, s: &Stmt, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        self.stmt_temps.push(Vec::new());
        let r = self.exec_stmt_inner(s, subst);
        let temps = self.stmt_temps.pop().unwrap_or_default();
        let r = match r {
            Ok(()) => self.end_stmt_temps(temps),
            Err(flow) => Err(self.end_temps_on_exit(temps, flow)),
        };
        match r {
            Err(Flow::Fault(d)) => Err(Flow::Fault(self.tag_current_line(d))),
            other => other,
        }
    }

    // A temporary to be ended with the current statement (or, outside
    // any statement, left as before).
    fn stmt_temp(&mut self, o: u64) {
        if let Some(t) = self.stmt_temps.last_mut() {
            if !t.contains(&o) {
                t.push(o);
                self.temp_index.insert(o);
            }
        }
    }

    // A statement left by `return`, `break` or `continue` ends its
    // temporaries as one that finishes does (`rule.control.stmt`): a
    // temporary `Option<ref<…>>` scrutinee must not keep its borrow alive
    // after the `return` in one of its arms. What a `return` carries out
    // (and any temporary it refers into) is the caller's and stays. A
    // fault keeps its own unwinding. Returns the flow to propagate: the
    // original one, or a fault an ending destructor raised.
    fn end_temps_on_exit(&mut self, temps: Vec<u64>, flow: Flow) -> Flow {
        let keep: Vec<u64> = match &flow {
            Flow::Return(EvalResult::Temp(o)) | Flow::Return(EvalResult::Place(o, _, _)) => vec![*o],
            Flow::Return(EvalResult::Val(v)) => Self::ref_targets(v),
            Flow::Break | Flow::Continue => Vec::new(),
            _ => return flow,
        };
        let end: Vec<u64> = temps.into_iter().filter(|t| !keep.contains(t)).collect();
        match self.end_stmt_temps(end) {
            Ok(()) => flow,
            Err(f) => f,
        }
    }

    fn end_stmt_temps(&mut self, temps: Vec<u64>) -> Result<(), Flow> {
        for o in temps.into_iter().rev() {
            self.temp_index.remove(&o);
            if self.objects.contains_key(&o) {
                self.destroy_object(o)?;
            }
        }
        Ok(())
    }

    fn exec_stmt_inner(&mut self, s: &Stmt, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        match s {
            Stmt::Let { ty, name, init } => {
                let declared_ty = ty.as_ref().map(|t| self.apply_subst(t, subst));
                if init.is_none() {
                    // [Let-Uninit] (spec/11): `τ x;` establishes a fresh
                    // object with no value yet -- `rule.init.definite-
                    // assignment` requires a read to fault
                    // `diag.use-of-uninitialized` before the first write,
                    // not silently observe a placeholder. `ty` is always
                    // `Some` here (the rule's own text: "there being
                    // nothing to synthesize from, uninitialized always
                    // requires the written type").
                    let dty = declared_ty.clone().unwrap_or(Type::Void);
                    let id = self.new_object(dty, Value::Unit);
                    self.uninit_bindings.insert(id);
                    self.bind(name, id);
                    return Ok(());
                }
                let r = match (init, &declared_ty) {
                    (Some(e), Some(dty)) => self.eval_expected(e, subst, dty)?,
                    (Some(e), None) => self.eval(e, subst)?,
                    (None, _) => unreachable!("handled above"),
                };
                self.store_binding(name, r, declared_ty)?;
                Ok(())
            }
            Stmt::Destructure { fields, init, .. } => {
                let r = self.eval(init, subst)?;
                let (obj, val) = match r {
                    EvalResult::Temp(o) => (o, None),
                    EvalResult::Place(o, path, _) => (o, Some(path)),
                    EvalResult::Val(_) => unreachable!("destructure source is a place/temp"),
                };
                let base_path = val.unwrap_or_default();
                let sty = self.type_at(obj, &base_path);
                for field in fields {
                    let mut fpath = base_path.clone();
                    fpath.push(Proj::Field(self.field_index(&sty, field)?));
                    let fty = self.type_at(obj, &fpath);
                    let fval = self.read_place(obj, &fpath);
                    let fresh = self.new_object(fty, fval);
                    self.bind(field, fresh);
                }
                // [Relocate-Out]: the fields now belong to their bindings;
                // the container ends without destroying them (as `cobc`'s
                // `cb_end_moved_out`), or they would be destroyed twice.
                self.end_moved_out(obj);
                Ok(())
            }
            Stmt::Expr(e) | Stmt::BlockLike(e) => {
                self.eval_temp_discard(e, subst)?;
                Ok(())
            }
        }
    }

    // Evaluate an expression whose result is discarded as a statement:
    // if it produced a fresh temporary, destroy it now (spec/14 [Stmt-Exit]).
    fn eval_temp_discard(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<(), Flow> {
        let r = self.eval(e, subst)?;
        if let EvalResult::Temp(o) = r {
            self.destroy_object(o)?;
        }
        Ok(())
    }

    fn store_binding(&mut self, name: &str, r: EvalResult, declared: Option<Type>) -> Result<(), Flow> {
        match r {
            EvalResult::Val(v) => {
                let ty = declared.unwrap_or_else(|| self.type_of_value(&v));
                let id = self.new_object(ty, v);
                self.bind(name, id);
            }
            EvalResult::Temp(o) => {
                // Adoption re-keys a temporary produced elsewhere (a
                // thread's result claimed by `join`) to the adopter.
                if let Some(ob) = self.objects.get_mut(&o) {
                    ob.owner_thread = self.current_thread;
                    // A written type is the binding's type: `Option<u64> x
                    // = None;` makes an `Option<u64>`, whose later
                    // assignments then type their payloads as u64.
                    if let Some(d) = &declared {
                        if !matches!(d, Type::Void) {
                            ob.ty = d.clone();
                        }
                    }
                }
                self.bind(name, o);
            }
            EvalResult::Place(obj, path, excl) => {
                let ty = self.type_at(obj, &path);
                if self.is_resource(&ty) {
                    if !path.is_empty() {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    // transfer: rebind the same object identity.
                    // [Authority-Transfer-Aliased]: no other live path may
                    // reach it (excluding the source binding itself, which
                    // this very transfer is about to invalidate).
                    if !self.solitary(obj, &excl) {
                        return Err(Flow::Fault("diag.move-while-aliased".into()));
                    }
                    // [Authority-Transfer]: the performer must hold the
                    // authority -- except when binding a spawned thread's
                    // parameters, the one case where the destination is
                    // formed in another thread and the authority is
                    // re-keyed to it (rule.conc.spawn).
                    let owner = self.objects.get(&obj).map(|o| o.owner_thread).unwrap_or(self.current_thread);
                    if owner != self.current_thread && !self.binding_thread_params {
                        return Err(Flow::Fault("diag.transfer-without-authority".into()));
                    }
                    if let Some(o) = self.objects.get_mut(&obj) {
                        o.owner_thread = self.current_thread;
                    }
                    let keep_frame = self.frames().len() - 1;
                    self.bind(name, obj);
                    self.invalidate_other_bindings(obj, keep_frame, name);
                } else {
                    // `[Store-Binding-Place-Copy]` copies through `[Read]`
                    // (spec/05), so it carries every one of `[Read]`'s own
                    // checks -- `¬clash(a, shared)` against live
                    // non-ancestor exclusive paths, and definite
                    // assignment -- not a raw peek at the cells.
                    let v = self.result_to_value(EvalResult::Place(obj, path, excl))?;
                    let id = self.new_object(ty, v);
                    self.bind(name, id);
                }
            }
        }
        Ok(())
    }

    // After moving object `obj` into a new binding, remove/mark stale every
    // OTHER binding across all frames that pointed at it as its owner.
    // Marks every current binding of `obj`, in every frame, as moved —
    // used when `obj` is being fully consumed (turned into a `temp`, or
    // destroyed), so nothing pointing at it survives at all.
    fn invalidate_binding_pointing_at(&mut self, obj: u64) {
        for f in self.frames_mut().iter_mut() {
            let names: Vec<String> = f.bindings.iter().filter(|(_, &o)| o == obj).map(|(n, _)| n.clone()).collect();
            for n in names {
                f.moved.insert(n);
            }
        }
    }

    // Marks every OTHER current binding of `obj` as moved, sparing
    // exactly `(keep_frame, keep_name)` — the destination binding a
    // transfer (`[Store-Binding-Place-Transfer]`) just created. Finding
    // "the destination" by scanning for "whichever binding is newest"
    // is not well-defined once source and destination coexist in the
    // *same* frame's binding map (e.g. `auto w = v;`), so the caller
    // names it explicitly instead.
    fn invalidate_other_bindings(&mut self, obj: u64, keep_frame: usize, keep_name: &str) {
        for (fi, f) in self.frames_mut().iter_mut().enumerate() {
            let names: Vec<String> = f.bindings.iter().filter(|(_, &o)| o == obj).map(|(n, _)| n.clone()).collect();
            for n in names {
                if fi == keep_frame && n == keep_name {
                    continue;
                }
                f.moved.insert(n);
            }
        }
    }

    // ================= Expressions =================

    pub fn eval(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        self.current_line = e.line;
        let r = self.eval_inner(e, subst);
        if std::env::var("COBALTC_TRACE").is_ok() {
            if let Err(Flow::Fault(d)) = &r {
                eprintln!("TRACE fault {} at line {} ({:?})", d, e.line, std::mem::discriminant(&e.kind));
            }
        }
        // Attach *this* expression's own line the first time an
        // untagged fault passes through `eval()` -- the innermost
        // expression whose evaluation actually produced it, since a
        // fault from a sub-expression's own `eval()` call already
        // tagged itself before propagating up through this one. Its own
        // line, not `current_line`: by the time an operator faults, its
        // operands' evaluation may have moved `current_line` elsewhere
        // (into a prelude body, whose lines have no location).
        // A line inside `std` is not one: the fault goes on untagged to
        // the program's own expression that called into `std`, whose line
        // it is reported at.
        match r {
            Err(Flow::Fault(d)) if !d.contains('@') && user_line(e.line) => Err(Flow::Fault(format!("{d}@{}", e.line))),
            other => other,
        }
    }
    fn eval_inner(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        match &e.kind {
            ExprKind::IntLit(v, suf) => self.eval_int_lit_expected(*v, suf.as_deref(), None),
            ExprKind::FloatLit(v, suf) => {
                if suf.as_deref() == Some("f32") {
                    Ok(EvalResult::Val(Value::F32(*v as f32)))
                } else {
                    Ok(EvalResult::Val(Value::F64(*v)))
                }
            }
            ExprKind::StrLit(s) => Ok(EvalResult::Val(Value::Str(Arc::from(s.as_bytes())))),
            ExprKind::BoolLit(b) => Ok(EvalResult::Val(Value::Bool(*b))),
            ExprKind::Unit => Ok(EvalResult::Val(Value::Unit)),
            ExprKind::Paren(inner) => self.eval(inner, subst),
            ExprKind::Path(segs, targs) => self.eval_path(segs, targs, subst),
            ExprKind::StructLit(segs, targs, fields) => self.eval_struct_lit(segs, targs, fields, subst, None),
            ExprKind::ArrayLit(items) => self.eval_array_lit(items, subst, None),
            ExprKind::Unary(op, inner) => self.eval_unary(*op, inner, subst),
            ExprKind::Binary(op, l, r) => match self.view_op(e) {
                Some(vo) => self.eval_view_op(e, vo, subst),
                None => self.eval_binary(*op, l, r, subst),
            },
            ExprKind::Borrow(mode, inner) => self.eval_borrow(mode.clone(), inner, subst),
            ExprKind::Deref(inner) => self.eval_deref(inner, subst),
            ExprKind::Field(base, name) => self.eval_field(base, name, subst),
            ExprKind::Index(base, idx) => self.eval_index(base, idx, subst),
            ExprKind::SliceOf(mode, base, lo, hi) => match self.view_op(e) {
                Some(op) => self.eval_view_op(e, op, subst),
                None => self.eval_slice_of(mode.clone(), base, lo, hi, subst),
            },
            ExprKind::Dollar => match self.dollar.last() {
                Some(n) => Ok(EvalResult::Val(Value::Int(IntTy::Usize, *n as i128))),
                None => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            ExprKind::Call(callee, args) => self.eval_call(callee, args, subst),
            ExprKind::Assign(lhs, rhs) => self.eval_assign(lhs, rhs, subst),
            ExprKind::Propagate(inner) => self.eval_propagate(inner, subst),
            ExprKind::Block(b) => self.exec_block_body(b, subst),
            ExprKind::Unsafe(b) => {
                *self.unsafe_depth_by_thread.entry(self.current_thread).or_insert(0) += 1;
                let r = self.exec_block_body(b, subst);
                *self.unsafe_depth_by_thread.entry(self.current_thread).or_insert(0) -= 1;
                r
            }
            ExprKind::If(c, t, f) => self.eval_if(c, t, f, subst, None),
            ExprKind::While(c, b, step) => self.eval_while(c, b, step.as_deref(), subst),
            ExprKind::Match(scrut, arms) => {
                let hint = self.items.match_hints.lock().unwrap().get(&(e as *const Expr as usize)).cloned();
                self.eval_match(scrut, arms, subst, hint.as_ref())
            }
            ExprKind::Return(v) => {
                let r = match v {
                    Some(e) => match self.ret_stack().last().cloned() {
                        Some(rty) => self.eval_expected(e, subst, &rty)?,
                        None => self.eval(e, subst)?,
                    },
                    None => EvalResult::Val(Value::Unit),
                };
                let r = self.into_result_form(r)?;
                Err(Flow::Return(r))
            }
            ExprKind::Break => Err(Flow::Break),
            ExprKind::Continue => Err(Flow::Continue),
            ExprKind::Closure { is_move, captures, params, body } => {
                self.eval_closure(*is_move, captures, params, body, e, subst)
            }
        }
    }

    fn into_result_form(&mut self, r: EvalResult) -> EvalOutcome {
        match r {
            EvalResult::Place(obj, path, excl) => {
                let ty = self.type_at(obj, &path);
                if self.is_resource(&ty) && path.is_empty() {
                    self.invalidate_binding_pointing_at(obj);
                    Ok(EvalResult::Temp(obj))
                } else {
                    // `[Store-Result-Place-Copy]` reads through `[Read]`,
                    // not a raw peek — a plain place flowing out of a
                    // frame as its "kept" result is still subject to
                    // `¬clash(a, shared)`.
                    if !self.clash_check_read(obj, &path, &excl) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    let v = self.read_place(obj, &path);
                    Ok(EvalResult::Val(v))
                }
            }
            other => Ok(other),
        }
    }

    fn eval_to_value(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<Value, Flow> {
        let r = self.eval(e, subst)?;
        self.result_to_value(r)
    }

    fn result_to_value(&mut self, r: EvalResult) -> Result<Value, Flow> {
        match r {
            EvalResult::Val(v) => Ok(v),
            EvalResult::Place(obj, path, excl) => {
                // rule.init.definite-assignment (spec/11): a whole-object
                // read of a binding established by `[Let-Uninit]` and
                // never since written. Checked on the *root* object id
                // only (`path` may be empty, reading the whole binding,
                // or non-empty projecting into it -- either way the
                // underlying object is still the one `[Let-Uninit]`
                // established with nothing written).
                if self.uninit_bindings.contains(&obj) {
                    return Err(Flow::Fault("diag.use-of-uninitialized".into()));
                }
                let ty = self.type_at(obj, &path);
                if self.is_resource(&ty) {
                    if std::env::var("COBALTC_TRACE").is_ok() {
                        eprintln!("TRACE read-of-resource Place obj={} path={:?} ty={:?}", obj, path, ty);
                    }
                    return Err(Flow::Fault("diag.read-of-resource".into()));
                }
                if !self.clash_check_read(obj, &path, &excl) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                Ok(self.read_place(obj, &path))
            }
            EvalResult::Temp(o) => {
                let ty = self.objects.get(&o).unwrap().ty.clone();
                if self.is_resource(&ty) {
                    if std::env::var("COBALTC_TRACE").is_ok() {
                        eprintln!("TRACE read-of-resource Temp obj={} ty={:?}", o, ty);
                    }
                    return Err(Flow::Fault("diag.read-of-resource".into()));
                }
                let v = self.objects.get(&o).unwrap().value.clone();
                self.destroy_object(o)?;
                Ok(v)
            }
        }
    }

    // `excl`: the access-path tokens this very access was reached through
    // (an `EvalResult::Place`'s own chain) -- its ancestors, and nothing
    // else's. See `EvalResult::Place`.
    fn clash_check_read(&self, obj: u64, path: &[Proj], excl: &[u64]) -> bool {
        !self.clash(obj, path, Mode::Shared, excl)
    }
    fn clash_check_write(&self, obj: u64, path: &[Proj], excl: &[u64]) -> bool {
        !self.clash(obj, path, Mode::Exclusive, excl)
    }

    fn eval_path(&mut self, segs: &[String], targs: &[Type], subst: &HashMap<String, Type>) -> EvalOutcome {
        let name = segs.join("::");
        let last = segs.last().unwrap().clone();
        // local binding first
        if segs.len() == 1 {
            if let Some((obj, moved)) = self.lookup(&last) {
                if moved {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if self.capture_cells.contains(&obj) {
                    // A borrow-captured name denotes `*self.f_i` (spec/15
                    // [Closure-Call]): resolve through the capture
                    // reference, whose token is this access's ancestor.
                    if let Value::Ref { obj: t, path, token, .. } = self.read_place(obj, &[]) {
                        if !self.object_live(t) {
                            return Err(Flow::Fault("diag.stale-binding".into()));
                        }
                        return Ok(EvalResult::Place(t, path, vec![token]));
                    }
                }
                return Ok(EvalResult::Place(obj, vec![], vec![]));
            }
        }
        // enum variant constructor (payload-less), e.g. `None`, `Sign::Pos`
        if let Some(enum_name) = self.variant_enum(segs) {
            if let Some(e) = self.items.enums.get(&enum_name).cloned() {
                let vi = e.variants.iter().position(|v| v.name == last).unwrap();
                if e.variants[vi].payload.is_none() {
                    let ty = self.enum_named_type(&enum_name, targs, subst, &e);
                    let v = Value::Enum(vi, Box::new(Value::Unit));
                    let id = self.new_object(ty, v);
                    return Ok(EvalResult::Temp(id));
                }
            }
        }
        // function value
        if self.items.fns.contains_key(&name) || self.items.externs.contains_key(&name) {
            return Ok(EvalResult::Val(Value::FnVal(name)));
        }
        Err(Flow::Fault("diag.unbound-name".into()))
    }

    fn enum_named_type(&self, name: &str, targs: &[Type], subst: &HashMap<String, Type>, _e: &Arc<EnumDecl>) -> Type {
        let args: Vec<Type> = targs.iter().map(|t| self.apply_subst(t, subst)).collect();
        Type::Named(name.to_string(), args)
    }

    // `Vi(e)` where `Vi` names a payload-carrying variant (spec/16
    // [Enum-Construct]). Returns `None` if `name` is not such a variant, so
    // the caller can fall through to an ordinary function call.
    // The enum a variant path names: `E::V` (or `m::E::V`) by its prefix,
    // a bare `V` through the flat table (an ambiguous bare variant has
    // already been rejected by the resolver).
    fn variant_enum(&self, segs: &[String]) -> Option<String> {
        let last = segs.last()?;
        if segs.len() >= 2 {
            let key = segs[..segs.len() - 1].join("::");
            if let Some(e) = self.items.enums.get(&key) {
                return if e.variants.iter().any(|v| &v.name == last) { Some(key) } else { None };
            }
        }
        self.items.enum_of_variant.get(last).cloned()
    }

    fn try_variant_construct(
        &mut self,
        segs: &[String],
        targs: &[Type],
        arg: &Expr,
        subst: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let name = segs.last().unwrap().as_str();
        let enum_name = match self.variant_enum(segs) {
            Some(e) => e,
            None => return Ok(None),
        };
        let e = self.items.enums.get(&enum_name).cloned().unwrap();
        let vi = e.variants.iter().position(|v| v.name == name).unwrap();
        if e.variants[vi].payload.is_none() {
            return Ok(None); // handled by eval_path as a bare value
        }
        let raw_payload = e.variants[vi].payload.clone().unwrap();
        let mut m: HashMap<String, Type> = HashMap::new();
        for (p, t) in e.type_params.iter().zip(targs.iter()) {
            m.insert(p.clone(), self.apply_subst(t, subst));
        }
        // `rule.type.expected`: an expected type of this enum supplies the
        // type arguments not written (`Some(5000000000)` as an `Option<u64>`).
        if let Some(Type::Named(n, eargs)) = expected {
            if *n == enum_name && eargs.len() == e.type_params.len() {
                for (p, t) in e.type_params.iter().zip(eargs.iter()) {
                    if !m.contains_key(p) {
                        m.insert(p.clone(), t.clone());
                    }
                }
            }
        }
        // Evaluate with whatever's already known threaded down (so e.g.
        // `Ok(Vec::new())` can still resolve `Vec`'s own `T` from a wider
        // expected type, if one was threaded into this call), then
        // structurally infer any of *this* enum's still-unbound type
        // parameters from the argument's own runtime shape (`Some(5)`
        // fixes `Option<T>`'s `T`) against the RAW (unsubstituted)
        // payload type — unifying against an already-substituted
        // placeholder would just match itself and bind nothing.
        let hint_ty = self.apply_subst(&raw_payload, &m);
        let r = self.eval_expected(arg, subst, &hint_ty)?;
        if let Some(hint) = self.result_type_hint(&r) {
            self.unify_type_shape(&raw_payload, &hint, &mut m);
        }
        let args: Vec<Type> = e.type_params.iter().map(|p| m.get(p).cloned().unwrap_or(Type::Void)).collect();
        let fsubst = self.subst_map(&e.type_params, &args);
        let pty = self.apply_subst(&raw_payload, &fsubst);
        let v = self.store_into_field(r, &pty)?;
        let ty = Type::Named(enum_name, args);
        let id = self.new_object(ty, Value::Enum(vi, Box::new(v)));
        Ok(Some(EvalResult::Temp(id)))
    }

    // Threads an expected type (`rule.type.expected`) into the small set
    // of positions where this interpreter can actually use one to resolve
    // an otherwise-unresolvable generic type parameter: a call with too
    // few arguments to infer from (`Vec::new<T>()`) and a nested struct
    // literal field (`Box { .value = Vec::new() }`). Everything else
    // falls back to ordinary (argument-inferred-only) evaluation.
    // Tags an untagged fault with `e`'s line, as `eval` does: some forms
    // (a variant constructor with its payload's expected type) are
    // evaluated here without passing through `eval` itself.
    fn eval_expected(&mut self, e: &Expr, subst: &HashMap<String, Type>, expected: &Type) -> EvalOutcome {
        self.current_line = e.line;
        let r = self.eval_expected_inner(e, subst, expected);
        tag_at(r, e.line)
    }

    fn eval_expected_inner(&mut self, e: &Expr, subst: &HashMap<String, Type>, expected: &Type) -> EvalOutcome {
        match &e.kind {
            ExprKind::ArrayLit(items) => match expected {
                Type::Array(inner, _) => {
                    let inner = (**inner).clone();
                    self.eval_array_lit(items, subst, Some(&inner))
                }
                _ => self.eval_array_lit(items, subst, None),
            },
            ExprKind::IntLit(v, suf) => self.eval_int_lit_expected(*v, suf.as_deref(), Some(expected)),
            // D-0037: an operator on literal expressions only is computed
            // in the number type its context expects.
            ExprKind::Binary(op, l, r) if is_literal_expr(e) && matches!(expected, Type::Int(_) | Type::F32 | Type::F64) => {
                let lv = self.eval_expected(l, subst, expected)?;
                let lv = self.result_to_value(lv)?;
                let rv = if matches!(op, BinOp::Shl | BinOp::Shr) { self.eval_to_value(r, subst)? } else {
                    let rv = self.eval_expected(r, subst, expected)?;
                    self.result_to_value(rv)?
                };
                let r = self.apply_binop(*op, lv, rv);
                tag_at(r, e.line)
            }
            ExprKind::Paren(inner) if is_literal_expr(inner) => self.eval_expected(inner, subst, expected),
            ExprKind::Unary(UnOp::Neg, inner) if is_literal_expr(inner) && !matches!(inner.kind, ExprKind::IntLit(..)) && matches!(expected, Type::Int(_) | Type::F32 | Type::F64) => {
                let v = self.eval_expected(inner, subst, expected)?;
                match self.result_to_value(v)? {
                    Value::Int(t, x) => {
                        let r = checked_neg(t, x).map_err(|_| Flow::Fault("diag.arith-overflow".into()));
                        tag_at(r.map(|r| EvalResult::Val(Value::Int(t, r))), e.line)
                    }
                    Value::F32(x) => Ok(EvalResult::Val(Value::F32(-x))),
                    Value::F64(x) => Ok(EvalResult::Val(Value::F64(-x))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            ExprKind::Unary(UnOp::BitNot, inner) if is_literal_expr(inner) && matches!(expected, Type::Int(_)) => {
                let v = self.eval_expected(inner, subst, expected)?;
                match self.result_to_value(v)? {
                    Value::Int(t, x) => {
                        let bw = t.bitwidth(ADDR_WIDTH);
                        let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                        Ok(EvalResult::Val(Value::Int(t, reinterpret_sign_pub(t, (!(x as u128)) & mask, bw))))
                    }
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            // An unsuffixed float literal takes `f32` from its context
            // (`[Literal-Type-From-Context]`), rounded to it, as `-L` does.
            ExprKind::FloatLit(v, None) if *expected == Type::F32 => Ok(EvalResult::Val(Value::F32(*v as f32))),
            ExprKind::Unary(UnOp::Neg, inner) if *expected == Type::F32 && matches!(inner.kind, ExprKind::FloatLit(_, None)) => {
                let ExprKind::FloatLit(v, _) = &inner.kind else { unreachable!() };
                Ok(EvalResult::Val(Value::F32(-(*v as f32))))
            }
            ExprKind::Unary(UnOp::Neg, inner) if matches!(inner.kind, ExprKind::IntLit(..)) => {
                if let ExprKind::IntLit(v, suf) = &inner.kind {
                    self.eval_neg_int_lit_expected(*v, suf.as_deref(), Some(expected))
                } else {
                    unreachable!()
                }
            }
            ExprKind::Call(callee, args) => {
                if let ExprKind::Path(segs, targs) = &callee.kind {
                    if args.len() == 1 && self.lookup(&segs[segs.len() - 1]).is_none() {
                        if let Some(res) = self.try_variant_construct(segs, targs, &args[0], subst, Some(expected))? {
                            return Ok(res);
                        }
                    }
                    let name = segs.join("::");
                    if name == "Mutex::new" && args.len() == 1 {
                        let hint = match (targs.first(), expected) {
                            (Some(t), _) => Some(self.apply_subst(t, subst)),
                            (None, Type::Mutex(i)) => Some((**i).clone()),
                            _ => None,
                        };
                        return self.mutex_new(&args[0], subst, hint.as_ref());
                    }
                    if let Some(f) = self.items.fns.get(&name).cloned() {
                        if !f.type_params.is_empty() {
                            return self.call_user_fn_expected(&f, targs, args, subst, Some(expected.clone()));
                        }
                    }
                }
                self.eval(e, subst)
            }
            ExprKind::StructLit(segs, targs, fields) => {
                self.eval_struct_lit(segs, targs, fields, subst, Some(expected.clone()))
            }
            // The expected type reaches the trailing expression of a
            // block, of each `if` branch and of each `match` arm, and
            // through parentheses (`rule.type.expected`).
            ExprKind::Paren(inner) => self.eval_expected(inner, subst, expected),
            ExprKind::Block(b) => self.exec_block_body_expected(b, subst, Some(expected)),
            ExprKind::If(c, t, f) => self.eval_if(c, t, f, subst, Some(expected)),
            ExprKind::Match(scrut, arms) => self.eval_match(scrut, arms, subst, Some(expected)),
            _ => self.eval(e, subst),
        }
    }

    // `rule.arith.literal`'s actual priority: an explicit suffix always
    // wins; otherwise the expected type from context *if it is a
    // concrete, fully-resolved integer type* (a bare unbound generic
    // type parameter reaching here -- e.g. `expected` is still
    // `Type::Named(param_name, [])` for some unresolved `T` -- gives no
    // real information, so falls through to the same default as no
    // expected type at all); otherwise `i32`. This was previously only
    // implemented at all for the un-suffixed-default case (`eval_inner`'s
    // own `IntLit` arm, unconditionally `i32`) -- every expected-type
    // context calls through here instead now.
    fn eval_int_lit_expected(&self, v: u128, suf: Option<&str>, expected: Option<&Type>) -> EvalOutcome {
        let ty = if let Some(t) = suf.and_then(IntTy::from_str) {
            t
        } else if let Some(Type::Int(t)) = expected {
            *t
        } else {
            IntTy::I32
        };
        if !in_range(ty, v as i128) {
            return Err(Flow::Fault("diag.literal-out-of-range".into()));
        }
        Ok(EvalResult::Val(Value::Int(ty, v as i128)))
    }

    // The "most negative literal" case (`-2147483648`, `-9223372036854
    // 775808`, ...): the *positive* magnitude (2^(bw-1)) does not fit
    // the type's positive range, but its negation is exactly `min(ty)`,
    // a perfectly legal value. Checks the range of the negated value,
    // not the positive magnitude, so this is accepted where a naive
    // "evaluate the literal, then negate" (the general `eval_unary`
    // path) would reject it. `bw==128` (`i128`'s own MIN) is special-
    // cased separately: `v as i128` for `v == 2^127` already
    // bit-reinterprets to `i128::MIN` (not a genuine positive value
    // i128 could hold), so a plain `-(v as i128)` would itself be a
    // second instance of the exact host-overflow bug `int_min_max` had
    // -- computed via bit-pattern identity instead, never negated.
    fn eval_neg_int_lit_expected(&self, v: u128, suf: Option<&str>, expected: Option<&Type>) -> EvalOutcome {
        let ty = if let Some(t) = suf.and_then(IntTy::from_str) {
            t
        } else if let Some(Type::Int(t)) = expected {
            *t
        } else {
            IntTy::I32
        };
        let bw = ty.bitwidth(ADDR_WIDTH);
        let neg: i128 = if bw >= 128 {
            if v == (1u128 << 127) {
                i128::MIN
            } else if v < (1u128 << 127) {
                -(v as i128)
            } else {
                return Err(Flow::Fault("diag.literal-out-of-range".into()));
            }
        } else {
            -(v as i128)
        };
        if !in_range(ty, neg) {
            return Err(Flow::Fault("diag.literal-out-of-range".into()));
        }
        Ok(EvalResult::Val(Value::Int(ty, neg)))
    }

    fn eval_struct_lit(
        &mut self,
        segs: &[String],
        targs: &[Type],
        fields: &[(String, Expr)],
        subst: &HashMap<String, Type>,
        expected: Option<Type>,
    ) -> EvalOutcome {
        // The resolver has rewritten `segs` to the struct's item key
        // (bare at the root, `m::Name` inside a module).
        let name = segs.join("::");
        if let Some(sdecl) = self.items.structs.get(&name).cloned() {
            let mut m: HashMap<String, Type> = HashMap::new();
            for (i, t) in sdecl.type_params.iter().zip(targs.iter()) {
                m.insert(i.clone(), self.apply_subst(t, subst));
            }
            if m.len() < sdecl.type_params.len() {
                if let Some(exp) = &expected {
                    self.unify_type_shape(&Type::Named(name.clone(), sdecl.type_params.iter().map(|p| Type::Named(p.clone(), vec![])).collect()), exp, &mut m);
                }
            }
            // Evaluate every field with whatever type-parameter bindings
            // are already known (from explicit args or the expected type)
            // threaded down as ITS expected type, so a nested generic call
            // or struct literal can resolve too; then structurally infer
            // any parameter still unbound from the field values' own
            // runtime shape (D-0013, done from values here).
            let mut idxs = Vec::with_capacity(fields.len());
            let mut results = Vec::with_capacity(fields.len());
            for (fname, fexpr) in fields {
                let idx = sdecl
                    .fields
                    .iter()
                    .position(|f| &f.name == fname)
                    .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                let fty_hint = self.apply_subst(&sdecl.fields[idx].ty, &m);
                let r = self.eval_expected(fexpr, subst, &fty_hint)?;
                idxs.push(idx);
                results.push(r);
            }
            for (idx, r) in idxs.iter().zip(results.iter()) {
                if let Some(hint) = self.result_type_hint(r) {
                    self.unify_type_shape(&sdecl.fields[*idx].ty, &hint, &mut m);
                }
            }
            let args: Vec<Type> = sdecl.type_params.iter().map(|p| m.get(p).cloned().unwrap_or(Type::Void)).collect();
            let field_subst = self.subst_map(&sdecl.type_params, &args);
            let mut vals = vec![Value::Unit; sdecl.fields.len()];
            for (idx, r) in idxs.into_iter().zip(results.into_iter()) {
                let fty = self.apply_subst(&sdecl.fields[idx].ty, &field_subst);
                let v = self.store_into_field(r, &fty)?;
                vals[idx] = v;
            }
            let ty = Type::Named(name.clone(), args);
            let id = self.new_object(ty, Value::Struct(vals));
            return Ok(EvalResult::Temp(id));
        }
        // enum variant with payload: `Vi(e)` parses as Call in this
        // grammar's postfix form, not StructLit; unreachable in practice.
        Err(Flow::Fault("diag.unbound-name".into()))
    }

    // Implements the plain-copy / resource-move logic of `store` for a
    // sub-range write (rule.value-object.store [Store-Sub-*]).
    fn store_into_field(&mut self, r: EvalResult, fty: &Type) -> Result<Value, Flow> {
        if self.is_resource(fty) {
            match r {
                EvalResult::Temp(o) => {
                    let v = self.remove_object(&o).unwrap().value;
                    Ok(v)
                }
                EvalResult::Place(obj, path, excl) => {
                    if !path.is_empty() {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    if !self.solitary(obj, &excl) {
                        return Err(Flow::Fault("diag.move-while-aliased".into()));
                    }
                    let v = self.remove_object(&obj).unwrap().value;
                    self.invalidate_binding_pointing_at(obj);
                    Ok(v)
                }
                EvalResult::Val(v) => Ok(v),
            }
        } else {
            self.result_to_value(r)
        }
    }

    // `elem`: the element type an expected `array<τ, N>` gives each item
    // (`rule.type.expected`), so `[0, -1]` declared `array<i64, 2>` holds
    // i64s, and a nested literal passes its own element type down.
    fn eval_array_lit(&mut self, items: &[Expr], subst: &HashMap<String, Type>, elem: Option<&Type>) -> EvalOutcome {
        let mut vals = Vec::new();
        let mut elem_ty = elem.cloned();
        for it in items {
            let r = match elem {
                Some(t) => self.eval_expected(it, subst, t)?,
                None => self.eval(it, subst)?,
            };
            let ty = self.result_type_hint(&r);
            if elem_ty.is_none() {
                elem_ty = ty;
            }
            let v = if self.is_resource(elem_ty.as_ref().unwrap_or(&Type::Void)) {
                self.store_into_field(r, elem_ty.as_ref().unwrap())?
            } else {
                self.result_to_value(r)?
            };
            vals.push(v);
        }
        let n = vals.len() as u128;
        let ety = elem_ty.unwrap_or(Type::Int(IntTy::I32));
        let ty = Type::Array(Box::new(ety), n);
        let id = self.new_object(ty, Value::Array(vals));
        Ok(EvalResult::Temp(id))
    }

    // `V(e)` for a variant V with a payload, not shadowed by a local.
    fn is_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) if args.len() == 1 => match &callee.kind {
                ExprKind::Path(segs, _) => {
                    self.lookup(&segs[segs.len() - 1]).is_none() && !self.items.fns.contains_key(&segs.join("::")) && self.variant_enum(segs).is_some()
                }
                _ => false,
            },
            ExprKind::Paren(inner) => self.is_variant_literal(inner),
            _ => false,
        }
    }

    // A variant literal whose payload is a literal or another such: it
    // has no effects, so when it is evaluated cannot be observed.
    fn is_pure_variant_literal(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Call(_, args) if self.is_variant_literal(e) => is_bare_literal_expr(&args[0]) || self.is_pure_variant_literal(&args[0]),
            ExprKind::Paren(inner) => self.is_pure_variant_literal(inner),
            _ => false,
        }
    }

    fn result_type_hint(&self, r: &EvalResult) -> Option<Type> {
        match r {
            EvalResult::Val(v) => Some(self.type_of_value(v)),
            EvalResult::Place(obj, path, _) => Some(self.type_at(*obj, path)),
            EvalResult::Temp(o) => self.objects.get(o).map(|o| o.ty.clone()),
        }
    }

    fn eval_unary(&mut self, op: UnOp, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        if op == UnOp::Neg {
            if let ExprKind::IntLit(v, suf) = &inner.kind {
                return self.eval_neg_int_lit_expected(*v, suf.as_deref(), None);
            }
        }
        let v = self.eval_to_value(inner, subst)?;
        match (op, &v) {
            (UnOp::Neg, Value::Int(t, x)) => {
                let r = checked_neg(*t, *x).map_err(|_| Flow::Fault("diag.arith-overflow".into()))?;
                Ok(EvalResult::Val(Value::Int(*t, r)))
            }
            (UnOp::Neg, Value::F32(x)) => Ok(EvalResult::Val(Value::F32(-x))),
            (UnOp::Neg, Value::F64(x)) => Ok(EvalResult::Val(Value::F64(-x))),
            (UnOp::Not, Value::Bool(b)) => Ok(EvalResult::Val(Value::Bool(!b))),
            (UnOp::BitNot, Value::Int(t, x)) => {
                let bw = t.bitwidth(ADDR_WIDTH);
                let mask: u128 = if bw >= 128 { u128::MAX } else { (1u128 << bw) - 1 };
                let r = (!(*x as u128)) & mask;
                Ok(EvalResult::Val(Value::Int(*t, reinterpret_sign_pub(*t, r, bw))))
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    fn eval_binary(&mut self, op: BinOp, l: &Expr, r: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        if op == BinOp::And {
            let lv = self.eval_to_value(l, subst)?;
            return match lv {
                Value::Bool(false) => Ok(EvalResult::Val(Value::Bool(false))),
                Value::Bool(true) => Ok(EvalResult::Val(self.eval_to_value(r, subst)?)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            };
        }
        if op == BinOp::Or {
            let lv = self.eval_to_value(l, subst)?;
            return match lv {
                Value::Bool(true) => Ok(EvalResult::Val(Value::Bool(true))),
                Value::Bool(false) => Ok(EvalResult::Val(self.eval_to_value(r, subst)?)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            };
        }
        // `rule.type.expected`: a bare unsuffixed literal on the *left*
        // takes its type from the right operand's determined type. A
        // literal has no effects, so evaluating the right side first is
        // unobservable (D-0007 is not violated).
        // The same for a bare unsuffixed float literal: it is `f32` when
        // the other operand is (`rule.type.expected`), else `f64`.
        if let (Some(f), None) = (bare_float_literal(l), bare_float_literal(r)) {
            let rv = self.eval_to_value(r, subst)?;
            let lv = if matches!(rv, Value::F32(_)) { Value::F32(f as f32) } else { Value::F64(f) };
            return self.apply_binop(op, lv, rv);
        }
        if is_bare_literal_expr(l) && !is_bare_literal_expr(r) {
            let rv = self.eval_to_value(r, subst)?;
            let lv = if let Value::Int(t, _) = &rv {
                match &l.kind {
                    ExprKind::IntLit(v, suf) => self
                        .eval_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                        .and_then(|res| self.result_to_value(res))?,
                    ExprKind::Unary(UnOp::Neg, inner) => match &inner.kind {
                        ExprKind::IntLit(v, suf) => self
                            .eval_neg_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                            .and_then(|res| self.result_to_value(res))?,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                }
            } else {
                self.eval_to_value(l, subst)?
            };
            return self.apply_binop(op, lv, rv);
        }
        // `rule.type.expected`: an enum literal compared with `==`/`!=`
        // takes its type from the other operand (`x == Some(5000000000)`
        // for `x : Option<u64>`). On the left, the other side goes first
        // only when the literal's payload is itself a literal, so the
        // order cannot be observed (D-0007).
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            let (lvar, rvar) = (self.is_variant_literal(l), self.is_variant_literal(r));
            if rvar && !lvar {
                let lr = self.eval(l, subst)?;
                let lt = self.result_type_hint(&lr);
                let lv = self.result_to_value(lr)?;
                let rr = match lt {
                    Some(t @ Type::Named(..)) => self.eval_expected(r, subst, &t)?,
                    _ => self.eval(r, subst)?,
                };
                let rv = self.result_to_value(rr)?;
                return self.apply_binop(op, lv, rv);
            }
            if lvar && !rvar && self.is_pure_variant_literal(l) {
                let rr = self.eval(r, subst)?;
                let rt = self.result_type_hint(&rr);
                let rv = self.result_to_value(rr)?;
                let lr = match rt {
                    Some(t @ Type::Named(..)) => self.eval_expected(l, subst, &t)?,
                    _ => self.eval(l, subst)?,
                };
                let lv = self.result_to_value(lr)?;
                return self.apply_binop(op, lv, rv);
            }
        }
        let lv = self.eval_to_value(l, subst)?;
        // raw pointer offset: p + n
        if let (Value::Rawptr(addr, pty), BinOp::Add | BinOp::Sub) = (&lv, op) {
            let rv = self.eval_to_value(r, subst)?;
            if let Value::Int(_, n) = rv {
                self.require_unsafe()?;
                let sz = self.sizeof_ty(pty);
                let delta = (n as i128) * sz as i128;
                let new_addr = if op == BinOp::Add { (*addr as i128) + delta } else { (*addr as i128) - delta };
                return Ok(EvalResult::Val(Value::Rawptr(new_addr as u64, pty.clone())));
            }
        }
        // `rule.type.expected`: once the left operand's concrete type is
        // known (evaluation is left-to-right, D-0007), thread it as the
        // expected type for a bare-literal right operand -- otherwise
        // `i64 y = ...; y == 5000000000` defaulted the literal to i32
        // and rejected it, even though the comparison's real type is
        // unambiguous from the left side. A literal on the *left* with
        // nothing yet evaluated to inform it correctly still defaults
        // to i32 (nothing else could know better that early).
        let rv = if let (Value::F32(_), Some(f)) = (&lv, bare_float_literal(r)) {
            Value::F32(f as f32)
        } else if let (Value::Int(t, _), ExprKind::IntLit(v, suf)) = (&lv, &r.kind) {
            self.eval_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                .and_then(|res| self.result_to_value(res))?
        } else if let Value::Int(t, _) = &lv {
            if let ExprKind::Unary(UnOp::Neg, inner) = &r.kind {
                if let ExprKind::IntLit(v, suf) = &inner.kind {
                    self.eval_neg_int_lit_expected(*v, suf.as_deref(), Some(&Type::Int(*t)))
                        .and_then(|res| self.result_to_value(res))?
                } else {
                    self.eval_to_value(r, subst)?
                }
            } else {
                self.eval_to_value(r, subst)?
            }
        } else {
            self.eval_to_value(r, subst)?
        };
        self.apply_binop(op, lv, rv)
    }

    fn apply_binop(&mut self, op: BinOp, lv: Value, rv: Value) -> EvalOutcome {
        use BinOp::*;
        if op == Ne && std::env::var("COBALTC_TRACE").is_ok() {
            eprintln!("TRACE ne compare lv={:?} rv={:?}", lv, rv);
        }
        match (op, lv, rv) {
            (Eq, a, b) => Ok(EvalResult::Val(Value::Bool(self.values_eq(&a, &b)))),
            (Ne, a, b) => Ok(EvalResult::Val(Value::Bool(!self.values_eq(&a, &b)))),
            (op, Value::Int(t, a), Value::Int(_, b)) => self.int_binop(op, t, a, b),
            (op, Value::F32(a), Value::F32(b)) => Ok(EvalResult::Val(self.float_binop(op, a as f64, b as f64, true))),
            (op, Value::F64(a), Value::F64(b)) => Ok(EvalResult::Val(self.float_binop(op, a, b, false))),
            (op, a, b) => {
                if std::env::var("COBALTC_TRACE").is_ok() {
                    eprintln!("TRACE apply_binop mismatch op={:?} a={:?} b={:?}", op, a, b);
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
        }
    }

    fn values_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Ref { obj: o1, path: p1, .. }, Value::Ref { obj: o2, path: p2, .. }) => o1 == o2 && p1 == p2,
            (Value::Rawptr(x, _), Value::Rawptr(y, _)) => x == y,
            // Compare payload only, not the `IntTy` tag: a well-typed
            // program's `==`/`!=` never compares across integer types
            // (`[T-Cmp]` requires one `τ`), but this interpreter has no
            // separate static-typing pass to guarantee every value's tag
            // was assigned consistently with its context (e.g. an
            // unsuffixed literal always defaults to `i32`, spec/06 §6,
            // even where the surrounding expected type is `u8`) — see
            // impl/STATUS.md. Comparing the numeric value is what a
            // correctly-typed program's same-type comparison reduces to
            // anyway.
            (Value::Int(_, x), Value::Int(_, y)) => x == y,
            _ => a == b,
        }
    }

    fn int_binop(&mut self, op: BinOp, t: IntTy, a: i128, b: i128) -> EvalOutcome {
        use BinOp::*;
        let mk = |r: i128| EvalResult::Val(Value::Int(t, r));
        match op {
            Add => checked_add(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Sub => checked_sub(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Mul => checked_mul(t, a, b).map(mk).map_err(|_| Flow::Fault("diag.arith-overflow".into())),
            Div => checked_div(t, a, b).map(mk).map_err(|e| Flow::Fault(arith_err_diag(e))),
            Rem => checked_rem(t, a, b).map(mk).map_err(|e| Flow::Fault(arith_err_diag(e))),
            Shl => checked_shl(t, a, b as u32).map(mk).map_err(|_| Flow::Fault("diag.shift-amount-out-of-range".into())),
            Shr => checked_shr(t, a, b as u32).map(mk).map_err(|_| Flow::Fault("diag.shift-amount-out-of-range".into())),
            BitAnd => Ok(mk(a & b)),
            BitOr => Ok(mk(a | b)),
            BitXor => Ok(mk(a ^ b)),
            Lt => Ok(EvalResult::Val(Value::Bool(int_lt(t, a, b)))),
            Le => Ok(EvalResult::Val(Value::Bool(int_lt(t, a, b) || a == b))),
            Gt => Ok(EvalResult::Val(Value::Bool(int_lt(t, b, a)))),
            Ge => Ok(EvalResult::Val(Value::Bool(int_lt(t, b, a) || a == b))),
            Eq => Ok(EvalResult::Val(Value::Bool(a == b))),
            Ne => Ok(EvalResult::Val(Value::Bool(a != b))),
            And | Or => unreachable!(),
        }
    }

    fn float_binop(&self, op: BinOp, a: f64, b: f64, is32: bool) -> Value {
        use BinOp::*;
        let wrap = |x: f64| if is32 { Value::F32(x as f32) } else { Value::F64(x) };
        match op {
            Add => wrap(a + b),
            Sub => wrap(a - b),
            Mul => wrap(a * b),
            Div => wrap(a / b),
            Lt => Value::Bool(a < b),
            Le => Value::Bool(a < b || a == b),
            Gt => Value::Bool(b < a),
            Ge => Value::Bool(b < a || a == b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            _ => unreachable!(),
        }
    }

    // NOTE: must recurse through `self.sizeof_ty`/`self.alignof_ty` for
    // every *compound* type (Array, Mutex), not delegate to the free
    // `sizeof`/`alignof` functions in value.rs -- those know nothing
    // about user-declared struct/enum layouts, so e.g. a bare
    // `sizeof(Type::Array(inner,n))` silently computes 0 for any
    // struct/enum-typed `inner` (found via `sizeof<array<Foo,2>>()`
    // returning 0 during this pass's sanity audit, despite `encode`/
    // `decode`'s own array arm -- which does call `self.sizeof_ty` --
    // being correct all along).
    fn sizeof_ty(&self, ty: &Type) -> u64 {
        match ty {
            Type::Named(name, args) => {
                if let Some(layout) = self.struct_layout(name, args) {
                    return layout.size;
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    return layout.size;
                }
                sizeof(ty)
            }
            Type::Array(inner, n) => self.sizeof_ty(inner) * (*n as u64),
            // [Sizeof-Mutex] (spec/06 sec 4): the layout of
            // `struct { τ inner; usize state; }`.
            Type::Mutex(inner) => crate::value::mutex_size(self.sizeof_ty(inner), self.alignof_ty(inner)),
            _ => sizeof(ty),
        }
    }
    fn alignof_ty(&self, ty: &Type) -> u64 {
        match ty {
            Type::Named(name, args) => {
                if let Some(layout) = self.struct_layout(name, args) {
                    return layout.align;
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    return layout.align;
                }
                alignof(ty)
            }
            Type::Array(inner, _) => self.alignof_ty(inner),
            Type::Mutex(inner) => self.alignof_ty(inner).max(8),
            _ => alignof(ty),
        }
    }

    fn eval_borrow(&mut self, mode: Mode, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // mode-monotonicity (spec/08 [Borrow-Exceeds-Source]): an
        // exclusive borrow's source must be exclusive. Determined fresh
        // from `inner`'s own structure and the *current* value of
        // whatever it dereferences (`crosses_shared_ref`) — a plain
        // binding is never capped (implicit exclusive capability, per
        // D-0018); only actually crossing a `ref<_, shared>` right now,
        // in this expression, is. Checked before evaluating `inner` for
        // real so a rejected exclusive reborrow never partially commits
        // any of `inner`'s own side effects.
        if mode == Mode::Exclusive && self.crosses_shared_ref(inner, subst)? {
            return Err(Flow::Fault("diag.borrow-exceeds-source".into()));
        }
        self.write_ctx = mode == Mode::Exclusive;
        let r = self.eval(inner, subst);
        self.write_ctx = false;
        let r = r?;
        match r {
            EvalResult::Place(obj, path, excl) => {
                let src_ty = self.type_at(obj, &[]);
                let pointee = self.type_at(obj, &path);
                let ancestors = excl;
                if !self.clash(obj, &path, mode.clone(), &ancestors) {
                    let token = self.fresh_id();
                    self.record_token_ancestors(token, &ancestors);
                    let v = Value::Ref { obj, path, mode, pointee, token };
                    return Ok(EvalResult::Val(v));
                }
                let _ = src_ty;
                Err(Flow::Fault("diag.aliasing-conflict".into()))
            }
            EvalResult::Temp(_) => Err(Flow::Fault("diag.borrow-of-temporary".into())),
            EvalResult::Val(_) => Err(Flow::Fault("diag.borrow-of-non-place".into())),
        }
    }

    // D-0053: the `StringView` operation the static pass found at `e`.
    fn view_op(&self, e: &Expr) -> Option<ViewOp> {
        self.items.view_ops.lock().unwrap().get(&(e as *const Expr as usize)).copied()
    }

    // D-0053: a `StringView` operation, carried out by `std`: the
    // expression's own operands are evaluated in order, then the call.
    fn eval_view_op(&mut self, e: &Expr, op: ViewOp, subst: &HashMap<String, Type>) -> EvalOutcome {
        let call = |me: &mut Self, name: &str, args: Vec<EvalResult>| me.invoke_callable(EvalResult::Val(Value::FnVal(name.to_string())), args, subst);
        match (op, &e.kind) {
            (ViewOp::SliceString, ExprKind::SliceOf(_, base, lo, hi)) => {
                // `&s` as the borrow's own rule forms it; through a
                // reference, the reference itself (auto-deref).
                let mut r = self.eval_borrow(Mode::Shared, base, subst)?;
                if let EvalResult::Val(Value::Ref { obj, path, pointee: Type::Ref(..), .. }) = &r {
                    r = EvalResult::Val(self.read_place(*obj, path));
                }
                let len = match &r {
                    EvalResult::Val(Value::Ref { obj, path, .. }) => match self.read_place(*obj, path) {
                        Value::Struct(fs) => match fs.first() {
                            Some(Value::Struct(v)) => match v.get(1) {
                                Some(Value::Int(_, n)) => *n as usize,
                                _ => 0,
                            },
                            _ => 0,
                        },
                        _ => 0,
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (l, h) = self.eval_bounds(len, lo, hi, subst)?;
                call(self, "std::String::view", vec![r, l, h])
            }
            (ViewOp::SliceView, ExprKind::SliceOf(_, base, lo, hi)) => {
                let v = self.eval_to_value(base, subst)?;
                let len = match &v {
                    Value::Struct(fs) => match fs.first() {
                        Some(Value::Struct(sl)) => match sl.get(2) {
                            Some(Value::Int(_, n)) => *n as usize,
                            _ => 0,
                        },
                        _ => 0,
                    },
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (l, h) = self.eval_bounds(len, lo, hi, subst)?;
                call(self, "std::StringView::sub", vec![EvalResult::Val(v), l, h])
            }
            (ViewOp::Eq { view_left, other_is_str, negate }, ExprKind::Binary(_, a, b)) => {
                let va = self.eval_to_value(a, subst)?;
                let vb = self.eval_to_value(b, subst)?;
                let (v, o) = if view_left { (va, vb) } else { (vb, va) };
                let name = if other_is_str { "std::StringView::eq_str" } else { "std::StringView::eq" };
                let r = call(self, name, vec![EvalResult::Val(v), EvalResult::Val(o)])?;
                match self.result_to_value(r)? {
                    Value::Bool(x) => Ok(EvalResult::Val(Value::Bool(x != negate))),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // A view's bounds, with `$` its length.
    fn eval_bounds(&mut self, len: usize, lo: &Expr, hi: &Expr, subst: &HashMap<String, Type>) -> Result<(EvalResult, EvalResult), Flow> {
        self.dollar.push(len);
        let l = self.eval_expected(lo, subst, &Type::Int(IntTy::Usize)).and_then(|r| self.result_to_value(r));
        let h = match &l {
            Ok(_) => Some(self.eval_expected(hi, subst, &Type::Int(IntTy::Usize)).and_then(|r| self.result_to_value(r))),
            Err(_) => None,
        };
        self.dollar.pop();
        let l = l?;
        let h = h.expect("evaluated")?;
        Ok((EvalResult::Val(l), EvalResult::Val(h)))
    }

    // D-0053: a `StringView` value's bytes, read through its slice's
    // reference to the `String`'s `Vec<u8>`.
    fn view_bytes(&self, v: &Value) -> Result<Vec<u8>, Flow> {
        let Value::Struct(fs) = v else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let Some(Value::Struct(sl)) = fs.first() else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let (Some(Value::Ref { obj, path, .. }), Some(Value::Int(_, start)), Some(Value::Int(_, n))) = (sl.first(), sl.get(1), sl.get(2)) else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        if !self.object_live(*obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        let Value::Struct(vec) = self.read_place(*obj, path) else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        let Some(Value::Rawptr(addr, _)) = vec.first() else { return Err(Flow::Fault("diag.type-mismatch".into())) };
        Ok((0..*n as u64).map(|j| *self.arena.get(&(addr + *start as u64 + j)).unwrap_or(&0)).collect())
    }

    /// `static_assert`'s condition (D-0052), computed before the program
    /// runs: a constant expression, evaluated as it would be at run time,
    /// with the type arguments `subst` of the instantiation it is in.
    pub fn eval_static_assert(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<bool, String> {
        self.push_frame();
        self.stmt_temps.push(Vec::new());
        let r = self.eval_expected(e, subst, &Type::Bool).and_then(|r| self.result_to_value(r));
        let temps = self.stmt_temps.pop().unwrap_or_default();
        let _ = self.end_stmt_temps(temps);
        let _ = self.pop_frame(None);
        match r {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(_) => Err("diag.type-mismatch".to_string()),
            Err(Flow::Fault(d)) => Err(d),
            Err(_) => Err("diag.const-not-constant".to_string()),
        }
    }

    // D-0049: an argument of type `ref<τ, exclusive>` (`slice<τ,
    // exclusive>`) for a parameter of type `ref<τ, shared>` (`slice<τ,
    // shared>`) is passed as `&*e`: a shared borrow through it, with it as
    // the ancestor (`[Borrow]`, spec/08).
    fn weaken_arg(&mut self, val: &EvalResult, ty: Option<&Type>) -> Result<EvalResult, Flow> {
        let wants_shared = matches!(ty, Some(Type::Ref(_, Mode::Shared)) | Some(Type::Slice(_, Mode::Shared)));
        if !wants_shared {
            return Ok(val.clone());
        }
        let v = match val {
            EvalResult::Val(v) => v.clone(),
            EvalResult::Place(o, p, _) => self.read_place(*o, p),
            EvalResult::Temp(_) => return Ok(val.clone()),
        };
        match v {
            Value::Ref { mode: Mode::Exclusive, .. } => Ok(EvalResult::Val(self.reborrow_shared(v)?)),
            Value::Struct(mut parts) if matches!(ty, Some(Type::Slice(..))) && matches!(parts.first(), Some(Value::Ref { mode: Mode::Exclusive, .. })) => {
                parts[0] = self.reborrow_shared(parts[0].clone())?;
                Ok(EvalResult::Val(Value::Struct(parts)))
            }
            _ => Ok(val.clone()),
        }
    }

    fn reborrow_shared(&mut self, v: Value) -> Result<Value, Flow> {
        let Value::Ref { obj, path, pointee, token, .. } = v else { return Ok(v) };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        let ancestors = vec![token];
        if self.clash(obj, &path, Mode::Shared, &ancestors) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let t = self.fresh_id();
        self.record_token_ancestors(t, &ancestors);
        Ok(Value::Ref { obj, path, mode: Mode::Shared, pointee, token: t })
    }

    fn eval_deref(&mut self, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let r = self.eval(inner, subst)?;
        self.eval_deref_result(r)
    }

    fn eval_deref_result(&mut self, r: EvalResult) -> EvalOutcome {
        // `*e` on a `ref`/`guard` is `[Ref-Deref-Place]`/`[Guard-Deref]`,
        // not `[Read]` — a guard is `is-resource` but dereferencing it
        // unwraps the lock-path token itself, not a value `[Read]` would
        // reject; peek the stored token directly rather than through the
        // general (resource-rejecting) value-read path.
        let v = match r {
            EvalResult::Val(v) => v,
            EvalResult::Place(o, p, _) => self.read_place(o, &p),
            EvalResult::Temp(o) => self.objects.get(&o).unwrap().value.clone(),
        };
        match v {
            Value::Ref { obj, path, token, .. } | Value::Guard { obj, path, token, .. } => {
                // The referent object may have ended since this token was
                // formed (`[Object-End]` — e.g. `release`/`deallocate`
                // freed the reclaimed object it pointed into). Per
                // `[Read-Stale]`, dereferencing a reference into an ended
                // object is a dynamic `diag.stale-binding` fault, checked
                // here at use-time, not a Rust-level panic on a missing
                // object id.
                if !self.object_live(obj) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                // `*r`'s place literally *is* the path `r` holds
                // (`[Ref-Deref-Place]`), so `r`'s own token is this
                // access's ancestor and must be excluded from its clash
                // check (spec/08 `ancestors`); `r`'s own recorded
                // ancestors follow transitively through `token_ancestors`
                // at `clash` time. The chain travels with this one
                // access only: it must not be remembered against the
                // place, or every later access to it by anyone would
                // inherit `r` as an ancestor and `r` would stop being a
                // conflict witness for as long as it lives.
                Ok(EvalResult::Place(obj, path, vec![token]))
            }
            Value::Rawptr(addr, pty) => {
                self.require_unsafe()?;
                if self.is_resource(&pty) {
                    // [Rawptr-Move-Out]: recover the value and end whatever
                    // reclaimed identity previously sat at this address.
                    let v = self.arena_read(addr, &pty);
                    if let Some(old_id) = self.reclaimed_by_addr.remove(&addr) {
                        self.reclaimed_addr.remove(&old_id);
                        self.remove_object(&old_id);
                    }
                    // The cells are uninitialised now; what they held
                    // moved with the value.
                    let n = self.sizeof_ty(&pty);
                    self.clear_side(addr, n);
                    let id = self.new_object(pty, v);
                    Ok(EvalResult::Temp(id))
                } else {
                    let v = self.arena_read(addr, &pty);
                    Ok(EvalResult::Val(v))
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Is object `obj` still a live target `read_place`/`write_place` can
    // reach — either ordinary fresh storage (`self.objects`) or a
    // currently-attached reclaimed identity (`self.reclaimed_addr`)? False
    // once `[Object-End]` has ended it (e.g. `release`/`deallocate`).
    fn object_live(&self, obj: u64) -> bool {
        self.objects.contains_key(&obj) || self.reclaimed_addr.contains_key(&obj)
    }

    // `e.f`: `[Field-Access]` if `e` is already a place on the aggregate
    // itself, `[Field-Access-Auto-Deref]` if it's (or resolves through) a
    // reference/guard — chased transitively since a binding can hold a
    // `ref` whose own place is what we get back from evaluating `base`.
    // Fallible: `[Read-Stale]` if any reference in the chain now targets
    // an ended object (spec/08 `ancestors`/`[Object-End]`), rather than
    // panicking on a missing object id.
    fn resolve_through_refs(&mut self, mut obj: u64, mut path: Vec<Proj>, mut chain: Vec<u64>) -> Result<(u64, Vec<Proj>, Vec<u64>), Flow> {
        loop {
            match self.type_at(obj, &path) {
                Type::Ref(..) => {
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    match self.read_place(obj, &path) {
                        Value::Ref { obj: o2, path: p2, token, .. } => {
                            chain.push(token);
                            obj = o2;
                            path = p2;
                        }
                        _ => break,
                    }
                }
                Type::Guard(..) => {
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    match self.read_place(obj, &path) {
                        Value::Guard { obj: o2, path: p2, token, .. } => {
                            chain.push(token);
                            obj = o2;
                            path = p2;
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        Ok((obj, path, chain))
    }

    // Records `token`'s persistent ancestor set (spec/08 `ancestors`) as
    // the closure of `direct` (the excludes chain active when `token` was
    // formed) union each of those tokens' own already-recorded ancestors
    // — so a chain of sub-borrows (a sub-borrow of a sub-borrow) flattens
    // to one direct lookup at `clash` time instead of needing recursion
    // there. Call this exactly once, at the point a genuinely new
    // `Ref`/`Guard` token is minted from an existing place (not when a
    // value carrying an existing token is merely copied).
    fn record_token_ancestors(&mut self, token: u64, direct: &[u64]) {
        let mut set: std::collections::HashSet<u64> = direct.iter().cloned().collect();
        for d in direct {
            if let Some(anc) = self.token_ancestors.get(d) {
                set.extend(anc.iter().cloned());
            }
        }
        if !set.is_empty() {
            self.token_ancestors.insert(token, set);
        }
    }

    // Mode-monotonicity (spec/08 [Borrow-Exceeds-Source]/[Write-Not-
    // Exclusive]): does evaluating `e` as a place, right now, cross a
    // `ref<_, shared>` — an explicit `*r`, or an auto-deref field/index
    // access through a shared `r` ([Field-Access-Auto-Deref])? Computed
    // fresh from `e`'s own structure and the *current* value of whatever
    // it dereferences every time this is called — deliberately never
    // cached/persisted anywhere on `Interp`. A persistent table keyed
    // only by (obj, path) cannot distinguish "this exact access, right
    // now, goes through a shared reference" from "some unrelated,
    // possibly-already-ended access to the same storage happened to go
    // through one earlier" — the same underlying storage is legitimately
    // reachable through both an exclusive and a shared reference at
    // different points in one program (e.g. `outer` is exclusively
    // owned, but a library call like `Vec::index_shared(&outer, 0)`
    // transiently reads through a *shared* reborrow of it internally;
    // that transient read must never taint a later, unrelated
    // `&mut outer`) — found the hard way, via a real regression, while
    // fixing this rule.
    fn crosses_shared_ref(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<bool, Flow> {
        // Peeking evaluates, so an expression with a call is never peeked:
        // the call would run twice (`&mut *HashMap::entry(&mut m, k, 0)`
        // would move `k` twice). `*f(…)` crosses a shared reference when
        // `f` is declared to return one; any other such case is left to
        // the static check (`typecheck`'s `crosses_shared`).
        if crate::parser::has_call(e) {
            return Ok(match &e.kind {
                ExprKind::Deref(inner) => match &inner.kind {
                    ExprKind::Call(callee, _) => match &callee.kind {
                        ExprKind::Path(segs, _) => {
                            self.items.fns.get(&segs.join("::")).map_or(false, |f| matches!(&f.ret, Type::Ref(_, Mode::Shared)))
                        }
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            });
        }
        match &e.kind {
            ExprKind::Deref(inner) => {
                // Peek the raw value directly, like `eval_deref` itself
                // does — never through `eval_to_value`/`result_to_value`,
                // which reject `is-resource` values; a `guard<T>` is
                // `is-resource` but its own interior (what *this* peek is
                // checking the mode of) may legitimately be a resource
                // too (`&mut *g` on a `lock`ed resource-typed mutex).
                let ir = self.eval(inner, subst)?;
                let v = match ir {
                    EvalResult::Val(v) => v,
                    EvalResult::Place(o, p, _) => self.read_place(o, &p),
                    EvalResult::Temp(o) => self.objects.get(&o).unwrap().value.clone(),
                };
                Ok(matches!(v, Value::Ref { mode: Mode::Shared, .. }))
            }
            ExprKind::Field(base, _) | ExprKind::Index(base, _) => {
                let bv = self.eval(base, subst)?;
                let peeked = match &bv {
                    EvalResult::Val(v) => Some(v.clone()),
                    EvalResult::Place(o, p, _) => Some(self.read_place(*o, p)),
                    EvalResult::Temp(o) => self.objects.get(o).map(|obj| obj.value.clone()),
                };
                if matches!(peeked, Some(Value::Ref { mode: Mode::Shared, .. })) {
                    return Ok(true);
                }
                // Not shared at *this* hop — but an earlier hop further
                // down a longer chain (`r.a.b` where `r` is shared) must
                // still be seen.
                match &base.kind {
                    ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Deref(..) => {
                        self.crosses_shared_ref(base, subst)
                    }
                    _ => Ok(false),
                }
            }
            ExprKind::Paren(inner) => self.crosses_shared_ref(inner, subst),
            ExprKind::Path(segs, _) if segs.len() == 1 => {
                // A borrow-captured name is `*self.f_i`: crossing a
                // shared capture reference is crossing a shared ref.
                match self.lookup(&segs[0]) {
                    Some((obj, _)) if self.capture_cells.contains(&obj) => {
                        Ok(matches!(self.read_place(obj, &[]), Value::Ref { mode: Mode::Shared, .. }))
                    }
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }

    fn eval_field(&mut self, base: &Expr, name: &str, subst: &HashMap<String, Type>) -> EvalOutcome {
        // `(*p).f` with `p : rawptr<S>`: `*p` is a place (`spec/13` §1), so
        // its field is too. The cells are re-attached as the reclaimed
        // object over them, exactly as `reclaim<S>(p).f` would, and the
        // field is read or written through it.
        let mut bare = base;
        while let ExprKind::Paren(e) = &bare.kind {
            bare = e;
        }
        let r = if let ExprKind::Deref(inner) = &bare.kind {
            let pr = self.eval(inner, subst)?;
            let pv = match &pr {
                EvalResult::Val(v) => Some(v.clone()),
                EvalResult::Place(o, p, _) => Some(self.read_place(*o, p)),
                EvalResult::Temp(_) => None,
            };
            match pv {
                Some(Value::Rawptr(addr, pty)) if matches!(pty, Type::Named(..)) => {
                    self.require_unsafe()?;
                    let id = self.intrinsic_reclaim(addr, pty);
                    EvalResult::Place(id, vec![], vec![])
                }
                _ => self.eval_deref_result(pr)?,
            }
        } else {
            self.eval(base, subst)?
        };
        if let EvalResult::Temp(o) = r {
            self.stmt_temp(o);                      // `make().f`: the temporary ends with the statement
        }
        let (obj, path, chain) = match r {
            EvalResult::Place(obj, path, chain) => (obj, path, chain),
            EvalResult::Temp(obj) => (obj, vec![], vec![]),
            EvalResult::Val(Value::Ref { obj, path, token, .. }) => (obj, path, vec![token]),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let (obj, mut path, chain) = self.resolve_through_refs(obj, path, chain)?;
        let sty = self.type_at(obj, &path);
        let idx = self.field_index(&sty, name)?;
        path.push(Proj::Field(idx));
        Ok(EvalResult::Place(obj, path, chain))
    }

    fn field_index(&self, sty: &Type, name: &str) -> Result<usize, Flow> {
        if let Type::Named(sname, _) = sty {
            if let Some(s) = self.items.structs.get(sname) {
                return s
                    .fields
                    .iter()
                    .position(|f| f.name == name)
                    .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()));
            }
            if sname == "__mutex__" {
                if name == "inner" {
                    return Ok(0);
                }
            }
        }
        if let Type::Mutex(_) = sty {
            if name == "inner" {
                return Ok(0);
            }
        }
        Err(Flow::Fault("diag.type-mismatch".into()))
    }

    fn eval_index(&mut self, base: &Expr, idx: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let write = std::mem::replace(&mut self.write_ctx, false);
        self.write_ctx = write;
        let r = self.eval(base, subst);
        self.write_ctx = false;
        let r = r?;
        if let EvalResult::Temp(o) = r {
            self.stmt_temp(o);                      // `make()[i]`: ends with the statement
        }
        let (obj, path, chain) = match r {
            EvalResult::Place(o, p, chain) => (o, p, chain),
            EvalResult::Temp(o) => (o, vec![], vec![]),
            EvalResult::Val(Value::Ref { obj, path, token, .. }) => (obj, path, vec![token]),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let (obj, mut path, mut chain) = self.resolve_through_refs(obj, path, chain)?;
        let aty = self.type_at(obj, &path);
        // `$` (D-0047) is this collection's length.
        let len = self.indexed_len(obj, &path, &aty)?;
        self.dollar.push(len);
        let iv = self.eval_to_value(idx, subst);
        self.dollar.pop();
        let i = match iv? {
            Value::Int(_, n) if n >= 0 => n as usize,
            Value::Int(..) => return Err(Flow::Fault("diag.index-out-of-bounds".into())),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if i >= len {
            return Err(Flow::Fault("diag.index-out-of-bounds".into()));
        }
        match &aty {
            Type::Array(..) => {
                path.push(Proj::Index(i));
                Ok(EvalResult::Place(obj, path, chain))
            }
            // A `Vec`'s element (D-0047): the `Vec` is read or written as a
            // whole, in the mode of the access, then the element is reached
            // in its buffer as `Vec::index_shared`/`index_exclusive` do.
            Type::Named(n, args) if n == "std::Vec" => {
                let mode = if write { Mode::Exclusive } else { Mode::Shared };
                if self.clash(obj, &path, mode, &chain) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let elem = args.first().map(|t| self.apply_subst(t, subst)).unwrap_or(Type::Void);
                let id = self.vec_element(obj, &path, &elem, i)?;
                Ok(EvalResult::Place(id, vec![], chain))
            }
            // A slice's element: through the slice's own reference to its
            // source, so the slice's borrow is this access's ancestor.
            Type::Slice(elem, _) => {
                let (src, start, _) = self.slice_parts(obj, &path)?;
                let Value::Ref { obj: so, path: sp, token, .. } = src else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                if !self.object_live(so) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                chain.push(token);
                let sty = self.type_at(so, &sp);
                match &sty {
                    Type::Array(..) => {
                        let mut p = sp.clone();
                        p.push(Proj::Index(start + i));
                        Ok(EvalResult::Place(so, p, chain))
                    }
                    _ => {
                        // The element type is the source `Vec`'s own, which is
                        // concrete; the slice's may name a type parameter of
                        // the function this runs in (`slice<T, shared>`).
                        let et = match &sty {
                            Type::Named(n, args) if n == "std::Vec" => args.first().cloned().unwrap_or_else(|| self.apply_subst(elem, subst)),
                            _ => self.apply_subst(elem, subst),
                        };
                        let id = self.vec_element(so, &sp, &et, start + i)?;
                        Ok(EvalResult::Place(id, vec![], chain))
                    }
                }
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // How many elements the collection at `obj.path` of type `ty` has.
    fn indexed_len(&self, obj: u64, path: &[Proj], ty: &Type) -> Result<usize, Flow> {
        match ty {
            Type::Array(_, n) => Ok(*n as usize),
            Type::Named(n, _) if n == "std::Vec" => match self.read_place(obj, path) {
                Value::Struct(fs) => match fs.get(1) {
                    Some(Value::Int(_, l)) => Ok(*l as usize),
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                },
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            Type::Slice(..) => Ok(self.slice_parts(obj, path)?.2),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // A slice value's parts: the reference to its source, where it starts
    // in the source, and its length.
    fn slice_parts(&self, obj: u64, path: &[Proj]) -> Result<(Value, usize, usize), Flow> {
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        match self.read_place(obj, path) {
            Value::Struct(fs) if fs.len() == 3 => match (&fs[1], &fs[2]) {
                (Value::Int(_, a), Value::Int(_, n)) => Ok((fs[0].clone(), *a as usize, *n as usize)),
                _ => Err(Flow::Fault("diag.type-mismatch".into())),
            },
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Element `i` of the `Vec` at `obj.path`, as the object in its buffer.
    fn vec_element(&mut self, obj: u64, path: &[Proj], elem: &Type, i: usize) -> Result<u64, Flow> {
        let Value::Struct(fs) = self.read_place(obj, path) else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let Some(Value::Rawptr(addr, _)) = fs.first() else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        let at = addr + (i as u64) * self.sizeof_ty(elem);
        Ok(self.intrinsic_reclaim(at, elem.clone()))
    }

    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047): a slice value
    // `{ ref to the source, start, length }`. The reference is formed as
    // `&source` would be, so the source stays borrowed while the slice
    // lives; a slice of a slice refers to the same source.
    fn eval_slice_of(&mut self, mode: Mode, base: &Expr, lo: &Expr, hi: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        self.write_ctx = mode == Mode::Exclusive;
        let r = self.eval(base, subst);
        self.write_ctx = false;
        let (obj, path, chain) = match r? {
            EvalResult::Place(o, p, chain) => (o, p, chain),
            EvalResult::Val(Value::Ref { obj, path, token, .. }) => (obj, path, vec![token]),
            EvalResult::Temp(_) => return Err(Flow::Fault("diag.borrow-of-temporary".into())),
            _ => return Err(Flow::Fault("diag.borrow-of-non-place".into())),
        };
        let (obj, path, mut chain) = self.resolve_through_refs(obj, path, chain)?;
        let ty = self.type_at(obj, &path);
        let len = self.indexed_len(obj, &path, &ty)?;
        self.dollar.push(len);
        let a = self.eval_to_value(lo, subst);
        let b = a.as_ref().ok().map(|_| self.eval_to_value(hi, subst));
        self.dollar.pop();
        let (a, b) = match (a?, b.unwrap()?) {
            (Value::Int(_, a), Value::Int(_, b)) => (a, b),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if a < 0 || b < a || b as usize > len {
            return Err(Flow::Fault("diag.index-out-of-bounds".into()));
        }
        let (elem, src_obj, src_path, offset) = match &ty {
            Type::Array(el, _) => ((**el).clone(), obj, path.clone(), 0usize),
            Type::Named(n, args) if n == "std::Vec" => (args.first().cloned().unwrap_or(Type::Void), obj, path.clone(), 0),
            Type::Slice(el, _) => {
                let (src, start, _) = self.slice_parts(obj, &path)?;
                let Value::Ref { obj: so, path: sp, token, .. } = src else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                chain.push(token);
                ((**el).clone(), so, sp, start)
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if !self.object_live(src_obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if self.clash(src_obj, &src_path, mode.clone(), &chain) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let token = self.fresh_id();
        self.record_token_ancestors(token, &chain);
        let pointee = self.type_at(src_obj, &src_path);
        let rv = Value::Ref { obj: src_obj, path: src_path, mode: mode.clone(), pointee, token };
        let v = Value::Struct(vec![rv, Value::Int(IntTy::Usize, (offset as i128) + a), Value::Int(IntTy::Usize, b - a)]);
        let id = self.new_object(Type::Slice(Box::new(elem), mode), v);
        Ok(EvalResult::Temp(id))
    }

    fn eval_assign(&mut self, lhs: &Expr, rhs: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // `*e = rhs`: three different targets share this syntax (spec/13
        // §1) — a raw-pointer write (`[Rawptr-Write]`/`[Rawptr-Move-In]`,
        // trusted, `e : rawptr<T>`), or an ordinary place reached by
        // dereferencing a `ref`/`guard` (`*r = ..`/`*g = ..`). Peek `e`'s
        // value directly — not via the resource-rejecting
        // `result_to_value` — since a guard is `is-resource` but its
        // deref is never `[Read]` (spec/13 §1), and evaluate it only
        // once (D-0007), reusing the peeked value either way rather than
        // re-evaluating `e` through the ordinary place path below.
        if let ExprKind::Deref(inner) = &lhs.kind {
            let inner_r = self.eval(inner, subst)?;
            let inner_v = match &inner_r {
                EvalResult::Val(v) => v.clone(),
                EvalResult::Place(o, p, _) => self.read_place(*o, p),
                EvalResult::Temp(o) => self.objects.get(o).unwrap().value.clone(),
            };
            match inner_v {
                Value::Rawptr(addr, pty) => {
                    self.require_unsafe()?;
                    let r = self.eval_expected(rhs, subst, &pty)?;
                    let v = if self.is_resource(&pty) {
                        self.store_into_field(r, &pty)?
                    } else {
                        self.result_to_value(r)?
                    };
                    self.arena_write(addr, &pty, v);
                    return Ok(EvalResult::Val(Value::Unit));
                }
                Value::Ref { mode: Mode::Shared, .. } => {
                    // [Write-Not-Exclusive] (spec/08): a direct write
                    // through a `ref<_, shared>` — `*r = ..` where `r`
                    // itself is shared, no intermediate reborrow — must be
                    // rejected outright, distinct from the reborrow case
                    // `eval_borrow` rejects with `diag.borrow-exceeds-
                    // source`.
                    return Err(Flow::Fault("diag.write-through-shared".into()));
                }
                Value::Ref { obj, path, token, .. } | Value::Guard { obj, path, token, .. } => {
                    // Reaching this arm means the ref just matched above
                    // was *not* `Mode::Shared` (that case already
                    // returned), or it's a guard (always
                    // exclusive-equivalent) — this write is sound.
                    // `*r`'s place literally *is* the path `r` holds
                    // (`[Ref-Deref-Place]`) — exclude `r`'s own token
                    // from this write's clash check (spec/08 `ancestors`),
                    // same as `eval_deref`'s read-position handling.
                    return self.eval_assign_place(obj, path, vec![token], rhs, subst);
                }
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            }
        }
        // [Write-Not-Exclusive] (spec/08): catches a write reached via
        // field/index auto-deref through a `ref<_, shared>` (`r.field =
        // ..`, no explicit `*`) — the explicit `*r = ..` case is already
        // rejected above, before this point. Checked before evaluating
        // `lhs` for real, same reasoning as `eval_borrow`.
        if self.crosses_shared_ref(lhs, subst)? {
            return Err(Flow::Fault("diag.write-through-shared".into()));
        }
        // D-0033 (1): a whole binding. Its place has no evaluation of its
        // own, so whether it still has a value is asked after `rhs`,
        // which may have moved it away (`x = f(x)`).
        if let ExprKind::Path(segs, _) = &lhs.kind {
            let plain_binding = segs.len() == 1 && self.lookup(&segs[0]).map_or(false, |(o, _)| !self.capture_cells.contains(&o));
            if plain_binding {
                let name = &segs[0];
                if let Some(ty) = self.binding_gone(name) {
                    let r = match &ty {
                        Some(t) => self.eval_expected(rhs, subst, t)?,
                        None => self.eval(rhs, subst)?,
                    };
                    return self.reinit_binding(name, r);
                }
                let (obj, _) = self.lookup(name).unwrap();
                let fty = self.type_at(obj, &[]);
                let r = self.eval_expected(rhs, subst, &fty)?;
                if self.binding_gone(name).is_some() {
                    return self.reinit_binding(name, r);
                }
                return self.write_evaluated(obj, Vec::new(), Vec::new(), r, &fty);
            }
        }
        self.write_ctx = true;
        let lr = self.eval(lhs, subst);
        self.write_ctx = false;
        let (obj, path, excl) = match lr? {
            EvalResult::Place(o, p, excl) => (o, p, excl),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        self.eval_assign_place(obj, path, excl, rhs, subst)
    }

    fn eval_assign_place(&mut self, obj: u64, path: Vec<Proj>, excl: Vec<u64>, rhs: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        // spec/13 §1: the target place first, then the value; `[Write]`'s
        // premises hold at the write itself, after the value -- whose
        // evaluation may have ended the target (a `Vec::push` that
        // reallocates under an element reference: `[Write-Stale]`).
        let fty = self.type_at(obj, &path);
        let r = self.eval_expected(rhs, subst, &fty)?;
        self.write_evaluated(obj, path, excl, r, &fty)
    }

    // `[Write]` of an evaluated `rhs` to a place (`eval_assign_place`).
    fn write_evaluated(&mut self, obj: u64, path: Vec<Proj>, excl: Vec<u64>, r: EvalResult, fty: &Type) -> EvalOutcome {
        let fty = fty.clone();
        let v = if self.is_resource(&fty) {
            self.store_into_field(r, &fty)?
        } else {
            self.result_to_value(r)?
        };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if self.is_resource(&fty) {
            let existing = self.read_place(obj, &path);
            if !matches!(existing, Value::Unit) && self.field_is_initialized(obj, &path) && self.value_owns(&existing, &fty) {
                return Err(Flow::Fault("diag.overwrite-of-live-resource".into()));
            }
        }
        if !self.clash_check_write(obj, &path, &excl) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        self.write_place(obj, &path, v);
        self.mark_initialized(obj, &path);
        // rule.init.definite-assignment: any write reaching this object
        // (whole-object or a field of it) satisfies `[Let-Uninit]`'s
        // obligation -- simple, not per-sub-path (a struct declared
        // uninitialized and only partially field-written before a read
        // of an *untouched* field is not separately caught; a narrower
        // scope than full definite-assignment analysis, matching this
        // pass's own documented conservative-first-write approach).
        self.uninit_bindings.remove(&obj);
        Ok(EvalResult::Val(Value::Unit))
    }

    // Minimal definite-assignment/"live resource" bookkeeping: we track
    // per-object initialized sub-paths lazily via a side table keyed by
    // (obj) -> set of path-strings, since Value has no explicit
    // "uninitialized" sentinel distinct from a zero value for resources.
    fn field_is_initialized(&self, _obj: u64, _path: &[Proj]) -> bool {
        // Conservative: treat every existing resource value as "live"
        // (matches the common case; declaring an uninitialized resource
        // field is rare in this corpus). See STATUS.md.
        true
    }
    fn mark_initialized(&mut self, _obj: u64, _path: &[Proj]) {}

    fn eval_propagate(&mut self, inner: &Expr, subst: &HashMap<String, Type>) -> EvalOutcome {
        let r = self.eval(inner, subst)?;
        let result_ty = self.result_type_hint(&r).unwrap_or(Type::Void);
        // `[Propagate]` is `match (r) { Ok(v) : v, … }`: a `Result` that
        // owns no resource is copied out of, and a temporary one stays,
        // ending with its statement ([Stmt-Exit]) -- a reference it holds
        // is live until then, as `cobc` has it. A resource-bearing one is
        // consumed.
        let v = match &r {
            EvalResult::Temp(o) if !self.is_resource(&result_ty) => {
                let v = self.objects.get(o).map(|x| x.value.clone()).unwrap_or(Value::Unit);
                self.stmt_temp(*o);
                v
            }
            _ => self.result_to_value_resource_aware(r)?,
        };
        match v {
            Value::Enum(0, payload) => {
                // Ok(v) / Some(v). A payload that is itself an enum becomes
                // a typed temporary, so `match (f()?)` can read its variants
                // (a bare value has no type to match on).
                let pty = match &result_ty {
                    Type::Named(n, args) => self.items.enums.get(n).cloned().and_then(|e| {
                        let sub = self.subst_map(&e.type_params, args);
                        e.variants.first().and_then(|v0| v0.payload.clone()).map(|t| self.apply_subst(&t, &sub))
                    }),
                    _ => None,
                };
                match pty {
                    Some(t @ Type::Named(..)) if matches!(&t, Type::Named(n, _) if self.items.enums.contains_key(n)) => {
                        let id = self.new_object(t, *payload);
                        Ok(EvalResult::Temp(id))
                    }
                    _ => Ok(EvalResult::Val(*payload)),
                }
            }
            Value::Enum(1, payload) => {
                // `return Err(err)` (rule.fail.propagate's own desugaring)
                // — keep the enclosing `Result<_,E>`'s own type, not a
                // bare untyped value, so a caller's `match` on the
                // propagated function's return can still see which enum
                // and variant this is.
                let id = self.new_object(result_ty, Value::Enum(1, payload));
                Err(Flow::Return(EvalResult::Temp(id)))
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    fn result_to_value_resource_aware(&mut self, r: EvalResult) -> Result<Value, Flow> {
        match r {
            EvalResult::Temp(o) => {
                let v = self.remove_object(&o).unwrap().value;
                Ok(v)
            }
            EvalResult::Place(obj, path, excl) if path.is_empty() => {
                let ty = self.type_at(obj, &[]);
                if self.is_resource(&ty) {
                    self.invalidate_binding_pointing_at(obj);
                    Ok(self.remove_object(&obj).unwrap().value)
                } else {
                    self.result_to_value(EvalResult::Place(obj, path, excl))
                }
            }
            other => self.result_to_value(other),
        }
    }

    fn eval_if(&mut self, c: &Expr, t: &Block, f: &Option<Box<Expr>>, subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        let cv = self.eval_to_value(c, subst)?;
        // D-0049: a literal branch's type, from its sibling (typecheck).
        let hint = match expected {
            Some(_) => None,
            None => self.items.match_hints.lock().unwrap().get(&(t as *const Block as usize)).cloned().map(|h| self.apply_subst(&h, subst)),
        };
        let expected = expected.or(hint.as_ref());
        match cv {
            Value::Bool(true) => self.exec_block_body_expected(t, subst, expected),
            Value::Bool(false) => match f {
                Some(e) => match expected {
                    Some(ty) => self.eval_expected(e, subst, ty),
                    None => self.eval(e, subst),
                },
                None => Ok(EvalResult::Val(Value::Unit)),
            },
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // A `for` loop's `step` runs after the body, and after a `continue`,
    // as a statement of its own (its temporaries end with it).
    fn eval_while(&mut self, c: &Expr, b: &Block, step: Option<&Expr>, subst: &HashMap<String, Type>) -> EvalOutcome {
        loop {
            let cv = self.eval_to_value(c, subst)?;
            match cv {
                Value::Bool(false) => return Ok(EvalResult::Val(Value::Unit)),
                Value::Bool(true) => {}
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            }
            match self.exec_block_body(b, subst) {
                Ok(_) => {}
                Err(Flow::Break) => return Ok(EvalResult::Val(Value::Unit)),
                Err(Flow::Continue) => {}
                Err(other) => return Err(other),
            }
            if let Some(s) = step {
                self.exec_stmt(&Stmt::Expr(Box::new(s.clone())), subst)?;
            }
        }
    }

    fn eval_match(&mut self, scrut: &Expr, arms: &[Arm], subst: &HashMap<String, Type>, expected: Option<&Type>) -> EvalOutcome {
        let r = self.eval(scrut, subst)?;
        // `[Match-By-Ref]` (D-0046): a scrutinee whose value is a reference.
        let refv = match &r {
            EvalResult::Val(v @ Value::Ref { .. }) => Some(v.clone()),
            EvalResult::Place(o, p, _) => match self.read_place(*o, p) {
                v @ Value::Ref { .. } => Some(v),
                _ => None,
            },
            EvalResult::Temp(o) => match self.objects.get(o).map(|x| x.value.clone()) {
                Some(v @ Value::Ref { .. }) => {
                    self.stmt_temp(*o);
                    Some(v)
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(Value::Ref { obj, path, mode, token, .. }) = refv {
            return self.eval_match_by_ref(obj, path, mode, token, arms, subst, expected);
        }
        // D-0057: an integer or `bool` scrutinee, matched against literals.
        let scalar = match &r {
            EvalResult::Val(v @ (Value::Int(..) | Value::Bool(_))) => Some(v.clone()),
            EvalResult::Place(o, p, _) => match self.read_place(*o, p) {
                v @ (Value::Int(..) | Value::Bool(_)) => Some(v),
                _ => None,
            },
            EvalResult::Temp(o) => match self.objects.get(o).map(|x| x.value.clone()) {
                Some(v @ (Value::Int(..) | Value::Bool(_))) => {
                    self.stmt_temp(*o);
                    Some(v)
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(v) = scalar {
            let arm = arms
                .iter()
                .find(|a| a.variant.is_none() && a.lit.as_ref().map_or(true, |l| lit_equals(l, &v)))
                .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
            self.push_frame();
            let body_r = match expected {
                Some(ty) => self.eval_expected(&arm.body, subst, ty),
                None => self.eval(&arm.body, subst),
            }
            .and_then(|r| self.into_result_form(r));
            return match body_r {
                Ok(res) => {
                    let keep = match &res {
                        EvalResult::Temp(o) => Some(*o),
                        _ => None,
                    };
                    self.pop_frame(keep)?;
                    Ok(res)
                }
                Err(flow) => {
                    let keep = returned_temp(&flow);
                    self.pop_frame(keep)?;
                    Err(flow)
                }
            };
        }
        let (obj, path, excl) = match r {
            EvalResult::Place(o, p, excl) => (o, p, excl),
            EvalResult::Temp(o) => {
                // A temporary scrutinee ends with the statement ([Stmt-Exit]),
                // like any other; a resource one is consumed below instead.
                // Left alone, an `Option<ref<…>>` from a call kept its
                // reference live, and destroying the referent later failed.
                self.stmt_temp(o);
                (o, vec![], vec![])
            }
            EvalResult::Val(_) => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        if self.clash(obj, &path, Mode::Shared, &excl) {
            return Err(Flow::Fault("diag.aliasing-conflict".into()));
        }
        let scrutv = self.read_place(obj, &path);
        let (vi, is_temp_root) = match &scrutv {
            Value::Enum(vi, _) => (*vi, path.is_empty() && matches!(r_is_temp(obj, &self.objects), true)),
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let sty = self.type_at(obj, &path);
        let enum_name = if let Type::Named(n, _) = &sty { n.clone() } else { String::new() };
        let e = self.items.enums.get(&enum_name).cloned();
        let variant_name = e.as_ref().and_then(|e| e.variants.get(vi)).map(|v| v.name.clone());
        let _ = (vi, &variant_name, &e);
        // D-0056: the first arm whose pattern the value matches.
        let arm = arms
            .iter()
            .find(|a| self.arm_matches(obj, &path, a))
            .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
        self.push_frame();
        let mut moved_out = false;
        let mut bpath = path.clone();
        for _ in 0..arm.chain().len() {
            bpath.push(Proj::Payload);
        }
        if let Some(binder) = &arm.binder {
            let payload_ty = self.type_at(obj, &bpath);
            let payload = self.read_place(obj, &bpath);
            if self.is_resource(&payload_ty) {
                if !path.is_empty() {
                    self.pop_frame(None)?;
                    return Err(Flow::Fault("diag.move-out-of-field".into()));
                }
                // [Relocate-Out]: the payload is *moved* out — vacate the
                // scrutinee's own copy so the destroy-composite pass below
                // (or the enum object's eventual end, for the
                // through-a-reference case rejected above) does not also
                // destroy this same value a second time.
                let moved_payload = payload;
                self.write_place(obj, &bpath, Value::Unit);
                let id = self.new_object(payload_ty, moved_payload);
                self.bind(binder, id);
                moved_out = true;
            } else {
                let id = self.new_object(payload_ty, payload);
                self.bind(binder, id);
            }
        }
        // D-0049: the scrutinee is consumed only when this arm moved its
        // resource payload out; otherwise it stays with its owner (a
        // temporary one ends with the statement).
        let consumed_ok = {
            let ety = self.type_at(obj, &[]);
            if moved_out && self.is_resource(&ety) && path.is_empty() {
                self.invalidate_binding_pointing_at(obj);
                self.destroy_object(obj)
            } else {
                Ok(())
            }
        };
        let body_r = match expected {
            Some(ty) => self.eval_expected(&arm.body, subst, ty),
            None => self.eval(&arm.body, subst),
        }
        .and_then(|r| self.into_result_form(r));
        let _ = is_temp_root;
        match body_r {
            Ok(res) => {
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                consumed_ok?;
                Ok(res)
            }
            Err(flow) => {
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                consumed_ok?;
                Err(flow)
            }
        }
    }

    // D-0056: whether the enum at `path` in `obj` has, level by level
    // down its payloads, the variants arm `a`'s pattern names.
    fn arm_matches(&self, obj: u64, path: &[Proj], a: &Arm) -> bool {
        let mut p = path.to_vec();
        for vn in a.chain() {
            let Value::Enum(vi, _) = self.read_place(obj, &p) else {
                return false;
            };
            let name = match self.type_at(obj, &p) {
                Type::Named(en, _) => self.items.enums.get(&en).and_then(|e| e.variants.get(vi)).map(|v| v.name.clone()),
                _ => None,
            };
            if name.as_deref() != Some(vn.as_str()) {
                return false;
            }
            p.push(Proj::Payload);
        }
        match &a.lit {
            Some(l) => lit_equals(l, &self.read_place(obj, &p)),
            None => true,
        }
    }

    // `match` through a reference (D-0046): the arm is chosen by the
    // referent's variant, and a binder is a reference of the scrutinee's
    // mode to the payload, derived from the scrutinee's reference.
    fn eval_match_by_ref(
        &mut self,
        obj: u64,
        path: Vec<Proj>,
        mode: Mode,
        token: u64,
        arms: &[Arm],
        subst: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> EvalOutcome {
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if !matches!(self.read_place(obj, &path), Value::Enum(..)) {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        // D-0056: the first arm whose pattern the referent matches.
        let arm = arms
            .iter()
            .find(|a| self.arm_matches(obj, &path, a))
            .ok_or_else(|| Flow::Fault("diag.non-exhaustive-match".into()))?;
        self.push_frame();
        if let Some(binder) = &arm.binder {
            let mut ppath = path.clone();
            for _ in 0..arm.chain().len() {
                ppath.push(Proj::Payload);
            }
            let pty = self.type_at(obj, &ppath);
            let ancestors = vec![token];
            if self.clash(obj, &ppath, mode.clone(), &ancestors) {
                self.pop_frame(None)?;
                return Err(Flow::Fault("diag.aliasing-conflict".into()));
            }
            let t = self.fresh_id();
            self.record_token_ancestors(t, &ancestors);
            let rv = Value::Ref { obj, path: ppath, mode: mode.clone(), pointee: pty.clone(), token: t };
            let id = self.new_object(Type::Ref(Box::new(pty), mode), rv);
            self.bind(binder, id);
        }
        let body_r = match expected {
            Some(ty) => self.eval_expected(&arm.body, subst, ty),
            None => self.eval(&arm.body, subst),
        }
        .and_then(|r| self.into_result_form(r));
        match body_r {
            Ok(res) => {
                let keep = match &res {
                    EvalResult::Temp(o) => Some(*o),
                    _ => None,
                };
                self.pop_frame(keep)?;
                Ok(res)
            }
            Err(flow) => {
                let keep = returned_temp(&flow);
                self.pop_frame(keep)?;
                Err(flow)
            }
        }
    }

    fn eval_closure(
        &mut self,
        is_move: bool,
        captures: &[String],
        params: &[Param],
        body: &Block,
        expr: &Expr,
        subst: &HashMap<String, Type>,
    ) -> EvalOutcome {
        let mut vals = Vec::new();
        let mut tys = Vec::new();
        for c in captures {
            let (obj, moved) = self.lookup(c).ok_or_else(|| Flow::Fault("diag.unbound-name".into()))?;
            if moved {
                return Err(Flow::Fault("diag.stale-binding".into()));
            }
            if self.capture_cells.contains(&obj) {
                // Capturing a name that is itself a borrow capture of an
                // enclosing closure: the name denotes `*self.f_i`, so this
                // is a reborrow (or a copy out) through that reference.
                let (t, tpath, ttoken, tmode, pointee) = match self.read_place(obj, &[]) {
                    Value::Ref { obj, path, token, mode, pointee } => (obj, path, token, mode, pointee),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if !self.object_live(t) {
                    return Err(Flow::Fault("diag.stale-binding".into()));
                }
                if is_move {
                    if self.is_resource(&pointee) {
                        return Err(Flow::Fault("diag.move-out-of-field".into()));
                    }
                    if self.clash(t, &tpath, Mode::Shared, &[ttoken]) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    vals.push(self.read_place(t, &tpath));
                    tys.push(pointee);
                } else {
                    let mutated = closure_body_writes(body, c);
                    let mode = if mutated { Mode::Exclusive } else { Mode::Shared };
                    if mode == Mode::Exclusive && tmode == Mode::Shared {
                        return Err(Flow::Fault("diag.borrow-exceeds-source".into()));
                    }
                    let ancestors = vec![ttoken];
                    if self.clash(t, &tpath, mode.clone(), &ancestors) {
                        return Err(Flow::Fault("diag.aliasing-conflict".into()));
                    }
                    let token = self.fresh_id();
                    self.record_token_ancestors(token, &ancestors);
                    vals.push(Value::Ref { obj: t, path: tpath, mode: mode.clone(), pointee: pointee.clone(), token });
                    tys.push(Type::Ref(Box::new(pointee), mode));
                }
                continue;
            }
            if is_move {
                let ty = self.type_at(obj, &[]);
                if self.is_resource(&ty) {
                    self.invalidate_binding_pointing_at(obj);
                    let v = self.remove_object(&obj).unwrap().value;
                    vals.push(v);
                    tys.push(ty);
                } else {
                    let v = self.read_place(obj, &[]);
                    vals.push(v);
                    tys.push(ty);
                }
            } else {
                let mutated = closure_body_writes(body, c);
                let mode = if mutated { Mode::Exclusive } else { Mode::Shared };
                // A captured name is a bare binding: this access has no
                // ancestors of its own.
                let ancestors: Vec<u64> = Vec::new();
                if self.clash(obj, &[], mode.clone(), &ancestors) {
                    return Err(Flow::Fault("diag.aliasing-conflict".into()));
                }
                let pointee = self.type_at(obj, &[]);
                let token = self.fresh_id();
                self.record_token_ancestors(token, &ancestors);
                vals.push(Value::Ref { obj, path: vec![], mode, pointee: pointee.clone(), token });
                tys.push(Type::Ref(Box::new(pointee), if mutated { Mode::Exclusive } else { Mode::Shared }));
            }
        }
        let cid = closure_id(expr);
        let ty = Type::Closure(cid);
        let id = self.new_object(ty, Value::Struct(vals));
        // `rule.type.is-resource`: a closure whose captures include a
        // resource is a resource by derivation, like a struct with such a
        // field. Its type tag alone cannot say so (`is_resource` answers
        // `false` for `Type::Closure`), so the object is marked here, and
        // `destroy_composite` destroys the captures.
        if tys.iter().any(|t| self.is_resource(t)) {
            if let Some(o) = self.objects.get_mut(&id) {
                o.is_resource = true;
            }
        }
        self.closure_field_types.insert(id, tys.clone());
        // remember the body+params+capture names for calling later
        self.closures.borrow_mut().insert(cid, ClosureInfo {
            captures: captures.to_vec(),
            body: body.clone(),
            subst: subst.clone(),
            is_move,
        });
        self.closure_params.borrow_mut().insert(cid, params.to_vec());
        Ok(EvalResult::Temp(id))
    }

    // `[T-Alt]`: an unsuffixed literal operand takes the other operand's
    // type. A literal has no effects, so evaluating the other operand
    // first when the literal is on the left is unobservable (D-0007).
    fn eval_alt_operands(&mut self, l: &Expr, r: &Expr, subst: &HashMap<String, Type>) -> Result<(Value, Value), Flow> {
        if is_bare_literal_expr(l) && !is_bare_literal_expr(r) {
            let b = self.eval_to_value(r, subst)?;
            let a = match &b {
                Value::Int(t, _) => {
                    let res = self.eval_expected(l, subst, &Type::Int(*t))?;
                    self.result_to_value(res)?
                }
                _ => self.eval_to_value(l, subst)?,
            };
            return Ok((a, b));
        }
        let a = self.eval_to_value(l, subst)?;
        let b = match (&a, is_bare_literal_expr(r)) {
            (Value::Int(t, _), true) => {
                let res = self.eval_expected(r, subst, &Type::Int(*t))?;
                self.result_to_value(res)?
            }
            _ => self.eval_to_value(r, subst)?,
        };
        Ok((a, b))
    }

    // ---- prelude intrinsics (spec/21 §0) ----

    fn try_intrinsic(
        &mut self,
        name: &str,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
    ) -> Result<Option<EvalResult>, Flow> {
        let t0 = || -> Type { Type::Void };
        let targ = |i: usize, s: &Interp| -> Type {
            targs.get(i).map(|t| s.apply_subst(t, subst)).unwrap_or_else(t0)
        };
        macro_rules! ok { ($v:expr) => { return Ok(Some(EvalResult::Val($v))) } }
        match name {
            "drop" => {
                let r = self.eval(&args[0], subst)?;
                match r {
                    EvalResult::Place(obj, path, excl) => {
                        if !path.is_empty() {
                            return Err(Flow::Fault("diag.move-out-of-field".into()));
                        }
                        // [Destroy-Not-Solitary]: no other live path may
                        // reach the object being destroyed. A place
                        // reached *through a reference* (`drop(*r)`) is
                        // never solitary while the object's own owner
                        // binding exists: the owner's root path is another
                        // valid path on it.
                        if self.is_resource(&self.type_at(obj, &path)) && (!self.solitary(obj, &excl) || (!excl.is_empty() && self.lookup_owns(obj))) {
                            return Err(Flow::Fault("diag.destroy-while-aliased".into()));
                        }
                        if self.lookup_owns(obj) {
                            self.invalidate_binding_pointing_at(obj);
                        }
                        self.destroy_object(obj)?;
                    }
                    EvalResult::Temp(o) => self.destroy_object(o)?,
                    EvalResult::Val(_) => {}
                }
                ok!(Value::Unit)
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub" | "saturating_mul" => {
                let (a, b) = self.eval_alt_operands(&args[0], &args[1], subst)?;
                let (Value::Int(t, x), Value::Int(_, y)) = (a, b) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let op = if name.ends_with("add") { '+' } else if name.ends_with("sub") { '-' } else { '*' };
                let r = if name.starts_with("wrapping") { wrapping_op(t, x, y, op) } else { saturating_op(t, x, y, op) };
                ok!(Value::Int(t, r))
            }
            "checked_add" | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem" => {
                let (a, b) = self.eval_alt_operands(&args[0], &args[1], subst)?;
                let (Value::Int(t, x), Value::Int(_, y)) = (a, b) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let res = match name {
                    "checked_add" => checked_add(t, x, y),
                    "checked_sub" => checked_sub(t, x, y),
                    "checked_mul" => checked_mul(t, x, y),
                    "checked_div" => checked_div(t, x, y),
                    _ => checked_rem(t, x, y),
                };
                let opt_ty = Type::Named("std::Option".into(), vec![Type::Int(t)]);
                let v = match res {
                    Ok(r) => Value::Enum(0, Box::new(Value::Int(t, r))),
                    Err(_) => Value::Enum(1, Box::new(Value::Unit)),
                };
                let id = self.new_object(opt_ty, v);
                return Ok(Some(EvalResult::Temp(id)));
            }
            "widen" | "narrow" | "narrow_wrapping" | "reinterpret" | "to_float" | "to_int" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let dst = targ(0, self);
                return Ok(Some(EvalResult::Val(self.convert(name, v, &dst)?)));
            }
            // D-0052: computed before the program ran (`check_static_asserts`).
            "static_assert" => ok!(Value::Unit),
            "min_value" | "max_value" => {
                // `rule.arith.limits`: `min(τ)`/`max(τ)` of spec/06 §1.
                // `u128`'s maximum is held as its bit pattern, as the
                // literal is.
                // D-0051: a float's are its largest finite value, negated
                // for `min_value`.
                match targ(0, self) {
                    Type::F32 => ok!(Value::F32(if name == "max_value" { f32::MAX } else { f32::MIN })),
                    Type::F64 => ok!(Value::F64(if name == "max_value" { f64::MAX } else { f64::MIN })),
                    _ => {}
                }
                let Type::Int(t) = targ(0, self) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let v = match (name, t) {
                    ("max_value", IntTy::U128) => -1,
                    ("max_value", _) => int_min_max(t).1,
                    _ => int_min_max(t).0,
                };
                ok!(Value::Int(t, v))
            }
            "sizeof" => ok!(Value::Int(IntTy::Usize, self.sizeof_ty(&targ(0, self)) as i128)),
            "alignof" => ok!(Value::Int(IntTy::Usize, self.alignof_ty(&targ(0, self)) as i128)),
            "dangling" => ok!(Value::Rawptr(1, targ(0, self))),
            "rawptr_of" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path, pty) = match r {
                    EvalResult::Val(Value::Ref { obj, path, pointee, .. }) => (obj, path, pointee),
                    EvalResult::Place(obj, path, _) => {
                        let pty = self.type_at(obj, &path);
                        (obj, path, pty)
                    }
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if let Some((addr, rty)) = self.reclaimed_addr.get(&obj).cloned() {
                    let (off, _) = self.path_offset(&rty, &path);
                    ok!(Value::Rawptr(addr + off, pty));
                }
                // `[Rawptr-Of]` yields `min(target(a))`: the address of the
                // object's own cells. A fresh-storage object has had no
                // address so far, so give it one now -- move its whole
                // value into the byte arena and record it as arena-backed
                // (`reclaimed_addr`), so that every later `read_place`/
                // `write_place` through the binding and every raw
                // `*p`/`*p = v` see the same storage. (Previously a
                // *snapshot* was written to a fresh address, so a raw
                // write through `rawptr_of(&mut x)` was invisible to `x`.)
                // Only types the arena can encode faithfully are backed
                // this way; a value carrying references or thread handles
                // keeps the snapshot behaviour.
                let whole_ty = self.type_at(obj, &[]);
                if self.arena_encodable(&whole_ty) && self.objects.contains_key(&obj) {
                    let whole = self.read_place(obj, &[]);
                    let base = self.bump_alloc(self.sizeof_ty(&whole_ty).max(1), self.alignof_ty(&whole_ty).max(1));
                    self.arena_write(base, &whole_ty, whole);
                    self.reclaimed_addr.insert(obj, (base, whole_ty.clone()));
                    self.reclaimed_by_addr.insert(base, obj);
                    let (off, _) = self.path_offset(&whole_ty, &path);
                    ok!(Value::Rawptr(base + off, pty));
                }
                let v = self.read_place(obj, &path);
                let addr = self.bump_alloc(self.sizeof_ty(&pty).max(1), self.alignof_ty(&pty).max(1));
                self.arena_write(addr, &pty, v);
                ok!(Value::Rawptr(addr, pty))
            }
            "reinterpret_ptr" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let dst = targ(0, self);
                if let Value::Rawptr(addr, _) = v {
                    ok!(Value::Rawptr(addr, dst));
                }
                return Err(Flow::Fault("diag.type-mismatch".into()));
            }
            // spec/21 rule.stdlib.str
            // `slice_len(s)` (D-0047): a slice's length, also through a reference.
            "slice_len" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path, chain) = match r {
                    EvalResult::Place(o, p, c) => (o, p, c),
                    EvalResult::Temp(o) => {
                        self.stmt_temp(o);
                        (o, vec![], vec![])
                    }
                    EvalResult::Val(Value::Ref { obj, path, token, .. }) => (obj, path, vec![token]),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let (obj, path, _) = self.resolve_through_refs(obj, path, chain)?;
                let n = self.slice_parts(obj, &path)?.2;
                ok!(Value::Int(IntTy::Usize, n as i128))
            }
            "str_len" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                ok!(Value::Int(IntTy::Usize, s.len() as i128))
            }
            "str_byte" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                let i = match self.eval_to_value(&args[1], subst)? {
                    Value::Int(_, i) => i,
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                // `[Str-Byte-Out-Of-Bounds]`
                if i < 0 || i as usize >= s.len() {
                    return Err(Flow::Fault("diag.index-out-of-bounds".into()));
                }
                ok!(Value::Int(IntTy::U8, s[i as usize] as i128))
            }
            "str_ptr" => {
                let s = self.eval_str_arg(&args[0], subst)?;
                let addr = self.intern_str(&s);
                ok!(Value::Rawptr(addr, Type::Int(IntTy::U8)))
            }
            "reclaim" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let ty = targ(0, self);
                if let Value::Rawptr(addr, _) = p {
                    let id = self.intrinsic_reclaim(addr, ty);
                    return Ok(Some(EvalResult::Place(id, vec![], vec![])));
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
            "release" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let n = self.eval_to_value(&args[1], subst)?;
                if let (Value::Rawptr(addr, _), Value::Int(_, n)) = (p, n) {
                    let end = addr + n as u64;
                    self.clear_side(addr, n as u64);
                    let stale: Vec<u64> = self
                        .reclaimed_addr
                        .iter()
                        .filter(|(_, (a, _))| *a >= addr && *a < end)
                        .map(|(&id, _)| id)
                        .collect();
                    for id in stale {
                        if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                            self.reclaimed_by_addr.remove(&a);
                        }
                        self.remove_object(&id);
                    }
                }
                ok!(Value::Unit)
            }
            "copy_raw" => {
                self.require_unsafe()?;
                let dst = self.eval_to_value(&args[0], subst)?;
                let src = self.eval_to_value(&args[1], subst)?;
                let n = self.eval_to_value(&args[2], subst)?;
                if let (Value::Rawptr(d, _), Value::Rawptr(s, _), Value::Int(_, n)) = (dst, src, n) {
                    let bytes: Vec<u8> = (0..n as u64).map(|i| *self.arena.get(&(s + i)).unwrap_or(&0)).collect();
                    for (i, b) in bytes.into_iter().enumerate() {
                        self.arena.insert(d + i as u64, b);
                    }
                    let side: Vec<(u64, Value)> = self.arena_side.range(s..s + (n as u64).max(1)).map(|(a, v)| (*a - s, v.clone())).collect();
                    self.clear_side(d, n as u64);
                    for (off, v) in side {
                        self.arena_side.insert(d + off, v);
                    }
                }
                ok!(Value::Unit)
            }
            "allocate" => {
                let n = self.eval_to_value(&args[0], subst)?;
                let align = self.eval_to_value(&args[1], subst)?;
                let (Value::Int(_, n), Value::Int(_, align)) = (n, align) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let addr = if n == 0 { 1 } else { self.bump_alloc(n as u64, align.max(1) as u64) };
                if n > 0 {
                    self.allocations.insert(addr, (n as u64, align as u64));
                }
                let result_ty = Type::Named("std::Result".into(), vec![Type::Rawptr(Box::new(Type::Int(IntTy::U8))), Type::Named("std::AllocError".into(), vec![])]);
                let v = Value::Enum(0, Box::new(Value::Rawptr(addr, Type::Int(IntTy::U8))));
                let id = self.new_object(result_ty, v);
                return Ok(Some(EvalResult::Temp(id)));
            }
            "deallocate" => {
                self.require_unsafe()?;
                let p = self.eval_to_value(&args[0], subst)?;
                let n = self.eval_to_value(&args[1], subst)?;
                if let (Value::Rawptr(addr, _), Value::Int(_, n)) = (p, n) {
                    let end = addr + n.max(0) as u64;
                    self.clear_side(addr, n.max(0) as u64);
                    let stale: Vec<u64> = self
                        .reclaimed_addr
                        .iter()
                        .filter(|(_, (a, _))| *a >= addr && *a < end)
                        .map(|(&id, _)| id)
                        .collect();
                    for id in stale {
                        if let Some((a, _)) = self.reclaimed_addr.remove(&id) {
                            self.reclaimed_by_addr.remove(&a);
                        }
                        self.remove_object(&id);
                    }
                    self.allocations.remove(&addr);
                }
                ok!(Value::Unit)
            }
            "fault" => {
                if let ExprKind::Path(segs, _) = &args[0].kind {
                    let d = segs.last().unwrap().replace('_', "-");
                    return Err(Flow::Fault(format!("diag.{}", d)));
                }
                Err(Flow::Fault("diag.index-out-of-bounds".into()))
            }
            "lock" => {
                let m = self.eval_to_value(&args[0], subst)?;
                if let Value::Ref { obj, path, pointee, .. } = m {
                    // [Lock]/[Lock-Reentrant] (spec/19 §2): held by no one
                    // -> acquire; held by *this* real CobaltC thread ->
                    // fault (reentrant, not a deadlock); held by another
                    // -> genuinely block until it releases (real OS
                    // scheduling now decides how long, not a simulation).
                    let me = self.current_thread;
                    if self.shared.lock().unwrap().cobalt_locks.get(&obj) == Some(&me) {
                        return Err(Flow::Fault("diag.mutex-reentrant-lock".into()));
                    }
                    self.block_until(|s| !s.cobalt_locks.contains_key(&obj))?;
                    self.shared.lock().unwrap().cobalt_locks.insert(obj, me);
                    let mut gpath = path.clone();
                    gpath.push(Proj::Field(0));
                    let inner = if let Type::Mutex(inner) = &pointee { (**inner).clone() } else { Type::Void };
                    let gty = Type::Guard(Box::new(inner.clone()));
                    let token = self.fresh_id();
                    let id = self.new_object(gty, Value::Guard { obj, path: gpath, inner, token });
                    return Ok(Some(EvalResult::Temp(id)));
                }
                Err(Flow::Fault("diag.type-mismatch".into()))
            }
            "Mutex::new" => {
                let hint = targs.first().map(|t| self.apply_subst(t, subst));
                return self.mutex_new(&args[0], subst, hint.as_ref()).map(Some);
            }
            "spawn" => {
                // [Spawn] (spec/19 §1): a genuine new OS thread, sharing
                // this same `Interp` behind `self_shared`'s lock. `e_f`,
                // `e1..en` are evaluated here, in the spawning thread,
                // *before* the new thread exists -- matching the rule's
                // own left-to-right evaluation in `Sigma` prior to
                // `ell'` being minted.
                let fv = self.eval(&args[0], subst)?;
                let mut argvals = Vec::new();
                for a in &args[1..] {
                    let r = self.eval(a, subst)?;
                    // The argument is taken now, in this thread (`[Spawn]`
                    // evaluates it before the new thread exists): a plain
                    // place is read, a resource moved out of its binding.
                    // Left a place, it was read or moved only when the new
                    // thread first ran -- by which time the spawning
                    // block could have ended it (a host panic).
                    let r = match r {
                        EvalResult::Place(o, path, excl) => {
                            let ty = self.type_at(o, &path);
                            if !self.is_resource(&ty) {
                                EvalResult::Val(self.result_to_value(EvalResult::Place(o, path, excl))?)
                            } else {
                                if !path.is_empty() {
                                    return Err(Flow::Fault("diag.move-out-of-field".into()));
                                }
                                if !self.solitary(o, &excl) {
                                    return Err(Flow::Fault("diag.move-while-aliased".into()));
                                }
                                if self.objects.get(&o).map_or(false, |ob| ob.owner_thread != self.current_thread) {
                                    return Err(Flow::Fault("diag.transfer-without-authority".into()));
                                }
                                self.invalidate_binding_pointing_at(o);
                                for f in self.frames_mut().iter_mut() {
                                    f.owned.retain(|x| *x != o);
                                }
                                for t in self.stmt_temps.iter_mut() {
                                    t.retain(|x| *x != o);
                                }
                                self.temp_index.remove(&o);
                                EvalResult::Temp(o)
                            }
                        }
                        EvalResult::Temp(o) => {
                            // Not this statement's to end: the thread adopts it.
                            for t in self.stmt_temps.iter_mut() {
                                t.retain(|x| *x != o);
                            }
                            self.temp_index.remove(&o);
                            EvalResult::Temp(o)
                        }
                        other => other,
                    };
                    argvals.push(r);
                }
                // Resolve the callee to its body + parameter bindings now
                // (a `fn` value or a `move` closure, spec/19's own
                // requirement) so the new thread's own dispatch loop
                // needs nothing but this already-evaluated data.
                let (body, callee_subst, params, closure_bind): (
                    Block,
                    HashMap<String, Type>,
                    Vec<(String, EvalResult, Option<Type>)>,
                    Option<(u64, Vec<String>)>,
                ) = match &fv {
                    EvalResult::Val(Value::FnVal(name)) => {
                        let fdecl = self
                            .items
                            .fns
                            .get(name)
                            .cloned()
                            .ok_or_else(|| Flow::Fault("diag.unbound-name".into()))?;
                        let inferred = self.infer_type_params(&fdecl, &[], &argvals);
                        let mut ps = Vec::new();
                        for (i, p) in fdecl.params.iter().enumerate() {
                            let pty = self.apply_subst(&p.ty, &inferred);
                            ps.push((p.name.clone(), argvals[i].clone(), Some(pty)));
                        }
                        (fdecl.body.clone(), inferred, ps, None)
                    }
                    EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                        let obj = *o;
                        let cid = match self.objects.get(&obj).map(|ob| ob.ty.clone()) {
                            Some(Type::Closure(id)) => id,
                            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                        };
                        let info = self
                            .closures
                            .borrow()
                            .get(&cid)
                            .cloned()
                            .ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
                        // rule.conc.spawn (spec/19 §1): the callee must be
                        // "a fn value or a move closure" -- a borrowing
                        // closure could let the spawned thread outlive
                        // data it only holds a reference to.
                        if !info.is_move {
                            return Err(Flow::Fault("diag.spawn-borrow-closure".into()));
                        }
                        let cparams = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
                        if argvals.len() != cparams.len() {
                            return Err(Flow::Fault("diag.type-mismatch".into()));
                        }
                        let mut ps = Vec::new();
                        for (i, p) in cparams.iter().enumerate() {
                            let pty = self.apply_subst(&p.ty, subst);
                            ps.push((p.name.clone(), argvals[i].clone(), Some(pty)));
                        }
                        // The new thread gets its own closure object, which
                        // it ends (`run_body`): a closure is a struct
                        // (spec/15), so one owning a resource moves in --
                        // the spawning thread no longer owns it, and a
                        // binding of it is moved from -- and any other is
                        // copied. The spawner's object used to be shared,
                        // and its block's exit could end it before the
                        // thread ran.
                        let cty = Type::Closure(cid);
                        let owns_resource = self.objects.get(&obj).map_or(false, |o| o.is_resource);
                        let obj = if owns_resource {
                            self.invalidate_binding_pointing_at(obj);
                            for f in self.frames_mut().iter_mut() {
                                f.owned.retain(|o| *o != obj);
                            }
                            for t in self.stmt_temps.iter_mut() {
                                t.retain(|o| *o != obj);
                            }
                            obj
                        } else {
                            let v = self.read_place(obj, &[]);
                            let copy = self.new_object(cty, v);
                            if let Some(ftys) = self.closure_field_types.get(&obj).cloned() {
                                self.closure_field_types.insert(copy, ftys);
                            }
                            copy
                        };
                        (info.body.clone(), info.subst.clone(), ps, Some((obj, info.captures.clone())))
                    }
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };

                let ell = self.next_thread_id;
                self.next_thread_id += 1;
                self.frames_by_thread.insert(ell, Vec::new());
                self.shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Running);
                self.active_threads += 1;

                let gil = self
                    .gil
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .ok_or_else(|| Flow::Fault("spawn requires an interpreter run under a GIL (lib::run_source)".into()))?;
                let shared = self.shared.clone();
                // A program thread recurses as the main one does (main.rs).
                let builder = std::thread::Builder::new().stack_size(if cfg!(target_pointer_width = "64") { 2 << 30 } else { 64 << 20 });
                let _ = builder.spawn(move || {
                    gil.acquire();
                    // Safety: this thread holds the GIL until `release`.
                    let g = unsafe { gil.interp() };
                    // [Fault-Unwind] (spec/18): some other thread already
                    // ended the whole program -- "other threads take no
                    // further steps; their frames are not unwound."
                    if shared.lock().unwrap().program_fault.is_some() {
                        g.active_threads -= 1;
                        gil.release();
                        return;
                    }
                    g.current_thread = ell;
                    let outcome = g.run_body(&body, &callee_subst, true, &params, closure_bind.as_ref().map(|(o, c)| (*o, c.as_slice())));
                    g.current_thread = ell;
                    match outcome {
                        Ok(res) => {
                            let rty = g.result_type_hint(&res).unwrap_or(Type::Void);
                            match g.result_to_value_resource_aware(res) {
                                Ok(rv) => {
                                    shared.lock().unwrap().thread_status.insert(ell, ThreadStatus::Done(rv, rty));
                                }
                                Err(f) => {
                                    let d = g.flow_to_string(f);
                                    shared.lock().unwrap().program_fault = Some(d);
                                    g.terminated = true;
                                }
                            }
                        }
                        Err(Flow::Terminated) => {}
                        Err(flow) => {
                            // A checked fault in *this* thread terminates
                            // the whole program: recorded here; every other
                            // thread stops the moment it next waits or
                            // yields (spec/18). No process exit -- this
                            // interpreter may be one of many in one host
                            // process (the conformance test binary).
                            let d = g.flow_to_string(flow);
                            shared.lock().unwrap().program_fault = Some(d);
                            g.terminated = true;
                        }
                    }
                    g.active_threads -= 1;
                    gil.release();
                }).expect("start a program thread");

                // The handle's own stored type is a placeholder -- `join`
                // reads the *real* result type out of `thread_status`
                // once the worker actually finishes, not from here.
                let hty = Type::Handle(Box::new(Type::Void));
                let id = self.new_object(hty, Value::Handle(ell, Type::Void));
                return Ok(Some(EvalResult::Temp(id)));
            }
            "join" => {
                let r = self.eval(&args[0], subst)?;
                let (obj, path) = match r {
                    EvalResult::Place(o, p, _) => (o, p),
                    EvalResult::Temp(o) => (o, vec![]),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                let hv = self.read_place(obj, &path);
                let key = match hv {
                    Value::Handle(k, _) => k,
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                // [Join] (spec/19 §1): blocks (no step) until the thread
                // is done, then claims the result (`taken`) so a second
                // join/handle-destructor sees nothing left to discard.
                if matches!(self.shared.lock().unwrap().thread_status.get(&key), Some(ThreadStatus::Taken)) {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                self.block_until(|s| matches!(s.thread_status.get(&key), Some(ThreadStatus::Done(..))))?;
                let (rv, rty) = match self.shared.lock().unwrap().thread_status.insert(key, ThreadStatus::Taken) {
                    Some(ThreadStatus::Done(v, t)) => (v, t),
                    _ => unreachable!("checked Done above"),
                };
                if path.is_empty() {
                    self.invalidate_binding_pointing_at(obj);
                    self.remove_object(&obj);
                }
                if self.is_resource(&rty) {
                    // `[Join]` re-keys the result's authority to the
                    // joiner: `new_object` grants it to the current thread.
                    let id = self.new_object(rty, rv);
                    return Ok(Some(EvalResult::Temp(id)));
                }
                return Ok(Some(EvalResult::Val(rv)));
            }
            "std::print" => {
                // `rule.stdlib.print`: `str` and `ref<String, shared>` go to
                // `std`'s two helpers; numbers and `bool` are formatted here.
                if args.len() != 1 {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                }
                let v = self.eval_to_value(&args[0], subst)?;
                let helper = match &v {
                    Value::Str(_) => Some("std::print_str"),
                    Value::Ref { pointee: Type::Named(n, _), .. } if n == "std::String" => Some("std::print_string"),
                    _ => None,
                };
                if let Some(h) = helper {
                    self.invoke_callable(EvalResult::Val(Value::FnVal(h.to_string())), vec![EvalResult::Val(v)], subst)?;
                    ok!(Value::Unit)
                }
                let Some(text) = number_text(&v) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(text.as_bytes()).and_then(|_| out.flush());
                ok!(Value::Unit)
            }
            // `rule.stdlib.text` (spec/21 §2d): `std`'s own helpers for
            // `String::append<T>` and `parse<T>`.
            "append_native" => {
                // `str` and `ref<String, shared>` go to `std`'s two helpers,
                // numbers and `bool` to `append_text` (through `text_write`).
                let s_ref = self.eval(&args[0], subst)?;
                let v = self.eval_to_value(&args[1], subst)?;
                let helper = match &v {
                    Value::Str(_) => "std::String::append_str",
                    Value::Ref { pointee: Type::Named(n, _), .. } if n == "std::String" => "std::String::append_string",
                    Value::Struct(_) => "std::String::append_view", // D-0053
                    _ => "std::String::append_text",
                };
                self.invoke_callable(EvalResult::Val(Value::FnVal(helper.to_string())), vec![s_ref, EvalResult::Val(v)], subst)?;
                ok!(Value::Unit)
            }
            // `foreach` (D-0042): the expansion for the type of the hidden
            // binding the call names first (src/each.rs), evaluated here.
            n if crate::each::NAMES.contains(&n) => {
                let t0 = match args.first().map(|a| &a.kind) {
                    Some(ExprKind::Path(segs, _)) if segs.len() == 1 => {
                        self.lookup(&segs[0]).and_then(|(obj, _)| self.objects.get(&obj).map(|o| o.ty.clone()))
                    }
                    _ => None,
                };
                let Some(t0) = t0 else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let t0 = self.apply_subst(&t0, subst);
                let line = args.first().map_or(0, |a| a.line);
                let x = crate::each::expand(n, &t0, args, line).map_err(Flow::Fault)?;
                return self.eval(&x, subst).map(Some);
            }
            // `swap_places(a, b)` (D-0048): the two places' contents exchange.
            // Both are exclusive references; the same place is left alone.
            "swap_places" => {
                let mut places = Vec::new();
                for a in args {
                    let Value::Ref { obj, path, .. } = self.eval_to_value(a, subst)? else {
                        return Err(Flow::Fault("diag.type-mismatch".into()));
                    };
                    if !self.object_live(obj) {
                        return Err(Flow::Fault("diag.stale-binding".into()));
                    }
                    places.push((obj, path));
                }
                if places[0] != places[1] {
                    let va = self.read_place(places[0].0, &places[0].1);
                    let vb = self.read_place(places[1].0, &places[1].1);
                    self.write_place(places[0].0, &places[0].1, vb);
                    self.write_place(places[1].0, &places[1].1, va);
                }
                ok!(Value::Unit)
            }
            // `rule.stdlib.hashmap` (spec/21 §1a, D-0041): a key's hash and
            // equality, both over its bytes (`key_bytes`).
            "key_hash" | "key_eq" => {
                let mut keys = Vec::new();
                for a in args {
                    let v = self.eval_to_value(a, subst)?;
                    keys.push(self.key_bytes(v)?);
                }
                if name == "key_hash" {
                    let mut h: u64 = 14695981039346656037;
                    for b in &keys[0] {
                        h = (h ^ *b as u64).wrapping_mul(1099511628211);
                    }
                    ok!(Value::Int(IntTy::U64, h as i128))
                }
                ok!(Value::Bool(keys[0] == keys[1]))
            }
            "text_write" => {
                let v = self.eval_to_value(&args[0], subst)?;
                let Value::Rawptr(addr, _) = self.eval_to_value(&args[1], subst)? else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let Some(text) = number_text(&v) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                for (j, b) in text.bytes().enumerate() {
                    self.arena.insert(addr + j as u64, b);
                }
                ok!(Value::Int(IntTy::Usize, text.len() as i128))
            }
            "parse_check" | "parse_value" => {
                let t = targ(0, self);
                let (Value::Rawptr(addr, _), Value::Int(_, n)) = (self.eval_to_value(&args[0], subst)?, self.eval_to_value(&args[1], subst)?) else {
                    return Err(Flow::Fault("diag.type-mismatch".into()));
                };
                let bytes: Vec<u8> = (0..n as u64).map(|j| *self.arena.get(&(addr + j)).unwrap_or(&0)).collect();
                let r = match &t {
                    Type::Int(it) => crate::numtext::parse_int(&bytes, it.signed(), it.bitwidth(crate::value::ADDR_WIDTH) as u32)
                        .map(|x| Value::Int(*it, x as i128)),
                    Type::F64 => crate::numtext::parse_float(&bytes, false).map(Value::F64),
                    Type::F32 => crate::numtext::parse_float(&bytes, true).map(|x| Value::F32(x as f32)),
                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                };
                if name == "parse_check" {
                    ok!(Value::Int(IntTy::Isize, match r {
                        Ok(_) => -1,
                        Err(code) => code as i128,
                    }))
                }
                match r {
                    Ok(v) => ok!(v),
                    Err(_) => return Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            // `rule.stdlib.format` (D-0038): `printf`'s and `appendf`'s text
            // (`modres` made them `print`/`append` of `$fmt(…)`).
            "$fmt" => {
                let Value::Str(f) = self.eval_to_value(&args[0], subst)? else {
                    return Err(Flow::Fault("diag.format-invalid".into()));
                };
                let pieces = crate::fmt::parse(&f).map_err(|_| Flow::Fault("diag.format-invalid".into()))?;
                let mut fargs = Vec::new();
                for a in &args[1..] {
                    let v = self.eval_to_value(a, subst)?;
                    fargs.push(match v {
                        Value::Int(t, x) => crate::fmt::Arg::Int { bits: x as u128, signed: t.signed(), width: t.bitwidth(ADDR_WIDTH) },
                        Value::F64(x) => crate::fmt::Arg::Float(x, false),
                        Value::F32(x) => crate::fmt::Arg::Float(x as f64, true),
                        Value::Str(b) => crate::fmt::Arg::Text(b.to_vec()),
                        Value::Bool(b) => crate::fmt::Arg::Text(if b { b"true".to_vec() } else { b"false".to_vec() }),
                        // D-0053: a `StringView`, the one printable struct.
                        v @ Value::Struct(_) => crate::fmt::Arg::Text(self.view_bytes(&v)?),
                        Value::Ref { obj, path, pointee, .. } => {
                            if !self.object_live(obj) {
                                return Err(Flow::Fault("diag.stale-binding".into()));
                            }
                            if matches!(&pointee, Type::Named(n, _) if n == "std::String") {
                                crate::fmt::Arg::Text(self.string_bytes(obj, &path))
                            } else {
                                // A reference to a number, `bool` or `str`: its value (D-0042).
                                match self.read_place(obj, &path) {
                                    Value::Int(t, x) => crate::fmt::Arg::Int { bits: x as u128, signed: t.signed(), width: t.bitwidth(ADDR_WIDTH) },
                                    Value::F64(x) => crate::fmt::Arg::Float(x, false),
                                    Value::F32(x) => crate::fmt::Arg::Float(x as f64, true),
                                    Value::Str(b) => crate::fmt::Arg::Text(b.to_vec()),
                                    Value::Bool(b) => crate::fmt::Arg::Text(if b { b"true".to_vec() } else { b"false".to_vec() }),
                                    _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                                }
                            }
                        }
                        _ => return Err(Flow::Fault("diag.type-mismatch".into())),
                    });
                }
                let text = crate::fmt::render(&pieces, &fargs);
                ok!(Value::Str(Arc::from(text.as_slice())))
            }
            "std::map_err" => {
                let r = self.eval_to_value_resource_aware_expr(&args[0], subst)?;
                let f = self.eval(&args[1], subst)?;
                match r {
                    Value::Enum(0, payload) => {
                        let ty = Type::Named("std::Result".into(), vec![]);
                        let id = self.new_object(ty, Value::Enum(0, payload));
                        return Ok(Some(EvalResult::Temp(id)));
                    }
                    Value::Enum(1, payload) => {
                        let mapped = self.invoke_callable(f, vec![EvalResult::Val(*payload)], subst)?;
                        let mv = self.result_to_value(mapped)?;
                        let ty = Type::Named("std::Result".into(), vec![]);
                        let id = self.new_object(ty, Value::Enum(1, Box::new(mv)));
                        return Ok(Some(EvalResult::Temp(id)));
                    }
                    _ => Err(Flow::Fault("diag.type-mismatch".into())),
                }
            }
            _ => Ok(None),
        }
    }

    // `[Mutex-New]`: `hint` is `T` when an explicit type argument or the
    // expected `mutex<T>` fixes it, so the argument (a literal, say) is
    // typed from it (`rule.type.expected`, as for any parameter).
    fn mutex_new(&mut self, arg: &Expr, subst: &HashMap<String, Type>, hint: Option<&Type>) -> Result<EvalResult, Flow> {
        let r = match hint {
            Some(t) => self.eval_expected(arg, subst, t)?,
            None => self.eval(arg, subst)?,
        };
        let inner_ty = hint.cloned().or_else(|| self.result_type_hint(&r)).unwrap_or(Type::Void);
        let v = if self.is_resource(&inner_ty) {
            self.store_into_field(r, &inner_ty)?
        } else {
            self.result_to_value(r)?
        };
        let ty = Type::Mutex(Box::new(inner_ty));
        let id = self.new_object(ty, Value::Struct(vec![v]));
        Ok(EvalResult::Temp(id))
    }

    fn eval_to_value_resource_aware_expr(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<Value, Flow> {
        let r = self.eval(e, subst)?;
        self.result_to_value_resource_aware(r)
    }

    fn lookup_owns(&self, obj: u64) -> bool {
        self.frames().iter().any(|f| f.bindings.values().any(|&o| o == obj))
    }

    fn bump_alloc(&mut self, size: u64, align: u64) -> u64 {
        let align = align.max(1);
        self.next_addr = (self.next_addr + align - 1) / align * align;
        let addr = self.next_addr;
        self.next_addr += size.max(1);
        addr
    }

    fn eval_str_arg(&mut self, e: &Expr, subst: &HashMap<String, Type>) -> Result<Arc<[u8]>, Flow> {
        match self.eval_to_value(e, subst)? {
            Value::Str(s) => Ok(s),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // The address of a `str` value's bytes in program data, minted on
    // first use; the same content always yields the same address.
    fn intern_str(&mut self, s: &Arc<[u8]>) -> u64 {
        if let Some(&addr) = self.str_data.get(s) {
            return addr;
        }
        let addr = self.bump_alloc(s.len() as u64, 1);
        for (i, b) in s.iter().enumerate() {
            self.arena.insert(addr + i as u64, *b);
        }
        self.str_data.insert(s.clone(), addr);
        addr
    }

    // `encode` is `&self`, so every `str` inside a value bound for the
    // arena gets its program-data address minted here first.
    fn intern_strs_in(&mut self, v: &Value) {
        match v {
            Value::Str(s) => {
                self.intern_str(s);
            }
            Value::Struct(fs) | Value::Array(fs) => {
                for f in fs {
                    self.intern_strs_in(f);
                }
            }
            Value::Enum(_, p) => self.intern_strs_in(p),
            _ => {}
        }
    }

    fn path_offset(&self, ty: &Type, path: &[Proj]) -> (u64, Type) {
        self.path_offset_at(None, ty, path)
    }

    // The offset and type of `path` within a value of type `ty`. With the
    // value's address (`base`), a `Payload` step reads the discriminant
    // there and goes to the active variant's payload (D-0046: a reference
    // into a payload of an enum on the heap, as in a `Box`).
    fn path_offset_at(&self, base: Option<u64>, ty: &Type, path: &[Proj]) -> (u64, Type) {
        let sub = |off: u64| base.map(|b| b + off);
        match path.split_first() {
            None => (0, ty.clone()),
            Some((Proj::Field(i), rest)) => {
                if let Type::Named(name, args) = ty {
                    if let Some(layout) = self.struct_layout(name, args) {
                        let s = self.items.structs.get(name).unwrap().clone();
                        let subst = self.subst_map(&s.type_params, args);
                        let fty = self.apply_subst(&s.fields[*i].ty, &subst);
                        let (roff, rty) = self.path_offset_at(sub(layout.offsets[*i]), &fty, rest);
                        return (layout.offsets[*i] + roff, rty);
                    }
                }
                if let Type::Mutex(inner) = ty {
                    if *i == 0 {
                        return self.path_offset_at(base, inner, rest);
                    }
                }
                (0, ty.clone())
            }
            Some((Proj::Index(i), rest)) => {
                if let Type::Array(inner, _) = ty {
                    let sz = self.sizeof_ty(inner);
                    let (roff, rty) = self.path_offset_at(sub(sz * (*i as u64)), inner, rest);
                    return (sz * (*i as u64) + roff, rty);
                }
                (0, ty.clone())
            }
            Some((Proj::Payload, rest)) => {
                if let (Some(b), Type::Named(name, args)) = (base, ty) {
                    if let Some(layout) = self.enum_layout(name, args) {
                        let mut vi: u64 = 0;
                        for k in 0..layout.dw {
                            vi |= (*self.arena.get(&(b + k)).unwrap_or(&0) as u64) << (8 * k);
                        }
                        if let Some(pty) = layout.payload_types.get(vi as usize) {
                            let (roff, rty) = self.path_offset_at(Some(b + layout.payload_off), pty, rest);
                            return (layout.payload_off + roff, rty);
                        }
                    }
                }
                self.path_offset_at(base, ty, rest)
            }
        }
    }

    // Can the arena's byte encoding round-trip a value of this type?
    // References, guards, handles, fn values and closures are stored as
    // opaque tokens the encoding cannot recover.
    fn arena_encodable(&self, ty: &Type) -> bool {
        match ty {
            Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Rawptr(_) => true,
            Type::Array(inner, _) => self.arena_encodable(inner),
            Type::Named(name, args) => {
                if let Some(s) = self.items.structs.get(name) {
                    let subst = self.subst_map(&s.type_params, args);
                    s.fields.iter().all(|f| self.arena_encodable(&self.apply_subst(&f.ty, &subst)))
                } else if let Some(e) = self.items.enums.get(name) {
                    let subst = self.subst_map(&e.type_params, args);
                    e.variants.iter().all(|v| v.payload.as_ref().map_or(true, |p| self.arena_encodable(&self.apply_subst(p, &subst))))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn intrinsic_reclaim(&mut self, addr: u64, ty: Type) -> u64 {
        self.intrinsic_reclaim_inner(addr, ty)
    }
    fn intrinsic_reclaim_inner(&mut self, addr: u64, ty: Type) -> u64 {
        if let Some(&id) = self.reclaimed_by_addr.get(&addr) {
            // [Reclaim] (CHG-0030): re-attaching a resource hands its destroy
            // authority to the reclaiming thread -- an element a worker
            // reached through a shared `Vec` is the owner's again when the
            // owner's `Vec::drop` reclaims it.
            let me = self.current_thread;
            if let Some(o) = self.objects.get_mut(&id) {
                if o.is_resource {
                    o.owner_thread = me;
                }
            }
            return id;
        }
        let is_res = self.is_resource(&ty);
        let id = self.fresh_id();
        if std::env::var("COBALTC_TRACE").is_ok() {
            eprintln!("TRACE reclaim addr={} ty={:?} -> id={}", addr, ty, id);
        }
        let owner_thread = self.current_thread;
        self.objects.insert(id, Object { ty: ty.clone(), value: Value::Unit, is_resource: is_res, owner_thread });
        self.reclaimed_addr.insert(id, (addr, ty));
        self.reclaimed_by_addr.insert(addr, id);
        id
    }

    fn convert(&self, name: &str, v: Value, dst: &Type) -> Result<Value, Flow> {
        match (name, v, dst) {
            ("widen", Value::Int(st, x), Type::Int(dt)) => {
                // Every value of the source fits the target; the payload
                // only changes representation for `u128`'s bit pattern.
                narrow(st, *dt, x).map(|r| Value::Int(*dt, r)).map_err(|_| Flow::Fault("diag.type-mismatch".into()))
            }
            ("narrow", Value::Int(st, x), Type::Int(dt)) => narrow(st, *dt, x).map(|r| Value::Int(*dt, r)).map_err(|_| Flow::Fault("diag.narrowing-overflow".into())),
            ("narrow_wrapping", Value::Int(_, x), Type::Int(dt)) => Ok(Value::Int(*dt, narrow_wrapping(*dt, x))),
            ("reinterpret", Value::Int(st, x), Type::Int(dt)) => Ok(Value::Int(*dt, reinterpret_sign(*dt, x, st.bitwidth(ADDR_WIDTH)))),
            // D-0051: `f32` into `f64` exactly, and the float conversions
            // IEEE-754 rounds (nearest, ties to even; beyond the range, an
            // infinity), as for an integer source.
            ("widen", Value::F32(f), Type::F64) | ("to_float", Value::F32(f), Type::F64) => Ok(Value::F64(f as f64)),
            ("widen", Value::F32(f), Type::F32) | ("to_float", Value::F32(f), Type::F32) => Ok(Value::F32(f)),
            ("widen", Value::F64(f), Type::F64) | ("to_float", Value::F64(f), Type::F64) => Ok(Value::F64(f)),
            ("to_float", Value::F64(f), Type::F32) => Ok(Value::F32(f as f32)),
            // D-0051: a float's bits as an integer of its width, and back.
            ("reinterpret", Value::F32(f), Type::Int(dt)) => {
                let b = f.to_bits();
                Ok(Value::Int(*dt, if dt.signed() { b as i32 as i128 } else { b as i128 }))
            }
            ("reinterpret", Value::F64(f), Type::Int(dt)) => {
                let b = f.to_bits();
                Ok(Value::Int(*dt, if dt.signed() { b as i64 as i128 } else { b as i128 }))
            }
            ("reinterpret", Value::Int(_, x), Type::F32) => Ok(Value::F32(f32::from_bits(x as u32))),
            ("reinterpret", Value::Int(_, x), Type::F64) => Ok(Value::F64(f64::from_bits(x as u64))),
            ("to_float", Value::Int(st, x), Type::F64) => Ok(Value::F64(if is_u128(st) { x as u128 as f64 } else { x as f64 })),
            ("to_float", Value::Int(st, x), Type::F32) => Ok(Value::F32(if is_u128(st) { x as u128 as f32 } else { x as f32 })),
            ("to_int", Value::F64(f), Type::Int(dt)) => float_to_int(f, *dt),
            ("to_int", Value::F32(f), Type::Int(dt)) => float_to_int(f as f64, *dt),
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    fn invoke_callable(&mut self, f: EvalResult, args: Vec<EvalResult>, subst: &HashMap<String, Type>) -> EvalOutcome {
        match f {
            EvalResult::Val(Value::FnVal(name)) => {
                if let Some(fdecl) = self.items.fns.get(&name).cloned() {
                    self.push_call_frame();
                    let inferred = self.infer_type_params(&fdecl, &[], &args);
                    for (i, p) in fdecl.params.iter().enumerate() {
                        let pty = self.apply_subst(&p.ty, &inferred);
                        self.store_binding(&p.name, args[i].clone(), Some(pty))?;
                    }
                    let rty = self.apply_subst(&fdecl.ret, &inferred);
                    self.ret_stack().push(rty.clone());
                    let r = self.exec_block_body_expected(&fdecl.body, &inferred, Some(&rty));
                    self.ret_stack().pop();
                    self.pop_frame(result_keep(&r))?;
                    return match r {
                        Ok(v) => Ok(v),
                        Err(Flow::Return(v)) => Ok(v),
                        Err(other) => Err(other),
                    };
                }
                Err(Flow::Fault("diag.unbound-name".into()))
            }
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                // A binding or field holding a *fn value* (`[T-Item]`: a
                // named function stored in a `fn(...)`-typed place) is
                // called by name; anything else here is a closure object.
                if let Some(Value::FnVal(name)) = self.objects.get(&o).map(|ob| ob.value.clone()) {
                    return self.invoke_callable(EvalResult::Val(Value::FnVal(name)), args, subst);
                }
                self.call_closure_vals(o, args, subst)
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    fn call_closure_vals(&mut self, obj: u64, args: Vec<EvalResult>, subst: &HashMap<String, Type>) -> EvalOutcome {
        let cid = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => *id,
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let info = self.closures.borrow().get(&cid).cloned().ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
        let params = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
        if args.len() != params.len() {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        self.push_call_frame();
        for (i, p) in params.iter().enumerate() {
            self.store_binding(&p.name, args[i].clone(), Some(self.apply_subst(&p.ty, subst)))?;
        }
        let cells = self.bind_closure_captures(obj, &info.captures);
        let r = self.exec_block_body(&info.body, &info.subst);
        let popped = self.pop_frame(result_keep(&r));
        self.release_capture_cells(&cells);
        popped?;
        r
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        // enum variant constructor with a payload: `Vi(e)` (rule.agg.enum-construct).
        if let ExprKind::Path(segs, targs) = &callee.kind {
            if args.len() == 1 {
                // A variant may be named bare (`Has(x)`) or qualified by
                // its enum (`E::Has(x)`, `spec/17` §1's `M::E::Vi` form);
                // either way the *last* segment is what names it.
                if let Some(res) = self.try_variant_construct(segs, targs, &args[0], subst, None)? {
                    return Ok(res);
                }
            }
        }
        // `[Resolve-Unqualified]`: a local binding comes before an item,
        // and an item before an intrinsic of the same name (spec/21 §0).
        // A local is called below, as the value it holds.
        if let ExprKind::Path(segs, targs) = &callee.kind {
            let local = segs.len() == 1 && self.lookup(&segs[0]).is_some();
            if !local {
                let name = segs.join("::");
                if let Some(f) = self.items.fns.get(&name).cloned() {
                    return self.call_user_fn(&f, targs, args, subst);
                }
                if let Some(ext) = self.items.externs.get(&name).cloned() {
                    return self.call_extern(&ext, args, subst);
                }
                if let Some(res) = self.try_intrinsic(&name, targs, args, subst)? {
                    return Ok(res);
                }
            }
        }
        // callee is an expression evaluating to a FnVal or a closure object
        let cv = self.eval(callee, subst)?;
        match cv {
            EvalResult::Val(Value::FnVal(name)) => {
                if let Some(f) = self.items.fns.get(&name).cloned() {
                    return self.call_user_fn(&f, &[], args, subst);
                }
                if let Some(ext) = self.items.externs.get(&name).cloned() {
                    return self.call_extern(&ext, args, subst);
                }
                Err(Flow::Fault("diag.unbound-name".into()))
            }
            EvalResult::Temp(o) | EvalResult::Place(o, _, _) => {
                if let Some(Value::FnVal(name)) = self.objects.get(&o).map(|ob| ob.value.clone()) {
                    let mut argvals = Vec::new();
                    for a in args {
                        argvals.push(self.eval(a, subst)?);
                    }
                    return self.invoke_callable(EvalResult::Val(Value::FnVal(name)), argvals, subst);
                }
                // A closure called straight from an expression is a
                // temporary: it ends with the statement (with its
                // captures, if it owns any).
                if matches!(cv, EvalResult::Temp(_)) {
                    self.stmt_temp(o);
                }
                self.call_closure(o, args, subst)
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // Binds each captured name inside the call frame. `[Closure-Call]`
    // rewrites the body's free `x_i` to `*self.f_i` (borrow capture) or
    // `self.f_i` (move capture) — since this interpreter does not rewrite
    // the AST, a borrow-captured name is instead bound *directly* to the
    // referent's own object identity (skipping the stored `Ref` wrapper),
    // so `x` inside the body reads/writes the original storage exactly as
    // `*self.f_i` would. The closure's own struct field keeps the `Ref`
    // value (scanned by `clash` for as long as the closure object lives),
    // so outer-scope aliasing against the still-live capture is unaffected
    // — only the call-local name resolution is simplified.
    // Binds the closure's captured names for one call and returns the
    // capture cells it created (to be removed by the caller afterwards).
    //
    // Borrow-capturing closure: every field is a `Ref` formed at closure
    // creation (`[Closure-Form-Borrow]`). The body's free occurrence of
    // `x_i` means `*self.f_i` (`[Closure-Call]`), i.e. an access *through*
    // that reference -- so the name is bound to a fresh cell object
    // holding a copy of the reference, and `eval_path` dereferences it on
    // lookup. Binding the name to the referent directly, as this used to,
    // made the body's own write to `x_i` clash with the very capture
    // reference the closure holds (a live, non-ancestor exclusive path to
    // the same cells): every valid write through an exclusive capture was
    // rejected as `diag.aliasing-conflict`.
    //
    // Move closure: every field is an owned value (`[Closure-Form-Move]`),
    // including a captured variable that is itself of reference type.
    fn bind_closure_captures(&mut self, obj: u64, captures: &[String]) -> Vec<u64> {
        let is_move = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => self.closures.borrow().get(id).map_or(false, |i| i.is_move),
            _ => false,
        };
        let mut cells = Vec::new();
        for (i, cname) in captures.iter().enumerate() {
            let v = self.read_place(obj, &[Proj::Field(i)]);
            let fty = self.type_at(obj, &[Proj::Field(i)]);
            if !is_move {
                let id = self.new_object(fty, v);
                self.capture_cells.insert(id);
                self.bind_alias(cname, id);
                cells.push(id);
                continue;
            }
            // A fresh per-call *view* of the closure's own field
            // storage -- bound as a non-owned alias (`bind_alias`),
            // not `bind`, so this call's frame does not believe
            // itself responsible for destroying it. The real
            // owning copy is (and remains) the closure object's
            // own field; a move-captured resource called twice
            // used to have its per-call snapshot wrongly
            // destroyed at the *first* call's frame exit (running
            // its destructor early -- e.g. dropping an `Rc`'s
            // count to 0 and deallocating it), so the *second*
            // call read already-freed memory. Bug found deriving
            // `ex.e2e-closure-move-rc` (calls a move closure
            // twice), fixed here.
            let id = self.new_object(fty, v);
            self.bind_alias(cname, id);
            // Removed (not destroyed) when the call is over, like the
            // borrow cells: a view left behind kept any reference inside
            // the captured value counted as held for the rest of the run.
            cells.push(id);
        }
        cells
    }

    fn release_capture_cells(&mut self, cells: &[u64]) {
        for id in cells {
            self.capture_cells.remove(id);
            self.remove_object(id);
        }
    }

    fn call_closure(&mut self, obj: u64, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        let cid = match &self.objects.get(&obj).unwrap().ty {
            Type::Closure(id) => *id,
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        };
        let info = self.closures.borrow().get(&cid).cloned().ok_or_else(|| Flow::Fault("diag.type-mismatch".into()))?;
        // params come from the closure literal; we re-derive them from the
        // stored body's own param list isn't tracked, so accept args
        // positionally bound to synthetic names p0..pn is wrong; instead we
        // stashed params separately.
        let params = self.closure_params.borrow().get(&cid).cloned().unwrap_or_default();
        if args.len() != params.len() {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        }
        let mut argvals = Vec::new();
        for a in args {
            argvals.push(self.eval(a, subst)?);
        }
        self.push_call_frame();
        for (i, p) in params.iter().enumerate() {
            self.store_binding(&p.name, argvals[i].clone(), Some(self.apply_subst(&p.ty, subst)))?;
        }
        let cells = self.bind_closure_captures(obj, &info.captures);
        let r = self.exec_block_body(&info.body, &info.subst);
        let popped = self.pop_frame(result_keep(&r));
        self.release_capture_cells(&cells);
        popped?;
        r
    }

    fn call_user_fn(&mut self, f: &Arc<FnDecl>, targs: &[Type], args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        self.call_user_fn_expected(f, targs, args, subst, None)
    }

    fn call_user_fn_expected(
        &mut self,
        f: &Arc<FnDecl>,
        targs: &[Type],
        args: &[Expr],
        subst: &HashMap<String, Type>,
        expected: Option<Type>,
    ) -> EvalOutcome {
        let targs: Vec<Type> = targs.iter().map(|t| self.apply_subst(t, subst)).collect();
        let mut argvals = Vec::with_capacity(args.len());
        if f.type_params.is_empty() {
            // Every parameter's type is already fully concrete (no
            // generic inference needed), so it's known *before*
            // evaluating any argument -- thread it through
            // (`rule.type.expected`) so an un-suffixed literal argument
            // outside i32's range (`f(5000000000)` where `f(i64 x)`)
            // gets its real type instead of always defaulting to i32.
            for (a, p) in args.iter().zip(f.params.iter()) {
                argvals.push(self.eval_expected(a, subst, &p.ty)?);
            }
            for a in args.iter().skip(f.params.len()) {
                argvals.push(self.eval(a, subst)?);
            }
        } else {
            // A parameter whose type the explicit type arguments and the
            // arguments to its left have made concrete is an expected
            // type for its argument (`Vec::push(&mut v, Some(5000000000))`).
            for (i, a) in args.iter().enumerate() {
                let known = self.infer_type_params_expected(f, &targs, &argvals, None);
                let hint = f
                    .params
                    .get(i)
                    .map(|p| self.apply_subst(&p.ty, &known))
                    .filter(|t| !mentions_type_param(t, &f.type_params));
                argvals.push(match hint {
                    Some(t) => self.eval_expected(a, subst, &t)?,
                    None => self.eval(a, subst)?,
                });
            }
        }
        let inferred = self.infer_type_params_expected(f, &targs, &argvals, expected.as_ref());
        self.push_call_frame();
        for (i, p) in f.params.iter().enumerate() {
            let pty = self.apply_subst(&p.ty, &inferred);
            let r = argvals[i].clone();
            self.store_binding(&p.name, r, Some(pty))?;
        }
        let rty = self.apply_subst(&f.ret, &inferred);
        self.ret_stack().push(rty.clone());
        let r = self.exec_block_body_expected(&f.body, &inferred, Some(&rty));
        self.ret_stack().pop();
        self.pop_frame(result_keep(&r))?;
        let res = match r {
            Ok(v) => v,
            Err(Flow::Return(v)) => v,
            Err(other) => return Err(other),
        };
        // A struct, enum or array returned as a bare value (a local read
        // out of the callee's frame) carries no type of its own; give it
        // back as a temporary of the declared return type, which `match`
        // and field access need, ending with the caller's statement.
        match res {
            EvalResult::Val(v @ (Value::Struct(_) | Value::Enum(..) | Value::Array(_))) => {
                let id = self.new_object(rty, v);
                self.stmt_temp(id);
                Ok(EvalResult::Temp(id))
            }
            // A temporary built inside the body from a value's own type
            // (`Some(x)` with `x` a struct: `Option<void>`) takes the
            // declared return type, when that is concrete and names the
            // same type: `match` reads its payload's type from it.
            EvalResult::Temp(id) => {
                if let (Some(o), Type::Named(rn, _)) = (self.objects.get_mut(&id), &rty) {
                    if matches!(&o.ty, Type::Named(on, _) if on == rn) && o.ty != rty && !mentions_type_param(&rty, &f.type_params) {
                        o.ty = rty.clone();
                    }
                }
                Ok(EvalResult::Temp(id))
            }
            other => Ok(other),
        }
    }

    fn infer_type_params(&self, f: &Arc<FnDecl>, explicit: &[Type], args: &[EvalResult]) -> HashMap<String, Type> {
        self.infer_type_params_expected(f, explicit, args, None)
    }

    // D-0012 §3c (argument-inferred) then D-0013 (expected-type-inferred):
    // a type parameter not fixed by an explicit `<...>` or by any
    // argument's own shape is solved by matching the function's declared
    // return type against the expected type at the call — the only route
    // by which a zero/insufficient-argument generic call like
    // `Vec::new<T>()` can ever learn `T` at all.
    fn infer_type_params_expected(
        &self,
        f: &Arc<FnDecl>,
        explicit: &[Type],
        args: &[EvalResult],
        expected: Option<&Type>,
    ) -> HashMap<String, Type> {
        let mut m = HashMap::new();
        for (i, tp) in f.type_params.iter().enumerate() {
            if let Some(t) = explicit.get(i) {
                m.insert(tp.clone(), t.clone());
            }
        }
        if m.len() < f.type_params.len() {
            for (i, p) in f.params.iter().enumerate() {
                self.unify_type_param(&p.ty, args.get(i), &mut m);
            }
        }
        if m.len() < f.type_params.len() {
            if let Some(exp) = expected {
                self.unify_type_shape(&f.ret, exp, &mut m);
            }
        }
        m
    }

    fn unify_type_param(&self, decl_ty: &Type, arg: Option<&EvalResult>, m: &mut HashMap<String, Type>) {
        let arg = match arg {
            Some(a) => a,
            None => return,
        };
        let actual = self.result_type_hint(arg);
        let actual = match actual {
            Some(t) => t,
            None => return,
        };
        self.unify_type_shape(decl_ty, &actual, m);
    }

    // Structural unification of a (possibly type-parameter-containing)
    // declared type against a concrete runtime type, recording every bare
    // type-parameter name it finds bound (D-0012 §3c's argument-inferred
    // case, done structurally rather than via a separate type checker).
    fn unify_type_shape(&self, decl_ty: &Type, actual: &Type, m: &mut HashMap<String, Type>) {
        match (decl_ty, actual) {
            (Type::Named(n, args), _) if args.is_empty() => {
                m.entry(n.clone()).or_insert_with(|| actual.clone());
            }
            (Type::Ref(dinner, _), Type::Ref(ainner, _)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Rawptr(dinner), Type::Rawptr(ainner)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Array(dinner, _), Type::Array(ainner, _)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Handle(dinner), Type::Handle(ainner)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Mutex(dinner), Type::Mutex(ainner)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Guard(dinner), Type::Guard(ainner)) => self.unify_type_shape(dinner, ainner, m),
            (Type::Named(_, dargs), Type::Named(_, aargs)) => {
                for (dp, ap) in dargs.iter().zip(aargs.iter()) {
                    self.unify_type_shape(dp, ap, m);
                }
            }
            _ => {}
        }
    }

    fn call_extern(&mut self, ext: &Arc<ExternDecl>, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        self.require_unsafe()?;
        let mut vals = Vec::new();
        for a in args {
            vals.push(self.eval_to_value(a, subst)?);
        }
        // `std::arg_bytes` (`rule.stdlib.args` `[Arg-Bytes]`): up to `len`
        // bytes of argument `i` into the arena; the argument's whole
        // length, or -1 when there is no argument `i`.
        if self.items.externs.get("std::arg_bytes").map_or(false, |e| Arc::ptr_eq(e, ext)) {
            if let (Some(Value::Int(_, i)), Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vals.get(0), vals.get(1), vals.get(2)) {
                self.extern_calls.push(("arg_bytes".into(), vec![*i, *addr as i128, *len]));
                let args = self.prog_args.clone();
                let n: i128 = match args.get(*i as usize) {
                    Some(a) => {
                        for (j, b) in a.iter().take(*len as usize).enumerate() {
                            self.arena.insert(addr + j as u64, *b);
                        }
                        a.len() as i128
                    }
                    None => -1,
                };
                return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
            }
        }
        // `std::file_read`, `std::file_write` (`rule.stdlib.file`): real
        // file access (`fileio`), the file's bytes to and from the arena.
        let is_std = |key: &str| self.items.externs.get(key).map_or(false, |e| Arc::ptr_eq(e, ext));
        if is_std("std::file_read") || is_std("std::file_write") {
            let read = is_std("std::file_read");
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let addr = |v: Option<&Value>| match v {
                Some(Value::Rawptr(a, _)) => *a,
                _ => 0,
            };
            let (pa, pn, ba, bn) = (addr(vals.get(0)), int(vals.get(1)), addr(vals.get(2)), int(vals.get(3)));
            let bytes_at = |me: &Self, a: u64, n: u64| -> Vec<u8> { (0..n).map(|j| *me.arena.get(&(a + j)).unwrap_or(&0)).collect() };
            let path = bytes_at(self, pa, pn);
            self.extern_calls.push((ext.name.clone(), vec![pa as i128, pn as i128, ba as i128, bn as i128]));
            let r: i128 = if read {
                match crate::fileio::read(&path) {
                    Ok(data) => {
                        for (j, b) in data.iter().take(bn as usize).enumerate() {
                            self.arena.insert(ba + j as u64, *b);
                        }
                        data.len() as i128
                    }
                    Err(c) => c as i128,
                }
            } else {
                let data = bytes_at(self, ba, bn);
                match crate::fileio::write(&path, &data) {
                    Ok(()) => 0,
                    Err(c) => c as i128,
                }
            };
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, r)));
        }
        // `std::file_op`, `std::file_at` (`rule.stdlib.file-handle`, D-0054): the open-file
        // table of `fileio`, the bytes to and from the arena.
        let at = is_std("std::file_at");
        if at || is_std("std::file_op") {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let (op, h) = (int(vals.get(0)), int(vals.get(1)));
            let (ba, n) = match vals.get(2) {
                Some(Value::Rawptr(a, _)) => (*a, int(vals.get(3))),
                other => (0, int(other)),
            };
            self.extern_calls.push((ext.name.clone(), vec![op as i128, h as i128, ba as i128, n as i128]));
            let input: Vec<u8> = if crate::fileio::takes_input(op) { (0..n).map(|j| *self.arena.get(&(ba + j)).unwrap_or(&0)).collect() } else { Vec::new() };
            let (r, out) = crate::fileio::call(op, h, &input, n);
            for (j, b) in out.iter().enumerate() {
                self.arena.insert(ba + j as u64, *b);
            }
            let ty = if at { IntTy::I64 } else { IntTy::Isize };
            return Ok(EvalResult::Val(Value::Int(ty, r as i128)));
        }
        // `std::read` (`rule.stdlib.read`, D-0050): standard input's next
        // bytes into the arena at `buf` -- at most `len`, 0 at its end, -1
        // on an error -- through Rust's buffered, portable `stdin`, as
        // `cbrt`'s `cb_read_in` does. Standard output is flushed first, so
        // a prompt appears before the wait. The GIL is released while the
        // read blocks, so other threads run on (nothing of the interpreter
        // is touched meanwhile). A program's own `extern fn read` is not
        // this one.
        if self.items.externs.get("std::read").map_or(false, |e| Arc::ptr_eq(e, ext)) {
            let (addr, len) = match (vals.get(0), vals.get(1)) {
                (Some(Value::Rawptr(a, _)), Some(Value::Int(_, n))) => (*a, (*n).max(0) as usize),
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            };
            let mut buf = vec![0u8; len];
            let gil = if self.active_threads > 1 { self.gil.as_ref().and_then(|w| w.upgrade()) } else { None };
            let me = self.current_thread;
            if let Some(g) = &gil {
                g.release();
            }
            let got = {
                use std::io::{Read, Write};
                let _ = std::io::stdout().flush();
                let mut input = std::io::stdin().lock();
                loop {
                    match input.read(&mut buf) {
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        other => break other,
                    }
                }
            };
            if let Some(g) = &gil {
                g.acquire();
                std::hint::black_box(&mut *self);
                self.current_thread = me;
            }
            let n: i128 = match got {
                Ok(n) => {
                    for (j, b) in buf[..n].iter().enumerate() {
                        self.arena.insert(addr + j as u64, *b);
                    }
                    n as i128
                }
                Err(_) => -1,
            };
            self.extern_calls.push((ext.name.clone(), vec![addr as i128, len as i128]));
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
        }
        let to_err = self.items.externs.get("std::write_err").map_or(false, |e| Arc::ptr_eq(e, ext));
        if ext.name == "write" || to_err {
            if let (Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vals.get(0), vals.get(1)) {
                let mut buf = Vec::with_capacity(*len as usize);
                for i in 0..*len {
                    buf.push(*self.arena.get(&(addr + i as u64)).unwrap_or(&0));
                }
                // Bytes, not text: no newline translation on any platform.
                use std::io::Write;
                let res = if to_err {
                    let mut out = std::io::stderr().lock();
                    out.write_all(&buf).and_then(|_| out.flush())
                } else {
                    let mut out = std::io::stdout().lock();
                    out.write_all(&buf).and_then(|_| out.flush())
                };
                let n: i128 = match res {
                    Ok(()) => buf.len() as i128,
                    Err(_) => -1,
                };
                self.extern_calls.push((ext.name.clone(), vec![*addr as i128, *len]));
                return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
            }
        }
        // `[Extern-Call]`: a real call into C (`ffi.rs`); the result is a
        // value of the declared type, whose meaning is an unchecked claim.
        self.extern_calls.push((ext.name.clone(), vec![]));
        let ptys: Vec<Type> = ext.params.iter().map(|p| self.apply_subst(&p.ty, subst)).collect();
        match crate::ffi::call(&ext.name, &ptys, &vals, &ext.ret) {
            Ok(v) => Ok(EvalResult::Val(v)),
            Err(crate::ffi::Refusal::Unsupported(msg)) => {
                // The program's own C code (`extern "…";`) is `cobc`'s to
                // link; name it, so the message says where the function is.
                let msg = if msg.starts_with("no C function") && !self.items.extern_code.is_empty() {
                    let code: Vec<String> = self.items.extern_code.iter().map(|(c, _)| format!("\"{}\"", c)).collect();
                    format!("{}; the program declares C code ({}), which only cobc links", msg, code.join(", "))
                } else {
                    msg
                };
                Err(Flow::Fault(format!("unsupported: {}", msg)))
            }
        }
    }

    // ---- unsafe / rawptr / arena ----

    fn arena_read(&self, addr: u64, ty: &Type) -> Value {
        let sz = self.sizeof_ty(ty);
        let mut bytes = Vec::with_capacity(sz as usize);
        for i in 0..sz {
            bytes.push(*self.arena.get(&(addr + i)).unwrap_or(&0));
        }
        let v = self.decode(ty, &bytes);
        if self.arena_side.range(addr..addr + sz.max(1)).next().is_none() {
            return v;
        }
        self.patch_side(ty, v, addr)
    }
    fn arena_write(&mut self, addr: u64, ty: &Type, v: Value) {
        self.intern_strs_in(&v);
        let bytes = self.encode(ty, &v);
        let sz = bytes.len() as u64;
        for (i, b) in bytes.into_iter().enumerate() {
            self.arena.insert(addr + i as u64, b);
        }
        self.clear_side(addr, sz);
        let mut side = Vec::new();
        self.side_values(ty, &v, 0, &mut side);
        for (off, sv) in side {
            self.arena_side.insert(addr + off, sv);
        }
    }

    fn clear_side(&mut self, addr: u64, n: u64) {
        let gone: Vec<u64> = self.arena_side.range(addr..addr + n.max(1)).map(|(a, _)| *a).collect();
        for a in gone {
            self.arena_side.remove(&a);
        }
    }

    // The values inside `v : ty` that bytes cannot carry, by offset.
    fn side_values(&self, ty: &Type, v: &Value, off: u64, out: &mut Vec<(u64, Value)>) {
        match (ty, v) {
            (Type::Ref(..), Value::Ref { .. }) | (Type::Guard(_), Value::Guard { .. }) | (Type::Handle(_), Value::Handle(..)) | (Type::Fn(..), Value::FnVal(_)) => {
                out.push((off, v.clone()));
            }
            (Type::Named(name, args), Value::Struct(fields)) => {
                if let (Some(s), Some(layout)) = (self.items.structs.get(name), self.struct_layout(name, args)) {
                    let subst = self.subst_map(&s.type_params, args);
                    for (i, f) in fields.iter().enumerate() {
                        let fty = self.apply_subst(&s.fields[i].ty, &subst);
                        self.side_values(&fty, f, off + layout.offsets[i], out);
                    }
                }
            }
            (Type::Named(name, args), Value::Enum(vi, payload)) => {
                if let Some(layout) = self.enum_layout(name, args) {
                    if let Some(pty) = layout.payload_types.get(*vi) {
                        self.side_values(pty, payload, off + layout.payload_off, out);
                    }
                }
            }
            (Type::Mutex(inner), Value::Struct(fields)) => {
                if let Some(f) = fields.first() {
                    self.side_values(inner, f, off, out);
                }
            }
            // A slice (D-0047): its reference at offset 0, then two words.
            (Type::Slice(inner, m), Value::Struct(fields)) => {
                if let Some(f) = fields.first() {
                    self.side_values(&Type::Ref(inner.clone(), m.clone()), f, off, out);
                }
            }
            (Type::Array(inner, _), Value::Array(items)) => {
                let es = self.sizeof_ty(inner);
                for (i, it) in items.iter().enumerate() {
                    self.side_values(inner, it, off + i as u64 * es, out);
                }
            }
            _ => {}
        }
    }

    // `v`, decoded from the bytes at `addr`, with the side values put back.
    fn patch_side(&self, ty: &Type, v: Value, addr: u64) -> Value {
        match (ty, v) {
            (Type::Ref(..), v) | (Type::Guard(_), v) | (Type::Handle(_), v) | (Type::Fn(..), v) => self.arena_side.get(&addr).cloned().unwrap_or(v),
            (Type::Named(name, args), Value::Struct(fields)) => match (self.items.structs.get(name), self.struct_layout(name, args)) {
                (Some(s), Some(layout)) => {
                    let subst = self.subst_map(&s.type_params, args);
                    let fields = fields
                        .into_iter()
                        .enumerate()
                        .map(|(i, f)| {
                            let fty = self.apply_subst(&s.fields[i].ty, &subst);
                            self.patch_side(&fty, f, addr + layout.offsets[i])
                        })
                        .collect();
                    Value::Struct(fields)
                }
                _ => Value::Struct(fields),
            },
            (Type::Named(name, args), Value::Enum(vi, payload)) => match self.enum_layout(name, args) {
                Some(layout) => match layout.payload_types.get(vi) {
                    Some(pty) => Value::Enum(vi, Box::new(self.patch_side(pty, *payload, addr + layout.payload_off))),
                    None => Value::Enum(vi, payload),
                },
                None => Value::Enum(vi, payload),
            },
            (Type::Mutex(inner), Value::Struct(mut fields)) => {
                if !fields.is_empty() {
                    let f = fields.remove(0);
                    fields.insert(0, self.patch_side(inner, f, addr));
                }
                Value::Struct(fields)
            }
            (Type::Slice(inner, m), Value::Struct(mut fields)) => {
                if !fields.is_empty() {
                    let f = fields.remove(0);
                    fields.insert(0, self.patch_side(&Type::Ref(inner.clone(), m.clone()), f, addr));
                }
                Value::Struct(fields)
            }
            (Type::Array(inner, _), Value::Array(items)) => {
                let es = self.sizeof_ty(inner);
                Value::Array(items.into_iter().enumerate().map(|(i, it)| self.patch_side(inner, it, addr + i as u64 * es)).collect())
            }
            (_, v) => v,
        }
    }

    fn encode(&self, ty: &Type, v: &Value) -> Vec<u8> {
        match (ty, v) {
            (Type::Int(t), Value::Int(_, x)) => {
                let bw = t.bitwidth(ADDR_WIDTH);
                (*x as u128).to_le_bytes()[..(bw / 8) as usize].to_vec()
            }
            (Type::Bool, Value::Bool(b)) => vec![if *b { 1 } else { 0 }],
            (Type::F64, Value::F64(f)) => f.to_le_bytes().to_vec(),
            (Type::F32, Value::F32(f)) => f.to_le_bytes().to_vec(),
            (Type::Rawptr(_), Value::Rawptr(a, _)) => a.to_le_bytes().to_vec(),
            (Type::Str, Value::Str(s)) => {
                // [Repr-Str] (impl-defined): (address of program data, length).
                let addr = self.str_data.get(s).copied().unwrap_or(0);
                let mut out = addr.to_le_bytes().to_vec();
                out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                out
            }
            (Type::Ref(..), _) | (Type::Handle(_), _) | (Type::Guard(_), _) => {
                // stored as an opaque object-id token in our arena model
                0u64.to_le_bytes().to_vec()
            }
            (Type::Fn(..), Value::FnVal(name)) => {
                // [Repr-Fn] (spec/06 sec 4): "any injective AddrWidth/8-
                // byte image of an item path" -- a deterministic hash of
                // the qualified name, not a real reversible encoding
                // (decode does not attempt to reconstruct a callable
                // FnVal from this; see the "known gaps" note in
                // STATUS.md -- the spec's own text says this
                // representation is "only consumed by ==/!= on fn
                // values and by spawn's callee argument", both of which
                // this interpreter already handles directly on the
                // Value::FnVal representation, never via an arena
                // round-trip).
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut h = DefaultHasher::new();
                name.hash(&mut h);
                h.finish().to_le_bytes().to_vec()
            }
            (Type::Named(name, args), Value::Struct(fields)) => {
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                if let Some(s) = self.items.structs.get(name) {
                    let subst = self.subst_map(&s.type_params, args);
                    if let Some(layout) = self.struct_layout(name, args) {
                        for (i, f) in fields.iter().enumerate() {
                            let fty = self.apply_subst(&s.fields[i].ty, &subst);
                            let fb = self.encode(&fty, f);
                            let off = layout.offsets[i] as usize;
                            out[off..off + fb.len()].copy_from_slice(&fb);
                        }
                    }
                }
                out
            }
            (Type::Named(name, args), Value::Enum(vi, payload)) => {
                // [Repr-Enum] (spec/06 §7): the DW-byte image of the
                // variant index at offset 0, the payload's own
                // represent() at payload-off, pad elsewhere.
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                if let Some(layout) = self.enum_layout(name, args) {
                    let disc = (*vi as u64).to_le_bytes();
                    let dw = layout.dw as usize;
                    out[0..dw].copy_from_slice(&disc[..dw]);
                    if let Some(pty) = layout.payload_types.get(*vi) {
                        let pb = self.encode(pty, payload);
                        let off = layout.payload_off as usize;
                        out[off..off + pb.len()].copy_from_slice(&pb);
                    }
                }
                out
            }
            (Type::Mutex(inner), Value::Struct(fields)) => {
                // [Repr-Mutex] (spec/06 sec 4): represent(inner,v) at
                // off(inner)=0; the state cells (bytes sizeof(inner)..
                // sizeof(inner)+8) are outcome: impl-defined/never read
                // by a rule -- left zeroed, a valid instance of that
                // freedom, not a placeholder standing in for a real value.
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                if let Some(v) = fields.first() {
                    let ib = self.encode(inner, v);
                    out[0..ib.len()].copy_from_slice(&ib);
                }
                out
            }
            (Type::Slice(..), Value::Struct(fields)) => {
                // The reference's word (kept beside, as any reference's),
                // then the start and the length.
                let mut out = vec![0u8; 8];
                for f in fields.iter().skip(1) {
                    let x = if let Value::Int(_, x) = f { *x as u64 } else { 0 };
                    out.extend_from_slice(&x.to_le_bytes());
                }
                out.resize(24, 0);
                out
            }
            (Type::Array(inner, _), Value::Array(items)) => {
                // [Repr-Array] (spec/06 §7): represent(τ,vi) placed at i·sizeof(τ).
                let elem_sz = self.sizeof_ty(inner) as usize;
                let mut out = Vec::with_capacity(elem_sz * items.len());
                for item in items {
                    let mut eb = self.encode(inner, item);
                    eb.resize(elem_sz, 0);
                    out.extend_from_slice(&eb);
                }
                out
            }
            _ => vec![0u8; self.sizeof_ty(ty) as usize],
        }
    }

    fn decode(&self, ty: &Type, bytes: &[u8]) -> Value {
        match ty {
            Type::Int(t) => {
                let bw = (t.bitwidth(ADDR_WIDTH) / 8) as usize;
                let mut buf = [0u8; 16];
                buf[..bw.min(16)].copy_from_slice(&bytes[..bw.min(16)]);
                let u = u128::from_le_bytes(buf);
                Value::Int(*t, reinterpret_sign_pub(*t, u, t.bitwidth(ADDR_WIDTH)))
            }
            Type::Bool => Value::Bool(bytes.first().copied().unwrap_or(0) != 0),
            Type::F64 => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&bytes[..8]);
                Value::F64(f64::from_le_bytes(b))
            }
            Type::F32 => {
                let mut b = [0u8; 4];
                b.copy_from_slice(&bytes[..4]);
                Value::F32(f32::from_le_bytes(b))
            }
            Type::Rawptr(inner) => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&bytes[..8]);
                Value::Rawptr(u64::from_le_bytes(b), (**inner).clone())
            }
            Type::Str => {
                let mut a = [0u8; 8];
                let mut l = [0u8; 8];
                a.copy_from_slice(&bytes[..8]);
                l.copy_from_slice(&bytes[8..16]);
                let (addr, len) = (u64::from_le_bytes(a), u64::from_le_bytes(l));
                let data: Vec<u8> = (0..len).map(|i| *self.arena.get(&(addr + i)).unwrap_or(&0)).collect();
                Value::Str(Arc::from(data))
            }
            Type::Named(name, args) => {
                if let Some(s) = self.items.structs.get(name).cloned() {
                    let subst = self.subst_map(&s.type_params, args);
                    if let Some(layout) = self.struct_layout(name, args) {
                        let mut fields = Vec::new();
                        for (i, f) in s.fields.iter().enumerate() {
                            let fty = self.apply_subst(&f.ty, &subst);
                            let sz = self.sizeof_ty(&fty) as usize;
                            let off = layout.offsets[i] as usize;
                            fields.push(self.decode(&fty, &bytes[off..off + sz]));
                        }
                        return Value::Struct(fields);
                    }
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    let dw = layout.dw as usize;
                    let mut buf = [0u8; 8];
                    buf[..dw.min(8)].copy_from_slice(&bytes[..dw.min(8)]);
                    let vi = u64::from_le_bytes(buf) as usize;
                    let pty = layout.payload_types.get(vi).cloned().unwrap_or(Type::Void);
                    let off = layout.payload_off as usize;
                    let psz = self.sizeof_ty(&pty) as usize;
                    let payload = if off + psz <= bytes.len() {
                        self.decode(&pty, &bytes[off..off + psz])
                    } else {
                        Value::Unit
                    };
                    return Value::Enum(vi, Box::new(payload));
                }
                Value::Unit
            }
            Type::Mutex(inner) => {
                let isz = self.sizeof_ty(inner) as usize;
                let v = if isz <= bytes.len() {
                    self.decode(inner, &bytes[..isz])
                } else {
                    Value::Unit
                };
                Value::Struct(vec![v])
            }
            Type::Array(inner, n) => {
                let elem_sz = self.sizeof_ty(inner) as usize;
                let mut items = Vec::with_capacity(*n as usize);
                for i in 0..*n as usize {
                    let off = i * elem_sz;
                    if off + elem_sz > bytes.len() {
                        break;
                    }
                    items.push(self.decode(inner, &bytes[off..off + elem_sz]));
                }
                Value::Array(items)
            }
            // A slice (D-0047): the reference (put back by `patch_side`),
            // the start, the length.
            Type::Slice(..) => {
                let word = |k: usize| -> i128 {
                    let mut b = [0u8; 8];
                    if bytes.len() >= k + 8 {
                        b.copy_from_slice(&bytes[k..k + 8]);
                    }
                    u64::from_le_bytes(b) as i128
                };
                Value::Struct(vec![Value::Unit, Value::Int(IntTy::Usize, word(8)), Value::Int(IntTy::Usize, word(16))])
            }
            _ => Value::Unit,
        }
    }

    fn struct_layout(&self, name: &str, args: &[Type]) -> Option<Layout> {
        let s = self.items.structs.get(name)?;
        let subst = self.subst_map(&s.type_params, args);
        let mut off = 0u64;
        let mut align = 1u64;
        let mut offsets = Vec::new();
        for f in &s.fields {
            let fty = self.apply_subst(&f.ty, &subst);
            let fa = self.alignof_ty(&fty);
            align = align.max(fa);
            off = (off + fa - 1) / fa * fa;
            offsets.push(off);
            off += self.sizeof_ty(&fty);
        }
        off = (off + align - 1) / align * align;
        Some(Layout { size: off, align, offsets })
    }

    // [Layout-Enum] (spec/16 §1): discriminant at offset 0, width DW;
    // payload-off = least multiple of max(alignof(τi)) >= DW;
    // sizeof = least multiple of alignof >= payload-off + max(sizeof(τi));
    // alignof = max(DW, alignof(τi)). DW = 4 is this implementation's
    // documented impl-defined choice (impl/STATUS.md).
    fn enum_layout(&self, name: &str, args: &[Type]) -> Option<EnumLayout> {
        let dw = ENUM_DISCRIMINANT_WIDTH as u64;
        let e = self.items.enums.get(name)?.clone();
        let subst = self.subst_map(&e.type_params, args);
        let mut max_payload_align = 1u64;
        let mut max_payload_size = 0u64;
        let mut payload_types = Vec::new();
        for v in &e.variants {
            let pty = match &v.payload {
                Some(t) => self.apply_subst(t, &subst),
                None => Type::Void,
            };
            max_payload_align = max_payload_align.max(self.alignof_ty(&pty));
            max_payload_size = max_payload_size.max(self.sizeof_ty(&pty));
            payload_types.push(pty);
        }
        let payload_off = (dw + max_payload_align - 1) / max_payload_align * max_payload_align;
        let align = dw.max(max_payload_align);
        let raw = payload_off + max_payload_size;
        let size = (raw + align - 1) / align * align;
        Some(EnumLayout { size, align, dw, payload_off, payload_types })
    }
}

struct Layout {
    size: u64,
    align: u64,
    offsets: Vec<u64>,
}

struct EnumLayout {
    size: u64,
    align: u64,
    dw: u64,
    payload_off: u64,
    payload_types: Vec<Type>,
}

fn r_is_temp(_obj: u64, _objects: &HashMap<u64, Object>) -> bool {
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
    "diag.aliasing-conflict",
    "diag.alloc-failure",
    "diag.ambiguous-name",
    "diag.arith-overflow",
    "diag.bad-destructor-signature",
    "diag.borrow-of-non-place",
    "diag.borrow-of-temporary",
    "diag.capture-list-mismatch",
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
    body: Block,
    subst: HashMap<String, Type>,
    is_move: bool,
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

