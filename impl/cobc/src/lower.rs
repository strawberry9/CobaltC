// Typed lowering of an accepted program to C (impl/COBC-PLAN.md §3).
//
// One recursive walk computes each expression's type and emits C for
// it, in three-address form: every intermediate value is a named C
// local. The program was accepted by `typecheck`, so nothing here
// rejects; a type this walk cannot determine is an internal error, and
// a construct this stage does not compile yet is `Unsupported` (exit 3
// at the driver).
//
// Every binding is a stack local *and* a runtime object with a root
// access path; every place access names its path (a binding's root, or
// the token of the reference it goes through) so the runtime can run
// `[Read]`/`[Write]`/`[Borrow]`'s checks. A reference is fat in
// registers (`cb_ref`: pointer + token) and thin in memory (the pointer;
// the token in the runtime's slot table). A resource-typed value always
// travels with its object id: moved into a binding (`cb_move_to` +
// `cb_bind`), absorbed into a container (`cb_absorb`), or sent across
// a call (`cb_send`/`cb_recv`). Temporaries that hold references or
// resources are registered so a statement's exit can end them.
//
// Layouts: C's struct layout for the same scalars *is* `[Layout-Struct]`
// and `struct { uint32_t tag; union {…} u; }` *is* `[Layout-Enum]` with
// `DW = 4`; `size_align` recomputes both from the spec's rules and the
// emitted C asserts they agree.

use coby::ast::*;
use coby::interp::Items;
use coby::loader::SourceMap;
use coby::prelude;
use coby::value::int_min_max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

// WI-15: the generator's parts, by concern (each `impl` of Gen or Fx
// continues the ones here).
mod analysis_refs;
mod analysis_strings;
mod analysis_vec;
mod calls;
mod closures;
mod cnames;
mod equality;
mod exprs;
mod frames;
mod header;
mod flights;
mod fresh;
mod functions;
mod intrinsics;
mod layout;
mod literal_calls;
mod matching;
mod native_io;
mod native_map;
mod native_misc;
mod native_text;
mod native_vec;
mod operators;
mod passes;
mod places;
mod program;
mod simd;
mod statics;
mod stmts;
mod syntax;
mod typeops;
mod user_calls;
mod uses;
mod values;
mod vec_calls;
mod vec_scan;
use self::{cnames::*, passes::*, syntax::*, typeops::*, uses::*};

pub struct Unsupported(pub String);
type R<T> = Result<T, Unsupported>;

fn unsupported<T>(what: &str) -> R<T> {
    Err(Unsupported(what.to_string()))
}

fn internal<T>(what: &str) -> R<T> {
    Err(Unsupported(format!("internal error in cobc: {}", what)))
}

const HEADER: &str = include_str!("../../cbrt/include/cbrt.h");
const NOLOC: &str = "CB_NOLOC, 0";

// `std::Vec` calls, besides `push`, `pop`, `reserve`, `len` and indexing,
// that keep a vector confined (`Gen::confined_vecs`): for a plain element
// type their native bodies move or compare elements only as bytes, form
// no element path and keep nothing (for another element type the body,
// called through the root path, leaves no path or object behind it).
// `Fx::lower_confined_call` calls their unchecked forms (`nc_variant`).
const CONFINING_CALLS: &[&str] = &[
    "std::Vec::swap", "std::Vec::reverse", "std::Vec::insert", "std::Vec::remove", "std::Vec::truncate", "std::Vec::clear",
    "std::Vec::contains", "std::Vec::index_of", "std::Vec::binary_search", "std::Vec::sort",
];

// D-0192: the `Vec` calls `Fx::lower_vec_checked_once` serves.
const ONCE_CALLS: &[&str] = &[
    "std::Vec::swap", "std::Vec::reverse", "std::Vec::insert", "std::Vec::remove", "std::Vec::truncate", "std::Vec::clear", "std::Vec::pop",
    "std::Vec::contains", "std::Vec::index_of", "std::Vec::binary_search",
];

// The longest fast half, in lines of C, that a dual body (`lower_fn_dual`)
// is emitted with: past it, the function is emitted once, checked.
const DUAL_LINES: usize = 600;

// CHG-0237: the longest frozen body (`lower_fn_frozen`) kept beside its
// checked one. Larger than `DUAL_LINES`: the loops a frozen body speeds up
// are what make a body long.
const FROZEN_LINES: usize = 3000;

// A value: a C expression of the value's register representation
// (`cb_ref` for a reference), its type, and — when the value lives in
// a registered temporary — the C local holding that object's id.
#[derive(Clone)]
struct V {
    c: String,
    ty: Type,
    obj: Option<String>,
}

// A place: a C lvalue, its type, the token of the path it is reached
// through (`None` for a temporary or a raw pointer: unchecked), the
// projection from that path, and whether it is a binding itself.
#[derive(Clone)]
struct P {
    c: String,
    ty: Type,
    base: Option<String>,
    projs: Vec<String>,
    is_binding: bool,
    // The pointer expression when this place is `*p` on a raw pointer:
    // reads and writes are trusted, moves relocate through it.
    raw: Option<String>,
}

#[derive(Clone)]
enum BindKind {
    Local,
    // A borrow-captured name inside a closure body: `*self.f_i`.
    CaptureRef(usize),
    // A move-captured name: `self.f_i`.
    CaptureVal(usize),
}

#[derive(Clone)]
struct Binding {
    c: String,
    ty: Type,
    root: String,
    kind: BindKind,
}

