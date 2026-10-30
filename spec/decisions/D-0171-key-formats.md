# D-0171 — Keys in their standard encodings, and Ed25519 in certificates

Status: ACCEPTED (2026-10-05, the owner chose "Key formats: PKCS#8, SPKI, PEM" from the extras offered)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0160 (`std::x509`, DER, `CertKey`, `SignatureScheme`), D-0167 (Ed25519), D-0168 (EC keys), D-0169 (RSA private keys)
Affects: `spec/21` §0, §2n (4.36.0), `spec/conformance.md`, the guide §21, `impl/std/x509.cb`; `CHG-0199`

## Problem

With signing in `std`, a program needs its key from somewhere: a file
`openssl` wrote, a secret store, a certificate's public key. Without the
standard encodings each program would carry raw bytes in a format of its
own and could exchange keys with nothing. Certificates with Ed25519 keys
or signatures were `UnsupportedKey`/`UnsupportedSignature`.

## Candidate mechanisms

1. **PKCS #8 for private keys, SubjectPublicKeyInfo for public keys, each
   in DER and PEM; PKCS #1 and SEC 1 read as well; `PrivateKey`, `sign`;
   `Ed25519Key` and `Ed25519` in certificates.** Selected.
2. **Raw byte formats only.** Interoperates with nothing. Rejected.
3. **Encrypted PKCS #8 (PBES2) read too.** Needs AES-CBC and the PBES2
   parameter grammar; passwords on key files are the rarer practice now.
   Deferred; read as `UnsupportedAlgorithm`.
4. **JWK.** A later decision alongside any JSON support.

## Selected design

**`spec/21` §2n:**

- `CertKey` gains `Ed25519Key(array<u8, 32>)`; `SignatureScheme` gains
  `Ed25519`; `Certificate::parse` reads both (OID 1.3.101.112);
  `verify_signature` checks Ed25519 over the whole message.
- `CertKey::from_der`, `from_pem` (`PUBLIC KEY`), `to_der : Option`,
  `to_pem : Option` (None for `UnsupportedKey`): SubjectPublicKeyInfo
  (RFC 5280 §4.1.2.7), `UnsupportedAlgorithm` for an algorithm outside
  RSA, P-256, P-384 and Ed25519.
- `EcPrivateKey` (`curve`, `scalar`); `PrivateKey` (`RsaPrivate`,
  `EcPrivate`, `Ed25519Private`); `PrivateKey::from_der` reads PKCS #8
  (RFC 5958), PKCS #1 RSAPrivateKey and SEC 1 ECPrivateKey, told apart by
  their first fields; `from_pem` reads `PRIVATE KEY`, `RSA PRIVATE KEY`
  and `EC PRIVATE KEY` blocks (`ENCRYPTED PRIVATE KEY` is
  `UnsupportedAlgorithm`); `to_der` and `to_pem` always write PKCS #8;
  `public_key`.
- `sign(key, scheme, message) : Option<Vec<u8>>`: the counterpart of
  `verify_signature`; None when the key is not of the scheme's kind.

## Compatibility impact

Additive. A `match` over `CertKey` or `SignatureScheme` that listed every
variant gains one; the conformance suite's did not.

## Revisit conditions

- Encrypted PKCS #8, JWK, SSH key formats, PKCS #12.
- Certificate issuing (a `Certificate` built and signed), now that
  `sign` exists.
