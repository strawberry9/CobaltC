# CHG-0185 — `BigUint`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0157)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0157
Affects: `spec/21` (4.22.0) §0, §3e (new); `spec/conformance.md` (3.147.0); the guide §21; `impl/std/math.cb`

## What changed

- **`spec/21`:** §3e `rule.stdlib.bigint`: `[BigUint]`, `[Big-Division]`, `[Big-Not-Constant-Time]`; `BigUint`, `BigDivision` and their functions; §0's tables and submodule rows.
- **`std`:** written in CobaltC; both tools run it as written.
- **Rows:** `conf.bigint-arithmetic`, `conf.bigint-text`, `conf.bigint-sub-underflow`.

## Compatibility classification

Additive.
