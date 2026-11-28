# CHG-0193 — AES-GCM, P-256 key agreement, and the TLS client's mandatory set

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0165)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0165
Affects: `spec/21` (4.30.0) §0, §2m, §2o; `spec/registry/diagnostics.md` (1.47.0); `spec/conformance.md` (3.156.0); the guide; `impl/std/crypto.cb`, `impl/std/tls.cb`

## What changed

- **`spec/21` §2m:** `[Aes]`, `[Aes-Gcm-Seal]`, `[Aes-Gcm-Open]`, `[P256-Ecdh]`; `[Crypto-Length]` and `[Constant-Time]` cover them; the field arithmetic shared with `ecdsa_verify` reduces by masks; the listing.
- **`spec/21` §2o:** `[Tls-Client]` offers TLS_AES_128_GCM_SHA256 and P-256 and follows a HelloRetryRequest for P-256; the listing.
- **`diag.crypto-length`:** AES keys of 16, 24 or 32 bytes, AES-GCM nonces of 12, P-256 private keys of 32.
- **Rows:** `conf.aes-gcm-vectors`, `conf.p256-ecdh`, `conf.aes-key-length`.

## Compatibility classification

Additive.
