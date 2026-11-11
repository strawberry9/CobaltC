# D-0001 — Formal Semantic Metalanguage Selection

Status: ACCEPTED (as a design decision; the notation itself is versioned
independently in `spec/01-metalanguage.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §11 (Required Design
Derivation), §22 (Design-Decision Records), §13 (Normative Specification)
Depends on: term.specification, term.invariant, term.semantic-fact

## Problem

The CobaltC Specification (`term.specification`) must express static and
runtime semantics, invariant lifecycles (`term.invariant`), and
diagnostics precisely enough that two independent conforming
implementations cannot make materially different semantic decisions from
the same normative text (Master Instructions §20). §13 requires a
defined, versioned formal semantic metalanguage for this — prose alone is
explicitly disallowed as the normative vehicle.

## Constraints

Drawn from Master Instructions:

- Must express, at minimum: logical relations, equality/ordering,
  sets/membership, quantification, state lookup, identity/origin,
  temporal relations, arithmetic relations, type/inference judgments,
  state transitions, preconditions/postconditions, error judgments, proof
  obligations, permitted nondeterminism/environment dependence (§13).
- Must support the invariant lifecycle model: establishment,
  representation, preservation, transformation, weakening, invalidation,
  checking, reliance, failure behavior (§6, §15).
- Must support local, compositional reasoning about aliasing and
  concurrent access (§6, §17) — must not force restating the entire
  world's state to reason about one object or resource.
- Must remain mechanism-neutral: must not presuppose ownership,
  borrowing, GC, regions, capabilities, or any specific concurrency model
  (§7).
- Must support the closed disposition set for invariant-reliant
  operations required by §12: statically rejected, dynamically checked,
  explicitly fallible, explicitly trusted/unchecked, or unsupported —
  with no open-ended "undefined behavior" category available to safe
  programs.
- Must keep the specification queryable per §19 (which rule governs,
  what invariant is required, what invalidated it, is a runtime check
  required, etc.).
- Must remain authorable and reviewable by the design AI and the human
  owner without becoming an unreviewable complexity sink in its own
  right.

## Relevant invariants

All invariant dimensions enumerated in §6 (meaning, mathematical
validity, spatial validity, identity, origin/provenance, temporal
validity, resource authority, aliasing validity, initialization
validity, trust validity, concurrency validity, preservation through
lowering) must be statable as propositions in whatever assertion language
is chosen, since the forthcoming Invariant Registry (§15) will be written
in it.

## Invalid states to prevent

- A metalanguage expressive enough to *state* rules but not precise
  enough to make gaps *visible* — defeats the §20 completeness
  requirement, since an implementer could satisfy the letter of a rule
  while violating unstated intent.
- A metalanguage that forces restating unrelated global state to reason
  locally about one object or resource — makes aliasing/authority-transfer
  rules unreadable and error-prone, working against §6/§17.
- A metalanguage with an open "undefined behavior" escape usable from
  anywhere — directly violates §12.

## Candidate mechanisms considered

1. **Prose with ad hoc math notation.** Rejected outright by §13; this is
   the failure mode the constitution names explicitly.

2. **Pure denotational semantics** (meaning as mathematical functions
   state → state). Precise and compositional, but: awkward for exposing
   the intermediate/interleaved states needed for concurrency validity;
   awkward for treating diagnostics/error judgments as first-class
   results rather than encoding failure into the domain ad hoc; high
   authoring cost relative to current benefit.

3. **Pure big-step (natural) operational semantics.** Readable, good for
   straightforward evaluation to a final value. Rejected as the *base*
   semantics because a big-step judgment evaluates a construct to
   completion in one relation instance, hiding exactly the intermediate
   states where aliasing and concurrent-access invariants are checked or
   violated (§6, §17). Retained only as derived sugar over the selected
   small-step relation (see below).

4. **Small-step structural operational semantics (SOS)** over an
   explicit abstract machine state. Exposes every intermediate
   configuration, so any invariant check or violation is directly
   statable as a proposition over the state at that step. Composes
   naturally with concurrency via interleaving of independent threads'
   steps. Its shape already matches §6's invariant-lifecycle diagram,
   which is itself a state-transition picture. **Selected as the base
   runtime-semantics judgment form.**

5. **Pure axiomatic/Hoare-style assertions** as the sole semantic
   vehicle (no separate operational layer). Matches the
   "established / preserved / relied upon" vocabulary of §6 directly,
   but alone gives no account of *how* state evolves step by step
   (needed for `term.runtime-semantics`), and has the well-known framing
   problem for local reasoning about aliasing/ownership unless extended.
   Not selected as the sole layer.

6. **Small-step SOS + a separation-style assertion language** layered on
   top, using a separating-conjunction connective (`∗`) to state
   disjoint ownership of resources/storage within pre/postconditions and
   invariant propositions. Combines item 4's explicit step-by-step state
   evolution with item 5's ability to state invariants declaratively,
   while gaining local/compositional reasoning about aliasing and
   resource authority. **Selected.**

## Selected design

- **Base runtime semantics:** small-step SOS over an explicit, named
  abstract machine state `Σ`. `Σ`'s internal components are *not* fixed
  by this decision — per the §23 design order, the Abstract Semantic
  State artifact comes after the specification schema and invariant
  registry. This document only commits to `Σ` being a structured,
  componentized value that later artifacts extend.
- **Assertion language:** used for static judgments, invariant
  propositions, and pre/postconditions; includes standard logical
  connectives, quantifiers, set/relation notation, state lookup, and a
  separating conjunction `∗` for disjoint-ownership statements.
- **Safety-disposition taxonomy:** closed, five-valued, definitionally
  identical to §12's list — `rejected`, `checked`, `fallible`,
  `trusted-unchecked`, `unsupported` — attached to rules as an explicit
  tag rather than left implicit in prose.
- **Outcome-determinism taxonomy:** closed, three-valued —
  `deterministic`, `impl-defined` (over a documented finite set the
  implementation must commit to and expose), `unspecified` (over a
  documented finite set of safe outcomes, chosen freely per execution) —
  for cases where safety is already established but the *value* may
  legitimately vary. No fourth "undefined" value exists in this
  taxonomy; anything not safety-established falls under the
  safety-disposition taxonomy above instead (typically
  `trusted-unchecked`), never under outcome-determinism.
- **Big-step evaluation** is defined only as derived notation (the
  reflexive-transitive closure of the small-step relation, restricted to
  a single thread of control), for use in examples and rules where
  interleaving is provably irrelevant. It is never the primitive
  definition of anything normative.

## Rejected alternatives

Prose-only (item 1); pure denotational (item 2); pure big-step as the
*base* (item 3 — retained only as derived sugar); pure axiomatic without
an operational layer (item 5 — retained only as the assertion
sublanguage layered on the operational base, not as a replacement for
it).

## Semantic rationale

§6's invariant lifecycle model is already a state-transition diagram
(fact → representation → establishment → preservation/transformation →
reliance). A small-step operational base makes that diagram literally
the shape of the semantics: every step is a candidate point of
establishment, preservation, transformation, or invalidation, and the
assertion language lets any invariant be stated as a proposition true or
false of the state at that step.

Separating conjunction is retained from prior art rather than invented
fresh, because CobaltC's central concerns — resource authority, aliasing
validity, spatial validity — are precisely the class of properties
separation logic was independently developed to make locally and
compositionally statable. Per §7, retaining a technique because
independent reasoning confirms it is the strongest fit for the problem
is not the same as adopting it for familiarity, and is explicitly
permitted.

## Usability implications

Authoring rules requires comfort with inference-rule notation and
separating conjunction. This cost is paid by the design AI and human
reviewers of the specification — it does not leak into CobaltC's surface
syntax, which is designed independently and later, per §10.

## Explainability implications

Every rule is readable premise-by-premise; every invariant claim is a
state-indexed proposition. This makes §19's queryability list (which
rule governs, where an invariant was established, what invalidated it,
whether a runtime check is required) answerable by locating the relevant
judgment instance rather than by interpreting prose.

## Implementation-feasibility implications

A small-step relation over an explicit abstract state is directly
amenable to the implementation-feasibility analysis required by §20: an
analyst can mechanically check whether a candidate implementation's
observable transitions are consistent with the relation, without an
implementation needing to exist yet.

## Invariant traceability

This decision governs notation, not any specific invariant. It is a
precondition for every future Invariant Registry (§15) entry to be
statable at all.

## Revisit conditions

Revisit if a required expressiveness gap is found (a §6 invariant
dimension, or a §13 required capability, that cannot be stated in this
assertion language), or if authoring/reviewing rules at scale proves
impractical.
