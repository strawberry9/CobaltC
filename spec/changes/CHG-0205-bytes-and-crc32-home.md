# CHG-0205 — `read_le`/`read_be` to `std::collections`, `crc32` to `std::crypto`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0177)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0177
Affects: `spec/21` (4.41.0) §0, §2g, §2m; `spec/conformance.md` (3.169.0); the guide; `impl/std/`

## What changed

- **`spec/21` §0:** the submodule table: `read_le`/`read_be` under `std::collections`, `crc32` under `std::crypto`, neither under `std::text`.
- **`spec/21` §2g:** "Bytes and checksums" names each item's submodule.
- **`spec/21` §2m:** `crc32` is listed with the hashes and marked as not cryptographic.
- **Rows:** `conf.std-bytes-subjects`, `conf.std-text-has-no-bytes`.

## Compatibility classification

Additive for programs that import `std`; a program importing `std::text` alone for these names `std::collections` or `std::crypto`.
