# CHG-0137 — `Option::ok_or`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0114)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0114
Affects: `spec/21` §0 (3.44.0), `spec/conformance.md` (3.110.0), the guide §18 and §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0:** one row beside `Option::unwrap_or`.
- **`std`:** the function, written in CobaltC.
- **Rows:** `conf.option-ok-or`, `conf.option-ok-or-propagates`,
  `conf.option-ok-or-error-unused-destroyed`.
- **Guide:** §18's paragraph on `?` shows it; §21's table lists it.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a program's own `Option::ok_or` shadows it (D-0024).
