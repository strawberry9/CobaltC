# D-0159 — ECDSA verification on P-256 and P-384 in `std::crypto`

Status: ACCEPTED (2026-10-04, the owner: "proceed with the entire roadmap using your discretion", on the crypto roadmap agreed toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0156, D-0158 (`HashKind`)
Affects: `spec/21` §0, §2m (4.24.0), `spec/conformance.md` (3.149.0), the guide §21, `impl/std/crypto.cb`; `CHG-0187`

## Problem

The other half of the web's certificates, and most TLS 1.3 servers'
handshake signatures, are ECDSA over NIST P-256 or P-384.

## Candidate mechanisms

1. **Over `BigUint`.** Written first; correct, but one P-384
   verification took 242 s under `coby` and 0.6 s compiled. Rejected.
2. **Fixed-limb Montgomery arithmetic** for each curve's prime and order
   (up to six 64-bit limbs, products through `u128`, precomputed
   constants), Jacobian coordinates for the curve (a = −3), and
   Shamir's trick for u1·G + u2·Q. P-384 then takes 25 s under `coby`
   and under a millisecond compiled. Selected.

## Selected design

`spec/21` §2m `[Ecdsa]`: `EcCurve` (`P256`, `P384`) and
`ecdsa_verify(curve, public_key, digest, signature) : bool`, the key an
uncompressed point (`0x04 ‖ x ‖ y`) checked to lie on the curve, the
signature DER (`SEQUENCE { r, s }`) as X.509 and TLS carry it, strict
(minimal integers, no trailing bytes), r and s in [1, n−1], the digest's
leftmost bits taken (FIPS 186-4 §6.4). Variable time is acceptable:
every input is public.

Verified by vectors from Python's `cryptography` (both curves, all three
hashes, valid and with s changed), off-curve keys and truncated
signatures, as two cases and a stress run, and by the system roots'
self-signatures.

## Compatibility impact

Additive.

## Revisit conditions

- P-521, if a chain the client must accept needs it (one system root
  uses it).
- ECDH over P-256 and ECDSA signing, with constant-time point
  arithmetic.
