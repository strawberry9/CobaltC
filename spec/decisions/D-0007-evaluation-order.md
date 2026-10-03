# D-0007 — Sub-expression Evaluation Order

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §19
Depends on: inv.alias-validity

## Problem

Compound expressions (`e1 op e2`) need a defined order in which
sub-expressions are evaluated, since sub-expression evaluation can have
side effects (writes, borrows, destroys) that later sub-expressions or
the operation itself may observe.

## Candidate mechanisms

1. **Unspecified order** (classic C: the compiler may choose any order,
   differing between operands of the same expression across
   implementations or even calls). Rejected: directly conflicts with
   §19's deterministic-diagnostics goal and §12's general preference
   against implementation-defined behavior for anything not forced —
   here nothing forces it; a fixed order costs nothing.
2. **Deterministic, strictly left-to-right.** Every sub-expression is
   fully evaluated (to a value or access path) before the next one
   (left to right in source order) begins. `outcome: deterministic` for
   evaluation order itself — not an instance of CFN's `impl-defined`/
   `unspecified` outcome tags, since nothing is left open. **Selected.**

## Selected design

Strict left-to-right evaluation, encoded directly in the evaluation
context grammar (`spec/13-expression-semantics.md` §1) rather than
stated as a side condition: the grammar shape itself only ever permits
reducing the leftmost non-value sub-expression.

## Rejected alternatives

Unspecified/implementation-chosen order (1).

## Semantic rationale

An unspecified order is a standing source of exactly the "alias
conflict now, alias conflict never, depending on the compiler" class of
bug `inv.alias-validity`/`inv.temporal-validity` exist to rule out
statically or dynamically — leaving evaluation order open would
reintroduce nondeterminism at a layer below where those invariants are
checked, for no benefit (§8 priority order places performance below
invariant preservation, and a fixed left-to-right order costs nothing
compared to leaving it open).

## Usability / Explainability / Implementation-feasibility

Usability: predictable side-effect ordering. Explainability: a
diagnostic can name "the second operand, evaluated after the first" with
no ambiguity. Implementation-feasibility: strictly easier than
unspecified order (fewer valid implementation strategies to support).

## Compatibility / Prior-art / Invariant traceability

None yet impacted; independently derived, though left-to-right is also
the common choice elsewhere. Supports `inv.alias-validity` and
`inv.temporal-validity` by keeping side-effect order decidable wherever
those invariants' static checks need to reason about ordering.

## Revisit conditions

None anticipated; revisit only if Concurrency introduces expression-level
parallelism that would make "left-to-right" ill-defined (unlikely, since
concurrency is expected to be explicit at the statement/task level, not
implicit within one expression).

## Candidate-set review (`CHG-0024`)

`spec/AUDIT-2.md` B-26 flagged this decision's candidate set as thin
(one alternative). Reviewed, not expanded: the rejected alternative
(unspecified order) was eliminated by direct appeal to two already-
`ACCEPTED` doctrine sections (§19 determinism, §12's anti-impl-defined
default), not by a shallow search — a wider survey has no third
live candidate to find once determinism is required (an order is
either fixed or it is not). Five verification passes since have found
no case where left-to-right produced a wrong or awkward derivation.
The human owner reviewed this finding and closed it without directing
new candidates be added.
