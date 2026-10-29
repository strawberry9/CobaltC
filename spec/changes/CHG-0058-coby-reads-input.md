# CHG-0058 — `coby` reads standard input

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0050
Affects: `coby`'s `std::read`; the conformance runners

## Problem / motivation

D-0050.

## What changed

- **`coby`:** `std::read` reads standard input (flushing standard
  output first, releasing the GIL while it waits) instead of stopping
  with exit 3.
- **Conformance runners:** a case's `stdin-hex:` is fed to `coby` and
  to `cobc` alike, and `coby`'s output is the oracle for `cobc`'s; the
  three `read_line_*` cases lose `cobc-only:`, which now marks only
  cases that link their own C code (`impl/conformance/README.md`). A
  spec row whose file reads input runs `coby` as a process with its
  input. `impl/tests/read_input.rs`: `Err(Io)` (a directory as
  standard input, on Unix) and a thread that runs while input is read.
- **`spec/02-schema.md`:** §5's "in use" ranges.
- **Documentation:** the main README, the guide (§21), `impl/STATUS.md`.

## Compatibility classification

Relaxation.

## Conformance changes

**Changed:** `read_line_lines_ok.cb`, `read_line_end_of_input_ok.cb`,
`read_line_invalid_utf8_ok.cb` (run by both implementations).

## Prior-art status

See D-0050.

## Revisit conditions

None.
