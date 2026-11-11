# CHG-0145 — `checked_narrow`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0122)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0122
Affects: `spec/06` (1.15.0), `spec/21` §0, `spec/conformance.md`, the guide §06, both tools

## What changed

- **`spec/06`:** `[Checked-Narrow]`.
- **Tools:** the intrinsic, answering `Some`/`None` by `[Narrow-Overflow]`'s test.
- **Rows:** `conf.checked-narrow-some`, `conf.checked-narrow-none`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
