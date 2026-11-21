# D-0161 — `std::tls`: a TLS 1.3 client

Status: ACCEPTED (2026-10-04, the owner: "proceed with the entire roadmap using your discretion", on the crypto roadmap agreed toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0151–D-0154, D-0156–D-0160, D-0144 (`TcpStream`)
Affects: `spec/21` §0, §2m, §2o (new) (4.26.0), `spec/conformance.md` (3.151.0), the guide §21, `impl/std/tls.cb` (new), `impl/std/crypto.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`; `CHG-0189`

## Problem

The owner's flagship program is a DNS-over-TLS client. Every byte it
exchanges with a resolver travels in TLS; without a TLS client in `std`
it would have to wrap a C library, which would make it a wrapper and
break the rule that both tools compute the same thing.

## Candidate mechanisms

1. **Bind the platform's TLS** (OpenSSL, SChannel) through `extern`.
   Fast to write, different on every platform, outside the language's
   checks. Rejected (owner's agreed roadmap).
2. **TLS 1.3 client in CobaltC** over `std::crypto` and `std::x509`,
   with the smallest set of options every public server accepts.
   Selected.

## Selected design

`spec/21` §2o, `rule.stdlib.tls`, a new submodule `std::tls`, RFC 8446:

- `TlsStream::client(tcp, server_name, roots)` and
  `TlsStream::connect(addr, server_name, roots)`: one cipher suite,
  TLS_CHACHA20_POLY1305_SHA256; one group, X25519; signature schemes
  ECDSA P-256/P-384 and RSA-PSS for the handshake, plus PKCS #1 v1.5 for
  certificates; SNI for a DNS name; the middlebox-compatibility session
  ID and ChangeCipherSpec. A HelloRetryRequest (a server that wants
  another group) is `HandshakeFailed`.
- The server's chain is checked with `verify_chain` against `roots` and
  `server_name` at the current time; its CertificateVerify and Finished
  are checked; the client's Finished is sent.
- `read` (0 at close_notify), `read_exact`, `write`, `write_text`,
  `close` (sends close_notify). After the handshake, NewSessionTicket is
  ignored and KeyUpdate is followed (and answered when asked).
- `hkdf_expand_label` in `std::crypto` (the key schedule's primitive,
  also QUIC's).
- `TlsError` names the failure: `NetFailure(NetError)`,
  `CertRejected(CertError)`, `HandshakeFailed`, `ProtocolError`,
  `PeerAlert(u8)`, `RecordRejected`.

Verified: the key schedule against RFC 8448; a hermetic case refusing a
non-TLS peer and an immediate close; a local OpenSSL TLS 1.3 server
(Python's `ssl`) with the test PKI, EC and RSA leaves, names by DNS,
wildcard and IP, a refused wrong name, 100,000 bytes through
`read_exact`, close_notify, under coby and cobc; and DNS-over-TLS
queries to Cloudflare, Google, Quad9 and AdGuard's public resolvers
(`stress/dot/smoke.cb`), handshakes of 46–225 ms compiled.

Not adopted: TLS 1.2, AES-GCM suites, other groups, session resumption
and early data, client certificates, a server side, ALPN.

## Compatibility impact

Additive: a new submodule and `hkdf_expand_label`.

## Revisit conditions

- AES-128-GCM (bitsliced) and a second group (P-256), when a server is
  met that offers neither ChaCha20-Poly1305 nor X25519.
- ALPN, for DNS-over-HTTPS or other protocols over TLS.
- A TLS server, if a program asks.
