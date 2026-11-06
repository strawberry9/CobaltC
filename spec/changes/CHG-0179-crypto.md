# CHG-0179 — `std::crypto`, and hex and base64 in `std::text`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0151)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0151
Affects: `spec/21` (4.16.0) §0, §2d, §2m (new); `spec/conformance.md` (3.141.0); the guide §21; `impl/std/crypto.cb` (new), `impl/std/text.cb`, `impl/std/std.cb`

## What changed

- **`spec/21` §2m (new):** `rule.stdlib.crypto` with `[Sha256]`, `[Hmac-Sha256]`, `[Digest-Eq]` and the CobaltC source of `std::crypto`.
- **`spec/21` §2d `rule.stdlib.text`:** `[Hex]`, `[Base64]`; the listing gains `to_hex`, `from_hex`, `base64_encode`, `base64_decode`.
- **`spec/21` §0:** the submodule table and listing gain `std::crypto`; a `std::crypto` table; the `std::text` rows gain the four encodings.
- **`std`:** `impl/std/crypto.cb` (new), the four functions in `impl/std/text.cb`, `export module crypto` in `impl/std/std.cb`. All written in CobaltC.
- **Rows:** `conf.sha256-vectors`, `conf.sha256-streaming`, `conf.hmac-sha256-vectors`, `conf.digest-eq`, `conf.hex`, `conf.base64`.

## Compatibility classification

Additive: a new submodule and four functions of `std::text`, shadowed by a program's own.
