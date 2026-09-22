# D-0006 — Nominal Type Identity; No Implicit Conversion

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §11, §17
Depends on: inv.resource-authority, D-0002

## Problem

The Type System artifact needs a type-equality principle (when are two
types "the same type") and a stance on implicit conversion/subtyping,
before `is-resource(τ)`, value equality, or any future function-signature
rule can be stated precisely.

## Candidate mechanisms

1. **Structural type identity** (two types are equal iff their
   definitions are structurally identical). Rejected: under D-0003,
   `is-resource(τ)` and destroy-authority are attached per type; a
   structural scheme would let two independently-declared types with
   identical layout be silently treated as the same type, which could
   make a resource-bearing type and a plain-data type with the same
   layout interchangeable — a direct route to the "false claim becomes
   trusted" failure §6 warns against (a plain-data copy of a
   resource-shaped value, treated as if it still carried authority, or
   vice versa).
2. **Nominal type identity** (two types are equal iff they are the same
   declaration — same `type.<name>` id, per `spec/02-schema.md`).
   Already the de facto scheme every type introduced so far uses (each
   has a distinct `type.*` id); makes `is-resource` an unambiguous
   per-declaration fact. **Selected.**
3. **Implicit conversion/subtyping permitted** (e.g. automatic
   widening, automatic reference-to-value coercion). Rejected, extending
   D-0002's precedent: every conversion introduced so far
   (`widen`/`narrow`/`reinterpret`, `spec/06-arithmetic.md`) is an
   explicitly named operation, never automatic — adopting implicit
   conversion elsewhere would make the type system's cost model and
   `is-resource`/authority flow harder to read at a call site (§8
   priority #7, comprehensibility) for no invariant-preservation
   benefit.
4. **No implicit conversion or subtyping anywhere** — every type change
   is an explicit, named operation (widen/narrow/reinterpret for
   numerics today; more as later artifacts need them). **Selected.**

## Selected design

Nominal type identity (`type.<name>` id is the identity); no implicit
conversion or subtyping. Every future type-changing operation must be
explicitly named, following D-0002's precedent.

## Rejected alternatives

Structural identity (1); implicit conversion/subtyping (3).

## Semantic rationale

Both selections protect the same thing: that a value's type is never
ambiguous or silently reinterpreted, which is what lets `is-resource(τ)`
and authority-flow analysis (D-0003) be sound per-declaration facts
rather than per-shape facts that could be gamed by two coincidentally
identical layouts.

## Usability / Explainability / Implementation-feasibility

Usability: slightly more explicit code at conversion points; standard
tradeoff already accepted for arithmetic in D-0002. Explainability: a
type error always names the exact declared type, never "a type shaped
like X." Implementation-feasibility: nominal identity is a direct
lookup; no structural-equivalence algorithm is needed.

## Compatibility impact

None yet; establishes precedent for all future type-introducing
artifacts (Aggregates, Modules).

## Prior-art status

Nominal typing and explicit-only conversion are each well-established
elsewhere; retained here because independently required by D-0003's
per-declaration authority model, not adopted for familiarity.

## Invariant traceability

Protects `inv.resource-authority` (via unambiguous `is-resource`) and
`inv.arith.range-validity` (via D-0002's precedent, now generalized).

## Revisit conditions

Revisit if Aggregates or Modules finds a legitimate need for structural
typing (e.g. anonymous tuple types) that nominal identity cannot express
without excessive declaration ceremony — in that case, a structural
exception would need its own derivation, not a reversal of this
decision's core rationale for named, resource-bearing types.

## Candidate-set review (`CHG-0024`)

`spec/AUDIT-2.md` B-26 named this decision alongside D-0007–D-0009 as
having a thin candidate set; checked and found not applicable here —
this record already considers 4 named candidates (two orthogonal
questions, type identity and conversion, each with its own rejected
alternative), the widest set of the four B-26 named. No further action
taken on this record; closed by the same review as the other three.
