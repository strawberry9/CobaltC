# CobaltC Invariant Registry

Status: normative artifact
Version: 1.3.0
Conforms to: `spec/02-schema.md` (Kind: Invariant)
Governed by: `CobaltC_Master_Instructions.md` §6, §15

## Purpose

The semantic invariants CobaltC guarantees (Master Instructions §6,
§15): each with its proposition in CFN over `Σ` (`spec/04`), its
lifecycle, and the rules that enforce it. Mechanism-neutral in
statement; the enforcement fields name the rules the selected
mechanisms (D-0003, D-0004, D-0018, D-0019) provide.

**Coverage of §6's dimensions.** Meaning and mathematical validity:
`inv.arith.range-validity` and `rule.type.typing`. Preservation
through transformation: not a separate entry — every rule that moves
a value (`rule.resauth.transfer`, `relocate-in`/`-out`,
`rule.trust.rawptr` `[Rawptr-Move-*]`) states which of the entries
below it preserves and which it re-keys; `spec/04` §3 records the two
structural guarantees. The remaining nine dimensions are the entries
below.

Predicates used here are grounded in `spec/04` §2; none is deferred.

---

## `inv.identity`
**Status:** ACCEPTED
**Subjects:** objects.
**Proposition:** `∀ o1, o2. alive(o1,Σ) ∧ alive(o2,Σ) ∧ identity(o1,Σ) = identity(o2,Σ) ⇒ o1 = o2`
**Establishment:** `rule.value-object.object-establish` assigns a fresh identity.
**Preservation:** unchanged for the object's life. **Transformation:** none. **Weakening:** none.
**Invalidation:** `rule.value-object.object-end`; the identity is never reused (`o ∉ dom(Σ.objects)` is a premise of establishment).
**Checking / Enforcement:** by construction — `state.objects` is a partial function (`spec/04` §3).
**Reliance points:** every other invariant.
**Failure behavior:** unreachable.
**Depends on:** term.object, term.identity
**Affects:** inv.origin-stability, inv.spatial-validity, inv.temporal-validity, inv.resource-authority, inv.alias-validity

---

## `inv.origin-stability`
**Status:** ACCEPTED
**Subjects:** objects.
**Proposition:** `∀ o, Σ, Σ'. alive(o,Σ) ∧ alive(o,Σ') ⇒ origin(o,Σ) = origin(o,Σ')`
**Establishment:** with identity. **Preservation:** no rule assigns `origin` after establishment.
**Transformation:** relocation (`rule.resauth.relocate-in`/`-out`, `rule.trust.rawptr` `[Rawptr-Move-*]`) ends one identity and establishes another; it never rewrites `origin`.
**Weakening:** none. **Invalidation:** with the object's end.
**Enforcement:** by construction (`spec/04` §3).
**Reliance points:** `rule.control.block`'s destruction order (descending origin).
**Failure behavior:** unreachable.
**Depends on:** inv.identity, term.origin

---

## `inv.spatial-validity`
**Status:** ACCEPTED
**Subjects:** access paths and their objects.
**Proposition:** `∀ a. (an access uses a) ⇒ addr-in(a, of(a,Σ), Σ)`
**Establishment:** a root path targets its object's whole extent (`rule.value-object.binding-form`, `rule.trust.rawptr`, `rule.conc.lock`); a projection targets a sub-range of its base's target within `rule.agg.layout` (`rule.agg.field-access`, `rule.agg.index`).
**Preservation:** targets never change after formation. **Transformation:** narrowing by projection only.
**Weakening:** none. **Invalidation:** the object's end (then the path is temporally invalid, and `addr-in` is false because `alive` is).
**Checking / Enforcement:** by construction for every checked path (`spec/04` §3); the one dynamic check is the index bound `0 ≤ n < N` in `rule.agg.index` (`discharge: dynamic`, static for literal indices via `rule.control.flow-analysis`), and `Vec`'s length check (`spec/21` §1). Raw storage is outside this invariant (`rule.trust.rawptr`: asserted).
**Reliance points:** `rule.value-object.read`/`write`.
**Failure behavior:** `diag.index-out-of-bounds` (`checked`); never silent.
**Depends on:** inv.identity, inv.origin-stability, term.extent, term.access

---

