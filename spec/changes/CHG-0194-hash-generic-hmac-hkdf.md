# CHG-0194 — Hashing, HMAC and HKDF over any `HashKind`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0166)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0166
Affects: `spec/21` (4.31.0) §0, §2m; `spec/conformance.md` (3.158.0); the guide; `impl/std/crypto.cb`

## What changed

- **`spec/21` §2m:** `[Hasher]`, `[Hmac]`, `[Hkdf-With]`; the listing.
- **Rows:** `conf.hmac-generic`, `conf.hkdf-generic`.

## Compatibility classification

Additive.
