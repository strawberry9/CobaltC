# D-0166 — Hashing, HMAC and HKDF over any `HashKind` in `std::crypto`

Status: ACCEPTED (2026-10-05, the owner: "Make std::crypto feature complete")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0151 (SHA-256, HMAC-SHA-256), D-0152 (HKDF), D-0156 (SHA-384/512), D-0158 (`HashKind`, `hash`)
Affects: `spec/21` §0, §2m (4.31.0), the guide §21, `impl/std/crypto.cb`; `CHG-0194`

## Problem

`std::crypto` hashed with any of SHA-256, SHA-384 and SHA-512 (`hash`),
but HMAC and HKDF existed over SHA-256 alone. TLS 1.3's
TLS_AES_256_GCM_SHA384 runs its whole key schedule over SHA-384; RFC 6979
derives an ECDSA nonce with HMAC over the curve's hash; PBKDF2 is
specified over any HMAC; file formats name their hash in a header. Each
of these needs the MAC or the derivation chosen at run time, and a
streaming hash chosen the same way (a transcript hash whose suite is
not yet known).

## Candidate mechanisms

1. **Generic forms beside the SHA-256 ones: `Hasher`, `Hmac`, `hmac`,
   `hkdf_extract_with`, `hkdf_expand_with`, `hkdf_with`,
   `hkdf_expand_label_with`, each taking a `HashKind`.** Selected.
2. **Change `hkdf_extract`, `hkdf_expand`, `hmac_sha256` to take a
   `HashKind`.** Every existing caller changes, and the common case
   (SHA-256) gains an argument it never varies. Rejected.
3. **A struct and function per hash (`HmacSha384`, `hkdf_sha384`, …).**
   Three times the names, and a protocol that negotiates the hash still
   has to switch between them. Rejected.

## Selected design

**`spec/21` §2m:**

- `Hasher` (plain), `Hasher::new(HashKind)`, `update`, `finish : Vec<u8>`:
  the streaming form of `hash`.
- `Hmac` (plain), `Hmac::new(HashKind, key)`, `update`, `finish : Vec<u8>`;
  `hmac(kind, key, data)`: HMAC over the hash's block (64 or 128 bytes), a
  longer key hashed first. A copy of a keyed `Hmac` continues on its own.
- `hkdf_extract_with(kind, salt, ikm)`, `hkdf_expand_with(kind, prk, info,
  len)` (at most 255 digests, `diag.hkdf-length` above), `hkdf_with(kind,
  salt, ikm, info, len)`, `hkdf_expand_label_with(kind, secret, label,
  context, len)`: RFC 5869 and RFC 8446 §7.1 over `kind`.
- The SHA-256 forms stay, and compute the same as the generic ones with
  `Sha2_256`.

## Compatibility impact

Additive.

## Revisit conditions

- A hash outside SHA-2 (SHA-3, BLAKE2b) wanted under HMAC or HKDF:
  `HashKind` would grow, and the block length table with it.