#[derive(Clone)]
struct ClosureInfo {
    fields: Vec<(String, Type)>,
    is_move: bool,
    params: Vec<Type>,
    ret: Type,
    cname: String,
    // D-0239: a `move` closure whose body writes a capture: a call changes
    // its own storage (`[Closure-Call]`), so its box is marked (`closure`
    // = 3) and a call through a place checks an exclusive access there.
    writes_self: bool,
}

fn unit() -> V {
    V { c: "(cb_unit){}".to_string(), ty: Type::Void, obj: None }
}

// `str`, `String` or `StringView`: a level a text literal is matched
// against (D-0127).
fn is_text_ty(t: &Type) -> bool {
    match t {
        Type::Str => true,
        Type::Named(n, _) => n == "std::String" || n == "std::StringView",
        _ => false,
    }
}

// The pointer and length of the text value `c` of text type `ty`.
fn text_parts(c: &str, ty: &Type) -> (String, String) {
    match ty {
        Type::Str => (format!("(const void *)({}).p", c), format!("({}).n", c)),
        Type::Named(n, _) if n == "std::StringView" => (format!("(const void *)({}).bytes.data", c), format!("({}).bytes.len", c)),
        _ => (format!("(const void *)({}).bytes.ptr", c), format!("({}).bytes.len", c)),
    }
}

fn never() -> V {
    V { c: "(cb_unit){}".to_string(), ty: Type::Never, obj: None }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cleanup {
    Frame(u32),
    Stmt(u32),
    // An objectless local's drop (`Fx::drop_lines`), emitted where the
    // frame below it ends or a jump leaves it.
    Drop(u32),
}

pub struct Gen<'a> {
    items: &'a Items,
    map: &'a SourceMap,
    files: Vec<String>,
    file_ids: HashMap<String, u32>,
    fwd: String,
    types: String,
    type_names: HashMap<Type, String>,
    defined: HashSet<Type>,
    type_ids: HashMap<Type, u32>,
    // The descriptors in emission order, and each one's id: a 32-bit hash
    // of the type's mangled name, so the same type has the same id in
    // every program; `type_pos` finds a
    // descriptor's position from its id.
    descs: Vec<Option<String>>,
    desc_ids: Vec<u32>,
    type_pos: HashMap<u32, usize>,
    desc_aux: String,
    strings: String,
    nstr: u32,
    // String literals by their stable name (`cb_s_<hash>`), each defined
    // once; the bytes guard against two literals hashing alike.
    str_names: HashMap<String, Vec<u8>>,
    // The names `str_names` was given, in order: a probe's additions are
    // undone by `str_restore` (`strings` and `str_names` only grow).
    str_log: Vec<String>,
    // The lowered functions, each with its text and whether it is pure
    // `std` (`Unit`).
    units: Vec<Unit>,
    protos: String,
    fns: String,
    // `cbn_string_substring` has been written (`substring_helper`).
    substring_fn: bool,
    // `cbn_append_substring` has been written (`append_substring_helper`).
    append_substring_fn: bool,
    // The function a native body is being made for (`native_vec_index`):
    // its name, declaration, type arguments, C result type and C
    // signature, for `std_fallback`.
    native_ctx: Option<(String, std::sync::Arc<FnDecl>, Vec<Type>, String, String)>,
    // The confined fields of the next body `emit_fn_body` makes (a field
    // dual body's fast half, `lower_fn_field_dual`).
    pending_field_confined: Option<HashMap<(String, String), (String, Type)>>,
    // The next closure body is a predicate literal that only
    // `lower_pred_literal`'s loop calls: `self` and its parameter bound
    // directly, as a capture-free closure's are.
    pending_pred_closure: bool,
    // D-0200: the next closure body is a read-only closure (`lower_closure`):
    // its reference parameters, read through or lent to readers, keep
    // nothing even when handed on, so it is pure.
    pending_reader_closure: bool,
    // D-0192: the next body `emit_fn_body` makes is a dual body's bare form
    // (`lower_fn_dual`): its slice parameters taken as pure direct ones
    // (not received; the caller holds their paths).
    pending_bare: bool,
    // D-0196: the next body `emit_fn_body` makes is a `__raw` form, which
    // returns its objectless `String` with no object (`raw_ret_ok`).
    pending_raw_ret: bool,
    // CHG-0237: the body being emitted is a frozen one (`NAME__frz`): its
    // frozen parameters' reads go unchecked (`Fx::frozen_vec`, `frozen_root`).
    frozen_mode: bool,
    // D-0209: the dispatchers `localize` marks `always_inline`.
    inline_sigs: HashSet<String>,
    // D-0219: the dispatchers with an `__arrive` twin (`emit_arrive_twin`);
    // a call's `ARRIVE_MARK` resolves to the twin where there is one.
    arrive_twins: HashSet<String>,
    // D-0219: the body about to be emitted is entered only through an
    // arrival test (a direct, frozen or dual body): its direct parameters
    // may be passed on as vouched (`Fx::fresh_ref_arg`).
    pending_tested: bool,
    // D-0209: `cobc --all-checks` (`COBALTC_ALL_CHECKS`): no proof about a
    // function's reference parameters is used -- no dual, frozen, direct
    // or confined-parameter body, no bare call; every access through a
    // parameter is checked where it is made. For comparing against the
    // elided forms (`impl/battery.sh` stage 3); what a program proves of
    // its own locals is kept.
    all_checks: bool,
    // D-0274: `--bootstrap-form`, the general checked form only: the
    // stages of D-0274 switched off, 1..=`bootstrap` (`bootstrap_level`;
    // 0 is the default form, 10 every stage; `boot`).
    pub(crate) bootstrap: u8,
    // WI-11: `#line` directives naming the program's `.cb` lines before
    // the C of its own statements (`line_directives`); off with
    // `--no-line-directives`.
    pub line_directives: bool,
    // D-0258: the C globals emitted so far (statics `g_…`, addressed
    // constants `k_…`), each defined once in `strings`.
    globals: HashSet<String>,
    // D-0257: `cobc --lib`: no `main`, and a C-callable wrapper for each
    // `extern fn` with a body (`program.rs`); the header's text for C
    // callers is in `Generated::header_text`.
    pub lib: bool,
    // D-0281 step 4c: lower the program pathless when it is eligible
    // (`pathless_blocker`): every reference carries token 0 and `cbrt.h`'s
    // `CB_PATHLESS` block makes the path entries no-ops.
    pub pathless: bool,
    // D-0285: the program was found decided by a first lowering (D-0284),
    // and this lowering knows it: a reference or a slice in a value is
    // nothing the runtime tracks (`has_refs`), so such a value owns no
    // object and its copies copy no holds.
    pub decided: bool,
    // D-0192: closures whose reference parameters are all direct and never
    // handed on (`lower_closure_body`): their code keeps nothing of its
    // arguments, so a `fn` value holding one is pure (`cb_fn_mark_pure`).
    pure_closures: HashSet<u64>,
    fn_names: HashMap<(String, Vec<Type>), String>,
    queue: VecDeque<(String, Vec<Type>, u64)>,
    uid: u32,
    // The closures of the C function being emitted are numbered from here
    // (`closure_id`): a hash of its C name, so closures of different
    // functions, each numbered from 0 like their temporaries, never share
    // a C name, and each function's text stays independent of the rest of
    // the program.
    closure_base: u64,
    closures: HashMap<u64, ClosureInfo>,
    thunks: HashMap<Type, String>,
    thunks_out: String,
    eqs: HashMap<Type, String>,
    // D-0110: a struct or enum key type's hash and order functions.
    khs: HashMap<Type, String>,
    kcs: HashMap<Type, String>,
    externs_out: HashMap<String, String>,   // an extern's symbol -> its C-side name
    // While probing (lowering into a discarded buffer for a type), no
    // function body may be emitted.
    probing: u32,
    // Ids for statement scopes and frames (`resolve_scopes`).
    nscope: u32,
    // For each program function, which parameters are *confining*
    // (`confining_table`), and the C names of the variants compiled for
    // confined arguments, by the mask of those parameters.
    confining: HashMap<String, Vec<bool>>,
    variants: HashMap<(String, Vec<Type>, u64), String>,
    // `pure_direct`'s answers, by function and parameter position.
    pure_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    view_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    // `reader_param`'s answers.
    reader_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    // D-0200: `writer_param`'s answers.
    writer_memo: std::cell::RefCell<HashMap<(String, usize), bool>>,
    // The last segments of every path named other than as a callee
    // (`used_as_value`), gathered on first need.
    fn_values: std::cell::RefCell<Option<HashSet<String>>>,
}


