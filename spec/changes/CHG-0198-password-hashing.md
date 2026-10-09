# CHG-0198 — BLAKE2b, PBKDF2, Argon2id and password hashing

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0170)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0170
Affects: `spec/21` (4.35.0) §0, §2m; `spec/registry/diagnostics.md` (1.48.0); `spec/conformance.md` (3.162.0); the guide; `impl/std/crypto.cb`

## What changed

- **`spec/21` §2m:** `[Blake2b]`, `[Pbkdf2]`, `[Argon2id]`, `[Password-Hash]`, `[Kdf-Parameters]`; `[Crypto-Length]` covers BLAKE2b's lengths; the listing.
- **`diag.kdf-parameters`:** new.
- **Rows:** `conf.blake2b-vectors`, `conf.pbkdf2-vectors`, `conf.argon2id-vectors`, `conf.password-hash`, `conf.kdf-parameters`.

## Compatibility classification

Additive.
