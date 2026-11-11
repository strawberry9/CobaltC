# CHG-0180 — HKDF-SHA-256

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0152)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0152
Affects: `spec/21` (4.17.0) §0, §2m; `spec/registry/diagnostics.md` (1.45.0); `spec/conformance.md` (3.142.0); the guide §21; `impl/std/crypto.cb`

## What changed

- **`spec/21` §2m `rule.stdlib.crypto`:** `[Hkdf]`: `hkdf_extract`, `hkdf_expand` (`len` ≤ 8160, else `diag.hkdf-length`), `hkdf_sha256`; the listing gains them.
- **`spec/21` §0:** the `std::crypto` table and submodule row gain the three functions.
- **`spec/registry/diagnostics.md`:** `diag.hkdf-length`.
- **`std` (`impl/std/crypto.cb`):** the three functions, in CobaltC over `HmacSha256`.
- **Rows:** `conf.hkdf-vectors`, `conf.hkdf-bound`, `conf.hkdf-too-long`.

## Compatibility classification

Additive: three exported functions of `std::crypto` and a new dynamic diagnostic.
