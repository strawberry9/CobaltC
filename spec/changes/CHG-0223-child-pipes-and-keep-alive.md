# CHG-0223 — `ChildInput`, `ChildOutput`, `HttpClient::keep_alive`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0195)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0195
Affects: `spec/21` 4.47.0; `spec/conformance.md` 3.188.0 (`conf.process-child-pipes`, `conf.http-keep-alive`); `impl/`

## What changed

- **`spec/21`:** §2k `[Child-Pipes]` and table row; §2p `[Http-Send]`'s `Connection: close` now conditional,
  `[Http-Keep-Alive]`, `HttpClient`'s field; the §2k listing re-synced with `process.cb`; the §2p listing's
  `HttpClient`, `request_bytes` and `exchange` (now `origin_of`, `exchange`, `exchange_on`).
- **`std`:** `process.cb` (the two resources and five functions), `http/client.cb` (`keep_alive`, `IdleConn`, the
  exchange on a kept connection).
- **Both tools' OS layer (`procio.rs`):** a table of taken pipes; operations 20–25.

## What did not change

Every existing program: a `Child` whose pipes are not taken, and an `HttpClient` without `keep_alive`, behave as
before.
