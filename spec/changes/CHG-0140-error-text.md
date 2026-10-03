# CHG-0140 — `FileError::text`, `ReadError::text`, `ParseError::text`, `Utf8Error::text`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0117)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0117
Affects: `spec/21` §0 (3.46.0), `spec/conformance.md` (3.113.0), the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0:** one row with the four functions and their sentences.
- **`std`:** the four functions, written in CobaltC.
- **Rows:** `conf.file-error-text`, `conf.parse-error-text`.
- **Guide:** the files section names `FileError::text`; the std table
  lists the four.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a program's own function of the same name shadows it (D-0024).