## `inv.temporal-validity`
**Status:** ACCEPTED
**Subjects:** access paths.
**Proposition:** `∀ a. (an access uses a) ⇒ temporally-valid(a, Σ)`
**Establishment:** formation from a live object (every path-forming rule sets `valid := true`).
**Preservation:** while the object is alive, some holder of the path is alive or the forming statement is open, and no transfer/relocation of the object has occurred.
**Transformation:** a derived path (borrow, projection) is valid independently of its base once formed; its validity is tied to its own holders and its object (D-0018).
**Weakening:** none.
**Invalidation:** `rule.value-object.object-end` (object ends; last holder ends); `rule.value-object.write` (last holder overwrites); `rule.control.stmt` (unheld, not the result, statement ends); `rule.resauth.transfer`/`relocate-in`, `rule.trust.rawptr` `[Rawptr-Move-*]` (source path); `rule.conc.lock` `[Guard-Drop]`; `rule.resauth.destroy` `[Run-Destructor]` (the destructor's borrow).
**Checking:** `temporally-valid` is a premise of `rule.value-object.binding-lookup`, `read`, `write`, `rule.alias.borrow`, `rule.agg.field-access`/`index`, `rule.resauth.transfer`/`destroy`/`relocate-in`, `rule.conc.join`: `discharge: static` where `rule.control.flow-analysis` proves it, `dynamic` otherwise; `rule.temporal.ref-escape` and `rule.temporal.elision` are the static refinements (D-0005, D-0011).
**Reliance points:** every access. **Enforcement:** as Checking.
**Failure behavior:** `diag.stale-binding` (`checked`) or `diag.reference-escapes-scope` (`rejected`); never silent.
**Depends on:** inv.identity, inv.origin-stability, term.temporal-validity, D-0018

---

## `inv.initialization-validity`
**Status:** ACCEPTED
**Subjects:** objects.
**Proposition:** `∀ o, op. (op is applied to o) ⇒ init-state(o,Σ) ∈ op-requires(op)`, with `op-requires(read) = {valid}`, `op-requires(write-whole) = {uninitialized, valid}`, `op-requires(write-projection) = {valid}`.
**Establishment:** `rule.value-object.object-establish` sets `uninitialized`; a whole-object `rule.value-object.write`, `rule.agg.*-construct`, `rule.resauth.relocate-out`, `rule.trust.rawptr` set `valid`.
**Preservation:** `valid` is never reverted (D-0019: no partial states). **Transformation:** none. **Weakening:** none.
**Invalidation:** with the object's end.
**Checking:** premises of `rule.value-object.read` and `write` (`discharge: static` by `rule.init.definite-assignment`, which rejects on `unknown`; `dynamic` guard retained).
**Reliance points:** every read. **Enforcement:** as Checking.
**Failure behavior:** `diag.use-of-uninitialized` (`rejected` statically; `checked` fallback).
**Depends on:** inv.identity, term.initialization-state, D-0019

---

## `inv.resource-authority`
**Status:** ACCEPTED
**Subjects:** resources (`ResourceId`) and threads.
**Proposition:** `∀ ℓ, r. (ℓ destroys r) ⇒ authority(ℓ, destroy, r, Σ)`, and `∀ r ∈ Σ.destruction-obligations. exactly one of: r's holder's frame is open, or r is a temporary of an open statement scope, or r = (o, path) with o live, or r is a reclaimed object (`storage-kind = reclaimed`, `spec/20` §2)` — i.e. every obligation has exactly one owner that will discharge it; for a reclaimed object that owner is the `unsafe` author who reclaimed it, under `discharge: trusted` (`[Reclaim]`, `[Release]`).
**Establishment:** `rule.value-object.object-establish` (resource case), `rule.trust.rawptr`, `rule.resauth.relocate-out`.
**Preservation:** while no transfer, relocation, or destroy occurs. **Transformation:** `rule.resauth.transfer` (holder and, across threads, performer), `relocate-in`/`-out`, `rule.trust.rawptr` `[Rawptr-Move-*]` and `[Reclaim]`'s re-attachment (re-keyed to the performing thread, `CHG-0030`; never duplicated or dropped).
**Weakening:** none at the language level (shared ownership is a library pattern, `spec/21` §3).
**Invalidation:** consumed by `rule.resauth.destroy`/`destroy-composite`; forgotten only under `discharge: trusted` (`rule.trust.rawptr` `[Release]`, `[Deallocate]`).
**Checking:** premises of `transfer` and `destroy` (`static` via `rule.control.flow-analysis`, `dynamic` otherwise).
**Reliance points:** `rule.resauth.destroy`, `rule.control.block`/`stmt-exit` (D-0008's sweep). **Enforcement:** as Checking, plus the structural leak-freedom of `spec/07` §5.
**Failure behavior:** `diag.no-destroy-authority`, `diag.transfer-without-authority`, `diag.destroy-while-aliased`, `diag.move-while-aliased` (`checked`); leaks unreachable.
**Depends on:** term.resource, term.authority, inv.identity, D-0003, D-0008, D-0016, D-0019

---

## `inv.alias-validity`
**Status:** ACCEPTED
**Subjects:** simultaneous access paths to one object.
**Proposition (D-0018):** `∀ a, m. (an access of mode m is performed through a) ⇒ ¬clash(a, m, Σ)`, where `clash` (`spec/04` §2) holds iff a valid non-ancestor path with an overlapping target has a mode not jointly permitted with `m` (`permitted(m1,m2) ≝ m1 = shared ∧ m2 = shared`, D-0004), excluding the mutex exemption (`spec/19` §2).
**Establishment:** every path-forming rule records `base`; `rule.alias.borrow` additionally checks `¬clash` at formation.
**Preservation:** trivially — the proposition is re-evaluated at every access. **Transformation:** narrowing/borrowing derives new paths with a recorded ancestry. **Weakening:** none.
**Invalidation:** a path's participation ends with its temporal validity.
**Checking:** premises of `rule.value-object.read` (`¬clash(a, shared)`), `write` (`¬clash(a, exclusive)`), `rule.alias.borrow`, `rule.agg.match`; `discharge: static` via `rule.control.flow-analysis`, `dynamic` otherwise. `rule.resauth.destroy`/`transfer` require the stronger `solitary`.
**Reliance points:** every access. **Enforcement:** as Checking (`spec/08` §3).
**Failure behavior:** `diag.aliasing-conflict`, `diag.borrow-exceeds-source`, `diag.write-through-shared`, `diag.destroy-while-aliased`, `diag.move-while-aliased`; never silent.
**Depends on:** inv.identity, inv.temporal-validity, term.alias, term.access, D-0004, D-0018
**Affects:** inv.concurrency-validity

---

## `inv.trust-transition`
**Status:** ACCEPTED
**Subjects:** values from outside the program (`rule.trust.extern-call` results, raw storage).
**Proposition:** `∀ v. (v is used as a typed fact of type τ ∉ FfiType, or as a claim about storage) ⇒ origin-of-value(v,Σ) = internal ∨ transitioned(v,Σ) ∨ the use is under discharge: trusted`.
**Establishment:** `rule.trust.extern-call` marks results `unchecked-claim`; `rule.trust.extern-call` `[Trust-Transition]` (a checked validator, e.g. `spec/21` §2 `from_utf8`) or `rule.trust.rawptr` (explicit trust) transitions.
**Preservation:** a transitioned value is ordinary. **Transformation/Weakening:** none. **Invalidation:** none.
**Checking:** `[Extern-Non-Ffi-Type]` (`rejected`) confines what crosses; every safety-relevant use of an FFI scalar is checked by its consuming rule; every interpretation of foreign storage is `trusted` inside `unsafe` (`rule.trust.unsafe`, `rejected` outside).
**Reliance points:** `inv.spatial-validity`, `inv.initialization-validity` at raw-storage reads. **Enforcement:** as Checking.
**Failure behavior:** `diag.extern-non-ffi-type`, `diag.trusted-outside-unsafe` (`rejected`); a validator's `Err` (`fallible`).
**Depends on:** term.trust-boundary, term.semantic-fact, D-0017
**Affects:** inv.string.utf8-validity

---

## `inv.concurrency-validity`
**Status:** ACCEPTED
**Subjects:** accesses from different threads to one object.
**Proposition:** `∀ a1, a2, o. alive(o,Σ) ∧ thread-of(a1) ≠ thread-of(a2) ∧ (both access o) ⇒ synchronized(a1, a2, o, Σ)`, `synchronized` per `spec/04` §2: jointly permitted modes, or both under one thread's lock.
**Establishment:** `rule.conc.lock` sets `sync(o) := locked-by(ℓ)`. **Preservation:** while the guard lives. **Transformation:** `[Guard-Drop]` → `unlocked`. **Weakening:** none. **Invalidation:** with the mutex's end.
**Checking:** `clash` is thread-independent and evaluated at every access in every thread, so an unsynchronized conflicting pair is `diag.aliasing-conflict` at the second access regardless of interleaving; `[Lock]` blocks while another thread holds the lock; `rule.conc.spawn` rejects borrow-capturing closures.
**Reliance points:** every cross-thread access. **Enforcement:** as Checking (`spec/19`).
**Failure behavior:** `diag.aliasing-conflict`, `diag.mutex-reentrant-lock` (`checked`); `diag.spawn-borrow-closure` (`rejected`). Data races are unreachable; deadlock is out of scope.
**Depends on:** inv.alias-validity, term.access, D-0004, D-0018

---

## `inv.arith.range-validity`
**Status:** ACCEPTED
**Subjects:** results of default integer operations.
**Proposition:** `∀ op, v1, v2, τ. r = math-result(op, v1, v2) ⇒ r ∈ represented-domain(τ) ∨ the rule yields ↛`
**Establishment/Checking:** per operation, `rule.arith.checked`/`div`/`shift`/`neg`/`convert`/`literal` (`checked`; `rejected` for literals and constants via `rule.control.flow-analysis`).
**Preservation/Transformation/Weakening/Invalidation:** N/A (checked fresh).
**Reliance points:** `inv.spatial-validity` wherever arithmetic feeds an index or length.
**Failure behavior:** `diag.arith-overflow`, `diag.div-by-zero`, `diag.div-overflow`, `diag.shift-amount-out-of-range`, `diag.narrowing-overflow`, `diag.literal-out-of-range`.
**Depends on:** term.type, D-0002

---

## `inv.string.utf8-validity`
**Status:** ACCEPTED
**Subjects:** live `String` objects.
**Proposition:** `∀ o. alive(o,Σ) ∧ type-of(o) = String ⇒ well-formed-utf8(bytes of o.bytes)`
**Establishment:** `rule.stdlib.string` `from_utf8`'s validator, or `from_str`'s byte copy from a `str` value (whose bytes are well-formed by `inv.str.utf8-validity`) — the two constructors.
**Preservation:** no byte-mutation primitive exists. **Transformation/Weakening/Invalidation:** none.
**Checking:** `discharge: dynamic` in `from_utf8`; `discharge: static` in `from_str` (inherited from `inv.str.utf8-validity`). **Reliance points:** any future text operation. **Enforcement:** by construction.
**Failure behavior:** `Err(Utf8Error)` (`fallible`) from `from_utf8`; `from_str` cannot fail.
**Depends on:** inv.trust-transition, inv.str.utf8-validity, D-0006

---

## `inv.str.utf8-validity`
**Status:** ACCEPTED
**Subjects:** every value of type `str` (`type.str`, `spec/21` §2a).
**Proposition:** `∀ v. v : str ⇒ well-formed-utf8(bytes of v)`
**Establishment:** `rule.stdlib.str` `[Str-Literal]`: the only way to form a `str` is a `str-literal` (`spec/22` §1), whose grammar admits only characters of UTF-8 source text and escapes that each encode one Unicode scalar value — no byte escape exists in that literal form.
**Preservation:** `str` is a value (`term.value`), not an object: `[Read]` copies it whole, no operation writes into it, and no access path reaches its bytes. **Transformation/Weakening/Invalidation:** none.
**Checking:** `discharge: static` — by the grammar, before any semantic rule runs. **Reliance points:** `String::from_str` (`inv.string.utf8-validity`'s establishment); any future text operation on `str`. **Enforcement:** by construction.
**Failure behavior:** none at run time; an ill-formed literal does not parse.
**Depends on:** type.str, term.value, D-0020

## Change Log

- 1.3.0 — `CHG-0030`: `inv.resource-authority`'s transformations list
  `[Reclaim]`'s re-attachment, which now re-keys a reclaimed object's
  destroy authority to the reclaiming thread.
- 1.2.0 — `CHG-0025` (D-0020): `inv.str.utf8-validity` added for the
  new `str` value type, established statically by the literal grammar.
  `inv.string.utf8-validity`'s establishment names both constructors
  (`from_utf8`'s validator, and `from_str` carrying the `str`
  invariant over); its proposition is unchanged.

- 1.1.0 — `CHG-0011`: `inv.resource-authority`'s single-owner clause
  admits reclaimed objects, which have no holder and are not
  temporaries; every `Vec` element reclaimed by `index_shared` violated
  the clause as stated (found deriving `ex.e2e-vec-nested-realloc`).
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-01, B-19):
  `inv.alias-validity` restated as `¬clash` at every access;
  `inv.temporal-validity`'s invalidation events enumerated;
  `inv.resource-authority` gains the single-owner obligation clause;
  `inv.concurrency-validity` and `inv.trust-transition` restated
  against `spec/19`/`spec/20`; every entry names its enforcing rules
  by id; all promoted to `ACCEPTED`.
- 0.2.0 and earlier — superseded.
