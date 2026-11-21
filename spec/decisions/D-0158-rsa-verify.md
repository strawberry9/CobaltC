# D-0158 — RSA signature verification and `HashKind` in `std::crypto`

Status: ACCEPTED (2026-10-04, the owner: "proceed with the entire roadmap using your discretion", on the crypto roadmap agreed toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0156 (SHA-384/512), D-0157 (`BigUint`)
Affects: `spec/21` §0, §2m (4.23.0), `spec/conformance.md` (3.148.0), the guide §21, `impl/std/crypto.cb`; `CHG-0186`

## Problem

Most certificate chains on the web contain an RSA signature, and TLS 1.3
servers with RSA keys prove their identity with RSA-PSS. The client must
verify both PKCS #1 v1.5 (in certificates) and PSS (in certificates and
the handshake).

## Candidate mechanisms

1. **Verification only**, over `BigUint`, for PKCS #1 v1.5 and PSS with
   SHA-256/384/512. A client never needs an RSA private key. Selected.
2. **Signing as well.** Needs a constant-time big-number implementation,
   which `BigUint` is not; a later record if a program must sign.

## Selected design

`spec/21` §2m `[Rsa-Pkcs1v15]`, `[Rsa-Pss]`, `[Hash]`:

- `HashKind` (`Sha2_256`, `Sha2_384`, `Sha2_512`, variant names chosen to
  share none with the hash types) and `hash(kind, data) : Vec<u8>`.
- `RsaPublicKey { n, e }` (both `BigUint`).
- `rsa_verify_pkcs1v15(key, kind, digest, signature) : bool`: the
  encoded block is rebuilt from the digest and compared whole with the
  opened signature (RFC 8017 §8.2.2), so nothing of the signature is
  parsed and no BER leniency is possible.
- `rsa_verify_pss(key, kind, digest, signature) : bool`: RFC 8017
  §8.1.2 with MGF1 over the same hash and a salt as long as the digest,
  as TLS 1.3 requires and certificates use.
- A malformed signature (wrong length, not below n) is `false`.

Verified by vectors from Python's `cryptography` (2048- and 3072-bit
keys, all three hashes, valid and altered) as a case and a stress run of
36 more, and by the 115 RSA and ECDSA self-signatures of the system's
root store that use SHA-2 (the other six are SHA-1 or P-521, which are
not supported).

## Compatibility impact

Additive.

## Revisit conditions

- PSS with another salt length, if a certificate is met that uses one.
- RSA signing, with a constant-time big-number core.
