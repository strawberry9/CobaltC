# CHG-0196 — Elliptic-curve keys, P-384 key agreement and ECDSA signing

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0168)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0168
Affects: `spec/21` (4.33.0) §0, §2m; `spec/registry/diagnostics.md` (1.48.0); `spec/conformance.md` (3.160.0); the guide; `impl/std/crypto.cb`

## What changed

- **`spec/21` §2m:** `[Ec-Keys]`, `[Ecdh]`, `[Ecdsa-Sign]`; `[P256-Ecdh]` restated as the P256 case; `[Crypto-Length]` and `[Constant-Time]` cover them; the listing.
- **`diag.crypto-length`:** an EC private key of the curve's length (32 or 48 bytes).
- **Rows:** `conf.ecdsa-sign-vectors`, `conf.ecdh-p384`, `conf.ec-key-refusals`.

## Compatibility classification

Additive.
