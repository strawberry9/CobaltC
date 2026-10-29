# CHG-0201 — An HTTP/1.1 client in `std::http`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0173)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0173
Affects: `spec/21` (4.38.0) §0, §2p; `spec/conformance.md` (3.164.0); the guide; `impl/std/http.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`

## What changed

- **`spec/21` §2p (new):** `rule.stdlib.http`: `[Url-Parse]`, `[Percent]`, `[Header-Find]`, `[Http-Send]`, `[Http-Body]`, `[Http-Redirect]`, `[Http-Error]`; the listing.
- **Rows:** `conf.url-parse`, `conf.percent-encoding`, `conf.http-headers`, `conf.http-get-post`, `conf.http-redirects`, `conf.http-chunked`, `conf.http-until-close`, `conf.http-client-refusals`.

## Compatibility classification

Additive.
