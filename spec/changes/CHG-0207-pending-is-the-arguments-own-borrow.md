# CHG-0207 — "Pending" covers an argument's own borrow

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-06; D-0179)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0179
Affects: `spec/15` (1.12.1) `rule.fn.call`

## What changed

- **`spec/15` `rule.fn.call`:** the note on argument borrows says that a path held by a value an argument computes (a view, a struct holding a reference) is not pending.

## Compatibility classification

Clarification: no program's behaviour changes in either implementation.