struct Fx<'g, 'a> {
    g: &'g mut Gen<'a>,
    // D-0283: the site (the callee's line and column) of the call
    // `lower_call` is lowering, for its point checks; taken as it starts.
    pc_site: Option<(usize, usize)>,
    // A `match`'s fused scrutinee, lowered as its place (`lower_match`).
    fused_scrut: Option<V>,
    // `*HashMap::entry(…) op= e` (`lower_block_body`'s compound pair):
    // asks the native `entry` call for its `_acc` variant (the value's
    // address after one element access); `map_acc_used` says it did.
    map_acc: Option<usize>,
    map_acc_used: bool,
    // The compound pair's borrow (`lower_block_body`): a confined
    // vector's element, reached as an unchecked place, is borrowed as its
    // address with token 0, as a direct argument is (`lower_user_arg_direct`).
    compound_elem: bool,
    // `match (f(…))` on an `Option<ref<T>>` call result (`lower_match`):
    // asks that call (by its argument list) to receive the reference's
    // token alone (`cb_recv_datum_tok`), with no temporary object holding
    // it; `datum_tok_out` names the token's C variable when it did.
    datum_tok: Option<usize>,
    datum_tok_out: Option<String>,
    // `match (HashMap::get_mut(…))` whose `Some(c)` arm is only `*c op= x`
    // (`lower_match`): asks that call for the native `_acc` variant;
    // `map_get_acc_out` names the value's address when it was made so.
    map_get_acc: Option<usize>,
    map_get_acc_out: Option<String>,
    // The mode `native_elem_access` checks the element in, when it is not
    // the call's own (`&mut v[i]` formed through `index_shared`'s place).
    elem_mode: Option<&'static str>,
    subst: HashMap<String, Type>,
    scopes: Vec<HashMap<String, Binding>>,
    out: String,
    ret: Type,
    cleanup: Vec<Cleanup>,
    // Each open loop: the cleanup depth at its head, and for a `for` loop
    // (D-0035) the label before its step, which `continue` jumps to.
    loops: Vec<(usize, Option<String>)>,
    // Per open loop, whether a `break` leaves it (D-0175: a `while (true)`
    // that none leaves never completes).
    loop_broken: Vec<bool>,
    ind: usize,
    // Inside a closure body: the `self` parameter and the closure's id.
    self_ref: Option<(String, u64)>,
    // Names the body borrows, captures, or drops (`pinned_names`): a
    // plain local not among them is never checked at run time.
    pinned: HashSet<String>,
    // Names the body assigns to as a whole (`assigned_names`), and, for
    // each binding of one, its root's C name -> the C name of its frame
    // (`cb_frame_here`), which `cb_rebind` needs.
    assigned: HashSet<String>,
    rebind_frames: HashMap<String, String>,
    // D-0111: an unchecked holder's root, by its C name (its object is
    // ended at its last use, `end_ref_bindings`).
    holder_roots: HashMap<String, String>,
    // `x + 1` nodes (by address) that cannot overflow (`loop_increment`),
    // and whether the `lower_binary` about to run is one.
    no_overflow: HashSet<usize>,
    proven_next: bool,
    // Names `confined_vecs` found confined, and the C names of the
    // bindings of them that qualified (`bind_confined`).
    confined_names: HashSet<String>,
    confined: HashSet<String>,
    // `confined_fields`: the `(local, field)` pairs whose `Vec` is reached
    // only by indexing and `Vec::len` (`confined_field`).
    confined_fields: HashSet<(String, String)>,
    // `read_confined_vecs`: locals whose elements no exclusive path can
    // ever be held to, so an index read needs only its bounds check.
    read_confined: HashSet<String>,
    // `Gen::objectless_strings`: `String` locals with no runtime object,
    // the C names bound so, and the drop each `Cleanup::Drop` emits.
    objectless_names: HashSet<usize>,
    // The `let` being lowered, by its initializer's address (the key of
    // `objectless_names`), while its binding is made.
    cur_let: Option<usize>,
    // D-0196: this body is a `__raw` form; and the C name of a call whose
    // result goes into an objectless binding, to be made to its `__raw` form
    // (`finish_call`).
    raw_ret: bool,
    // D-0200: in a `__raw` form, the `match` that is the body's last
    // expression (by its arms' address): its result sink is raw
    // (`raw_sinks`), each arm's value's record ending as it lands, so the
    // result returns with no object to end.
    raw_tail: Option<usize>,
    raw_sinks: HashSet<String>,
    // D-0200: the `String::from_view(String::view(…))` (by its arguments'
    // address) whose result is wanted with no object (`lower_raw_string`).
    raw_substring: Option<usize>,
    // In a `__raw` form, the value being lowered is the function's result
    // itself, a local named alone (`return x`, or the last expression `x`):
    // its move leaves with no object (`read_place`).
    raw_return_path: bool,
    raw_call: Option<String>,
    objectless_c: HashSet<String>,
    // D-0286: by candidate, the fields the body reads by value
    // (`Gen::objectless_strings_with`).
    objectless_fields: HashMap<usize, HashSet<String>>,
    // D-0200: an objectless local that may be moved away: the C flag set
    // when it is (`read_place`), tested at each later use (the staleness a
    // moved binding's object would have reported) and at its drop.
    moved_flag: HashMap<String, String>,
    // D-0200: locals a fused text loop's body trims its bare piece into
    // (`lower_fused_text_loop`): bare views too, trimmed in place.
    bare_trims: HashSet<String>,
    // D-0200: the element-reference `let`s needing no path
    // (`Gen::elem_ref_lets`), by initializer address.
    elem_ref_ids: HashSet<usize>,
    // D-0200: the quiet container locals (`Gen::quiet_locals`), by
    // initializer address, and the roots of those bound so far: their
    // shared checks are not made (`drop_quiet_checks`).
    quiet_ids: HashSet<usize>,
    quiet_roots: HashSet<String>,
    drop_lines: Vec<String>,
    // In a variant for confined arguments: each such parameter's vector
    // (a C lvalue), the path its caller passed, and its element type.
    confined_params: HashMap<String, (String, String, Type)>,
    // `probe_type`'s answers in this body, by expression and expected type.
    // An expression's type is fixed by where it is, so probing it again --
    // as every enclosing `if`, `match` and block does before lowering it
    // for real -- need not lower it again: without this, nesting of depth
    // n was lowered 2^n times.
    probes: HashMap<(usize, String), Type>,
    // Expressions this body synthesizes (a cloned `then` block, `?`'s
    // match, a closure's captures), kept alive with it: `probes` is keyed
    // by address, which must not be reused by a later expression.
    kept: Vec<std::rc::Rc<dyn std::any::Any>>,
    // One block expression per `then` block, so that lowering an `if` a
    // second time (after an enclosing probe) finds its probes cached.
    // D-0047: the C expressions `$` stands for, innermost `[…]` last.
    dollar: Vec<String>,
    // `v[i]` on a `Vec` (D-0047 `[Index-Vec]`): the place is written (an
    // assignment's target, `&mut`'s operand), so it is `index_exclusive`.
    place_write: bool,
    // D-0103: the next temporary `lower_place` reaches is the root of a
    // borrow that outlives the expression: make it a checked object.
    temp_root: bool,
    // The line the next block's frame reports a destruction fault at
    // (`push_frame_at`), set by the construct that owns the block.
    frame_line: Option<usize>,
    // `v[i]` on a `Vec` as the `*Vec::index_*(&v, i)` it stands for, kept
    // (address-stable for `probe_type`'s cache) per expression and mode.
    vec_index_exprs: HashMap<(usize, bool), std::rc::Rc<Expr>>,
    // The first arguments (`&e`, `&mut e`) of those calls, by address:
    // `[Index-Vec]` checks their borrow as an access to element `k`.
    index_sugar: HashSet<usize>,
    // D-0060: the next `lower_match` is a `?`'s: a temporary scrutinee
    // that owns nothing ends as soon as the match has taken it apart.
    propagate_scrutinee: bool,
    // A `foreach` element over a shared `Vec` of plain elements that the
    // body only reads (`*x`): `x` stands for the checked element read
    // `(*$c)[$i]` instead of a reference bound each step (`foreach_alias`).
    elem_alias: HashMap<String, std::rc::Rc<Expr>>,
    // Direct reference parameters (`Gen::direct_ref_params`): by name, and
    // the C names of their bindings, whose places are reached unchecked.
    direct_params: HashSet<String>,
    // D-0219: this body runs only after an arrival test (`Gen::pending_tested`).
    entry_tested: bool,
    direct_refs: HashSet<String>,
    // For a direct slice's C name, the flag set at entry when a live object
    // lies over any of its elements (`cb_live_in`): its element accesses
    // then meet those objects' paths (`cb_elem_access_slow`).
    direct_live: HashMap<String, String>,
    // The fast half of a dual body (`lower_fn_dual`): its dispatcher has
    // found no live object over any slice argument's cells.
    fast_half: bool,
    // D-0192: an index read now being lowered for a reader parameter
    // (`Gen::reader_param`): a resource element holding no reference is
    // read in place, its checks made by `cb_elem_access`.
    elem_reader: bool,
    // CHG-0237: this body is a frozen one (`Gen::frozen_mode` when it began).
    frozen_on: bool,
    // D-0192: this body is a predicate `lower_pred_literal` calls with bare
    // element addresses (`pending_pred_closure`).
    pred_body: bool,
    // The fast half of a field dual body (`lower_fn_field_dual`): for the
    // reference parameter's confined `Vec` fields, by (parameter, field),
    // the vector as a C lvalue and its element type (`confined_local`).
    field_confined: HashMap<(String, String), (String, Type)>,
    // Pure direct slice parameters (`Gen::pure_direct`): not received --
    // every caller keeps the slice's path for the call instead.
    pure_slices: HashSet<String>,
    // Parameters that are pure views (`Gen::pure_view`): not received.
    pure_views: HashSet<String>,
    // Unstored reference parameters (`Gen::unstored_ref_params`): the C
    // name of the binding's address, and the incoming token used for it.
    unstored: HashMap<String, String>,
    unstored_params: HashSet<String>,
    // Locals of reference type whose path the frame can hold with no
    // binding object (`held_locals`), and the C names of those bound so.
    held_locals: HashSet<String>,
    held_c: HashSet<String>,
    // Arm binders with no path minted (their one use is `Box::get(b)` /
    // `Box::get_mut(b)`, made inline): C name -> (base path, projection).
    unminted_binders: HashMap<String, (String, Vec<String>)>,
    // Every alias made, kept to the function's end: the lowering caches
    // synthesized expressions by address (`vec_index_exprs`), and a freed
    // alias's address reused by a later one would hit a stale entry.
    alias_kept: Vec<std::rc::Rc<Expr>>,
    // A `match` that is a whole statement, its value unused (`lower_stmt`):
    // taken by that `match` (`lower_match`), whose arms then get a statement
    // scope each (`branch_scope`, taken by `lower_branch`). The arm is the
    // last thing the statement evaluates, so its temporaries ending with
    // the arm is their ending with the statement; an arm that forms none
    // pays for no scope (`resolve_scopes` drops it).
    stmt_match: bool,
    branch_scope: bool,
    // The literal about to be built (`lower_struct_lit`, `construct_variant`,
    // `lower_box_new`, which take it) goes straight into the aggregate or
    // `Box` being built around it and owns nothing a fault could leave
    // undestroyed (`inert_expr`): it gets no object. `inert_c`: the C
    // temporaries holding such values, which `into_temp` and the absorbing
    // literal accept without one.
    objectless: bool,
    inert_c: HashSet<String>,
    // The C temporaries `into_temp` made for a value with an object (a
    // resource, or plain data holding references), the object moved to
    // each. Such a temporary is written once, where it is declared, so a
    // later `into_temp` of the same value hands it back as it is rather
    // than copying it into yet another temporary and moving the object
    // there too: only the value's last address matters to the runtime.
    obj_temps: HashSet<String>,
    // D-0286: the call whose resource result the consumer being lowered
    // takes with no object (`arm_inert`, by its arguments' address, and
    // whether nothing is evaluated after it), and the same for the
    // `finish_call` about to run.
    inert_result: Option<(usize, bool)>,
    inert_now: Option<bool>,
    // D-0269: the objectless locals bound in this body so far, each an
    // entry of the C table `cb_nt` (`objectless_bound`), and the id of
    // the body's root frame, whose exits pop the table (`notes_finish`).
    notes: u32,
    root_frame: u32,
}


