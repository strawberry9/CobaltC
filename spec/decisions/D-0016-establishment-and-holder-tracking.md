# D-0016 — Unified Resource Establishment and Current-Holder Sweep Tracking

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §7, §9, §11, §17
(Resource Authority), §12
Depends on: inv.resource-authority, inv.origin-stability, D-0003, D-0008
Affects: rule.value-object.object-establish, rule.agg.struct-construct,
rule.agg.array-construct, rule.agg.enum-construct, rule.control.block-exit,
rule.conc.spawn, rule.conc.join, state.holder

Realizes the fix for `spec/AUDIT.md` A-04, A-05, A-24 (worked as one
problem per `spec/AUDIT.md` §6 step 2).

## Problem

Three constructs each independently grant or track destroy-capable
resource authority without going through
`rule.value-object.object-establish`'s `[Object-Establish-Resource]` —
the single rule D-0003 designates as the point authority and a
destruction obligation are atomically created — and one further rule
sweeps destruction obligations by a criterion that cannot express
authority that has moved:

1. **`rule.agg.struct-construct`/`-array-construct`** (`spec/16-
   aggregates.md` §1–§2) create `objects(o)` directly and call
   `Field-Transfer-In` only for resource-bearing *fields*. A struct
   declared `is-resource = true` with no resource-bearing field at all —
   every library resource type this specification defines (`Vec<T>`,
   `Rc<T>`; `String` derives its resource status from its one field, so
   is unaffected) — never receives `authority(p,destroy,o)` or a
   `destruction-obligations` entry. `drop(v)` on such a value fails
   (`diag.no-destroy-authority`) and `Block-Exit` never destroys it: a
   structural leak/non-destroyability defect in the specification's own
   worked examples.
2. **`rule.control.block-exit`**'s sweep set `O` (`spec/14-control-flow.md`
   §1) is `{ o | Σ.objects(o).origin was established in frame f }`.
   `inv.origin-stability` (`spec/03-invariants.md`) fixes `origin` for an
   object's entire lifetime by design — it is not, and must not become,
   a proxy for "which frame currently holds this object's destroy
   authority." A `Vec` returned from a function, or moved into an outer
   binding across a block boundary, keeps its original (now-popped, or
   simply wrong) frame as `origin` forever; no enclosing `Block-Exit`
   ever includes it in `O`. This is not a corner case — it is exactly
   the pattern ownership transfer (`rule.resauth.transfer`, `return`)
   exists to support, and D-0008's claim that automatic destruction makes
   leak-freedom "structurally impossible" is false for precisely this
   pattern.
3. **`rule.conc.spawn`/`-join`** (`spec/19-concurrency.md` §1) key
   `authority(current-performer, join, h)` directly on a bare value `h`,
   never establishing `h` as an object via `[Object-Establish-Resource]`
   — the same defect class as (1): a resource type's authority/
   obligation bookkeeping created ad hoc outside the one rule family
   designated to create it.

## Constraints

