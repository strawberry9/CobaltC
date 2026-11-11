# CHG-0071 — A read or write against a `File`'s mode is `Err(Io)`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, fix of a portability defect)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0054
Affects: rule.stdlib.file

## Problem / motivation

`write` on a `File` opened only for reading was "a failure the
environment reports", so its `FileError` was the operating system's:
`Io` on Linux (EBADF), `Denied` on Windows (access denied). The
conformance case `21-standard-library-semantics/file_printf_ok.cb`
expected `Io` and failed under `coby` on Windows.

## Decision

The mismatch is the program's, not the environment's: `read` or
`read_line` on a file opened only for writing, and `write` on one opened
only for reading, give exactly `Err(Io)`, checked before the operating
system is asked.

## What changed

- **`spec/21` 3.25.0:** the paragraph after `[File-Drop]`.
- **`spec/02-schema.md` 1.0.45:** §5's "in use" ranges.
- **Implementations:** `impl/src/fileio.rs` (shared by `coby` and
  `cbrt`) records whether a handle is readable and refuses a read, a
  line length or a write against the mode with `Io`; its unit tests
  check all three.

## Compatibility classification

Clarification: on Linux the observable behavior is unchanged.

## Conformance changes

None added; `file_printf_ok.cb` now holds on every platform.

## Revisit conditions

None.