// The C name of a function instance: `f_` and the sanitized path, then
// the mangled type arguments of a generic one.
fn fn_cname(name: &str, targs: &[Type]) -> String {
    let mut c = format!("f_{}", san(name));
    if !targs.is_empty() {
        c.push_str("_L");
        for t in targs {
            c.push_str(&mangle(t));
            c.push('_');
        }
        c.push('R');
    }
    c
}

// A declaration from the prelude's own source (a program may declare
// its own function of the same name), known by its body's first line.
fn in_prelude(f: &FnDecl) -> bool {
    let line = match f.body.stmts.first() {
        Some(Stmt::BlockLike(e)) | Some(Stmt::Expr(e)) | Some(Stmt::Let { init: Some(e), .. }) => Some(e.line),
        Some(_) => None,
        None => f.body.tail.as_ref().map(|e| e.line),
    };
    line.map_or(false, |l| l <= prelude::line_count())
}

fn stmt_line(s: &Stmt) -> Option<usize> {
    match s {
        Stmt::Let { init: Some(e), .. } => Some(e.line),
        Stmt::Let { .. } => None,
        Stmt::Destructure { init, .. } => Some(init.line),
        Stmt::Expr(e) | Stmt::BlockLike(e) => Some(e.line),
    }
}

fn id_of(g: &Gen, t: &Type) -> u32 {
    g.type_ids[t]
}

