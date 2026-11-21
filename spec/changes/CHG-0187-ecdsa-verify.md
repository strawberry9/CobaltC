# CHG-0187 — ECDSA verification

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0159)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0159
Affects: `spec/21` (4.24.0) §0, §2m; `spec/conformance.md` (3.149.0); the guide §21; `impl/std/crypto.cb`

## What changed

- **`spec/21`:** `[Ecdsa]`; `EcCurve`, `ecdsa_verify`; §0's tables and submodule rows.
- **`std`:** written in CobaltC; both tools run it as written.
- **Rows:** `conf.ecdsa-p256`, `conf.ecdsa-p384`.

## Compatibility classification

Additive.
