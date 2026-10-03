# CHG-0142 — `Vec::push_le`/`push_be`, `read_le`/`read_be`, `crc32`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0119)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0119
Affects: `spec/21` §0 and §2g (3.47.0), `spec/conformance.md` (3.115.0), the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21`:** two rows; `[Bytes-LE]`, `[Bytes-BE]`, `[CRC32]` with the
  CRC-32/IEEE parameters, the bitwise algorithm and its check values.
- **`std`:** the five functions, written in CobaltC.
- **Rows:** `conf.bytes-le-be`, `conf.bytes-signed-roundtrip`,
  `conf.bytes-short-buffer-faults`, `conf.crc32-check`, `conf.crc32-empty`.
- **Guide:** §21's `Vec` table rows and a paragraph with an example.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a program's own function of the same name shadows it (D-0024).
