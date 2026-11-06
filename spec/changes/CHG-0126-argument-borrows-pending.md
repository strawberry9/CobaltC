# CHG-0126 — Argument borrows are pending until bound

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0107
Affects: spec/04 `clash`, spec/15 `[Call]`

## What changed

- **spec/04:** `clash` excludes a `pending` path.
- **spec/15:** the paragraph defining pending argument paths.
- **Implementations:** none (they already behaved so).
- **Rows:** `conf.argument-borrow-then-shared-argument`,
  `conf.two-exclusive-arguments-fault-at-use`.

## Compatibility classification

Clarification: the text now states the implementations' behaviour.
