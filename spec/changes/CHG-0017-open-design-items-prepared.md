# CHG-0017 — Four Open Design Items Prepared for the Human Owner

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §18, §22
Depends on: D-0011, D-0006, D-0007, D-0008, D-0009, rule.temporal.ref-escape, rule.trust.extern-call, rule.conc.join
Affects: feat.explicit-lifetime-parameters, feat.minimal-io-extern-surface, feat.consolidate-join-typing

## Problem / motivation

Four open design items, each *prepared*
(the full Master Instructions §18 feature-proposal field
set, or an equivalent assessment) for the human owner to decide, not
decided here.

## What was prepared

- `feat.explicit-lifetime-parameters` (already `UNDER_INVESTIGATION`)
  expanded from a one-line note to the full field set: two concrete
  (if hypothetical — no existing `spec/21` shape needs it) motivating
  examples, three candidate mechanisms with their costs, and why the
  status remains `UNDER_INVESTIGATION`.
- `feat.minimal-io-extern-surface` (new): whether `spec/21` should
  declare one minimal `extern fn` so a conformance case's outcome
  could in principle be observed by a real running program, or whether
  that stays out of scope. A genuine project-scope question (Master
  Instructions §1), not resolvable from the invariants.
- `feat.consolidate-join-typing` (new): `CHG-0008`'s deferred question
  — is `[T-Join]` fully redundant with ordinary `[T-Call]`/
  `[Generic-Call-Inferred]` typing over `join`'s own prelude signature?
  Traced by hand to the same conclusion in every case checked, but
  resolving it outright depends on whether prelude intrinsics are
  modeled as ordinary `state.items` entries, which no existing rule
  settles either way and which could affect more than just `join` if
  answered — prepared, not decided.
- `spec/AUDIT-2.md` B-26 (thin candidate sets in D-0006–D-0009):
  checked against the actual records (`D-0006` has 4 candidates;
  `D-0007`/`D-0008`/`D-0009` have 2 each) and assessed in
  `spec/AUDIT-STATUS.md`'s tenth pass — recorded as still open, with
  the observation that five verification passes have found rule-
  completeness defects but never a wrong-mechanism defect in any of the
  four decisions.

## Rule changes

None. This record is preparation only, per its own §18/§22 framing.

## Affected invariants

None restated.

## Dependency impact

None on existing entities' `Depends on`/`Affects` beyond the new
`feat.*` entries' own citations.

## Compatibility classification

Non-normative (registry/documentation additions).

## Migration implications

None.

## Example changes

None.

## Conformance changes

None.

## Future implementation implications

None until the owner decides any of the four items.

## Prior-art status

`feat.minimal-io-extern-surface`: every systems language provides some
minimal write-to-descriptor primitive.
`feat.explicit-lifetime-parameters`'s named-lifetime-parameter
candidate is Rust-shaped.

## Revisit conditions

Each of the four items is revisited when the owner decides it, not on
any schedule.
