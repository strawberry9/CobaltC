# CHG-0160 — One I/O error: `ReadError` is gone

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0134)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0134
Affects: `spec/21` §0, §2b (`rule.stdlib.read`); `spec/conformance.md`; the guide; `impl/src/prelude.rs`

## What changed

- **Previous semantics:** `read_line() : Result<Option<String>, ReadError>`, `ReadError { Io, Utf8(Utf8Error) }`, `ReadError::text`.
- **New semantics:** `read_line() : Result<Option<String>, FileError>`; `ReadError` and `ReadError::text` are gone. The errors are `FileError::Io` and `FileError::Utf8`, as a `File`'s `read_line` gives; `NotFound` and `Denied` do not occur.
- **Rows:** `conf.read-line-typed` names `FileError`.

## Compatibility classification

Breaking (source): `ReadError` is `diag.unbound-name`; a `match` on `read_line`'s error naming only `Io` and `Utf8` is `diag.non-exhaustive-match` (add `_`).
