# CobaltC Implementation Notes

Status: non-normative artifact (Master Instructions §14, §24;
`spec/02-schema.md` §6 — this file carries no rule, invariant, type, or
state entity and needs no version-bump discipline beyond its own
Change Log)
Governed by: `CobaltC_Master_Instructions.md` §20

## Purpose

This is the Master Instructions §20 implementation-feasibility
analysis in its final form: for a reference
interpreter over `Σ` (`spec/04`), every choice a conforming
implementation must make and document, every outcome the language
permits to vary between runs of *the same* implementation, and every
condition the language does not check at all but trusts the author of
an `unsafe` block to have established. Nothing here is normative —
`spec/[0-2]*.md`, the registries, and the conformance suite remain the
sole normative definition (`spec/README.md`) — and nothing here
authorizes writing that interpreter: Master Instructions §1 reserves
that decision to the human owner, separately, after reading this file.

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
| Struct field offsets | `[Layout-Struct]` (`rule.agg.layout`, `spec/16` §1) | Any offset assignment satisfying the side-conditions (natural alignment, no field overlap); `sizeof`/`alignof` of every struct follow from it (`[Sizeof-Struct]`). |
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
| A fresh object's storage address | `[Object-Establish-Resource]`, `[Object-Establish-Plain]` | Any fresh, aligned, disjoint range — unobservable to a safe program (no rule converts a `ref` to an address); observable only across an `unsafe` `rawptr_of`/`extern` boundary, where the specification's own trust conditions, not this freedom, are what a program must not depend on. |
| A fresh allocation's address | `[Allocate]` | Same freedom, for `allocate`'s raw range; also `disposition: fallible` — even `allocate(n, align)` for a size that has succeeded before is not guaranteed to succeed again (no minimum-availability guarantee is made). |
| Multi-thread interleaving | `[Thread-Step]` | Which live thread's next `→^ℓ` step is taken; `conf.cross-thread-write-conflict`'s own derivation is the conformance suite's example of a case whose diagnostic legitimately differs by interleaving (`outcome: unspecified { ok, diag.aliasing-conflict }`) without either outcome being wrong. |
| Padding/reserved-cell bytes | The `bytes(cont)`/`bytes(pad)` byte view (`spec/06` §7) | Any byte `0..255`; read only through a raw pointer or `extern`, and only as uninterpreted padding — no rule ever assigns padding a meaning, so this is unobservable to any program that does not itself go out of its way to read struct padding through `rawptr`. |

## 3. Trusted (unchecked) conditions

Every `discharge: trusted` side-condition — the *only* place any of
these may occur is lexically inside an `unsafe { }` block
(`rule.trust.unsafe` `[Unsafe-Rejected]`). A reference interpreter
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
| `[Deallocate]` | `(p, n, align)` were returned by one `allocate`, not yet deallocated. |
| `[Extern-Call]` | The callee honors the declared `FfiType` signature; it reads/writes only cells reachable from the `rawptr` arguments and its own allocations; its returned image is a valid value of the declared return type. |

A **safe program** (`term.safe-program`, `spec/00`) contains no
`unsafe { }` block and therefore never reaches any row in this table
(`spec/00` §"Definition"); every proposition in `spec/03-invariants.md`
holds throughout its execution unconditionally. Every row above is
reachable only through `spec/21`'s own library bodies or a user's own
`unsafe` code — never through ordinary safe-language constructs.

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

## Change Log

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
