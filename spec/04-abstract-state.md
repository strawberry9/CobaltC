# CobaltC Abstract Semantic State

Status: normative artifact
Version: 1.3.2
Conforms to: `spec/02-schema.md` (Kind: State component, `state.<name>`)
Governed by: `CobaltC_Master_Instructions.md` §16, §23
Grounds: every predicate forward-referenced by `spec/03-invariants.md`
Revised by: D-0016, D-0018, D-0019

## Purpose

Defines the components of the abstract machine state `Σ` introduced
opaquely by `spec/01-metalanguage.md` §1, and grounds every predicate
the Invariant Registry uses. Per Master Instructions §16, every
component exists because a named rule reads or writes it; none is
speculative. Index sets `Identity`, `Address`, `Thread`, `Event`,
`AccessPathToken`, `Frame`, `TempScopeId`, `ValueToken`, `Path` are
opaque, pairwise-disjoint token sets.

`Performer ≝ Thread` (D-0019, `spec/AUDIT-2.md` B-12). The current
performer of a step labeled `→^ℓ` is `ℓ`; an unlabeled rule is read as
labeled with the thread evaluating it (`spec/01` §2.3). The
metavariable `ℓ` in this and every later artifact means that thread.

`Path ::= ε | field-name · Path | index · Path` — a sequence of field
names and array indices locating a sub-range of an object.
`ResourceId ::= Identity | (Identity × Path)` (`spec/07` §4).

## 1. Components of `Σ`