// `String::as_view(x)`: its argument.
// `Fx::snapshot_names`.
struct NameSnap {
    inert_c: HashSet<String>,
    obj_temps: HashSet<String>,
    objectless_c: HashSet<String>,
    moved_flag: HashMap<String, String>,
    direct_refs: HashSet<String>,
    confined: HashSet<String>,
    unstored: HashMap<String, String>,
    held_c: HashSet<String>,
    raw_sinks: HashSet<String>,
}

// D-0200: a fusable text loop's parts (`Gen::fusion_plan`): the text
// (the collection call's argument), the binder, the body (as the loop's
// block-like statement), whether the text is a `String` (lent) rather than
// a view, whether the pieces are characters rather than words, and the
// locals the body trims the piece into.
struct FusionPlan<'b> {
    source: &'b Expr,
    cname: String,
    be: &'b Expr,
    on_string: bool,
    chars: bool,
    derived: Vec<String>,
}

// The prelude's trims of a `StringView` (`native_view_trim`).
const TRIMS: &[&str] = &["std::StringView::trim", "std::StringView::trim_start", "std::StringView::trim_end"];

pub fn san(s: &str) -> String {
    let mut out: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    // CHG-0351: a CobaltC name C keeps (a field named `int`) gets one more
    // `_`, as does that name with `_`s after it, so that two names never
    // meet: `int` is `int_`, `int_` is `int__`.
    if C_RESERVED.contains(&out.trim_end_matches('_')) {
        out.push('_');
    }
    out
}

