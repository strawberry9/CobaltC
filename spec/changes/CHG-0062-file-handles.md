# CHG-0062 — `File`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0054
Affects: rule.stdlib.file-handle (new), rule.stdlib.file, rule.stdlib.prelude

## Problem / motivation

D-0054: a program could not read bytes that are not text, stream a
large file, or append to one.

## Decision

D-0054: `File`, a resource, with `open`, `create`, `append`, `open_rw`,
`read`, `read_to_end`, `read_line`, `write`, `write_str`, `seek`, `len`
and `close`; `read_bytes` and `write_bytes`.

## What changed

- **`spec/21` 3.21.0:** §2e gains `rule.stdlib.file-handle`
  (`[File-Open]`, `[File-Read]`, `[File-Read-Line]`, `[File-Write]`,
  `[File-Seek]`, `[File-Close]`, `[File-Drop]`, `[Read-Bytes]`,
  `[Write-Bytes]`, and the `std` source); `rule.stdlib.file`'s "Whole
  files" note points to it; §0's table and scope paragraph.
- **`spec/conformance.md` 3.48.0:** the cases below.
- **`spec/02-schema.md` 1.0.38:** §5's "in use" ranges.
- **Implementations:**
  - `std`'s source gains `File` and its functions, `read_bytes`,
    `write_bytes`, and two primitives private to `std`, `file_op` and
    `file_at`;
  - `impl/src/fileio.rs` gains the table of open files, with each
    file's read-ahead, for both tools; `coby` realizes both in
    `call_extern`, `cobc` compiles them to `cbrt`'s `cb_file_op` and
    `cb_file_at`;
  - `std` converts with `reinterpret` only: its first draft used
    `widen`/`narrow`, and `conf.item-shadows-intrinsic`'s own `widen`
    took the name from `std`'s code (`[Resolve-Unqualified]`).

## Affected entities

`rule.stdlib.file-handle` (new); `rule.stdlib.file`;
`rule.stdlib.prelude`.

## Previous semantics

Files only whole, and only as text.

## New semantics

`spec/21` §2e `rule.stdlib.file-handle`.

## Affected invariants

None changed.

## Compatibility classification

Extension.

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.file-handle-round-trip` (a file case, with the fixture
  `file_fixtures/not_utf8.txt`)
- `conf.file-destructor-closes`
- `conf.file-moved-to-thread`
- `conf.file-use-after-close-rejected`
- `conf.file-forged-rejected`

`impl/src/fileio.rs`'s unit tests check the table directly: lines
across read-ahead chunks, a write after reading on an `open_rw` file,
a write to a file opened for reading, a second close.
`conf.file-destructor-closes` detects a destructor that does not close
only where the limit of open files is below 2000 (commonly 1024).

## Future implementation implications

- `File::read` copies through the primitive into the `Vec`'s spare
  capacity; `coby` moves the bytes one arena cell at a time.

## Prior-art status

See D-0054.

## Revisit conditions

See D-0054.
