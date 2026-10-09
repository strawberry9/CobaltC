# CHG-0211 — `std::compress`, and compressed HTTP bodies

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0183)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0183
Affects: `spec/21` (4.44.0) §0, §2s, §2p; `spec/conformance.md` (3.174.0); the guide; `impl/std/compress.cb`, `impl/std/http.cb`, `impl/std/std.cb`, `src/prelude.rs`

## What changed

- **`spec/21` §2s `rule.stdlib.compress`:** `[Deflate]`, `[Inflate]`, `[Zlib]`, `[Gzip]`, `[Adler32]`, `[Compress-Error]`; the listing.
- **`spec/21` §2p `rule.stdlib.http`:** `[Content-Encoding]`: the client offers `gzip, deflate` and decodes them.
- **`spec/21` §0:** `std::compress` in the module table; its items.
- **Rows:** `conf.deflate-inflate`, `conf.zlib`, `conf.gzip`, `conf.adler32`, `conf.inflate-errors`, `conf.http-content-encoding`.

## Compatibility classification

Additive; the HTTP client sends `Accept-Encoding` by default and decodes `Content-Encoding: gzip`/`deflate` bodies it received as bytes before.
