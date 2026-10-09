# D-0152 — HKDF-SHA-256 in `std::crypto`

Status: ACCEPTED (2026-10-04, the owner: "agreed, write up and implement D-0152 HKDF once the checks pass", after agreeing to the crypto roadmap toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0151 (`std::crypto`, HMAC-SHA-256), D-0083 (`fault` with a registered diagnostic)
Affects: `spec/21` §0, §2m (4.17.0), `spec/registry/diagnostics.md` (1.45.0), `spec/conformance.md` (3.142.0), the guide §21, `impl/std/crypto.cb`; `CHG-0180`

## Problem

A key is rarely used as it arrives. A shared secret from a key exchange,
a password's derived key, a master key for a service: each has to become
several keys of chosen lengths, bound to what they are for, so that the
same secret never keys two purposes. TLS 1.3's entire key schedule is
one function doing this, HKDF (RFC 5869), over HMAC-SHA-256: `Extract`
condenses the input keying material with a salt into a pseudorandom
key, `Expand` stretches that key, mixed with a context label, to any
length up to 255 hash outputs. The DNS-over-TLS client needs it first of
everything after the hashes, and it names one very common intent:
"derive keys from a secret".

## Candidate mechanisms

1. **HKDF as two functions and a one-shot**, the RFC's shape, over
   D-0151's `HmacSha256`. Thirty lines of CobaltC. Selected.
2. **A `Kdf` object** with `derive(label, len)`. Hides the extract step
   TLS needs to call on its own (its `Derive-Secret` feeds `Expand` a
   pseudorandom key it computed with `Extract` from a different salt).
   Rejected.
3. **PBKDF2 or Argon2 alongside**, for passwords. Different intent
   (slowness is the point); a later record.

## Selected design

`spec/21` §2m `[Hkdf]`, in `std::crypto`, written in CobaltC:

- `hkdf_extract(slice<u8, shared> salt, slice<u8, shared> ikm) :
  array<u8, 32>`: HMAC-SHA-256 keyed by `salt`, of `ikm`; an empty salt
  stands for 32 zero bytes (RFC 5869 §2.2).
- `hkdf_expand(slice<u8, shared> prk, slice<u8, shared> info, usize
  len) : Vec<u8>`: the first `len` bytes of T(1) ‖ T(2) ‖ …, where T(i)
  = HMAC-SHA-256(prk, T(i−1) ‖ info ‖ i), T(0) empty and `i` one byte
  counted from 1. `len` above 8160 (255 blocks of 32) is the fault
  `diag.hkdf-length`, registered: it is misuse of the function, as an
  index out of bounds is, not a condition a program meets at run time.
- `hkdf_sha256(salt, ikm, info, len) : Vec<u8>`: extract, then expand.
- Verified by RFC 5869's three SHA-256 test cases (A.1–A.3: a short
  salt and info, 80-byte inputs with 82 bytes out, and empty salt and
  info), the 8160-byte bound exactly, the fault just above it, and the
  stress sweep against a Python HKDF over `hmac` under all three tools.

Not adopted: the SHA-512 and SHA-384 variants until those hashes exist;
TLS 1.3's `HKDF-Expand-Label` and `Derive-Secret`, which belong to the
TLS record.

## Compatibility impact

Additive: three exported functions in `std::crypto` and one new dynamic
diagnostic, shadowed by a program's own items of those names.

## Revisit conditions

- `hkdf_extract_sha384`/`hkdf_expand_sha384` when SHA-384 lands, for the
  AES-256 cipher suites.
- `Vec<u8>` output versus filling a caller's slice, if a program is found
  that derives keys in a loop and measures the allocation.
