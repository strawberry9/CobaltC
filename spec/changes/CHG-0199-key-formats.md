# CHG-0199 — Keys in their standard encodings, and Ed25519 in certificates

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0171)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0171
Affects: `spec/21` (4.36.0) §0, §2n; `spec/conformance.md` (3.163.0); the guide; `impl/std/x509.cb`

## What changed

- **`spec/21` §2n:** `[Cert-Parse]` reads Ed25519 keys and signatures; `[Key-Der]`, `[Key-Pem]`, `[Sign]`; `CertKey::Ed25519Key`, `SignatureScheme::Ed25519`, `EcPrivateKey`, `PrivateKey`; the listing.
- **Rows:** `conf.key-formats`, `conf.sign-verify`, `conf.x509-ed25519`.

## Compatibility classification

Additive (two enums gain a variant).
