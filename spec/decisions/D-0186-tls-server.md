# D-0186 — A TLS 1.3 server, and an https server

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0161 (the TLS 1.3 client), D-0165, D-0172 (the suites, groups and schemes), D-0168, D-0169, D-0167 (signing with EC, RSA and Ed25519 keys), D-0171 (`PrivateKey`, PEM), D-0174 (the HTTP server), D-0181 (`peer_certificate`)
Affects: `spec/21` §0, §2o, §2p (4.44.0), `spec/conformance.md` (3.174.0), the guide §21, `impl/std/tls.cb`, `impl/std/http.cb`; `CHG-0214`

## Problem

`std` could reach any TLS server and be none: a CobaltC program could
not serve https, nor any TLS-protected protocol inbound, without a
reverse proxy in front of it. The HTTP server's connection type already
had a `Secure(TlsStream)` arm waiting for this. Every piece the server
side needs was there: the record layer, the key schedule, the three
suites, the three groups, signing with every key kind `std` reads, and
PEM.

## Candidate mechanisms

1. **`TlsStream::server(tcp, key, chain)`, the handshake mirrored, and
   `HttpServer::bind_tls(addr, key, chain)`.** Selected.
2. **A TLS 1.2 server as well.** No: the client is 1.3 only, and every
   client of this decade speaks 1.3.
3. **Client certificates, session resumption, early data.** Each a
   later decision; the server ignores a session ticket request and
   offers none.
4. **Certificate generation (self-signed, CSR) in `std::x509`.** A
   server needs a certificate from somewhere; tools make them today.
   A later decision.

## Selected design

`spec/21` §2o `[Tls-Server]`: `TlsStream::server(tcp, key, chain)`
performs the handshake over an accepted `tcp` as the server holding
`key` (a `PrivateKey`: RSA, P-256, P-384 or Ed25519) and `chain` (the
server's certificate first, then its issuers up to but not including the
root):

- The ClientHello is read and taken apart; a client without TLS 1.3 in
  `supported_versions` is answered with alert 70 (protocol_version) and
  `HandshakeFailed`.
- The suite is the client's first among TLS_CHACHA20_POLY1305_SHA256,
  TLS_AES_128_GCM_SHA256 and TLS_AES_256_GCM_SHA384; the signature scheme
  is the key's own (rsa_pss_rsae_sha256, ecdsa_secp256r1_sha256,
  ecdsa_secp384r1_sha384 or ed25519) and must be in the client's
  `signature_algorithms`; none in common is alert 40 (handshake_failure).
- The group is the client's first key share among X25519, P-256 and
  P-384. A client that sent no usable share but lists one of them in
  `supported_groups` gets a HelloRetryRequest for its first such group
  (RFC 8446 §4.1.4, the transcript restarted from `message_hash`), and
  its second ClientHello must carry that share, the same suite and the
  same session id (alert 47, illegal_parameter, otherwise).
- ServerHello, a compatibility ChangeCipherSpec, then under the server
  handshake keys EncryptedExtensions (empty), Certificate (the chain,
  each entry without extensions), CertificateVerify (the key's signature
  over the 64 spaces, the context string and the transcript hash, by
  `sign`) and Finished; the client's Finished is read under its
  handshake keys and verified; the application keys follow RFC 8446
  §7.1 as the client's do. The stream then sends under the server
  application keys and receives under the client's; `peer_certificate`
  is `None` (clients send no certificate).
- A malformed message is `ProtocolError`; a client's alert is
  `PeerAlert`; the stream's own timeouts apply.

`spec/21` §2p `[Https-Bind]`: `HttpServer::bind_tls(addr, key, chain)`
binds as `bind` does, and `HttpServer::accept` completes the handshake
for each connection before giving it, as an `HttpConnection` over a
`TlsStream`; a client whose handshake fails is `Err(TlsFailure(e))`,
and the server goes on to the next. **`HttpServer::accept` now returns
`Result<HttpConnection, HttpError>`** (it was `NetError`): the one
breaking change of this round, since an https server's accept can fail
in TLS. A program that matched `Err(_)` (every one in the repository)
is unchanged; one that matched a `NetError` variant writes
`Err(NetFailure(Refused))` now. `HttpConnection::peer_addr` works for a
TLS connection too.

## Compatibility impact

`HttpServer::accept`'s error type changes (above). Otherwise additive:
`TlsStream::server`, `HttpServer::bind_tls`. No new tokens.

## Testing

Hermetic: `conf.tls-server` (`tls_server_loopback_ok.cb`) with a
self-signed Ed25519 certificate for `localhost` valid to 2100 embedded
in the case; `conf.https-server` (`https_loopback_ok.cb`) with a P-256
one, through `HttpClient` with that certificate as its only root. Each
is about a minute under `coby` (two sides of public-key arithmetic in
one interpreter), well within the harness's limit. Outside the suite
(`stress/std2`): `openssl s_client` against `tlsserve.cb` compiled by
`cobc`, every certificate kind (Ed25519, P-256, RSA-2048) under each
suite, verified (`Verify return code: 0`); P-256 and P-384 key shares;
a HelloRetryRequest (`-groups X448:X25519`); and `curl`-free interop of
the https server with our own client.

## Revisit conditions

- Client certificates (mutual TLS), now that both sides exist.
- Session resumption, if handshake cost matters to a server.
- Certificate generation in `std::x509`, for a self-signed development
  certificate or an ACME client.
- ALPN, for HTTP/2 or gRPC.
