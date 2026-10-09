# CHG-0186 — RSA verification and `HashKind`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0158)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0158
Affects: `spec/21` (4.23.0) §0, §2m; `spec/conformance.md` (3.148.0); the guide §21; `impl/std/crypto.cb`

## What changed

- **`spec/21`:** `[Hash]`, `[Rsa-Pkcs1v15]`, `[Rsa-Pss]`; `HashKind`, `hash`, `RsaPublicKey`, `rsa_verify_pkcs1v15`, `rsa_verify_pss`; §0's tables and submodule rows.
- **`std`:** written in CobaltC; both tools run it as written.
- **Rows:** `conf.rsa-pkcs1v15`, `conf.rsa-pss`.

## Compatibility classification

Additive.
