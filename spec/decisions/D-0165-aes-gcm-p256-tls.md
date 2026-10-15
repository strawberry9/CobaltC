# D-0165 — AES-GCM, P-256 key agreement, and RFC 8446's mandatory set in `std::tls`

Status: ACCEPTED (2026-10-05, the owner: "accept and implement AES-128-GCM and P-256 (and any other that would be worthwhile as well) as D-0165")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0153 (ChaCha20-Poly1305, which deferred AES-GCM), D-0154 (X25519), D-0159 (ECDSA over P-256), D-0161 (the TLS 1.3 client)
Affects: `spec/21` §0, §2m, §2o (4.30.0), `spec/registry/diagnostics.md` (1.47.0), `spec/conformance.md` (3.156.0), the guide §21, `impl/std/crypto.cb`, `impl/std/tls.cb`; `CHG-0193`

## Problem

Round 7's HTTPS tests reached 30 public sites with `std::tls`; eBay
refused the handshake, and a survey of 40 popular sites found 4 that
refuse TLS_CHACHA20_POLY1305_SHA256 with X25519, the client's only
offer. eBay and Facebook accept only AES-GCM with it; office.com takes
AES-GCM over P-256 alone. RFC 8446 §9.1 makes TLS_AES_128_GCM_SHA256 and
secp256r1 mandatory to implement, so a client without them is outside
the protocol's baseline. AES-GCM is also the AEAD most file formats and
protocols use, and D-0153 deferred it.

## Candidate mechanisms

1. **AES-GCM in `std::crypto` for 16-, 24- and 32-byte keys; P-256 key
   agreement; the TLS client offering both suites and P-256 through a
   HelloRetryRequest.** Selected.
2. **Only AES-128.** AES-192 and AES-256 differ only in the key schedule,
   and AES-256-GCM is what most formats choose; leaving them out saves
   nothing. Not adopted.
3. **A P-256 key share in every ClientHello.** No round trip for servers
   that want P-256, but a P-256 multiplication in every handshake,
   costly under the interpreter, for the few servers that need it.
   Rejected: the share is sent only when the server asks.
4. **TLS_AES_256_GCM_SHA384.** Needs the key schedule over SHA-384; no
   surveyed server required it (every one that offers it offers
   TLS_AES_128_GCM_SHA256). Deferred.
5. **A table-driven AES.** Fast, but the table index is a key byte, a
   timing channel through the cache. Rejected for the computed S-box.

## Selected design

**`std::crypto`** (`spec/21` §2m):

- `aes_encrypt_block(key, block)`: FIPS 197 for a 16-, 24- or 32-byte key.
- `aes_gcm_seal(key, nonce, aad, plaintext)` and `aes_gcm_open(…)`: NIST SP 800-38D with a 12-byte nonce and a 16-byte tag, the same shape as `chacha20_poly1305_seal` and `_open`.
- `p256_private_key()`, `p256_public_key(k) : Option<array<u8, 65>>` and `p256_ecdh(k, peer) : Option<array<u8, 32>>`, mirroring the X25519 functions. A private key outside [1, n-1] and a peer point that is not uncompressed or not on the curve give None.
- Another AES key length, or a nonce that is not 12 bytes, is `diag.crypto-length`, as is a P-256 private key that is not 32 bytes.

All of this is bound by `[Constant-Time]`:

- **AES** computes its S-box as x^254 in GF(2^8) by masked multiplications, eight bytes at a time.
- **GHASH** multiplies by masks.
- **P-256** makes the scalar 257 bits by adding n or 2n, then doubles and adds at every bit and keeps the sum by a mask.
- **The P-256 and P-384 field arithmetic**, shared with `ecdsa_verify`, now reduces by masks instead of branches.

**`std::tls`** (`spec/21` §2o):

- **Suites:** the client offers TLS_CHACHA20_POLY1305_SHA256 and then TLS_AES_128_GCM_SHA256.
- **Groups:** it offers X25519 and P-256, with an X25519 key share.
- **HelloRetryRequest:** a server that asks for P-256 gets a second ClientHello with a P-256 share and its cookie. The transcript restarts as RFC 8446 §4.4.1 says. A second HelloRetryRequest, or one asking for anything else, is HandshakeFailed.
- **Alert 40:** its text names what the client offers.

## Compatibility impact

Additive. A server that chose ChaCha20-Poly1305 with X25519 before still
can; one that refused now completes.

## Revisit conditions

- TLS_AES_256_GCM_SHA384 or P-384 key agreement, if a server needs them.
- A faster constant-time AES (bitsliced), if AES-GCM throughput matters.