// The names C keeps that are CobaltC names: C's keywords (C17 and C23, and
// GCC's), the macros the headers the generated C includes define
// (`stdint.h`, `stddef.h`, `stdlib.h`, `string.h`, `math.h`, `setjmp.h`), and
// those GCC and Clang predefine in GNU mode. Only a name given to C bare
// meets them: a field's (a local, a function, a type have a prefix).
const C_RESERVED: &[&str] = &[
    "alignas", "alignof", "asm", "bool", "case", "char", "complex", "constexpr", "default", "do", "double", "false", "float",
    "goto", "imaginary", "inline", "int", "long", "noreturn", "nullptr", "register", "restrict", "short", "signed", "sizeof",
    "static_assert", "switch", "thread_local", "true", "typedef", "typeof", "typeof_unqual", "union", "unsigned", "volatile",
    "linux", "unix", "i386", "NULL", "EOF", "EXIT_FAILURE", "EXIT_SUCCESS", "RAND_MAX", "MB_CUR_MAX", "INFINITY", "NAN",
    "HUGE_VAL", "HUGE_VALF", "HUGE_VALL", "FP_NAN", "FP_INFINITE", "FP_ZERO", "FP_SUBNORMAL", "FP_NORMAL", "FP_ILOGB0",
    "FP_ILOGBNAN", "MATH_ERRNO", "MATH_ERREXCEPT", "math_errhandling", "M_E", "M_LOG2E", "M_LOG10E", "M_LN2", "M_LN10", "M_PI",
    "M_PI_2", "M_PI_4", "M_1_PI", "M_2_PI", "M_2_SQRTPI", "M_SQRT2", "M_SQRT1_2", "SIZE_MAX", "PTRDIFF_MIN", "PTRDIFF_MAX",
    "INTPTR_MIN", "INTPTR_MAX", "UINTPTR_MAX", "INTMAX_MIN", "INTMAX_MAX", "UINTMAX_MAX", "SIG_ATOMIC_MIN", "SIG_ATOMIC_MAX",
    "WCHAR_MIN", "WCHAR_MAX", "WINT_MIN", "WINT_MAX", "INT8_MIN", "INT8_MAX", "INT16_MIN", "INT16_MAX", "INT32_MIN",
    "INT32_MAX", "INT64_MIN", "INT64_MAX", "UINT8_MAX", "UINT16_MAX", "UINT32_MAX", "UINT64_MAX", "errno", "stdin", "stdout",
    "stderr", "offsetof", "assert",
];

// One lowered function (with the helpers emitted beside it): its C name,
// its text, and whether it is pure `std` -- a `std` function instantiated
// at `std` types only -- whose text is the same in every program: it keeps
// external linkage and gets no `#line` directives (`localize`,
// `line_directives`). (D-0277: there was an object cache of these.)
pub struct Unit {
    pub text: String,
    pub pure_std: bool,
}

// The generated C in parts (`Gen::generate`); `combined` is the single
// file `cobc --keep-c` writes and the no-cache path compiles.
pub struct Generated {
    pub fwd: String,
    pub types: String,
    pub strings: String,
    pub protos: String,
    pub thunks: String,
    pub descs: String,
    pub units: Vec<Unit>,
    pub tail: String,
    // D-0257: the C header of a library's entry points (`--lib`).
    pub header_text: String,
    // D-0281 step 4c: lowered pathless, or why not (when it was asked).
    pub pathless: Result<(), String>,
    // D-0285: lowered knowing the program is decided (`Gen::decided`).
    pub decided: bool,
}

