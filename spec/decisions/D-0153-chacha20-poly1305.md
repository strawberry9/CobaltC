# D-0153 — ChaCha20-Poly1305 in `std::crypto`

Status: ACCEPTED (2026-10-04, the owner: "ok, now D-0153 ChaCha20-Poly1305 once the checks pass", the second step of the agreed crypto roadmap toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0151 (`std::crypto`, `digest_eq`), D-0152 (HKDF, which makes its keys), D-0142 (`os_random_bytes`), D-0083 (`fault`)
Affects: `spec/21` §0, §2m (4.18.0), `spec/registry/diagnostics.md` (1.46.0), `spec/conformance.md` (3.143.0), the guide §21, `impl/std/crypto.cb`; `CHG-0181`

## Problem

A program that sends secrets over a network, or stores them, needs to
encrypt them so that only the holder of a key can read them, and to know
on reading that nothing was changed. That is authenticated encryption
with associated data (AEAD): one key, a nonce per message, and data sent
in the clear (a header, a record number) that is protected but not
hidden. TLS 1.3 encrypts every record with an AEAD, and the DNS-over-TLS
client cannot exchange a byte without one.

## Candidate mechanisms

1. **AES-128-GCM**, TLS 1.3's mandatory suite. AES in software is a
   table of 256 entries indexed by key and data bytes, which leaks
   through the cache unless bitsliced, and a bitsliced AES is large;
   GCM's multiplication has the same problem. Deferred to its own
   record.
2. **ChaCha20-Poly1305** (RFC 8439), TLS 1.3's other suite: additions,
   rotations and exclusive ors on 32-bit words, and a multiplication
   modulo 2^130 − 5 in fixed-size limbs, all naturally constant-time in
   plain code. Offered by every resolver the client will meet.
   Selected.
3. **A `ChaCha20Poly1305` object** holding the key (Go's
   `cipher.AEAD`). One more type for what two functions say; a program
   keeps the key as bytes anyway (`hkdf_expand` gives them). Rejected.

## Selected design

`spec/21` §2m, in `std::crypto`, written in CobaltC:

- `chacha20_poly1305_seal(key, nonce, aad, plaintext) : Vec<u8>`: the
  ciphertext, as long as the plaintext, followed by the 16-byte tag.
- `chacha20_poly1305_open(key, nonce, aad, sealed) : Option<Vec<u8>>`:
  the plaintext when the tag checks (with `digest_eq`), `None` when
  anything was changed or `sealed` is shorter than a tag. `Option`
  rather than a new error type: there is exactly one way to fail, and a
  program must not be able to tell why. No byte of a refused message is
  returned.
- `chacha20(key, nonce, counter, data) : Vec<u8>`, the raw stream cipher
  (QUIC's header protection and protocols built on ChaCha20 need it),
  and `poly1305(key, message) : array<u8, 16>`, the one-time MAC; each
  documented as a building block, with the AEAD named as what to use.
- Keys, nonces and lengths are slices, as `hkdf_expand` and the network
  give them; a key that is not 32 bytes, a nonce that is not 12, or a
  ChaCha20 message running past block 2^32 − 1 is the fault
  `diag.crypto-length`, registered: a wrong length is a programming
  error, never a condition of the data.
- **Constant time**, stated in the spec as `[Constant-Time]`: no branch
  and no array index depends on a key, nonce or message byte. Poly1305
  is poly1305-donna's 26-bit-limb form in `u64`, its final reduction by
  mask.
- **Verification:** RFC 8439's vectors for ChaCha20 (§2.4.2), Poly1305
  (§2.5.2) and the AEAD (§2.8.2), a tampered message refused; a sweep
  (`stress/crypto/aead_sweep.py`) of 287 random keys, nonces, associated
  data and messages against Python's `cryptography` package under all
  three tools, each also opened back and refused after a one-bit change.

Not adopted: AES-GCM (above); XChaCha20 (24-byte nonces, not in TLS); a
streaming AEAD interface (TLS records are whole).

## Compatibility impact

Additive: four exported functions in `std::crypto` and a new dynamic
diagnostic.

## Revisit conditions

- AES-128-GCM and AES-256-GCM, bitsliced, when a peer is met that offers
  no ChaCha20 suite.
- A native realization of the ChaCha20 block if `coby` is measured too
  slow for a real transfer (D-0023's latitude); cobc is unaffected.
