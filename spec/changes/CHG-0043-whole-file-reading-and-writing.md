# CHG-0043 — Files: `read_file` and `write_file`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0034, rule.stdlib.string, rule.fn.program
Affects: rule.stdlib.file (new), rule.stdlib.prelude, the cases listed below

## Problem / motivation

D-0034: a program could not read or write a file.

## Decision

D-0034: whole-file `read_file` and `write_file`, with `FileError`, in
both implementations.

## What changed

- **`spec/21` 3.8.0:** §2e (new) `rule.stdlib.file` (`[Read-File]`,
  `[Write-File]`); §0's type list, table and scope paragraph.
- **`spec/conformance.md` 3.29.0:** the cases below; file cases run in
  their own directory, and `$TMP` in `args:` is a fresh directory.
- **`spec/02-schema.md` 1.0.19:** §5's "in use" ranges.
- **Implementations:**
  - `std`'s source gains `FileError`, `read_file`, `write_file`, and
    two primitives private to `std` (`file_read`, `file_write`);
    `read_line` names `ReadError::Io`/`ReadError::Utf8` qualified;
  - `impl/src/fileio.rs` does the file access for both tools (`cbrt`
    includes the file); `coby` realizes the primitives in
    `call_extern`, `cobc` compiles them to `cb_file_read` and
    `cb_file_write`;
  - `cobc`: a parameter of type `void` is `cb_unit` in C (it was an
    invalid `void p`), found through `Result::unwrap_or(write_file(…), ())`;
  - the runners run each file case in its directory, and give `$TMP`
    in `args:` a fresh directory per run.

## Affected entities

`rule.stdlib.file` (new); `rule.stdlib.prelude`.

## Previous semantics

None: `std` had no file access.

## New semantics

`[Read-File]` and `[Write-File]` as `spec/21` §2e gives them.

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
- `conf.read-file-not-found`
- `conf.write-file-missing-dir`
- `conf.file-round-trip` (a file case, with the fixture
  `file_fixtures/not_utf8.txt`)
- `conf.generic-void-argument`

`Denied`, and a directory read as `Io`, depend on the platform and on
who runs the tests; `impl/src/fileio.rs` checks them in a unit test on
Unix.

## Future implementation implications

- `read_file` copies the file through a buffer and one `Vec::push` per
  byte, as `std`'s CobaltC does elsewhere.

## Prior-art status

See D-0034.

## Revisit conditions

See D-0034.
