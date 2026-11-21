# CHG-0061 — `StringView`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0053
Affects: rule.stdlib.stringview (new), rule.agg.slice

## Problem / motivation

D-0053.

## What changed

- **`spec/21` 3.20.0:** §2h `rule.stdlib.stringview` (`[View-Form]`,
  `[View-Boundary]`, `[View-Eq]`, `[View-Mode-Rejected]`, and the `std`
  source); §0's table.
- **`spec/16` 1.8.0:** `rule.agg.slice` points to `[View-Form]`.
- **`spec/registry/diagnostics.md` 1.25.0:** `diag.not-char-boundary`.
- **`spec/conformance.md` 3.47.0:** the cases below.
- **`spec/02-schema.md`:** §5's "in use" ranges.
- **Implementations:** `std` (`StringView` and its functions); the
  static pass (the three forms, `%s`/`%v`/append of a view, a view as a
  borrowed type; a slice's or view's borrows now have the origin of the
  borrow it holds, so `std` can re-slice a parameter's slice); `coby` and
  `cobc` (`view_ops`, printing a view). The guide (§21).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.view-basic`, `conf.view-eq`, `conf.view-split-trim`,
`conf.view-parse`, `conf.view-push-while-held-rejected`,
`conf.view-escape-rejected`, `conf.view-char-boundary`,
`conf.view-exclusive-rejected`.

## Prior-art status

See D-0053.

## Revisit conditions

None.
