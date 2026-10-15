# CobaltC Implementation Notes

Status: non-normative artifact (Master Instructions §14, §24;
`spec/02-schema.md` §6 — this file carries no rule, invariant, type, or
state entity and needs no version-bump discipline beyond its own
Change Log)
Governed by: `CobaltC_Master_Instructions.md` §20

## Purpose

Every choice a conforming implementation must make and document, every outcome the language permits
to vary between runs of *the same* implementation, and every condition the language does not check at
all but trusts the author of an `unsafe` block to have established, each with its governing rule. It
began as the Master Instructions §20 feasibility analysis, written before any implementation existed.
Both implementations now exist (`coby`, the reference interpreter, and `cobc`, the native compiler), and
§5 records the choices they made: one line per choice, the tool, and the place in the source. Nothing
here is normative: `spec/[0-2]*.md`, the registries and the conformance suite remain the sole normative
definition (`spec/README.md`).

Three kinds of freedom, kept separate because they place different
demands on an implementer and a user:

- **`outcome: impl-defined`** — the implementation commits to *one*
  choice for its lifetime and documents it (`spec/01` §5). A program's
  behavior may depend on it, and two conforming implementations may
  disagree, but one implementation is internally consistent.
- **`outcome: unspecified`** — the implementation, or even one run of
  it, may pick *differently* each time (`spec/01` §5); a safe program
  must not depend on which. Never something to "document a choice"
  for, only something to avoid building a program's correctness on.
- **`discharge: trusted`** — the language does not check this at all,
  anywhere, ever; an `unsafe` block's text is the only record that a
  human asserted it (`spec/20` §1). A reference interpreter that wants
  to help authors catch a false assertion (e.g., a debug-mode bounds
  check on `reclaim`) is adding *diagnostics*, not changing semantics —
  the specification permits this (`rule.control.flow-analysis`'s
  own "may report additional warnings" clause, generalized) but does
  not require or define it.

## 1. Implementation-defined choices

Every `outcome: impl-defined` in the specification, restated from
`spec/22` §5 with its governing rule label and what committing to a
choice actually constrains:

| Choice | Rule(s) | What it constrains |
|---|---|---|
| `AddrWidth ∈ {16,32,64,128}` | `spec/06` §1 (parameter), every rule tagged `impl-defined` over it | The size of every address, `isize`/`usize`, `ref<τ,m>`, `rawptr<τ>`, `fn(...):τ`, `handle<τ>`, `guard<τ>` (`[Sizeof-Addr]`, `[Sizeof-Handle]`). |
| Byte order `BO` | `[Repr-Byte-Order]` | The byte layout of every multi-byte `represent(τ,·)` image (`[Repr-Int]`, `[Repr-Float]`, `[Repr-Ref]`, `[Repr-Rawptr]`, `[Repr-Fn]`, `[Repr-Enum]`'s discriminant) — must agree on both sides of every `extern`/raw-pointer boundary (`spec/06` §7's "FFI/`reclaim` well-definedness"). |
| Enum discriminant width `DW ∈ {1,2,4,8}` | `[Layout-Enum]`, `[Repr-Enum]` | The size of every enum's discriminant prefix; interacts with `[Sizeof-Enum]`. |
| `dangling<T>()`'s address | `spec/21` §0 (documented; no dedicated rule label — it is an ordinary intrinsic whose result is `outcome: impl-defined`) | Any `T`-aligned non-null address; never dereferenced by prelude code, so this choice's only consumer is a program that stores and compares it. |
| `fn` value encoding | `[Repr-Fn]` | Any injective `AddrWidth/8`-byte image of an item path; only consumed by `==`/`!=` on `fn` values and by `spawn`'s callee argument. |
| A mutex's extra state cells | `type.mutex`, `[Repr-Mutex]` | `sizeof(usize)` bytes beyond `inner`, meaning left entirely to the implementation (e.g., a lock word or a pointer to one); no rule ever reads them, so this choice has zero semantic consequence beyond `sizeof(mutex<τ>)` itself. |
| A `handle<τ>`'s encoding | `[Repr-Handle]` | Any injective `AddrWidth/8`-byte image of a thread identity; read only by `rule.conc.join`, never by user code (`handle` is not an `FfiType`). |
| Termination-outcome/diagnostic reporting form | `rule.fn.program`'s `[Terminate-Ok]`/`[Fault-Unwind]`; `spec/22` §5 (not otherwise ruled) | Exit status, a printed message, a debugger stop — the language requires only that `ok` and each diagnostic `d` be distinguishable to the environment, never prescribes the form. Since `spec/22` 2.10.0 this includes how a diagnostic's location names the file it lies in (a program may span files, `rule.module.file`). |
| File-backed module paths beyond the required core | `rule.module.file` (`spec/17` §5), `spec/22` §5 | The required core — a `/`-separated relative path, no `..`, no leading `/`, resolved against the declaring file's directory — must be accepted; whether `..`, absolute paths, platform separators, or environment/home expansion are accepted, and how a resolved path denotes a file, is the implementation's. Conformance cases use only the core. |
| Calling convention, name mangling, linking | `rule.trust.extern-call` | Explicitly disclaimed ("outside this specification... a conforming implementation documents them"); needed to make any `extern fn` actually callable. `write` (`spec/21` §0, `CHG-0020`) is this row's first concrete instance — an implementation must bind it to a real OS write-to-descriptor primitive; no other prelude entry needed a real binding before it. |

## 2. Points of permitted nondeterminism (`outcome: unspecified`)

Every `outcome: unspecified` in the specification. Unlike §1, a
program must be correct under *every* choice below, on *every* run:

