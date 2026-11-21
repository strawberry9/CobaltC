# D-0170 — BLAKE2b, PBKDF2, Argon2id and password hashing

Status: ACCEPTED (2026-10-05, the owner: "I'm not sure why … no password hashing, since that seems ok to have")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0151 (`std::crypto`, base64), D-0166 (HMAC over any hash), D-0142 (`os_random_bytes`)
Affects: `spec/21` §0, §2m (4.35.0), `spec/registry/diagnostics.md` (1.48.0), `spec/conformance.md`, the guide §21, `impl/std/crypto.cb`; `CHG-0198`

## Problem

A program that keeps user accounts stores passwords; D-0151 left password
hashing to "a later decision". A fast hash, even salted, lets a stolen
table be searched at hardware speed; the remedy is a function that costs
memory as well as time, with its parameters and salt stored beside the
result so they can be raised later. PBKDF2 remains in the formats that
predate it (WPA2, PKCS #12, older vaults).

## Candidate mechanisms

1. **Argon2id (RFC 9106) on BLAKE2b, with `hash_password` and
   `verify_password` over the PHC string; PBKDF2 over any `HashKind`;
   BLAKE2b exported as a hash.** Selected.
2. **bcrypt or scrypt.** bcrypt costs no memory and caps the password at
   72 bytes; scrypt is what Argon2 replaced. Not adopted; neither is
   needed by a format `std` reads.
3. **Argon2i and Argon2d as well.** RFC 9106 recommends Argon2id for every
   use that does not have a specific reason otherwise. Not adopted.
4. **Raw `argon2id` alone, no PHC string.** Every program would reinvent
   the storage of salt and parameters, differently. Rejected.

## Selected design

**`spec/21` §2m:**

- `Blake2b` (plain), `Blake2b::new(out_len)`, `new_keyed(key, out_len)`,
  `update`, `finish : Vec<u8>`; `blake2b(data, out_len)`: RFC 7693, a
  digest of 1 to 64 bytes under a key of up to 64; other lengths are
  `diag.crypto-length`.
- `pbkdf2(kind, password, salt, iterations, len)`: RFC 8018 §5.2 over
  HMAC of `kind`; no iterations is `diag.kdf-parameters`.
- `argon2id(password, salt, iterations, memory_kib, parallelism, len)`:
  RFC 9106 type 2, version 0x13; iterations ≥ 1, 1 ≤ parallelism ≤ 255,
  memory ≥ 8 KiB per lane, salt ≥ 8 bytes, len ≥ 4, else
  `diag.kdf-parameters`. Lanes run one after another.
- `hash_password(password) : String`: Argon2id with a 16-byte random salt,
  m = 19456 KiB, t = 2, p = 1 (the OWASP recommendation), a 32-byte result,
  as `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`.
- `verify_password(stored, password) : bool`: the parameters, salt and
  hash read from the string, the password hashed the same way, the result
  compared with `digest_eq`; a string that is not an Argon2id PHC string
  is false.

Argon2id addresses memory by the password in its later passes by design,
so `[Constant-Time]` does not cover it; the final comparison is constant
time.

## Compatibility impact

Additive.

## Revisit conditions

- Argon2's parallelism actually in parallel (threads), if hashing
  throughput matters.
- A secret key or associated data for Argon2 (RFC 9106's K and X), if a
  format needs them.
