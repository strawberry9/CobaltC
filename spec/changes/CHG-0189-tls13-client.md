# CHG-0189 — `std::tls` and `hkdf_expand_label`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0161)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0161
Affects: `spec/21` (4.26.0) §0, §2m, §2o (new); `spec/conformance.md` (3.151.0); the guide §21; `impl/std/tls.cb` (new), `impl/std/crypto.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`

## What changed

- **`spec/21`:** §2o `rule.stdlib.tls`: `[Tls-Client]`, `[Tls-Records]`, `[Tls-Error]`; §2m `[Hkdf-Expand-Label]`; the listings; §0's tables and submodule rows.
- **`std`:** written in CobaltC; both tools run it as written.
- **Rows:** `conf.hkdf-expand-label`, `conf.tls-not-tls`, `conf.tls-closed`.

## Compatibility classification

Additive.