| Point | Rule(s) | What varies |
|---|---|---|
| A fresh object's storage address | `[Object-Establish-Resource]`, `[Object-Establish-Plain]` | Any fresh, aligned, disjoint range — seen by a safe program only through `rawptr_of` and `==` on raw pointers (`[Rawptr-Of]`, `[Eq-Rawptr]`), never through a reference (D-0233); observable only across an `unsafe` `rawptr_of`/`extern` boundary, where the specification's own trust conditions, not this freedom, are what a program must not depend on. |
| A zero-sized object's anchor | `[Object-Establish-Resource]`, `[Object-Establish-Plain]` (D-0217) | Any aligned address no other live object has as its anchor. `coby` numbers it as any object; `cobc` declares a zero-sized local as an empty C struct (a GNU C extension), and GCC and Clang place two such locals whose addresses are taken at different addresses — observed with GCC 11 and Clang 14, not promised by either; `conf.zero-sized-addresses` checks it under both, and a compiler that folded them would need `cobc` to give such a local one byte. A heap allocation of 0 bytes is not an object: `allocate(0, ·)` may return the same non-null address every time. |
| A fresh allocation's address | `[Allocate]` | Same freedom, for `allocate`'s raw range; also `disposition: fallible` — even `allocate(n, align)` for a size that has succeeded before is not guaranteed to succeed again (no minimum-availability guarantee is made). |
| Multi-thread interleaving | `[Thread-Step]` | Which live thread's next `→^ℓ` step is taken; `conf.cross-thread-write-conflict`'s own derivation is the conformance suite's example of a case whose diagnostic legitimately differs by interleaving (`outcome: unspecified { ok, diag.aliasing-conflict }`) without either outcome being wrong. |
| Padding/reserved-cell bytes | The byte view's `cont` (outside its group) and `pad` bytes (`spec/06` §7) | Any byte `0..255`; read only through a raw pointer or `extern`, and only as uninterpreted padding — no rule ever assigns padding a meaning, so this is unobservable to any program that does not itself go out of its way to read struct padding through `rawptr`. |

## 3. Trusted (unchecked) conditions

Every `discharge: trusted` side-condition — an operation's may occur
only lexically inside an `unsafe { }` block (`rule.trust.unsafe`
`[Unsafe-Rejected]`); `[Extern-Decl]`'s is made by the `unsafe` before a
declaration's `extern` (D-0146). A reference interpreter
enforces none of these; each is exactly what a human author is
asserting by writing the surrounding `unsafe` block, and each is
listed in `spec/AUDIT-STATUS.md`'s seventh pass, per call site in
`spec/21`, with why it holds *there*. Restated here by rule, once each,
independent of any particular call site:

| Rule | Trusted condition (what the author asserts) |
|---|---|
| `[Rawptr-Offset]` | The result address lies within, or one past the end of, one allocation or one object's extent. |
| `[Rawptr-Read]` | The cells hold a valid image of `represent(τ,·)`; no live path of *any* object reaches them in exclusive mode; no other thread writes them concurrently. |
| `[Rawptr-Write]` | The cells are in bounds; no live path of *any* object reaches them (any mode); no other thread accesses them concurrently. (`CHG-0019`: both this and `[Rawptr-Read]`'s conditions previously exempted the cells' own already-reclaimed object, which was unsound for one reachable library access pattern — `spec/AUDIT-STATUS.md` finding F-05, closed by making `Vec::pop` release its slot for every element type.) |
| `[Rawptr-Move-In]` | The cells are in bounds and are not part of *any* live object (`[Rawptr-Read]`/`[Rawptr-Write]` are now equally strict, since `CHG-0019`). |
| `[Rawptr-Move-Out]` | The cells hold a valid `τ` no live fresh-storage object claims, and (if newly reclaimed) whose destroy obligation is not tracked anywhere else. |
| `[Reclaim]` | The cells hold a valid `τ` no live fresh-storage object claims (a reclaimed object already there is re-attached, not re-asserted), and, if `τ` is a resource, its destroy obligation is not tracked anywhere else. |
| `[Release]` | Every outstanding obligation of a reclaimed object in the released range has been discharged, or its cells have been copied (`copy_raw`) to storage a later `[Reclaim]`/`[Rawptr-Move-Out]` will re-establish it from. |
| `[Copy-Raw]` | Both ranges are in bounds; no live object overlaps the destination. |
| `[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]` | As `[Rawptr-Read]` and `[Rawptr-Write]`, for a plain `τ`; the access is made exactly once, in program order (D-0121). |
| `[Deallocate]` | `(p, n, align)` were returned by one `allocate`, not yet deallocated. |
| `[Extern-Decl]` | The linked code has a function of that name whose parameters and result are exactly the declared `FfiType`s, in order (D-0146). |
| `[Extern-Call]` | The callee honors the declared `FfiType` signature; it reads/writes only cells reachable from the `rawptr` arguments and its own allocations; its returned image is a valid value of the declared return type. |

A **safe client** (`term.safe-client`, `spec/00`) contains no
`unsafe { }` block of its own. Every row above is reachable only
through `spec/21`'s own library bodies — the trusted library base
(`term.trusted-library-base`), each of whose blocks `spec/21` §0 lists
with the fact that discharges it — or through a user's own `unsafe`
code, never through ordinary safe-language constructs. The guarantee
(`term.safe-program`) is unconditional for a safe client over a
conforming implementation, because keeping the trusted library base's
claims true is that implementation's conformance obligation (D-0206).

## 4. What this analysis found, and what it did not

Five verification passes (`spec/AUDIT.md`, `spec/AUDIT-2.md`,
`CHG-0009`–`CHG-0011`) and the sixth through twelfth passes recorded in
`spec/AUDIT-STATUS.md` derived every conformance case, four whole
end-to-end programs plus six more, and every `spec/21` library
function statement by statement, against the rules as written. No
two-implementation divergence was found anywhere *except* the two
points a conforming implementation must commit to per §1 and the three
points it must not depend on per §2 — i.e., every other decision an
independent implementer would need to make is already determined by
the normative artifacts, which is `Master Instructions` §26's success
criterion. Two defects were found and fixed during this analysis: a
cross-thread destroy-authority gap (`CHG-0015`) and a raw-pointer
trust-boundary soundness hole in `Vec::pop`/`Vec::push` (F-05,
`CHG-0019`, §3 above) — both closed, neither left open.

**Recommendation for the next stage.** The specification is, by this
analysis, sufficiently complete to authorize a reference
implementation: nothing outstanding is a known soundness gap. As of
`CHG-0022`/`CHG-0023`, `spec/registry/features.md` has zero `feat.*`
entries left `UNDER_INVESTIGATION` — the two that remained were
decided `REJECTED` (neither adds a capability the language does not
yet have), and `spec/AUDIT-2.md` B-26 is separately closed
(`CHG-0024`). Nothing in the registries, the decision log, or the
audit-closure log is open except the reference-implementation
authorization itself, which this recommendation remains advisory to;
Master Instructions §1 reserves that authorization to the human owner.