- §6: a false semantic claim (here: "this struct/handle has no
  outstanding authority to track") must not become trusted merely
  because the construct that created it took a shortcut around the
  designated establishment rule.
- §9: one authoritative establishment mechanism
  (`[Object-Establish-Resource]`/`-Plain`), not one per construct that
  happens to produce a resource-bearing value. Composite construction
  and thread-handle creation should be *specializations* of it, not
  parallel, independently-maintained copies of its bookkeeping.
- D-0008's revisit condition anticipates exactly this: "the expected
  resolution is that a `return` expression counts as an explicit
  transfer out of the block before block-exit's automatic step runs, not
  a reversal of this decision" — the fix must key the sweep on *current
  holder*, not reopen whether automatic destruction itself is the right
  mechanism (it is; D-0003/D-0008 stand).
- §12: the sweep criterion must be a fixed, language-defined fact
  (which frame currently holds an object's destroy authority),
  mechanically the same for every conforming implementation — not left
  to whatever bookkeeping a given implementation happens to keep.

## Candidate mechanisms

### For (1)/(3), establishment

1. **Leave each construct's bespoke bookkeeping in place, patch each one
   individually to also set `authority`/`destruction-obligations`.**
   Works locally but reproduces D-0003's establishment logic at every
   call site, in tension with §9 and with §14's "one authoritative
   source" — and is exactly the pattern that already produced the gap
   (two call sites already forgot it independently). Rejected.
2. **Route every resource-bearing construction through
   `[Object-Establish-Resource]`/`-Plain` as the object-establishment
   step, with construct-specific logic (field transfer-in, thread
   bookkeeping) layered strictly *after* it.** One authoritative
   establishment rule; every consumer specializes it rather than
   reimplementing it. **Selected.**

### For (2), sweep criterion

1. **Leave `origin`-keyed sweeping, add a special-cased "unless
   transferred out" exception enumerated per transferring construct.**
   Already exists in prose form ("not yet destroyed or transferred
   out") but was never actually sufficient, because `origin` itself
   never moves — the exception can only ever suppress sweeping at the
   *source*, it cannot make the *destination* frame's `Block-Exit` aware
   of an object it never established. Structurally cannot be patched
   into correctness without changing the keying fact itself. Rejected.
2. **Key the sweep on the object's *current holder***: introduce
   `state.holder: ResourceId ⇀ AccessPathToken`, updated atomically by
   every rule that grants or moves destroy authority
   (`[Object-Establish-Resource]`, `rule.resauth.transfer`,
   `[Field-Transfer-In]`), giving `Block-Exit` a direct, always-current
   answer to "which access path — and hence which frame — is
   responsible for this object" without re-deriving it from `origin` or
   scanning `state.access-paths`. **Selected.**

## Selected design

- **`rule.value-object.object-establish`** (`spec/05-value-object-
  semantics.md`) is unchanged in its own shape; it remains the sole rule
  that may set `authority(p,destroy,o) := fresh unconsumed token` and add
  `o` to `destruction-obligations`.
- **`rule.agg.struct-construct`/`-array-construct`/`-enum-construct`**
  (`spec/16-aggregates.md` §1–§3) are revised: each first calls
  `⟨establish-object(o, layout(τ), τ, p), Σ⟩` (i.e.
  `[Object-Establish-Resource]` when `is-resource(τ)`, else
  `[Object-Establish-Plain]`) to obtain `o` with `init(o) :=
  uninitialized` and, in the resource case, authority/obligation already
  granted; *then* performs `Field-Transfer-In` for each resource-bearing
  field/element/payload exactly as before, and finally sets `init(o) :=
  valid`. A struct/array/enum declared `is-resource = true` now always
  receives authority and an obligation at construction, whether or not
  any individual field is itself resource-bearing — closing A-04.
  `conf.leak-free-by-construction` (`spec/conformance.md`) is
  re-verified against this corrected rule as part of A-30's
  regeneration.
- **`state.holder: ResourceId ⇀ AccessPathToken`** (new
  `spec/04-abstract-state.md` component). Updated to point at the
  relevant access path by: `[Object-Establish-Resource]` (once the
  establishing binding is formed — see below), `rule.resauth.transfer`
  (destination `a2`), and `[Field-Transfer-In]` (the composite key
  `(o,path)`'s destination). Because establishment itself precedes any
  binding, `state.holder(o)` is set by whichever access-path-forming
  rule (`rule.value-object.binding-form`, or `rule.resauth.transfer`'s
  destination formation) is the *first* to associate a named or
  returned access path with a freshly-established resource `o` — no
  `Block-Exit` can observe the brief unbound window between
  establishment and that first association, since no frame boundary
  occurs mid-statement (D-0007's evaluation order makes a statement's
  evaluation atomic with respect to frame-stack transitions).
- **`rule.control.block-exit`**'s sweep set is revised:

      O = { o ∈ Σ.destruction-obligations |
            Σ.access-paths(Σ.holder(o)).frame = f
            ∧ o not yet destroyed or transferred out }

  keyed on the exiting frame `f` matching the *current holder's* frame,
  not `origin`. A `Vec` returned from a function and bound by the
  caller (`let result = f();`, via the corrected `[Let]` rule, A-06)
  acquires a fresh holder access path in the caller's frame the moment
  the binding is formed there — `state.holder` follows it automatically,
  with no special-casing of "was this returned." This closes A-05. A new
  conformance case (A-30) exercises exactly this: a `Vec` returned from
  a function is destroyed at the *caller's* block exit, not left
  unswept.
- **`rule.conc.spawn`/`-join`** (`spec/19-concurrency.md` §1) are
  revised: `Spawn` establishes the join handle via
  `⟨establish-object(o_h, ∅, type.handle<τr>, current-performer), Σ⟩`
  (empty extent — a handle has no addressable storage of its own) and
  binds it to `h` exactly as any other resource-typed result, so `h`'s
  authority is `authority(current-performer, destroy, o_h)` — not a
  separately-named `join` operation. `Join` is restated as `type.handle
  <τr>`'s own instance of `rule.resauth.destroy`: it requires
  `authority(p, destroy, o_h, Σ)` and `o_h`'s solitary access exactly as
  `[Destroy]` does, consumes the authority, and calls `end-object`,
  additionally reading the spawned thread's produced value `v : τr` as
  part of the same step (paralleling how `Vec<T>`'s destructor,
  `spec/21-standard-library-semantics.md` §2, layers type-specific work
  on top of the same generic destroy call). An un-joined handle still
  outstanding at its frame's `Block-Exit` is destroyed automatically —
  discarding the thread's result rather than failing to compile or leak
  — exactly as D-0008 already requires for any other undischarged
  obligation. This closes A-24 as the same defect class as A-04: one
  establishment mechanism, specialized, not a parallel bookkeeping path.

## Rejected alternatives

Establishment: per-construct patched bookkeeping (1). Sweep: origin-keyed
with per-site exceptions (1).

## Semantic rationale

Both fixes remove a place where a construct was answering an
authority-bookkeeping question (who has authority; who currently holds
it) using its own local logic instead of consulting — or updating — the
one state component the rest of the specification already treats as
authoritative for that question. `state.holder` is the minimal addition
that makes "which frame is responsible for destroying this" a lookup
rather than a re-derivation from a fact (`origin`) that was never meant
to answer it.

## Usability implications

None negative: every currently-well-formed program's observable
destruction points are unchanged (an object whose holder never moves
still gets swept where `origin`-keying would already have put it,
since the two coincide until a transfer/return moves it). The programs
this fixes were previously *rejected or leaking*; they now behave as
D-0008 always intended.

## Explainability implications

A leak diagnostic (should one ever fire — it should not, per D-0008's
structural guarantee, once this decision closes the gap) can now cite
`state.holder(o)`'s frame directly, rather than a stale `origin` that
may not even still exist as a frame.

## Implementation-feasibility implications

`state.holder` is a single additional total-ish partial map, updated at
exactly the same events that already update `state.authority` — no new
analysis pass, no new traversal.

## Compatibility impact

Revises `rule.agg.struct-construct`/`-array-construct`/`-enum-construct`
(`spec/16-aggregates.md`, `PROVISIONAL`), `rule.control.block-exit`
(`spec/14-control-flow.md`, `PROVISIONAL`), and `rule.conc.spawn`/`-join`
(`spec/19-concurrency.md`, `PROVISIONAL`) in place. Does not reverse
D-0003 (authority mechanism unchanged) or D-0008 (automatic destruction
at scope end unchanged in trigger and order) — it corrects the *sweep
criterion* `spec/14-control-flow.md` §1 chose when first concretizing
D-0008's obligation, which was always a `spec/14`-level implementation
detail of D-0008's selected design, not D-0008's own selected point.

## Prior-art status

Holder/current-owner tracking as the sweep key is independently derived
from `inv.resource-authority`'s own "single current holder" shape
(D-0003); not adopted for resemblance to any specific prior system.

## Invariant traceability

Closes the A-04/A-24 gaps in `inv.resource-authority`'s establishment
coverage and the A-05 gap in `rule.control.block-exit`'s enforcement of
D-0008's "no leak by omission" guarantee.

## Revisit conditions

Revisit if a future resource kind's authority can be held by more than
one performer at a time (D-0003's single-holder scope still applies
throughout this specification; `state.holder` presupposes single-holder
and would need to become a *set* if that scope is ever widened — not
needed by anything defined through `spec/21`).
