# D-0151 — `std::crypto`: SHA-256, HMAC-SHA-256, and bytes as text

Status: ACCEPTED (2026-10-04, the owner: "lets discuss a std::crypto module, as I use SHA-256 and HMAC very often", then "accept your recommendations and implement D-0151", with the stated goal of a DNS-over-TLS client written in CobaltC)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0072 (`[v; N]`), D-0091 (`rotate_right`, wrapping arithmetic), D-0119 (`crc32`, the precedent for an algorithm written in CobaltC), D-0134 (naming), D-0136 (`std`'s submodules), D-0142 (`os_random_bytes`)
Affects: `spec/21` §0, §2d, §2m (new) (4.16.0), `spec/conformance.md` (3.141.0), the guide §21, `impl/std/crypto.cb` (new), `impl/std/text.cb`, `impl/std/std.cb`; `CHG-0179`

## Problem

A deployed program signs and checks things: a webhook body, a session
cookie, a request to a service, a file's integrity. Each needs a hash
and a keyed hash, and the text forms a digest travels in. `std` had
`crc32`, which detects accidents and nothing else, and no way to write
bytes as hexadecimal or base64 but a loop over `printf("%02x")`. The
owner's next large program is a DNS-over-TLS client; TLS 1.3's key
schedule and transcript hash are HMAC-SHA-256 and SHA-256, so these are
the first stones of that road.

## Candidate mechanisms

1. **Realize natively in Rust.** Rust's standard library has no SHA-256
   either, so this would be a hand-written implementation in `src/`
   shared by both tools, with an FFI primitive. Fast under `coby`.
2. **Write it in CobaltC**, as `crc32` is. About 150 lines over `u32`
   with `rotate_right` and `wrapping_add`, both of which the language
   has; identical in both tools by construction; transcribed into the
   spec as a listing; checked against the published vectors. Slow
   under `coby` for large inputs (per byte through the interpreter),
   irrelevant for the common case, a MAC over a few kilobytes. Selected:
   the library exists to prove the language is sufficient, and a native
   realization can replace the body later without changing a signature
   (D-0023's latitude) if `coby` is ever asked to hash gigabytes.
3. **A `Digest` type** with `text`. Rejected for a plain
   `array<u8, 32>`: copyable, comparable, printable with `to_hex`, and
   no new type.
4. **Hashes beyond SHA-256** (SHA-1 and MD5 for interoperability,
   SHA-512). Left out until a program asks; SHA-1 and MD5 would have to
   be marked "not for security" and invite misuse.
5. **A separate `std::encoding` for hex and base64.** Rejected for
   `std::text`: fewer new names, and they are text in the same sense
   `sprintf` is.

## Selected design

`spec/21` §2m, `rule.stdlib.crypto`, a new submodule `std::crypto`:

- `sha256(slice<u8, shared> data) : array<u8, 32>`; `Sha256::new()`,
  `Sha256::update(ref<Sha256, exclusive> s, slice<u8, shared> data)`,
  `Sha256::finish(Sha256 s) : array<u8, 32>` for a message that arrives
  in pieces. `Sha256` is a plain value, so a prefix's state can be copied
  and continued two ways (TLS's transcript hash does this).
- `hmac_sha256(slice<u8, shared> key, slice<u8, shared> data) :
  array<u8, 32>`; `HmacSha256::new(key)`, `update`, `finish`. A key
  longer than 64 bytes is hashed first (RFC 2104).
- `digest_eq(slice<u8, shared> a, slice<u8, shared> b) : bool`: equal
  lengths and bytes, in a time that depends only on the lengths, so a
  MAC check leaks nothing through timing. The name says what it is for;
  the guide says why `==` is wrong there.
- In `std::text` (`rule.stdlib.text`): `to_hex(slice<u8, shared>) :
  String` (lowercase), `from_hex(StringView) : Result<Vec<u8>,
  ParseError>` (either case; `Empty`, `Invalid(i)` at the first bad byte
  or at the length when odd), `base64_encode(slice<u8, shared>) :
  String` and `base64_decode(StringView) : Result<Vec<u8>, ParseError>`
  (RFC 4648 §4, padding required, `Invalid(i)`; the empty text decodes
  to no bytes).
- **Verification:** FIPS 180-4's vectors (the empty message, `abc`, the
  56- and 112-byte messages), the padding boundaries (55, 56, 63, 64, 65
  bytes), RFC 4231's HMAC vectors 1–4, 6 and 7, RFC 4648's base64 vectors,
  all as conformance cases; a stress sweep (`stress/crypto/sweep.py`) of
  435 random messages and keys against Python's `hashlib`, `hmac` and
  `base64` under all three tools, the streaming form against the
  one-shot form, and base64 round trips.

Out of scope, each a later decision on the road to TLS: HKDF, an AEAD
cipher (ChaCha20-Poly1305 first), X25519, X.509 and signature
verification, the TLS 1.3 record layer and handshake; and, off that
road, password hashing, SHA-1, MD5, SHA-512.

## Compatibility impact

Additive: a new submodule `std::crypto` with `Sha256`, `HmacSha256`,
`sha256`, `hmac_sha256`, `digest_eq`, and four functions in `std::text`,
all shadowed by a program's own items of those names.

## Revisit conditions

- A native realization of `Sha256::compress` if `coby` is measured too
  slow for a real program's hashing.
- HKDF-SHA-256 next (RFC 5869), then ChaCha20-Poly1305 and X25519, as
  the TLS client needs them.
- SHA-512 (and HMAC-SHA-512) if a protocol a program speaks requires it.