**The executable reference checker** (D-0206). `coby --check` accepts
exactly the programs `rule.control.flow-analysis` and the static rules
accept: a program it accepts is accepted by the language, one it
rejects is rejected, with the diagnostic the registry names. `cobc`
shares its front end, so the two cannot disagree; a third
implementation is measured against `coby` on any program, not only on
the conformance cases, and may discharge more checks than `coby` does
but not accept or reject differently (`spec/14` §6).

**Reads through a shared path** (D-0218). `[Read-Ref-Through-Shared]`
forms a shared borrow of the stored reference. Both tools form it where
the reference is read out as a value: the front end rewrites `s.inner`
to `&*s.inner` (a slice `s.part` to `&s.part[0..$]`) and checks the
program again, so `coby` and `cobc` see an ordinary borrow. Where the
reference is only dereferenced (`*s.inner`), they access through the
stored path itself; the outcome is the same, since a read through the
borrow and through the stored path meet the same competitors (the borrow
and the path are each other's ancestors) and `[Thread-Affine]` is tested
on the stored path either way. `[Read-Fn-Through-Shared]` is checked by
each tool where the value is read or called, from the static pass's
record of those places: `coby` looks for an exclusive reference among
the closure's capture values, `cobc` for an exclusive path among the
reference slots of the closure's box (`cb_fn_shared`).

## 5. The choices `coby` and `cobc` made

Each `outcome: impl-defined` of §1, as the two implementations commit to it. They make the same choice
everywhere, so a program behaves alike under both.

| Choice | `coby` and `cobc` | Where |
|---|---|---|
| `AddrWidth` | 64 | `coby`: `ADDR_WIDTH`, `src/value.rs`; `cobc`: `size_align`, `cobc/src/lower/layout.rs` (x86_64, both targets) |
| Byte order | little-endian | `coby`: `LITTLE_ENDIAN`, `src/value.rs`; `cobc`: the x86_64 targets |
| Targets (D-0255) | `cobc`: `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` (`--target`), the same `AddrWidth`, byte order and layouts on both; Windows programs are compiled by MinGW-w64's GCC and linked with a runtime built by Rust 1.77.2, which runs on Windows 7. `coby` runs on both, its C calls on Linux only | `cobc/src/target.rs`, `cbrt/src/link_info.rs` |
| Struct field offsets | declaration order, each field at the next multiple of its alignment, the size rounded up to the struct's alignment | `coby`: `struct_layout`, `src/interp/layout.rs`; `cobc`: `size_align`, `cobc/src/lower/layout.rs`, checked in the C by `_Static_assert` |
| Enum discriminant width | 4 bytes (`u32`) | `coby`: `ENUM_DISCRIMINANT_WIDTH`, `src/value.rs`; `cobc`: the `uint32_t tag` of each enum's C struct, `cobc/src/lower/layout.rs` |
| `dangling<T>()` | the address `alignof<T>()`: `T`-aligned, not null, and below every address either tool allocates | `coby`: `try_intrinsic`, `src/interp/intrinsics.rs`; `cobc`: the `dangling` arm, `cobc/src/lower/intrinsics.rs` |
| `fn` value image | `coby`: a hash of the item's qualified name; `cobc`: the C function's address | `coby`: `encode`, `src/interp/extern_calls.rs`; `cobc`: `ctype` (`void *`), `cobc/src/lower/layout.rs` |
| A mutex's extra cells | one `usize` after `inner`, which no rule reads; `coby` leaves it zero, and `cbrt` keeps the lock state in a table keyed by the mutex's address, not in the cell | `coby`: `encode`, `src/interp/extern_calls.rs`; `cobc`: `ctype` for `mutex`, `cobc/src/lower/layout.rs`; `cbrt`: `locks`, `cbrt/src/lib.rs` |
| A `handle<τ>`'s image | the thread's id, a `u64` | `cobc`: `ctype`, `cobc/src/lower/layout.rs`; `coby`: an opaque token in its arena |
| Reporting form | the diagnostic on standard error (`error[diag.x]: …` before a run, `fault[diag.x]: …` during one, with the location), exit status 0 for `ok` or `main`'s `u8`, 1 for a diagnostic, 2 for a usage error, 3 for `unsupported:`, 4 for an internal error | `src/diagnostics.rs`, `src/internal.rs`, both tools' `main` |
| File-backed module paths beyond the core | accepted: `..`, an absolute path, the platform's separators; resolved against the declaring file's directory and canonicalized | `src/loader.rs` (shared) |
| Calling convention | `cobc`: the C compiler's, for x86-64 System V on Linux and the Microsoft x64 convention on Windows (D-0255); `coby`: x86-64 System V on Linux, other platforms refused (`unsupported:`) | `cobc`: the generated C; `coby`: `src/ffi.rs` |
| Libraries (D-0257) | `cobc --lib -o libx.a x.cb`: a program of `extern fn` definitions and no `main` becomes a static archive of its objects (and its own C and assembly files'), with the runtime `libcbrt.a` copied beside it (linked once, however many CobaltC libraries a program has) and a header `x.h`: each ffi-struct as C declares it with its layout asserted, `cobalt_init`/`cobalt_shutdown`/`cobalt_last_fault`, and each entry point as `τr name(…)` and `int name_try(…, τr *out)`. The runtime starts on the first call, sets no signal disposition and no console mode; the first thread to enter is its main thread, every other C thread is registered on its first call and released when it ends. A fault in `name` reports and ends the process; in `name_try` the call's frames unwind, the report is kept for `cobalt_last_fault`, and 1 comes back with `*out` untouched. The library's standard output is written out as each call returns; a C caller that buffers its own flushes it, or switches buffering off, to keep the order. `coby`: a library checks and runs its `unittest` blocks; it has nothing to run (`unsupported:`) | `cobc/src/main.rs` (`archive`, `write_header`), `cobc/src/lower/header.rs`, `cobc/src/lower/program.rs`; `cbrt/src/lib.rs` (`cb_lib_*`) |
| Self-checks (D-0261) | `COBALTC_VERIFY=N`: `coby` re-derives, at every Nth alias check, the references to the object from a scan of every live object and compares them with its holder index, and at every Nth statement's end checks the index's completeness, the mutexes against the guards and the single ownership of every live object; `cbrt` re-derives, at every Nth statement's end, the agreement of its path and object tables (a valid path's object lists it, a listed token is that object's path, the exclusive lists hold the non-root exclusive paths, every ancestors list is closed). Either tool checks no more often than its state's size allows (once per 64 entries' worth of events). Neither changes a program's behaviour; a violation is the tool's defect and panics naming it | `coby`: `src/interp/verify.rs`; `cbrt`: `verify_state`, `cbrt/src/lib.rs` |

Each `outcome: unspecified` of §2 is chosen afresh as the rule allows. Addresses come from each tool's
allocator (`coby`'s arena from `0x1000` upwards; `cobc` through `cbrt`, the C library's allocator). The
interleaving is the operating system's, or a seeded schedule under `--schedule-seed` and `--explore`.

## 6. The runtime's contract (D-0263)

What the generated C may ask of `cbrt`, declaration by declaration, as `impl/cbrt/include/cbrt.h`
states it: the rule steps a function performs (its `[Rule]` labels) and what it assumes of its caller.
The table is generated from the header by `impl/tools/runtime_contract.py`, which the battery runs with
`--check`: the header is the contract's home, and every declaration of a group that performs rule steps
names a rule or is marked internal.

<!-- runtime-contract:begin -->

| Function | Group | Contract (from `cbrt.h`) | Rules |
|---|---|---|---|
| `cb_fn_item` | types and helpers | A `fn` value: an 8-byte handle to a callable box — one interned box per fn item, or a heap box owning a moved-in closure object, called through an exclusive borrow of that object ([Closure-Call]). | `[Closure-Call]` |
| `cb_fn_item_pure` | types and helpers | a fn item whose reference parameters are all pure direct (D-0191) | — |
| `cb_fn_pure` | types and helpers | whether h is such a fn item or closure | — |
| `cb_fn_mark_pure` | types and helpers | a closure code that keeps nothing of its arguments (D-0192) | — |
| `cb_fn_box` | types and helpers | — | — |
| `cb_borrow_unstamped` | types and helpers | a borrow in no statement scope, ended by cb_call_done | — |
| `cb_call_self` | types and helpers | a closure call's `self`: an unstamped exclusive borrow | — |
| `cb_call_done` | types and helpers | ... ended after the call when nothing holds it | — |
| `cb_fn_copy` | types and helpers | [Read] of a fn value: clones a closure box | `[Read]` |
| `cb_fn_read` | types and helpers | D-0089: copy, or move out when the closure owns a resource | — |
| `cb_fn_split` | types and helpers | D-0089: the same for each fn slot of a value | — |
| `cb_fn_moved` | types and helpers | D-0089: a call through an emptied fn slot | — |
| `cb_drop_value` | types and helpers | D-0089: a plain temporary holding fn values ends | — |
| `cb_fn_shared` | types and helpers | D-0218: a fn value read through a shared reference holds no exclusive capture | — |
| `cb_fn_shared_value` | types and helpers | ... the same for each fn slot of a value | — |
| `cb_end_binding` | program | D-0111: a reference binding ends at its last use — [Ref-Binding-Last-Use] (D-0111): a reference binding's object and path end at its last use | `[Ref-Binding-Last-Use]` |
| `cb_init` | program | [Program]: Σ_0 — the files, the type descriptors and the static objects; internal to the start-up text | `[Program]` |
| `cb_fault` | program | [Fault-Unwind]: the diagnostic is raised at file:line; every open scope and frame of this thread is unwound ([Stmt-Exit], [Block-Exit], destructors run), then the report is written and the program ends with status 1; [Fault-Contain] in a supervised thread | `[Fault-Unwind]`, `[Stmt-Exit]`, `[Block-Exit]`, `[Fault-Contain]` |
| `cb_exit` | program | std::exit (D-0150): unwinds this thread, ends with status — `exit` (D-0150): unwinds this thread as [Fault-Unwind] does, reporting nothing, and ends with the status | `[Fault-Unwind]` |
| `cb_fault_msg` | program | assert (D-0065) — [Fault-Unwind] with a message (assert, D-0065) | `[Fault-Unwind]` |
| `cb_terminate_ok` | program | ok(s): main's value, or 0 — [Program]'s end: every frame has exited; the output is written out and the process ends with main's value | `[Program]` |
| `cb_set_args` | program | the program's arguments (rule.stdlib.args) — [Arg-Bytes]'s source: the program's arguments (rule.stdlib.args) | `[Arg-Bytes]` |
| `cb_frame_here` | program | D-0033 (1): a binding's frame, for cb_rebind — internal: the current frame's id, for cb_rebind | — |
| `cb_rebind` | program | [Let]'s re-initialisation of a moved-from binding (D-0033 (1)): a new object at `addr`, bound in the binding's own frame | `[Let]` |
| `cb_at` (inline) | program | internal: the statement now executing, for faults raised without a location | — |
| `cb_where` | program | defined by the generated program — internal: reads cb_at's location (defined by the generated program) | — |
| `cb_set_where` | program | likewise — internal: sets it | — |
| `cb_stack_floor` (variable) | program | internal: this thread's stack floor (D-0077) | — |
| `cb_set_stack_floor` | program | defined by the generated program — internal: set by the runtime when the thread starts | — |
| `cb_stack_exhausted` | program | [Call]'s stack limit (D-0077): `diag.stack-exhausted` raised as [Fault-Unwind] | `[Call]`, `[Fault-Unwind]` |
| `cb_run_main` | program | runs the program on a thread with a large stack — [Program]: runs main on a thread with a large stack (ℓ_0) | `[Program]` |
| `cb_lib_enter` | program | [Program] for a library (D-0257): starts the runtime once and registers the calling C thread | `[Program]` |
| `cb_lib_leave` | program | internal: writes out what the call printed | — |
| `cb_lib_shutdown` | program | internal: cobalt_shutdown | — |
| `cb_lib_jmp` | program | [Fault-Contain] for a library entry's `_try` form (D-0257): the stub's jump buffer | `[Fault-Contain]` |
| `cb_lib_fault_code` | program | internal: the contained fault's status, after the jump | — |
| `cb_lib_last_fault` | program | internal: cobalt_last_fault's report | — |
| `cb_stack_check` (inline) | program | [Call]'s stack-limit check at a function's entry (D-0077): below the floor is `diag.stack-exhausted` | `[Call]` |
| `cb_write_out` | standard library externs | the `stdout_write` extern | — |
| `cb_write_err` | standard library externs | `std`'s private `stderr_write` (eprintf) | — |
| `cb_flush_out` | standard library externs | `std`'s private `stdout_flush` (D-0178) | — |
| `cb_read_in` | standard library externs | the `stdin_read` extern | — |
| `cb_read_line_in` | standard library externs | `std`'s private `stdin_read_line` (read_line) | — |
| `cb_arg_bytes` | standard library externs | the `arg_bytes` extern | — |
| `cb_print_int` | standard library externs | `print` (rule.stdlib.print) | — |
| `cb_print_f64` | standard library externs | — | — |
| `cb_print_f32` | standard library externs | — | — |
| `cb_print_bool` | standard library externs | — | — |
| `cb_text_int` | standard library externs | `String::append` (rule.stdlib.text) | — |
| `cb_text_f64` | standard library externs | — | — |
| `cb_text_f32` | standard library externs | — | — |
| `cb_text_bool` | standard library externs | — | — |
| `cb_parse_int` | standard library externs | `parse` | — |
| `cb_parse_float` | standard library externs | — | — |
| `cb_parse_bool` | standard library externs | D-0181 | — |
| `cb_format` | standard library externs | `$fmt` (rule.stdlib.format) | — |
| `cb_file_read` | standard library externs | `read_file` (rule.stdlib.file) | — |
| `cb_file_write` | standard library externs | — | — |
| `cb_file_op` | standard library externs | `File` (rule.stdlib.file-handle) | — |
| `cb_file_at` | standard library externs | `File::seek`, `File::len` | — |
| `cb_fs_op` | standard library externs | filesystem (D-0061) | — |
| `cb_fs_query` | standard library externs | — | — |
| `cb_clock_read` | standard library externs | — | — |
| `cb_tz_offset` | standard library externs | local time (D-0147); `unix` is a C macro | — |
| `cb_sleep_ns` | standard library externs | D-0124 | — |
| `cb_path_op` | standard library externs | paths (D-0141) | — |
| `cb_proc_op` | standard library externs | processes (D-0143) | — |
| `cb_net_op` | standard library externs | networking (D-0144) | — |
| `cb_os_random` | standard library externs | D-0142 | — |
| `cb_popcount64` (inline) | bit operations | — | — |
| `cb_clz` (inline) | bit operations | — | — |
| `cb_ctz` (inline) | bit operations | — | — |
| `cb_rotl` (inline) | bit operations | — | — |
| `cb_rotr` (inline) | bit operations | — | — |
| `cb_popcount128` (inline) | bit operations | — | — |
| `cb_clz128` (inline) | bit operations | — | — |
| `cb_ctz128` (inline) | bit operations | — | — |
| `cb_rotl128` (inline) | bit operations | — | — |
| `cb_rotr128` (inline) | bit operations | — | — |
| `cb_event_op` | bit operations | channels' event counts (D-0063) | — |
| `cb_bytes_less` | bit operations | key_less on text (D-0062) | — |
| `cb_bytes_lt` (inline) | bit operations | The same, inline, for a native sort's inner loop: byte order, a prefix first. | — |
| `cb_sort_plain` | bit operations | Vec::sort of integers or bools (D-0062) | — |
| `cb_round` (inline) | bit operations | `round`, half away from zero, from `trunc` (exact: the dropped fraction is representable), not the C library's: MinGW's over msvcrt gives 1 for 0.49999999999999994 (D-0255). coby's `round_f64` is the same. | — |
| `cb_roundf` (inline) | bit operations | — | — |
| `cb_index_check` (inline) | bit operations | — | — |
| `cb_frame_push` | frames and statement scopes | [Block-Enter], [Call]: pushes a frame (state.frame-stack); its pop ends what it owns | `[Block-Enter]`, `[Call]` |
| `cb_frame_pop` | frames and statement scopes | [Block-Exit]: ends the frame's owned objects last bound first, [Destroy] for a resource (its destructor, [Run-Destructor], [Destroy-Composite]), [Object-End] for the rest; drops the frame's holds | `[Block-Exit]`, `[Destroy]`, `[Run-Destructor]`, `[Destroy-Composite]`, `[Object-End]` |
| `cb_frame_push_at` | frames and statement scopes | [Block-Enter] with the block's location, for a destruction fault | `[Block-Enter]` |
| `cb_stmt_push` | frames and statement scopes | [Stmt-Enter]: a statement scope for this statement's temporaries and unheld paths | `[Stmt-Enter]` |
| `cb_stmt_push_at` | frames and statement scopes | [Stmt-Enter] with the statement's location | `[Stmt-Enter]` |
| `cb_stmt_pop` | frames and statement scopes | [Stmt-Exit]: ends the scope's temporaries ([Destroy]/[Object-End]) and its unheld paths; the result was re-stamped out by cb_result | `[Stmt-Exit]`, `[Destroy]`, `[Object-End]` |
| `cb_new` | objects | a temporary of this statement — [Object-Establish-Plain]/[Object-Establish-Resource] over `addr` (storage the C provides, not the runtime's) with init valid, a temporary of the current statement; assumes `addr` holds a value of `ty` | `[Object-Establish-Plain]`, `[Object-Establish-Resource]` |
| `cb_new_uninit` | objects | [Let-Uninit] — [Let-Uninit]: [Object-Establish-*] with init uninitialized; assumes a whole-object [Write] before any read (definite assignment) | `[Let-Uninit]`, `[Write]` |
| `cb_bind` | objects | adopt into the current frame; the root token — [Binding-Form] and [Store-Binding-Temp]: the current frame adopts the temporary (state.holder); the root path's token is returned for the binding's accesses | `[Binding-Form]`, `[Store-Binding-Temp]` |
| `cb_move_to` | objects | the same object, now at this address — internal: the same object, its cells now at `addr` (the C moved the bytes; the record follows). Assumes no path's target was derived from the old address | — |
| `cb_temp_path` | objects | an owner-like path to a temporary (calling a closure temporary) — [Temp-Root]: an owner-like root path to a temporary (a closure temporary called) | `[Temp-Root]` |
| `cb_take` | objects | [Authority-Transfer] out of a binding — [Store-Binding-Place-Transfer]/[Authority-Transfer] out of a binding: solitary and authority checked; the root path ends, the object is in flight | `[Authority-Transfer]`, `[Store-Binding-Place-Transfer]` |
| `cb_absorb` | objects | [Relocate-In]: now part of a container — [Relocate-In]: the temporary's contents are now a sub-resource of a container; its record ends without destruction, its obligations move | `[Relocate-In]` |
| `cb_absorb_refs` | objects | the same for plain data holding references, moved in first: its slots stay — [Relocate-In] for plain data holding references, moved in first: its slots' holds stay | `[Relocate-In]` |
| `cb_result` | objects | a statement's result: re-stamped outward — [Stmt-Exit]'s re-stamp: the statement's result object belongs to the parent scope | `[Stmt-Exit]` |
| `cb_destroy` | objects | drop(x) — [Destroy] through the binding's root path: temporally valid, solitary, authority held; [Run-Destructor], [Destroy-Composite], [Object-End] | `[Destroy]`, `[Run-Destructor]`, `[Destroy-Composite]`, `[Object-End]` |
| `cb_destroy_via` | objects | drop(*r): through a reference — [Destroy] through a reference (drop(*r)): the same premises on the referent | `[Destroy]` |
| `cb_consume` | objects | destroy a temporary now — [Destroy] of a temporary now, as [Stmt-Exit] would at the statement's end | `[Destroy]`, `[Stmt-Exit]` |
| `cb_end_moved_out` | objects | end a temporary whose contents relocated out — [Relocate-Out]'s source after a destructuring: the container ends ([Object-End]) without destroying the parts that moved | `[Relocate-Out]`, `[Object-End]` |
| `cb_borrow` | access paths | [Borrow] from `base` through the projections `p` ([Field-Access], [Index-Checked] steps): temporally valid, mode no stronger than the source, ¬clash, [Thread-Affine]; mints the path, held by this statement until stored | `[Borrow]`, `[Field-Access]`, `[Index-Checked]`, `[Thread-Affine]` |
| `cb_borrow_range` | access paths | a slice's range (D-0070) — [Slice-Form]: [Borrow] whose last step is the range lo..hi (D-0070) | `[Slice-Form]`, `[Borrow]` |
| `cb_read` | access paths | [Read]'s premises on the place: temporally valid, ¬clash(shared), [Thread-Affine]; the C reads the value itself | `[Read]`, `[Thread-Affine]` |
| `cb_write` | access paths | [Write]'s premises: temporally valid, mode exclusive, ¬clash(exclusive), ¬live-resource-at when `target_resource`; the C stores the value itself | `[Write]` |
| `cb_write_quiet` | access paths | D-0194: a quiet place's old value destroyed by the write — [Write-Quiet-Replace] (D-0194): the old quiet value at `addr` is destroyed ([Destroy]) and the place written | `[Write-Quiet-Replace]`, `[Destroy]` |
| `cb_owns` | access paths | D-0049: the value there owns a resource now — `owns(τ, v)` of [Write]'s live-resource-at (D-0049): whether the value at `addr` owns a resource now | `[Write]` |
| `cb_store_ref` | reference data | [Write] of a reference value: the slot's holder gains the path (held-by), keyed by the slot's address | `[Write]` |
| `cb_load_ref` | reference data | [Read] of a reference value: the token the slot holds | `[Read]` |
| `cb_valid` | reference data | [Swap-Places]: temporally valid, no conflict check | `[Swap-Places]` |
| `cb_copy_datum` | reference data | [Read] then [Write] of a value holding references: the bytes and each slot's hold are copied | `[Read]`, `[Write]` |
| `cb_lend` | reference data | D-0213 — [Thread-Lend]: every exclusive path in the value at `addr` is lent (transit) to the thread that receives it | `[Thread-Lend]` |
| `cb_lend_fn` | reference data | D-0213 — [Thread-Lend] for a closure's captures | `[Thread-Lend]` |
| `cb_find_bytes` (inline) | text and hashing helpers | The first position of the `nn` bytes at `n` in the `hn` bytes at `h`, or UINT64_MAX (`StringView::find`'s answer natively; an empty needle is found at 0). Pure. | — |
| `cb_char_boundary` (inline) | text and hashing helpers | `StringView::boundary`: `i` at either end, or on a byte that does not continue a UTF-8 sequence. Pure. | — |
| `cb_reclaim` | raw storage | [Reclaim]: the object's root token | `[Reclaim]` |
| `cb_elem_borrow` | raw storage | [Reclaim] of the element's cells and [Borrow] of it (Vec::index_*): a path over raw storage | `[Reclaim]`, `[Borrow]` |
| `cb_elem_borrow_here` | raw storage | the same, recorded in the current scope (a `…_pre` body) — [Reclaim] and [Borrow] of an element, the path recorded in the current scope (a `…_pre` body) | `[Reclaim]`, `[Borrow]` |
| `cb_deallocate_plain` | raw storage | cells that never held a reference — [Deallocate] of cells that never held a reference: no [Release] of reference holders is needed | `[Deallocate]`, `[Release]` |
| `cb_elem_access_slow` | raw storage | [Read]/[Write] premises on an element over raw cells where an object lies (otherwise no object, no path, no check) | `[Read]`, `[Write]` |
| `cb_reclaimed_entries` (variable) | raw storage | entries over raw cells (an AtomicUsize); 0: no object over any element — internal: the count of reclaimed objects, the fast path's gate (0: no element has an object) | — |
| `cb_live_threads` (variable) | raw storage | threads running, the main one included (an AtomicU64); 1: this thread is alone (D-0209) — internal: the running threads, the gate of [Thread-Affine] checks (1: alone) | `[Thread-Affine]` |
| `cb_sched_flag` (variable) | raw storage | 1 under a schedule (`--explore`, `--schedule-seed`); an AtomicU8 — internal: whether a schedule runs the threads ([Thread-Step] under --explore) | `[Thread-Step]` |
| `cb_atomic_point` | raw storage | D-0249: an atomic operation, a point where a schedule may pass the turn — [Thread-Step] under a schedule: a point where the turn may pass | `[Thread-Step]` |
| `cb_atomic_wait` | raw storage | D-0253: an atomic's wake count: read, wait, notify — [Atomic-Wake-Read] (op 0), [Atomic-Wait] (1), [Atomic-Notify] (2, 3) on the atomic at `addr` | `[Atomic-Wake-Read]`, `[Atomic-Wait]`, `[Atomic-Notify]` |
| `cb_spin_hint` | raw storage | D-0253: the processor's pause — [Spin-Hint] | `[Spin-Hint]` |
| `cb_reclaimed_pages` (variable) | raw storage | the same by page, `(a >> 12) & 4095` (AtomicU32s) — internal: cb_reclaimed_entries by page, the gate of cb_reclaimed_near | — |
| `cb_reclaimed_near` (inline) | raw storage | internal: whether a reclaimed object may lie over the cells | — |
| `cb_elem_access` (inline) | raw storage | [Read]/[Write] on an element: checked (cb_elem_access_slow) only where an object may lie over it; otherwise no object exists there and no rule applies | `[Read]`, `[Write]` |
| `cb_drop_local` | raw storage | [Destroy]/[Object-End] of an objectless local's value (D-0200): the value is destroyed as its binding's end would | `[Destroy]`, `[Object-End]` |
| `cb_drop_plain_buf` (inline) | raw storage | [Deallocate] of an objectless String's or Vec's buffer at once when no object lies over it, else cb_drop_local | `[Deallocate]` |
| `cb_elem_access_if` (inline) | raw storage | [Read]/[Write] on an element of a direct slice: checked where cb_live_in found an object | `[Read]`, `[Write]` |
| `cb_borrow_range_unminted` | raw storage | `&e[lo..hi]` for a pure direct slice: the checks alone — [Slice-Form]'s premises without minting a path: a direct parameter's slice (D-0209) | `[Slice-Form]` |
| `cb_borrow_unminted` | raw storage | `&p` to a pure direct parameter: the check alone — [Borrow]'s premises without minting: `&p` to a pure direct parameter (D-0191) | `[Borrow]` |
| `cb_borrow_check` | raw storage | `&v` straight into Vec::index_*: checks only Vec::index_*: [Reclaim] + [Borrow] + send (spec/21 §0) — [Borrow]'s premises without minting: `&v` straight into Vec::index_* | `[Reclaim]`, `[Borrow]` |
| `cb_vec_drop_plain` | raw storage | Vec::drop element loop, plain T (spec/21 §0) — [Object-End] of every element object over the buffer, as Vec::drop's loop does for plain elements; [Deallocate] follows | `[Object-End]`, `[Deallocate]` |
| `cb_raw_move_in` | raw storage | `*p = v`, v a resource: [Rawptr-Move-In] | `[Rawptr-Move-In]` |
| `cb_live_at` | raw storage | internal: whether a live object lies at `addr` (a native body's gate) | — |
| `cb_live_in` | raw storage | a live object over any of n cells: a native body takes its general path — internal: whether a live object lies over any of the n cells | — |
| `cb_frozen_ok` | raw storage | CHG-0237: a frozen parameter's body may run — [Read]'s premises for a frozen parameter's whole body (D-0205, CHG-0237): valid, initialized, not lock-derived, no exclusive path over it | `[Read]` |
| `cb_live_in2` | raw storage | cb_live_in of two runs of n cells — internal: cb_live_in of two runs | — |
| `cb_elem_borrow_datum` | raw storage | Some(&elem) returned as reference data — [Reclaim] and [Borrow] of an element returned as reference data (Some(&elem)) | `[Reclaim]`, `[Borrow]` |
| `cb_peek_datum_tok` | raw storage | dual-body dispatch: the k-th of n data's token, left in flight — internal: dual-body dispatch (D-0188): the k-th in-flight datum's token | — |
| `cb_paths_disjoint` | raw storage | dual-body dispatch: valid and pairwise disjoint — internal: dual-body dispatch: the arguments' paths valid and pairwise disjoint, so the direct body may run | — |
| `cb_paths_arrive` | raw storage | D-0219: and each clash-free as it arrives — [Borrow]'s ¬clash for each argument as it arrives (D-0219), beside cb_paths_disjoint | `[Borrow]` |
| `cb_direct_ok` | raw storage | D-0209: direct parameters' paths valid, over fresh storage — [Read]/[Write] premises for direct parameters' paths (D-0209): valid, over fresh storage | `[Read]`, `[Write]` |
| `cb_send_tok_datum` | raw storage | a path formed in a native body, sent as one-reference data — internal: a path a native body formed, sent as one-reference data | — |
| `cb_send_datum_none` | raw storage | None returned as reference data — internal: None returned as reference data | — |
| `cb_frame_hold` | raw storage | a reference local with no object: the frame holds its path — a reference local without an object: the frame holds its path (an occurrence) until [Block-Exit] or [Ref-Binding-Last-Use] (D-0111) | `[Block-Exit]`, `[Ref-Binding-Last-Use]` |
| `cb_frame_unhold` | raw storage | … until it ends early (D-0111) a live object over these cells? (native drops) — [Ref-Binding-Last-Use] (D-0111): the frame's hold ends early | `[Ref-Binding-Last-Use]` |
| `cb_raw_move_out` | raw storage | `*p` read of a resource into dst: [Rawptr-Move-Out] | `[Rawptr-Move-Out]` |
| `cb_release` | raw storage | [Release]: end reclaimed objects starting in the range | `[Release]` |
| `cb_release_plain` | raw storage | cb_release for cells that never hold a reference — [Release] for cells that never hold a reference | `[Release]` |
| `cb_reallocate_plain` | raw storage | Vec growth of plain elements — Vec::grow of plain elements: [Allocate], [Copy-Raw], [Deallocate] of the old cells | `[Allocate]`, `[Copy-Raw]`, `[Deallocate]` |
| `cb_allocate` | raw storage | 0 on failure — [Allocate]: 0 on failure (Err) | `[Allocate]` |
| `cb_deallocate` | raw storage | [Deallocate]: [Release] of the objects over the cells, then the cells leave storage | `[Deallocate]`, `[Release]` |
| `cb_copy_raw` | raw storage | [Copy-Raw] | `[Copy-Raw]` |
| `cb_send` | values in flight | [Call]'s argument passing: the object is in flight from the caller to the callee's [Store-Binding-Temp] | `[Call]`, `[Store-Binding-Temp]` |
| `cb_recv` | values in flight | [Store-Binding-Temp] side of a call: the in-flight object, now at `addr`, bound by the callee | `[Store-Binding-Temp]` |
| `cb_send_ref` | values in flight | [Call]: a reference argument in flight, pending until [Store-Binding-Value] binds it (D-0107) | `[Call]`, `[Store-Binding-Value]` |
| `cb_result_ref` | values in flight | a statement's reference result: re-stamped outward — [Stmt-Exit]'s re-stamp for a reference result | `[Stmt-Exit]` |
| `cb_send_datum` | values in flight | [Call]: a value holding references in flight; its slots' holds move with it | `[Call]` |
| `cb_recv_datum` | values in flight | [Store-Binding-Value] of in-flight reference data into `dst` | `[Store-Binding-Value]` |
| `cb_recv_datum_tok` | values in flight | one reference's data, unheld: its token (0: none) — internal: one reference's in-flight data, unheld: its token | — |
| `cb_forget_obj` | values in flight | an objectless local's received object: its record ends, its value is kept — internal: an objectless local's received object (D-0200): its record ends, its value is kept by the C | — |
| `cb_drop_local` | values in flight | that local ends: its value is destroyed — internal: declared again above, [Destroy]/[Object-End] of an objectless local (D-0200) | `[Destroy]`, `[Object-End]` |
| `cb_env_alloc` | threads and mutexes | a spawned thread's argument block, zeroed, aligned as its struct — [Spawn]: the argument block, storage outside every object | `[Spawn]` |
| `cb_spawn` | threads and mutexes | [Spawn]: the thread id | `[Spawn]` |
| `cb_hold` | threads and mutexes | a resource argument waiting in a block — [Spawn]: a resource argument held in the block until the new thread's [Store-Binding-Temp] | `[Spawn]`, `[Store-Binding-Temp]` |
| `cb_thread_done` | threads and mutexes | [Thread-Body-Done]: the result is in the block | `[Thread-Body-Done]` |
| `cb_env_forget` | threads and mutexes | the block's references stop being occurrences — [Spawn]: the block's reference cells stop counting as holders once the parameters are bound | `[Spawn]` |
| `cb_drop_fn` | threads and mutexes | destroy a fn value (a closure box) — [Destroy] of a closure box: the closure object it owns ends (D-0089) | `[Destroy]` |
| `cb_thread_jmp` | threads and mutexes | [Fault-Contain] (D-0204): the trampoline's jump buffer, where a contained fault's unwind ends | `[Fault-Contain]` |
| `cb_thread_start` | threads and mutexes | [Spawn] in the new thread: registers it and binds its parameters from the block | `[Spawn]` |
| `cb_thread_failed` | threads and mutexes | [Fault-Contain]: the thread's failure is recorded (threads(ℓ).value := failed) | `[Fault-Contain]` |
| `cb_thread_lent` | threads and mutexes | [Thread-Lend]'s consequence for containment: a thread lent an exclusive reference is never contained | `[Thread-Lend]` |
| `cb_join_status` | threads and mutexes | [Try-Join] (mode 0, waits) and [Is-Finished] (mode 1) | `[Try-Join]`, `[Is-Finished]` |
| `cb_cancel` | threads and mutexes | [Cancel] (D-0204) | `[Cancel]` |
| `cb_task_op` | threads and mutexes | [Failure-Text] and the supervision operations' queries | `[Failure-Text]` |
| `cb_join` | threads and mutexes | [Join]: the result's object, or 0 | `[Join]` |
| `cb_lock` | threads and mutexes | [Lock]: the lock path's token | `[Lock]` |
| `cb_lock_bare` | threads and mutexes | D-0200: lock without a guard — [Lock] without a guard object (D-0200, std's own bodies): sync(o) := locked-by; unlocked by cb_unlock_bare | `[Lock]` |
| `cb_unlock_bare` | threads and mutexes | [Guard-Drop] without a guard object | `[Guard-Drop]` |
| `cb_unlock_signal` | threads and mutexes | D-0200: event_op(3, id) then cb_unlock_bare(m) — [Atomic-Notify]-like event signal (event_op(3, id)) then [Guard-Drop] without a guard, as one step | `[Atomic-Notify]`, `[Guard-Drop]` |
| `cb_str_eq` (inline) | text and hashing helpers | — | — |
| `cb_hash_bytes` (inline) | text and hashing helpers | A hash-table key's hash and equality (spec/21 rule.stdlib.hashmap): FNV-1a, 64-bit, over the key's bytes. | — |
| `cb_fnv_more` (inline) | text and hashing helpers | D-0110: FNV-1a continued from h over n more bytes (a composite key) | — |
| `cb_bytes_cmp` (inline) | text and hashing helpers | D-0108: text in byte order; negative, zero or positive | — |
| `cb_bytes_eq` (inline) | text and hashing helpers | — | — |

<!-- runtime-contract:end -->

## Change Log

- 2026-10-17 — `CHG-0327` (D-0263): §6, the runtime's contract, generated from `cbrt.h`.
- 2026-10-17 — `CHG-0325` (D-0261): §5 lists the verify modes of both tools.
- 2026-10-17 — `CHG-0319` (D-0257): §5 lists `cobc --lib`, the header, and the runtime at the boundary.
- 2026-10-16 — `CHG-0313` (D-0255): §5 lists `cobc`'s two targets and the Windows calling convention.
- 2026-10-16 — `CHG-0307` (D-0250): struct field offsets leave §1: `[Layout-Struct]` fixes them (C's rule).
- 2026-10-15 — `CHG-0280`: §5 names the files under `src/interp/` where `coby` makes three of its choices.
- 2026-10-14 — `CHG-0278` (WI-20): the Purpose says the implementations exist; §5 records their choices; §2's address row and §3's list follow D-0233.
- 2026-10-12 — `CHG-0257` (D-0218): §4 says how the tools carry out the
  reads through a shared path.
- 2026-10-11 — `CHG-0255` (D-0217): §2 lists a zero-sized object's anchor.
- 2026-10-11 — D-0206: §3's closing paragraph speaks of a safe client
  and the trusted library base; §4 names `coby --check` as the
  executable reference checker.

- 2026-09-22 (latest) — Updated for `CHG-0026`–`CHG-0029`: §1 gains
  the two `outcome: impl-defined` rows `spec/22` 2.10.0 added
  (file-backed module path forms; file naming in diagnostic
  locations), and the reporting-form row's stray "`spec/17` §..."
  placeholder now cites `spec/22` §5.
- 2026-09-20 — Updated for `CHG-0020`–`CHG-0024`: §1's
  calling-convention row notes `write` as its first concrete
  instance; §4 revised — zero `feat.*` entries remain
  `UNDER_INVESTIGATION` and `spec/AUDIT-2.md` B-26 is closed, so
  nothing is open in the registries or audit log except the
  reference-implementation authorization itself.
- 2026-09-20 (later) — Updated for `CHG-0019`: F-05 closed (the
  `[Rawptr-Write]` row and §4's conclusion/recommendation revised
  accordingly — implementation is no longer blocked on any known
  soundness question).
- 2026-09-20 — Initial version (`CHG-0018`).