impl Generated {
    // The single C file, `c_name` the name it is written under (for the
    // `#line` that returns to the C's own lines, `settle_lines`).
    pub fn combined(&self, c_name: &str) -> String {
        settle_lines(&self.combined_text(), c_name)
    }

    // D-0277: the program's C as `k` translation units, compiled at once
    // (`cc.rs`), each written under its own name (`settle_lines`). Every part
    // has the declarations. A function `localize` made `static` has external
    // linkage instead, since it may be called from another part, but for the
    // `static inline` ones, which every part defines; a static or addressed
    // constant (`g_…`, `k_…`) is defined in part 0 and declared in the rest;
    // a thunk is each part's own `static` copy, as the descriptors are. The
    // units' definitions are dealt out by size, each part keeping their
    // order, and the program's tail (`cb_program`, C's `main`) is part 0's.
    // With `k` 1, the combined text.
    pub fn parts(&self, k: usize) -> Vec<String> {
        if k <= 1 {
            return vec![self.combined_text()];
        }
        // The units' definitions: a definition's first line (at the start of
        // a line, ending in `)`, `{` after it but for `#line`s), the lines
        // before it since the last one ended, and its body to the `}` at the
        // start of a line; lines after a unit's last definition go with it.
        let mut defs: Vec<Vec<&str>> = Vec::new();
        for u in &self.units {
            let lines: Vec<&str> = u.text.split('\n').collect();
            let mut pending: Vec<&str> = Vec::new();
            let mut i = 0;
            let first = defs.len();
            while i < lines.len() {
                if starts_definition(&lines, i) {
                    let mut d = std::mem::take(&mut pending);
                    let mut j = i;
                    while j < lines.len() && lines[j] != "}" {
                        d.push(lines[j]);
                        j += 1;
                    }
                    if j < lines.len() {
                        d.push(lines[j]);
                    }
                    defs.push(d);
                    i = j + 1;
                } else {
                    pending.push(lines[i]);
                    i += 1;
                }
            }
            if !pending.is_empty() {
                match defs.len() > first {
                    true => defs.last_mut().unwrap().extend(pending),
                    false => defs.push(pending),
                }
            }
        }
        let head = |d: &[&str]| -> String { d.iter().find(|l| starts_signature(l)).map_or(String::new(), |l| l.to_string()) };
        let inline_names: std::collections::HashSet<String> = defs.iter().map(|d| head(d)).filter(|h| h.starts_with("static inline")).filter_map(|h| fn_name(&h).map(str::to_string)).collect();
        let mut everywhere: Vec<String> = Vec::new();
        let mut spread: Vec<String> = Vec::new();
        for d in &defs {
            let h = head(d);
            if h.starts_with("static inline") {
                everywhere.push(format!("{}\n", d.join("\n")));
                continue;
            }
            let text: Vec<String> = d.iter().map(|l| if *l == h && h.starts_with("static ") { h["static ".len()..].to_string() } else { l.to_string() }).collect();
            spread.push(format!("{}\n", text.join("\n")));
        }
        // Dealt out largest first to the smallest part, then each part in
        // the units' order.
        let mut order: Vec<usize> = (0..spread.len()).collect();
        order.sort_by(|&a, &b| spread[b].len().cmp(&spread[a].len()).then(a.cmp(&b)));
        let mut owner = vec![0usize; spread.len()];
        let mut sizes = vec![0usize; k];
        for i in order {
            let p = (0..k).min_by_key(|&p| (sizes[p], p)).unwrap();
            owner[i] = p;
            sizes[p] += spread[i].len();
        }
        let mut protos = String::with_capacity(self.protos.len());
        for l in self.protos.lines() {
            match l.strip_prefix("static ") {
                Some(rest) if !l.starts_with("static inline") && l.ends_with(");") && fn_name(l).map_or(false, |n| !inline_names.contains(n)) => protos.push_str(rest),
                _ => protos.push_str(l),
            }
            protos.push('\n');
        }
        let mut thunks = String::with_capacity(self.thunks.len());
        let tl: Vec<&str> = self.thunks.split('\n').collect();
        for (i, l) in tl.iter().enumerate() {
            if starts_definition(&tl, i) && !l.starts_with("static") {
                thunks.push_str("static ");
            }
            thunks.push_str(l);
            if i + 1 < tl.len() {
                thunks.push('\n');
            }
        }
        let mut out = Vec::new();
        for p in 0..k {
            let mut t = String::new();
            t.push_str(&self.prefix());
            t.push_str(HEADER);
            t.push_str("\n/* ---- types ---- */\n");
            t.push_str(&self.fwd);
            t.push_str(&self.types);
            t.push_str("\n/* ---- string literals ---- */\n");
            for l in self.strings.lines() {
                match global_decl(l) {
                    Some((ty, name)) if p > 0 => t.push_str(&format!("extern {} {};", ty, name)),
                    Some(_) => t.push_str(&l["static ".len()..]),
                    None => t.push_str(l),
                }
                t.push('\n');
            }
            t.push_str("\n/* ---- functions ---- */\n");
            t.push_str(&protos);
            t.push_str("\n/* ---- call thunks for fn values ---- */\n");
            t.push_str(&thunks);
            t.push_str("\n/* ---- type descriptors ---- */\n");
            t.push_str(&self.descs);
            for e in &everywhere {
                t.push_str(e);
            }
            for (i, d) in spread.iter().enumerate() {
                if owner[i] == p {
                    t.push_str(d);
                }
            }
            if p == 0 {
                t.push_str(&self.tail);
            }
            out.push(t);
        }
        out
    }

