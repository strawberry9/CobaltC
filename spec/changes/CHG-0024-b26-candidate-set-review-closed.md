# CHG-0024 — `spec/AUDIT-2.md` B-26 Reviewed and Closed

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §11, §21
Depends on: D-0006, D-0007, D-0008, D-0009
Affects: D-0006, D-0007, D-0008, D-0009

## Problem / motivation

`spec/AUDIT-2.md` finding B-26: "every decision converges on the same
design as one well-known prior-art language; candidate sets in
`D-0006`–`D-0009` are thin" — recorded for the human owner, not
treated as a defect by the audit itself. `spec/AUDIT-STATUS.md`'s
tenth pass checked the claim against the actual records: `D-0006`
(type identity/conversion) considered 4 named candidates (two
orthogonal questions); `D-0007` (evaluation order), `D-0008`
(scope-end destruction), and `D-0009` (failure channels) each
considered exactly 2 — the selected design and one alternative, not
the "multiple conceptually distinct candidates" Master Instructions
§11 asks for on foundational problems. The human owner directed a
decision on whether to invest effort backfilling additional candidates
for `D-0007`–`D-0009`, delegating the decision itself to the design
agent.

## Decision

**Close without expanding any candidate set.** No new candidate is
added to `D-0006`, `D-0007`, `D-0008`, or `D-0009`.

The apparent thinness does not reflect an unconsidered gap in any of
the three two-candidate decisions: in each case, the rejected
alternative was eliminated by direct appeal to an already-`ACCEPTED`
Master Instructions doctrine section, not by a shallow or incomplete
search —

- `D-0007`: unspecified evaluation order rejected via §19
  (deterministic diagnostics) and §12 (no impl-defined behavior
  without cause). Once determinism is required, there is no third
  live candidate between "fixed order" and "unspecified order" — the
  question is binary by its own nature.
- `D-0008`: static-only leak rejection rejected as strictly more
  mechanism for a strictly worse guarantee than automatic
  scope-end destruction (the record's own stated comparison) — not a
  missed alternative, a dominated one.
- `D-0009`: catchable checked faults rejected via §9 (one orthogonal
  mechanism per concern) — a second recoverable-failure channel
  alongside `fallible` is exactly what that doctrine already
  forecloses, leaving no live third candidate between "catchable" and
  "fatal."

`D-0006` was never actually thin (4 candidates across two questions)
and required no action.

Five verification passes (`spec/AUDIT.md`, `spec/AUDIT-2.md`,
`CHG-0009`–`CHG-0011`, and the sixth through fourteenth
`spec/AUDIT-STATUS.md` passes) have found defects exclusively in rule
*completeness* — a premise a rule didn't discharge, a case a rule
didn't cover — never a case where checked integers, left-to-right
evaluation, automatic scope-end destruction, or `Result`/`?` versus a
fatal `checked` fault produced a wrong or awkward answer under
adversarial derivation. That is evidence the four selections have
held up under real pressure, not merely proof a wider search would
not have changed them; `spec/AUDIT-STATUS.md`'s own tenth-pass
assessment already said as much. Backfilling alternative candidates
for entities already `ACCEPTED` and load-bearing throughout the corpus
would cost real effort for a rationale-quality improvement with no
expected semantic change — the same cost/benefit `spec/AUDIT-2.md`'s
original "not a defect" disposition already implied. The owner's
decision confirms that disposition rather than escalating it into
rework.

## What changed

**`spec/decisions/D-0006-type-identity-and-conversion.md`**,
**`D-0007-evaluation-order.md`**, **`D-0008-automatic-scope-end-
destruction.md`**, **`D-0009-failure-channels.md`**: each gains a
`## Candidate-set review (CHG-0024)` section recording this
conclusion. No decision's `Selected design`, `Rejected alternatives`,
or any other original content changed.

**`spec/AUDIT-STATUS.md`**: B-26's row changed `RECORDED → CLOSED`;
a fourteenth pass entry added recording this decision alongside
`CHG-0022`/`CHG-0023` (the other two tenth-pass items).

## Rule changes

None. Decision records carry no `rule.*`/`disposition` content of
their own to change.

## Affected invariants

None. No invariant depended on this finding being open.

## Dependency impact

None on existing entities' `Depends on`/`Affects` beyond the four
decision records' own new sections.

## Compatibility classification

Non-normative (documentation/rationale additions only; no selected
design changed).

## Migration implications

None.

## Example changes

None.

## Conformance changes

None.

## Future implementation implications

None. No selected mechanism changed, so no implementation obligation
changes.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

Each decision's own pre-existing `Revisit conditions` section remains
the operative trigger for reopening it; B-26 itself is closed and is
not revisited on any schedule.
