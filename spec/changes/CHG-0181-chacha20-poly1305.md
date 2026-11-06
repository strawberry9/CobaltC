# CHG-0181 — ChaCha20-Poly1305

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0153)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0153
Affects: `spec/21` (4.18.0) §0, §2m; `spec/registry/diagnostics.md` (1.46.0); `spec/conformance.md` (3.143.0); the guide §21; `impl/std/crypto.cb`

## What changed

- **`spec/21` §2m:** `[ChaCha20]`, `[Poly1305]`, `[Aead-Seal]`, `[Aead-Open]`, `[Crypto-Length]`, `[Constant-Time]`; the listing gains `chacha20`, `poly1305`, `chacha20_poly1305_seal`, `chacha20_poly1305_open` and their private helpers.
- **`spec/21` §0:** the `std::crypto` table and submodule row.
- **`spec/registry/diagnostics.md`:** `diag.crypto-length`.
- **`std` (`impl/std/crypto.cb`):** the four functions, in CobaltC.
- **Rows:** `conf.chacha20-vector`, `conf.poly1305-vector`, `conf.aead-vector`, `conf.aead-tamper`, `conf.crypto-length`.

## Compatibility classification

Additive: four exported functions of `std::crypto` and a new dynamic diagnostic.
