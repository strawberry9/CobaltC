# CHG-0022 — `feat.explicit-lifetime-parameters` Decided: `REJECTED`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §7, §9, §18, §21
Depends on: inv.temporal-validity, D-0011, rule.temporal.ref-escape, rule.temporal.elision
Affects: feat.explicit-lifetime-parameters, rule.temporal.elision

## Problem / motivation

`feat.explicit-lifetime-parameters` (`spec/registry/features.md`) sat
`UNDER_INVESTIGATION`: `rule.temporal.elision`'s single-parameter
elision (D-0011) has no answer for a function with zero or several
reference-typed parameters returning a reference —
`[Call-Multi-Ref-Return-Rejected]` rejects the *declaration* outright.
Three candidate mechanisms were prepared (`CHG-0017`): (a) named
lifetime parameters (Rust-shaped); (b) accept the zero-parameter case
unconditionally, reject only genuinely-ambiguous multi-parameter
cases; (c) keep the permanent rejection, requiring callers to
decompose into single-reference-parameter helpers. The human owner
directed a decision rather than leaving this open, delegating the
choice among (a)/(b)/(c) to the design agent.

## Decision

**(c) selected — the existing boundary stays, permanently.** (a) and
(b) rejected.

**Why (c), not (a):** no construct anywhere in `spec/21` (the only
library corpus this specification has) needs a zero-or-multi-reference-
parameter return — both of the feature's own motivating examples are
constructed illustrations, not encountered needs. Master Instructions
§9's minimal doctrine is explicit that a mechanism is added when
something concretely forces it, not pre-emptively; §7 additionally
warns against assuming a named-lifetime-annotation solution is needed
before a real problem demonstrates it. (a) is exactly that
assumption — it reintroduces the lifetime-annotation syntax and
learning burden (`<'a>`-shaped binders, substitution through generics)
the design has avoided everywhere else, for zero currently-encountered
cases.

**Why (c), not (b):** (b)'s own write-up already identifies its cost
correctly — accepting the *zero*-reference-parameter case
unconditionally requires proving "the returned reference's origin is
independent of every parameter," which is intraprocedural dataflow
*inside the function body*, not a signature-level check. No such
body-level provenance analysis exists anywhere in `rule.control.flow-
analysis` or `rule.temporal.*` today (both are explicitly scoped to
what a call site or the signature already states); building one for
this alone is new mechanism for a case with, again, zero encountered
instances. The asymmetry the feature write-up drew between "zero
parameters" (safe) and "several parameters" (ambiguous) does not
change this: proving the zero-parameter case safe is not free just
because the *answer* is simple — the analysis needed to reach that
answer does not exist.

**Cost symmetry confirming the choice:** every cost field in the
original proposal that has a real (non-"none") value points the same
direction — (a) is "highest on every axis," (b) is "low-cost but
partial" and still requires new mechanism, (c) requires literally
nothing (it is already the implemented behavior). Safety is identical
across all three (D-0018's dynamic baseline already covers every
rejected program soundly) so no invariant is traded away by declining
(a)/(b).

## What changed

**No rule changed.** `[Call-Multi-Ref-Return-Rejected]` already
implements (c); this record makes its permanence a decided design
fact rather than an open question.

**`spec/registry/features.md` 1.3.0**: `feat.explicit-lifetime-
parameters` moved `UNDER_INVESTIGATION → REJECTED`, citing this
record.

**`spec/10-temporal-validity.md` 1.0.3** (non-normative, `rule.temporal.
elision`'s closing note): restated to say the feature was considered
and `REJECTED`, not merely "recorded, undesigned" — a caller needing
the shape composes single-reference-parameter helpers, which is
already fully expressible today.

## Rule changes

None.

## Affected invariants

None restated. `inv.temporal-validity` is unaffected — the boundary
this record confirms was already how the invariant is enforced for
this shape.

## Dependency impact

`feat.explicit-lifetime-parameters` gains a `Depends on` citation of
this record. No other entity's meaning changes.

## Compatibility classification

None (no rule, example, or conformance case changes; a prose-only
confirmation of already-implemented behavior).

## Migration implications

None. No previously-accepted program's meaning changes; no previously-
rejected program becomes accepted.

## Example changes

None.

## Conformance changes

None. `[Call-Multi-Ref-Return-Rejected]`'s existing behavior is
unchanged; no new conformance case is needed to demonstrate a decision
that adds no new mechanism.

## Future implementation implications

None beyond what is already required — a conforming implementation
already rejects the zero/multi-reference-parameter return shape at
the declaration.

## Prior-art status

(a) is directly Rust-shaped (named lifetime parameters); declining it
keeps CobaltC's temporal-validity mechanism entirely elision/dynamic-
baseline-driven, with no named-lifetime surface syntax at all — a
deliberate divergence from Rust, not an oversight.

## Revisit conditions

Revisit only if a real construct (in `spec/21`'s corpus, or a
concrete program the owner brings) actually needs a function with
zero or several reference parameters returning a reference and
composing single-reference-parameter helpers is demonstrated to be
inadequate for it — not on any schedule, and not for hypothetical
motivation alone.
