# CHG-0054 — `match` through a reference

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0046
Affects: rule.agg.match, the cases listed below

## Problem / motivation

D-0046: a payload could not be inspected through a reference.

## What changed

- **`spec/16` 1.5.0:** `[Match-By-Ref]`; `[Match-Move-Through-Ref]`
  limited to by-value matches.
- **`spec/conformance.md` 3.41.0:** the cases below.
- **`spec/02-schema.md` 1.0.30:** §5's "in use" ranges.
- **Implementations:** typecheck (binder types); `coby`
  (`eval_match_by_ref`; `type_at` and `path_offset_at` resolve a
  payload step to the active variant, also in arena memory); `cobc`
  (`lower_match` borrows the payload with `CB_PAYLOAD`).

## Compatibility classification

Extension (a reference scrutinee faulted before).

## Conformance changes

**Added:** `conf.match-by-ref-shared`, `conf.match-by-ref-exclusive`,
`conf.match-by-ref-resource-payload`, `conf.match-by-ref-box-tree`,
`conf.match-by-ref-write-through-shared-rejected`,
`conf.match-by-ref-replace-while-bound-rejected`.
**Changed:** `12-type-system/type_mismatch_rejected.cb` matches a
reference to an `i32`.

## Prior-art status

See D-0046.

## Revisit conditions

See D-0046.
