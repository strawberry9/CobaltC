# D-0169 — RSA private keys, signing and key generation

Status: ACCEPTED (2026-10-05, the owner: "I'm not sure why no signing … since that seems ok to have")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0157 (`BigUint`), D-0158 (RSA verification, `HashKind`), D-0142 (`os_random_bytes`)
Affects: `spec/21` §0, §2m, §3e (4.34.0), `spec/registry/diagnostics.md` (1.48.0), `spec/conformance.md`, the guide §21, `impl/std/crypto.cb`, `impl/std/math.cb`; `CHG-0197`

## Problem

D-0158 verified RSA signatures and deferred signing because `BigUint`
takes time that depends on its values, which a private exponent cannot
afford. A program that answers a webhook, issues a token or serves TLS
with an RSA key must sign; one that provisions a service must make a
key. The owner asked why signing was absent; the answer was the missing
constant-time core, not a design objection.

## Candidate mechanisms

1. **A constant-time Montgomery exponentiation over 64-bit limbs, the
   private operation by the CRT on the same fixed-length arithmetic and
   checked against the public key; `RsaPrivateKey`, `rsa_sign_pkcs1v15`,
   `rsa_sign_pss`, `rsa_generate_key`.** Selected.
2. **Signing on `BigUint::mod_pow`.** The timing of each squaring and
   multiplication follows the exponent's bits. Rejected, as in D-0158.
3. **Blinding over `BigUint`.** Blinding hides the message, not the
   exponent's bit pattern in the operation count; and with the
   exponentiation already constant time it adds a modular inverse of a
   random number per signature (a minute under the interpreter) for no
   gain, which is why Go's `crypto/rsa` dropped it. Rejected.
4. **The CRT without the final check.** A single wrong result (a fault, a
   bit flip) factors the modulus (Boneh–DeMillo–Lipton). Rejected.
5. **Keys read from files only, no generation.** A program that needs a
   key then needs another tool. Rejected: generation is slow under an
   interpreter and that is accepted.

## Selected design

**`spec/21` §2m:**

- `RsaPrivateKey` (`n`, `e`, `d`, `p`, `q` exported, the CRT values kept
  inside); `RsaPrivateKey::new(n, e, d, p, q) : Option<RsaPrivateKey>`:
  None unless p·q = n, p and q odd and above 2, and e·d ≡ 1 modulo p-1 and
  q-1; `RsaPrivateKey::public_key`.
- `rsa_sign_pkcs1v15(key, kind, digest) : Vec<u8>` (RFC 8017 §8.2.1,
  deterministic) and `rsa_sign_pss(key, kind, digest) : Vec<u8>` (§8.1.1,
  MGF1 over the same hash, a random salt as long as the digest): what
  `rsa_verify_pkcs1v15` and `rsa_verify_pss` accept. A digest of the wrong
  length, or a modulus too short for the encoding, is `diag.crypto-length`.
- `rsa_generate_key(bits) : RsaPrivateKey`: two primes of half the length
  with their top two bits set, by trial division below 1000 and five
  rounds of Miller–Rabin (FIPS 186-4 B.3.3), e = 65537; `bits` outside
  1024 to 8192 or not a multiple of 64 is `diag.crypto-length`.
- The private operation: the two halves by a 4-bit fixed-window
  Montgomery exponentiation whose table entry is chosen by scanning the
  table with masks, Garner's combination on the same fixed-length limbs
  (the difference and the product by q⁻¹ by masks, the product by q
  schoolbook), and s^e checked against the message (a failing key is used
  without the CRT). `BigUint`, with its value-dependent timing, touches
  only public numbers: the padded message, the result and the public
  key.

**`spec/21` §3e:** `BigUint::clone` and `BigUint::mod_inverse(a, m) :
Option<BigUint>` (extended Euclid; None when a and m share a factor),
`[Big-Not-Constant-Time]` covering it.

## Compatibility impact

Additive.

## Revisit conditions

- RSA-OAEP encryption, which the private operation makes a small step.
- Speed under the interpreter: key generation of 2048 bits takes minutes
  there; `cobc` takes seconds.