### `state.items`
**Status:** ACCEPTED
**Type:** `QualifiedName ⇀ Item`, `Item ::= fn-item | extern-item |
struct-item | enum-item | mod-item`.
**Semantic purpose:** the program's declarations, fixed in `Σ_0` by
`rule.fn.program` (`spec/15` §7) and never changed afterwards. Name
resolution (`spec/17`) reads it; nothing writes it after `Σ_0`. `Item`'s
five alternatives are produced only by the `item` grammar production
(`spec/22` §3: `fn-decl | extern-decl | struct-decl | enum-decl |
module-decl | import-decl`) — a body-less prelude intrinsic
(`sizeof`, `spawn`, `join`, `rawptr_of`, … — `spec/21` §0's table)
is never an entry here; it is recognized directly by its own call
form and typed/evaluated by its own named rule (`CHG-0023`). A standard-
library function *with* a body (`map_err`) and the library types' own
methods are ordinary `fn-item` entries like any user declaration, under
the module `std` (`std::map_err`, `std::Vec::push`, …; `spec/21` §0,
`CHG-0033`).
**Depends on:** term.program, term.module, term.function, CHG-0023

### `state.objects`
**Status:** ACCEPTED
**Type:** `Identity ⇀ ObjectRecord`, a partial function.
**ObjectRecord:** `{ origin: Event, extent: Set(Address), type: Type,
storage-kind: fresh | reclaimed, temp-scope: TempScopeId }`.
`temp-scope` is the statement scope (`state.temp-scope-stack`) current
when the object was established, re-stamped by `rule.control.stmt`
when the object flows out of that statement as its result (D-0019).
`storage-kind` records whether `rule.value-object.object-end` releases
the extent (`fresh`) or leaves it to a raw allocation (`reclaimed`,
`rule.trust.rawptr`).
**Semantic purpose:** the single source of truth for which identities
denote live objects, and each one's origin, extent, and type. Because
this is a partial function, `inv.identity` holds by construction (§3).
**Depends on:** term.object, term.identity, term.origin, term.extent
**Affects:** inv.identity, inv.origin-stability, inv.spatial-validity

### `state.storage`
**Status:** ACCEPTED
**Type:** `Address ⇀ Content`, `Content ::= uninit | datum`, where
`datum` is an element of the cell alphabet fixed by
`rule.arith.represent` (`spec/06` §7).
**Semantic purpose:** raw addressable capacity (`term.storage`),
extended by `rule.value-object.object-establish` and
`rule.stdlib.prelude`, shrunk by `rule.value-object.object-end` and
`rule.stdlib.prelude`. An address can exist here without any
object claiming it (raw allocations), which keeps `term.storage` and
`term.object` distinct.
**Depends on:** term.storage

### `state.access-paths`
**Status:** ACCEPTED
**Type:** `AccessPathToken ⇀ AccessPathRecord`.
**AccessPathRecord (D-0018):** `{ target: Set(Address), of: Identity,
type: Type, mode: shared | exclusive, thread: Thread, valid: Bool,
formed-at: Event, frame: Frame, temp-scope: TempScopeId,
base: AccessPathToken?, held-by: Set(Identity) }`.
- `frame`: the frame current at formation; for an owner path this is
  the frame responsible for ending the object (`owned-by-frame`).
- `target ⊆ extent(of)`: the sub-range this path reaches; the whole
  extent for a root path, a field/element sub-range for a projection.
- `type`: the type of what `target` holds — `objects(of).type` for a
  root, the field/element/payload type for a projection.
- `base`: the path this one was derived from — `None` for a root
  (`rule.value-object.binding-form`, `rule.trust.rawptr`,
  `rule.conc.lock`), the borrowed-from path for `rule.alias.borrow`,
  the narrowed path for `rule.agg.field-access`/`rule.agg.index`.
- `held-by`: the set of live objects whose storage currently contains
  this path as a reference value (`rule.value-object.write` adds and
  removes members; `rule.value-object.object-end` removes members).
- `temp-scope`: the statement scope current at formation, re-stamped
  by `rule.control.stmt` when the path is the statement's result.
Every path-forming rule sets every field (`spec/AUDIT-2.md` B-04).
**Semantic purpose:** gives `term.access-path` a uniform
representation carrying its own target, type, validity, mode, and
derivation, independent of the object it targets.
**Depends on:** term.access-path, term.temporal-validity, term.alias,
D-0018

### `state.bindings`
**Status:** ACCEPTED
**Type:** `(Frame × Name) ⇀ AccessPathToken`.
**Semantic purpose:** name resolution (`term.binding`) — maps a surface
name in a frame to the root access path denoting it.
**Depends on:** term.binding

### `state.frame-stack`
**Status:** ACCEPTED
**Type:** `Thread ⇀ List(Frame)`, most-recently-pushed first (D-0019:
per thread, `spec/AUDIT-2.md` B-13). `current-frame(ℓ, Σ) ≝
head(Σ.frame-stack(ℓ))`.
**Semantic purpose:** tracks each thread's open blocks so
`rule.control.block` can determine which frame is ending.
**Depends on:** term.binding

### `state.temp-scope-stack`
**Status:** ACCEPTED
**Type:** `Thread ⇀ List(TempScopeId × Frame)`, most-recently-pushed
first; each scope records the frame that was current when it was
pushed, so `rule.control.unwind` can interleave statement exits and
block exits in LIFO order. `current-scope(ℓ, Σ) ≝
fst(head(Σ.temp-scope-stack(ℓ)))`, or `⊥` when the stack is empty —
the state of a thread between `rule.conc.spawn`'s parameter binding
and its body's first statement; an object or path stamped `⊥` belongs
to no statement scope and is never selected by `rule.control.stmt`'s
`Temps`/`Unheld` (a parameter object is owned by its frame, which is
what ends it). `scope-frame(t, Σ)` is the recorded frame.
**Semantic purpose:** tracks each thread's open statement scopes so
`rule.control.stmt` can end the temporaries and unheld references
a statement formed (D-0018, D-0019).
**Depends on:** term.access-path, D-0018, D-0019

### `state.holder`
**Status:** ACCEPTED
**Type:** `Identity ⇀ AccessPathToken`.
**Semantic purpose:** for every top-level object that some binding,
reclaim path, or lock guard currently owns, the root access path that
owns it — hence which frame is responsible for ending it
(`rule.control.block`). An identity absent from `dom(Σ.holder)`
is a *temporary* (D-0019): an object that exists but is not yet owned,
ended by `rule.control.stmt` if it is not stored or returned by
the statement that created it. Set by `rule.value-object.store`
(adopt/transfer), `rule.trust.rawptr`, `rule.conc.lock`; cleared by
`rule.fn.return`/result formation and by `rule.resauth.relocate-in`.
Composite sub-resources `(o, path)` have no entry: their owner is
`holder(o)`.
**Depends on:** term.resource, term.authority, D-0016, D-0019
**Affects:** inv.resource-authority

### `state.init`
**Status:** ACCEPTED
**Type:** `Identity ⇀ InitState`, `InitState ::= uninitialized | valid`.
`partial(Step)`, `partially-destroyed(Step)`, and `consumed` are
removed from the alphabet: D-0019 rejects partial initialization, and
`spec/05` §Read makes `consumed` unreachable. Removing unreachable
values is a non-normative simplification (no program could observe
them).
**Semantic purpose:** grounds `init-state(o, Σ)`
(`inv.initialization-validity`).
**Depends on:** term.initialization-state
**Affects:** inv.initialization-validity

### `state.destruction-obligations`
**Status:** ACCEPTED
**Type:** `Set(ResourceId)`.
**Semantic purpose:** the resources that must be destroyed exactly once
before their storage ends — what `inv.resource-authority`'s leak
failure mode is stated against. Added by
`rule.value-object.object-establish` (resource case) and
`rule.resauth.relocate-in`; removed by `rule.resauth.destroy`,
`rule.resauth.destroy-composite`, `rule.resauth.relocate-out`.
**Depends on:** term.resource
**Affects:** inv.resource-authority

### `state.authority`
**Status:** ACCEPTED
**Type:** `(Performer × Operation × ResourceId) ⇀ AuthorityToken`,
`AuthorityToken ::= { consumed: Bool }`. The only `Operation` any rule
uses is `destroy`.
**Semantic purpose:** grounds `authority(p, op, r, Σ)` and
`consumed(tok, Σ)`. Granted by `rule.value-object.object-establish`,
`rule.trust.rawptr`, `rule.resauth.relocate-out`; moved by
`rule.resauth.transfer`, `rule.resauth.relocate-in`; consumed by
`rule.resauth.destroy`.
**Depends on:** term.authority, term.resource
**Affects:** inv.resource-authority

### `state.sync`
**Status:** ACCEPTED
**Type:** `Identity ⇀ SyncState`, `SyncState ::= unlocked |
locked-by(Thread)`. Absence means `thread-local`.
**Semantic purpose:** grounds `sync-state(o, Σ)` for mutex-guarded
objects (`rule.conc.lock` `[Lock]`/`[Guard-Drop]`).
**Depends on:** term.access
**Affects:** inv.concurrency-validity

### `state.threads`
**Status:** ACCEPTED
**Type:** `Thread ⇀ { expr: Expr, handle: Identity?, value: Result | taken | None }`.
`expr` is the thread's in-progress expression; `handle` the
`type.handle` object `rule.conc.spawn` established for it (`None` for
the initial thread); `value` its final result `r_b` (in result form,
`spec/05`) once `expr` has reduced, or `taken` once `rule.conc.join`
`[Join]` has claimed it (so `[Handle-Destructor]` has nothing to
discard; `CHG-0009`).
**Semantic purpose:** gives the labeled step relation `→^ℓ` a place to
record each thread's progress, and lets `rule.conc.join` read a
finished thread's value.
**Depends on:** term.access, D-0016

### `state.trust`
**Status:** ACCEPTED
**Type:** `ValueToken ⇀ TrustState`, `TrustState ::= unchecked-claim |
transitioned`. Absence means internally produced.
**Semantic purpose:** grounds `origin-of-value(v, Σ)` and
`transitioned(v, Σ)` (`rule.trust.extern-call` `[Extern-Call]`/
`[Trust-Transition]`).
**Depends on:** term.trust-boundary
**Affects:** inv.trust-transition

### Removed components
`state.regions` (never read by any rule; `term.region` remains
vocabulary), `state.suspended` (D-0018 removed suspension). Their ids
are retired, not reused (`spec/02` §2).

## 2. Predicate grounding

Every predicate is total over the arguments its definition names;
where a component lookup is undefined the predicate is `false`.

| Predicate | Definition |
|---|---|
| `identity(o, Σ)` | `o`, when `o ∈ dom(Σ.objects)` |
| `alive(o, Σ)` | `o ∈ dom(Σ.objects)` |
| `origin(o, Σ)` | `Σ.objects(o).origin` |
| `extent(o, Σ)` | `Σ.objects(o).extent` |
| `type-of(o, Σ)` | `Σ.objects(o).type` |
| `target(a, Σ)` | `Σ.access-paths(a).target` |
| `type(a, Σ)` | `Σ.access-paths(a).type` |
| `of(a, Σ)` | `Σ.access-paths(a).of` |
| `mode(a, Σ)` | `Σ.access-paths(a).mode` |
| `base(a, Σ)` | `Σ.access-paths(a).base` |
| `addr-in(a, o, Σ)` | `alive(o, Σ) ∧ of(a,Σ) = o ∧ target(a, Σ) ⊆ extent(o, Σ)` |
| `access-rel(a, o, Σ)` | `mode(a, Σ)`, when `of(a, Σ) = o` |
| `temporally-valid(a, Σ)` | `a ∈ dom(Σ.access-paths) ∧ Σ.access-paths(a).valid ∧ alive(of(a,Σ), Σ)` |
| `ancestors(a, Σ)` | `{a}` if `base(a,Σ) = None`, else `{a} ∪ ancestors(base(a,Σ), Σ)` (D-0018) |
| `overlap(a1, a2, Σ)` | `target(a1,Σ) ∩ target(a2,Σ) ≠ ∅` |
| `permitted(m1, m2)` | `m1 = shared ∧ m2 = shared` (D-0004) |
| `clash(a, m, Σ)` | `∃ a'. temporally-valid(a',Σ) ∧ of(a',Σ) = of(a,Σ) ∧ a' ∉ ancestors(a,Σ) ∧ overlap(a,a',Σ) ∧ ¬permitted(m, mode(a',Σ)) ∧ ¬sync-exempt(a, m, a', Σ)` (D-0018; `sync-exempt` defined by `rule.conc.lock`, `spec/19` §2 — false for every object that is not a `mutex`) |
| `temporary(o, Σ)` | `alive(o,Σ) ∧ o ∉ dom(Σ.holder) ∧ Σ.objects(o).storage-kind = fresh` (a reclaimed object with no holder persists until released, `spec/20` §2) |
| `witness(a, m, Σ)` | any `a'` satisfying the body of `clash(a, m, Σ)` — the path a diagnostic names |
| `solitary(a, Σ)` | `∀ a' ≠ a. temporally-valid(a',Σ) ⇒ of(a',Σ) ≠ of(a,Σ)` |
| `init-state(o, Σ)` | `Σ.init(o)` |
| `op-requires(op)` | declared by each rule: `read ↦ {valid}`, `write-whole ↦ {uninitialized, valid}`, `write-projection ↦ {valid}` (`spec/05`) |
| `authority(p, op, r, Σ)` | `(p, op, r) ∈ dom(Σ.authority) ∧ ¬Σ.authority(p,op,r).consumed` |
| `consumed(tok, Σ)` | `tok.consumed` |
| `holder(o, Σ)` | `Σ.holder(o)` |
| `owned-by-frame(o, f, Σ)` | `o ∈ dom(Σ.holder) ∧ Σ.access-paths(Σ.holder(o)).frame = f` (independent of `state.bindings`, so a shadowed binding's object is still swept) |
| `refs-in(v)` | the set of access-path tokens occurring in value `v` (`spec/06` §7: a `ref` value is a token; an aggregate value contains its fields' tokens; every other value contains none) |
| `objs-in(r)` | `{o}` if result `r` is a temporary object `o` (D-0019), else `∅` |
| `origin-of-value(v, Σ)` | `internal` if `v ∉ dom(Σ.trust)`, else `external` |
| `transitioned(v, Σ)` | `Σ.trust(v) = transitioned` |
| `thread-of(a, Σ)` | `Σ.access-paths(a).thread` |
| `sync-state(o, Σ)` | `Σ.sync(o)` if defined, else `thread-local` |
| `synchronized(a1, a2, o, Σ)` | `permitted(mode(a1,Σ), mode(a2,Σ)) ∨ (sync-state(o,Σ) = locked-by(t) ∧ thread-of(a1,Σ) = thread-of(a2,Σ) = t)` (`spec/19` §2) |
| `current-frame(ℓ, Σ)` | `head(Σ.frame-stack(ℓ))` |
| `current-scope(ℓ, Σ)` | `fst(head(Σ.temp-scope-stack(ℓ)))`, or `⊥` if empty (matches no scope) |

No predicate is deferred: `permitted` and `synchronized` were closed by
D-0004 and `spec/19`; `op-requires` by `spec/05`.

## 3. Structural guarantees

- **`inv.identity` holds by construction.** `state.objects` is a
  partial function; no rule can map one identity to two records.
- **`inv.origin-stability` holds by construction.** No rule assigns
  `Σ.objects(o).origin` after establishment; every rule that changes
  where a value lives (`rule.resauth.relocate-in`/`-out`) ends one
  object and establishes another.
- **`addr-in` holds by construction for every projection.**
  `rule.agg.field-access`/`rule.agg.index` set `target` to a sub-range
  of `layout(type-of(o))`, which `rule.agg.layout` confines to the
  extent; `rule.value-object.read`/`write` therefore need only check
  `alive`.

## Change Log

- 1.3.2 — Non-normative (`CHG-0033`): `state.items`'s note names the
  standard library's entries by their `std::` keys.
- 1.3.1 — Non-normative (consistency pass, 2026-09-22): `state.items`'s
  prose said `Item` has "four alternatives"; its own grammar lists
  five (`mod-item` included). Count corrected; nothing else changed.
- 1.3.0 — Non-normative (`CHG-0023` §"Hygiene"): `state.items` gains a
  clarifying sentence — a body-less prelude intrinsic is never a
  `Σ.items` entry, settling `feat.consolidate-join-typing`. No rule
  changed.
- 1.2.0 — `CHG-0011`: `current-scope` defined as `⊥` on an empty
  scope stack. A spawned thread binds its parameters before its body
  opens any statement scope, so `[Store-Binding-Value]`'s
  establishment stamped an undefined `temp-scope` (found deriving
  `ex.e2e-threads-mutex`).
- 1.1.0 — `CHG-0009`: `state.threads.value` gains the `taken`
  alternative so `[Join]` and `[Handle-Destructor]` no longer both
  claim the same result. Non-normative in the same pass: two
  duplicated rule citations (`state.sync`, `state.trust`) corrected to
  name the two rule labels each meant.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-04, B-11,
  B-12, B-13): `AccessPathRecord` gains `type`, `held-by`; loses `durable`; `state.suspended` and `state.regions` removed;
  `state.items` added; `state.frame-stack`/`state.temp-scope-stack`
  per thread; `ObjectRecord` gains `storage-kind`, `temp-scope`;
  `state.holder` covers every owned top-level object; `InitState`
  reduced to its reachable values; `Performer ≝ Thread`; `clash`,
  `ancestors`, `overlap`, `solitary`, `temporary`, `refs-in`,
  `objs-in` grounded; `permitted`/`synchronized`/`op-requires` closed.
  Every component promoted to `ACCEPTED`.
- 0.6.0 and earlier — see D-0015/D-0016 history; superseded.