    // What comes before `cbrt.h`: its `CB_PATHLESS` switch.
    fn prefix(&self) -> String {
        match &self.pathless {
            Ok(()) if self.decided => "#define CB_PATHLESS 1\n#define CB_DECIDED 1\n".to_string(),
            Ok(()) => "#define CB_PATHLESS 1\n".to_string(),
            Err(why) if !why.is_empty() => format!("/* cobc: kept paths: {} */\n", why.replace("*/", "* /")),
            Err(_) => String::new(),
        }
    }

    fn combined_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.prefix());
        out.push_str(HEADER);
        out.push_str("\n/* ---- types ---- */\n");
        out.push_str(&self.fwd);
        out.push_str(&self.types);
        out.push_str("\n/* ---- string literals ---- */\n");
        out.push_str(&self.strings);
        out.push_str("\n/* ---- functions ---- */\n");
        out.push_str(&self.protos);
        out.push_str("\n/* ---- call thunks for fn values ---- */\n");
        out.push_str(&self.thunks);
        out.push_str("\n/* ---- type descriptors ---- */\n");
        out.push_str(&self.descs);
        for u in &self.units {
            out.push_str(&u.text);
        }
        out.push_str(&self.tail);
        out
    }
}

// Whether a type is made of `std` and built-in types alone, so that a
// `std` function instantiated at it is the same in every program.
pub fn std_only(t: &Type) -> bool {
    match t {
        Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Void | Type::Never => true,
        Type::Ref(i, _) | Type::Slice(i, _) | Type::Rawptr(i) | Type::Array(i, _) | Type::Handle(i) | Type::Mutex(i) | Type::Guard(i) => std_only(i),
        Type::Fn(ps, r) => ps.iter().all(std_only) && std_only(r),
        Type::Named(n, args) => n.starts_with("std::") && args.iter().all(std_only),
        _ => false,
    }
}

// FNV-1a, 32 bits: the stable id of a type (and the name of a string
// literal), the same in every program.
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

// WI-11: a `#line` before each line of the C of the program's own
// statements, naming the `.cb` line its `cb_at` gives, so that a C
// compiler, a debugger and a sanitizer report the program's lines. Each
// line needs its own: a directive numbers only the line after it, and
// the next one counts on. Where a function's code ends (a definition
// begins at column 0, or the unit ends), `LINE_RESET` returns to the
// C's own numbering; `settle_lines` gives it the line and the file once
// the translation unit is assembled. Run after every pass that reads
// the text line by line (`localize`), so none of them sees a directive.
const LINE_RESET: &str = "#line CB_RESET";
// A statement's location whose `cb_at` was dropped (`drop_dead_locations`).
const LINE_MARK: &str = "/*@line ";

// Each `LINE_RESET` as the directive that numbers the next line as what
// it is in the translation unit `c_name`.
pub fn settle_lines(text: &str, c_name: &str) -> String {
    if !text.contains(LINE_RESET) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 1024);
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if l.trim_end_matches('\n') == LINE_RESET {
            writeln!(out, "#line {} \"{}\"", i + 2, c_escape(c_name)).unwrap();
        } else {
            out.push_str(l);
        }
    }
    out
}

// D-0219: the marker a call site appends to its callee's name when an
// argument is an existing reference passed on (`Fx::vouched_args`).
const ARRIVE_MARK: &str = "__ARRIVE_MARK__";

// `parts`' reading of the generated C. A function's signature line: at the
// start of a line, not a directive or a brace, ending in `)`.
fn starts_signature(l: &str) -> bool {
    !l.is_empty() && !l.starts_with([' ', '{', '}', '#', '/']) && l.ends_with(')') && l.contains('(')
}

// A definition starts at line `i`: a signature with `{` after it, but for
// `#line` directives.
fn starts_definition(lines: &[&str], i: usize) -> bool {
    if !starts_signature(lines[i]) {
        return false;
    }
    let mut j = i + 1;
    while j < lines.len() && lines[j].starts_with("#line") {
        j += 1;
    }
    j < lines.len() && lines[j] == "{"
}

// The function a signature or prototype names: the word before its `(`, its
// attributes (`__attribute__((always_inline))`) left out.
fn fn_name(l: &str) -> Option<&str> {
    let mut rest = l;
    while let Some(i) = rest.find("__attribute__((") {
        let after = &rest[i..];
        let end = after.find("))").map_or(after.len(), |e| e + 2);
        rest = &rest[i + end..];
    }
    let open = rest.find('(')?;
    let head = rest[..open].trim_end();
    let start = head.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let name = &head[start..];
    (!name.is_empty()).then_some(name)
}

// A static's or an addressed constant's definition (`statics.rs`):
// `static [const ]T g_…/k_… = INIT;`, its type and name.
fn global_decl(l: &str) -> Option<(&str, &str)> {
    let rest = l.strip_prefix("static ")?;
    let eq = rest.find(" = ")?;
    let lhs = &rest[..eq];
    let sp = lhs.rfind(' ')?;
    let name = &lhs[sp + 1..];
    if !(name.starts_with("g_") || name.starts_with("k_")) || !l.ends_with(';') {
        return None;
    }
    Some((&lhs[..sp], name))
}
