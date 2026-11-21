# CHG-0197 — RSA private keys, signing and key generation

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0169)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0169
Affects: `spec/21` (4.34.0) §0, §2m, §3e; `spec/registry/diagnostics.md` (1.48.0); `spec/conformance.md` (3.161.0); the guide; `impl/std/crypto.cb`, `impl/std/math.cb`

## What changed

- **`spec/21` §2m:** `[Rsa-Private-Key]`, `[Rsa-Sign-Pkcs1v15]`, `[Rsa-Sign-Pss]`, `[Rsa-Generate]`, `[Rsa-Private-Op]`; `[Crypto-Length]` covers the digest length, the modulus length and the key size; the listing.
- **`spec/21` §3e:** `BigUint::clone`, `BigUint::mod_inverse` (`[Big-Mod-Inverse]`); the listing.
- **`diag.crypto-length`:** a digest of the hash's length, a modulus long enough for the encoding, a key size of 1024 to 8192 bits in multiples of 64.
- **Rows:** `conf.rsa-sign-vectors`, `conf.rsa-private-key-refusals`, `conf.rsa-generate`, `conf.biguint-mod-inverse`.

## Compatibility classification

Additive.
