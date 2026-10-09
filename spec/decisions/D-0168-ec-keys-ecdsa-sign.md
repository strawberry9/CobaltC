# D-0168 — Elliptic-curve keys, P-384 key agreement and ECDSA signing

Status: ACCEPTED (2026-10-05, the owner: "Make std::crypto feature complete"; RFC 6979 chosen over a hedged nonce)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0159 (ECDSA verification, the P-256/P-384 field arithmetic), D-0165 (P-256 key agreement), D-0166 (HMAC over the curve's hash)
Affects: `spec/21` §0, §2m (4.33.0), `spec/registry/diagnostics.md` (1.48.0), `spec/conformance.md`, the guide §21, `impl/std/crypto.cb`; `CHG-0196`

## Problem

D-0165 gave key agreement on P-256 alone, and D-0159 verified ECDSA on
P-256 and P-384 without signing. A program that holds an EC key (a TLS
server, a signed token, a certificate it issues) needs to sign with it,
and a peer may insist on P-384 (D-0165 deferred it for want of a server
that did).

## Candidate mechanisms

1. **Curve-generic keys: `ec_private_key(curve)`, `ec_public_key`,
   `ecdh`, `ecdsa_sign`, the P-256 functions kept as the P256 case; the
   nonce by RFC 6979.** Selected.
2. **A random nonce.** One repeated or biased nonce gives the key away
   (the PlayStation 3 break); a deterministic nonce cannot repeat and
   needs no random source. Rejected.
3. **A hedged nonce (RFC 6979 §3.6 with random extra input).** Resists
   fault attacks on hardware, at the price of signatures that cannot be
   reproduced or checked against the RFC's vectors. The owner chose the
   deterministic form; the hedged one is a later option.

## Selected design

**`spec/21` §2m:**

- `ec_private_key(curve) : Vec<u8>`: 32 or 48 bytes naming a number in
  [1, n-1]; `ec_public_key(curve, k) : Option<Vec<u8>>`: 0x04 ‖ x ‖ y, 65
  or 97 bytes; `ecdh(curve, k, q) : Option<Vec<u8>>`: the x-coordinate of
  k·Q, 32 or 48 bytes. None when k is outside [1, n-1] or q is not an
  uncompressed point on the curve. A private key of the wrong length for
  the curve is `diag.crypto-length`.
- `ecdsa_sign(curve, k, digest) : Option<Vec<u8>>`: FIPS 186-4 §6.4.1
  with e the digest's leftmost bits, the nonce RFC 6979's over
  HMAC-SHA-256 (P-256) or HMAC-SHA-384 (P-384), the result the strict DER
  `ecdsa_verify` reads; None when k is outside [1, n-1].
- `p256_private_key`, `p256_public_key`, `p256_ecdh` are the generic
  functions on P256 with their fixed-length results.

`[Constant-Time]` covers the scalar multiplication (the scalar made one
bit longer than the order, a doubling and an addition at every bit, the
sum kept by a mask) and the arithmetic modulo n (Montgomery form, the
inverse by Fermat). Whether a candidate nonce is below n is the one
branch, as in every RFC 6979 implementation.

## Compatibility impact

Additive. The P-256 functions compute exactly what D-0165 specified.

## Revisit conditions

- The hedged nonce, as an option, if signing on hardware exposed to fault
  attacks matters.
- Throughput: a fixed-window multiplication would cut the additions by
  four.
