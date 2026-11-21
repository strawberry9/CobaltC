# D-0010 — Generics: Monomorphic Type/Function Parameters

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §5, §7, §9, §11
Depends on: `spec/12-type-system.md` §1, D-0006

## Problem

`feat.user-defined-generics` (`spec/registry/features.md`) was left
`UNDER_INVESTIGATION` pending Function Semantics and Aggregates, on the
grounds that evaluating it earlier would mean designing generics before
having anything to state their signatures over. Both now exist
(`spec/15`, `spec/16`). CobaltC needs user-declared type/function
parametrization beyond the single built-in constructor `ref`.

## Constraints

- §5: "predictable data representation where requested" — a mechanism
  that erases type information at runtime (uniform representation,
  dynamic dispatch) works against this for a systems language.
- §9: derive capability before feature; the existing `Type → Type` kind
  for `ref` (`spec/12` §1) should generalize rather than be replaced.
- §7: mechanism-neutral derivation — do not adopt a specific generics
  strategy merely because other languages use it.
- Must compose with `is-resource` (D-0006/`spec/12` §3,
  `spec/16` §1's lower-bound refinement) and with D-0003's authority
  model without special-casing.

## Candidate mechanisms

1. **Type erasure / uniform representation** (all instantiations share
   one runtime representation, e.g. via boxing or a vtable). Rejected:
   directly conflicts with §5's predictable-representation requirement
   — a `Box<i32>` and a `Box<Vec<u8>>` would need the same physical
   layout, forcing indirection even where none is needed, and
   complicates `is-resource`'s derivation (a boxed value's resource-ness
   would not be a static, per-instantiation fact).
2. **Dictionary-passing / runtime type parameters** (a hidden parameter
   carrying per-instantiation operations, resolved at call time).
   Rejected: adds a runtime mechanism and calling-convention complexity
   with no invariant this specification has identified needing it;
   conflicts with §9 (spends complexity budget unforced).
3. **Monomorphization**: each distinct instantiation of a generic
   struct/enum/function is treated, for typing and `is-resource`
   purposes, as if the declaration were specialized to those concrete
   types — no runtime representation of "which instantiation this is"
   exists; `Box<i32>` and `Box<Vec<u8>>` are simply two unrelated
   concrete types that happen to share a declaration. **Selected.**

## Selected design

- The kind system (`spec/12-type-system.md` §1) generalizes from
  `Type → Type` (the one case, `ref`) to `Type^n → Type` for
  user-declared struct/enum constructors and `∀T1..Tn. (τ1,...,τn) →
  τr` for generic functions — the same shape already in use, extended
  to arbitrary declared names, not a new kind of kind.
- Instantiation `Name<τ1,...,τn>` (or, for functions, explicit
  `name<τ1,...,τn>(...)`) substitutes concrete types for parameters;
  the result is typed and `is-resource`-classified exactly as if the
  declaration had been written out for those specific types (§16 §1's
  lower-bound rule applies per-instantiation).
- **No trait/interface/bound system is introduced.** A generic
  declaration's body may only use its type parameters in ways valid for
  *any* type (store, move, pass by reference) — no arithmetic, no
  method calls on a parametrized value. This is a real, named
  limitation (see Deferred, `spec/12`), not silently assumed away.
- Explicit instantiation only; no type-argument inference. Simpler,
  smaller mechanism (§9); inference is a pure ergonomic addition that
  can be layered on later without changing the underlying semantics.

  **Partially superseded by `spec/decisions/D-0012-local-type-
  inference.md`:** argument-directed inference (a type parameter that
  appears in some parameter's declared type is inferred from the
  corresponding call argument) is now available, narrowing this bullet
  to the residual case — a type parameter appearing in no parameter
  position still requires explicit instantiation. This record is left
  otherwise unedited per `spec/02-schema.md` §6 (an `ACCEPTED` decision
  is superseded, not rewritten); D-0012 is the authoritative statement
  of the current rule.

## Rejected alternatives

Type erasure (1); dictionary-passing (2).

## Semantic rationale

Monomorphization is the only candidate that keeps `is-resource` a
per-concrete-type static fact (D-0006's whole point) without
introducing a new representation-uniformity requirement §5 explicitly
argues against. It also requires no new runtime mechanism: every rule
already in the specification (construction, destruction, borrowing,
arithmetic) applies unchanged to a monomorphized instantiation, because
after substitution there is nothing structurally different about it
from a hand-written concrete declaration.

## Usability / Explainability / Implementation-feasibility

Usability: explicit instantiation is more verbose than inference would
be; accepted as the smaller mechanism per §9, revisitable. Explainability:
a type error in a generic body is reported against the *instantiated*
concrete types, not an abstract parameter — never ambiguous.
Implementation-feasibility: monomorphization is directly checkable by
substitution and re-running already-`ACCEPTED` rules; no new analysis
required.

## Compatibility impact

None yet; `feat.user-defined-generics` moves `UNDER_INVESTIGATION` →
`ACCEPTED` (`spec/registry/features.md`).

## Prior-art status

Monomorphization is well-established prior art; retained here because
independently the only candidate compatible with D-0006 and §5, not
adopted for familiarity — type erasure and dictionary-passing were
considered and rejected on their merits above, not skipped.

## Invariant traceability

Composes with `inv.resource-authority` (via per-instantiation
`is-resource`) and `D-0006` (nominal identity — each instantiation is
its own concrete, nominally-identified type).

## Revisit conditions

Revisit if a demonstrated need arises for polymorphism over an
operation set (arithmetic-generic code, etc.) that only a trait/bound
system could express — that would be a new, separately-derived feature,
not a reversal of this decision.
