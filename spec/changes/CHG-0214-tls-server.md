# CHG-0214 — A TLS 1.3 server and `HttpServer::bind_tls`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0186)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0186
Affects: `spec/21` (4.44.0) §0, §2o, §2p; `spec/conformance.md` (3.174.0); the guide; `impl/std/tls.cb`, `impl/std/http.cb`

## What changed

- **`spec/21` §2o `rule.stdlib.tls`:** `[Tls-Server]`; the listing.
- **`spec/21` §2p `rule.stdlib.http`:** `[Https-Bind]`; `[Http-Accept]` gives `HttpError`; the listing.
- **`spec/21` §0:** `TlsStream::server`, `HttpServer::bind_tls`; `accept`'s type.
- **Rows:** `conf.tls-server`, `conf.https-server`.

## Compatibility classification

`HttpServer::accept` returns `Result<HttpConnection, HttpError>` in place of `Result<HttpConnection, NetError>`: a program matching a `NetError` variant of its error changes (none in the repository does; `Err(_)` is unchanged). Otherwise additive.
