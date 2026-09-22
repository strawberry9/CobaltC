# CobaltC Semantic Terminology

Status: normative artifact
Version: 1.0.0
Governed by: `CobaltC_Master_Instructions.md` §1
Conforms to: `spec/02-schema.md` (Kind: Term)

## Purpose

The authoritative vocabulary every other artifact uses. Terms define
what must be true, not how it is enforced (Master Instructions §7);
each term names the artifact that grounds it in `Σ`.

## Terms

### `term.program`
**Definition:** A finite set of items (`spec/22` §3) in one root module, with exactly one `main` (`rule.fn.program`).
**Status:** ACCEPTED

### `term.specification`
**Definition:** The versioned collection of normative artifacts under `spec/` that together define CobaltC (Master Instructions §13). Prose is explanatory unless it is a CFN rule or a field of an entity.
**Status:** ACCEPTED

### `term.semantic-fact`
**Definition:** A proposition about a value, object, access path, or `Σ` whose truth a later operation relies on.
**Status:** ACCEPTED

### `term.invariant`
**Definition:** A semantic fact that CobaltC requires to remain true while relied upon, with the lifecycle `spec/03` records for it.
**Status:** ACCEPTED
**Depends on:** term.semantic-fact

### `term.value`
**Definition:** An element of a type's represented domain (`spec/06` §7's `Value` grammar), independent of storage. Two values are the same iff `=_τ` (`rule.type.eq`) holds.
**Status:** ACCEPTED
**Depends on:** term.type

### `term.object`
**Definition:** An identity in `dom(Σ.objects)`: storage (its extent) together with a type and an initialization state (`state.objects`, `state.init`). Distinct from the value it holds.
**Status:** ACCEPTED
**Depends on:** term.storage, term.value, term.type

### `term.temporary-object`
**Definition:** A live object with no holder and fresh storage (`temporary(o, Σ)`, `spec/04` §2): produced by aggregate construction, calls, `spawn`, `relocate-out`; owned by the statement that created it until stored (D-0019).
**Status:** ACCEPTED
**Depends on:** term.object

### `term.storage`
**Definition:** Addressable cells (`state.storage`), each `uninit` or holding a datum (`spec/06` §7), existing independently of whether any object claims them.
**Status:** ACCEPTED

### `term.binding`
**Definition:** An entry of `state.bindings`: a name in a frame mapped to the root access path that owns an object.
**Status:** ACCEPTED
**Depends on:** term.access-path, term.object

### `term.identity`
**Definition:** The key of `state.objects`: what distinguishes one object from every other, independent of contents.
**Status:** ACCEPTED
**Depends on:** term.object

### `term.origin`
**Definition:** The establishing event of an object (`state.objects(o).origin`), fixed for its life.
**Status:** ACCEPTED
**Depends on:** term.object

### `term.extent`
**Definition:** The address set an object's storage occupies (`state.objects(o).extent`); access outside it is spatially invalid.
**Status:** ACCEPTED
**Depends on:** term.storage, term.object

### `term.region`
**Definition:** Informal: a set of objects sharing a validity determinant (a frame, a raw allocation). No `Σ` component; frames (`state.frame-stack`) and allocations (`rule.stdlib.prelude` `[Allocate]`) are the two instances.
**Status:** ACCEPTED
**Depends on:** term.storage

### `term.temporal-validity`
**Definition:** Whether an access path may still be used (`temporally-valid(a, Σ)`), as distinct from whether its storage exists.
**Status:** ACCEPTED
**Depends on:** term.access-path

### `term.resource`
**Definition:** An object (or sub-range, `ResourceId`) whose type has `is-resource = true`: it carries a destroy obligation and single-holder destroy authority (D-0003).
**Status:** ACCEPTED
**Depends on:** term.authority, term.object

### `term.authority`
**Definition:** An entry of `state.authority`: that a thread may perform an operation (`destroy`) on a resource, once.
**Status:** ACCEPTED

### `term.alias`
**Definition:** Two valid access paths targeting overlapping storage of one object.
**Status:** ACCEPTED
**Depends on:** term.access-path

### `term.access`
**Definition:** A read or write through an access path (`rule.value-object.read`/`write`).
**Status:** ACCEPTED
**Depends on:** term.access-path

### `term.access-path`
**Definition:** An entry of `state.access-paths`: a route to a sub-range of an object with its own mode, validity, ancestry (`base`), and holders (`held-by`). Roots are bindings, reclaims, and lock paths; borrows and projections derive from a base (D-0018).
**Status:** ACCEPTED
**Depends on:** term.object

### `term.place`
**Definition:** An evaluation result that is an access path (`place a`), produced by a name, `*e`, `e.f`, `e[i]`, `reclaim` (`spec/13` §1).
**Status:** ACCEPTED
**Depends on:** term.access-path

### `term.initialization-state`
**Definition:** `Σ.init(o) ∈ {uninitialized, valid}`: whether an object's storage holds a value of its type.
**Status:** ACCEPTED
**Depends on:** term.object

### `term.trust-boundary`
**Definition:** A point where data enters from outside the program's rules: `extern` calls and raw storage (`spec/20`). Data crossing it is a claim until validated or explicitly trusted.
**Status:** ACCEPTED
**Depends on:** term.semantic-fact

### `term.type`
**Definition:** A `type.<name>` entity with a represented domain, `is-resource`, `sizeof`/`alignof`, and representation (`spec/06`, `spec/12`, `spec/16`, `spec/19`, `spec/20`).
**Status:** ACCEPTED

### `term.module`
**Definition:** A named container of items governing visibility and qualified names (`spec/17`).
**Status:** ACCEPTED

### `term.function`
**Definition:** A `fn` item or closure: a callable whose signature states, for every parameter, whether the call takes, lends, or copies (`spec/15` §3).
**Status:** ACCEPTED

### `term.static-semantics`
**Definition:** Everything decidable from program text: `rule.type.typing`, every `disposition: rejected` rule, `rule.control.flow-analysis`, `rule.temporal.ref-escape`.
**Status:** ACCEPTED

### `term.runtime-semantics`
**Definition:** The `→^ℓ` reduction relation over `Σ` from `Σ_0` (`rule.fn.program`), including every `discharge: dynamic` check.
**Status:** ACCEPTED

### `term.diagnostic`
**Definition:** A `diag.<name>` entity (`spec/registry/diagnostics.md`) produced by a `↛` judgment or a static rejection.
**Status:** ACCEPTED
**Depends on:** term.invariant

### `term.conformance`
**Definition:** An implementation conforms iff for every program its observable behavior (`rule.fn.program`) is one the rules admit, it rejects exactly the statically ill-formed programs, and every case in `spec/conformance.md` holds.
**Status:** ACCEPTED
**Depends on:** term.specification

### `term.safe-program`
**Definition:** A well-formed program containing no `unsafe { }` block. Such a program never reaches a `discharge: trusted` side-condition (`rule.trust.unsafe`), and therefore (Master Instructions §12): every reachable `Σ` of its execution satisfies every proposition in `spec/03-invariants.md`.
**Status:** ACCEPTED
**Depends on:** term.program

### `term.implementation`
**Definition:** A realization of the specification able to execute programs. Building one is outside the design AI's authority (Master Instructions §1).
**Status:** ACCEPTED

## Change Log

- 1.0.0 — Definitions tightened to name their `Σ` grounding; added
  `term.temporary-object`, `term.place`; `term.safe-program`
  simplified (extern calls already require `unsafe`); every term
  promoted to `ACCEPTED` (`spec/AUDIT-2.md` B-19).
- 0.1.3 and earlier — superseded.
