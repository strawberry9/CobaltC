# CHG-0202 — An HTTP/1.1 server in `std::http`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0174)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0174
Affects: `spec/21` (4.39.0) §0, §2p; `spec/conformance.md` (3.165.0); the guide; `impl/std/http.cb`

## What changed

- **`spec/21` §2p:** `[Http-Accept]`, `[Http-Request]`, `[Http-Respond]`; the listing.
- **Rows:** `conf.http-server-keep-alive`, `conf.http-server-no-body`, `conf.http-server-refusals`.

## Compatibility classification

Additive.
