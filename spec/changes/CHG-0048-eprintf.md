# CHG-0048 — Standard error: `eprintf`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0040
Affects: rule.stdlib.format, rule.stdlib.prelude, diag.format-invalid, the cases listed below

## Problem / motivation

D-0040: a program could not write to standard error.

## What changed

- **`spec/21` 3.11.0:** `[Eprintf]` in `rule.stdlib.format`; §0's
  table and scope paragraph list `eprintf`.
- **`spec/registry/diagnostics.md` 1.17.0:** `diag.format-invalid`
  covers `eprintf`.
- **`spec/conformance.md` 3.34.0:** the cases below.
- **`spec/02-schema.md` 1.0.24:** §5's "in use" ranges.
- **Implementations:**
  - `std` (src/prelude.rs): the private extern `write_err` and helper
    `eprint_str`;
  - `modres`: `eprintf(f, a…)` → `std::eprint_str($fmt(f, a…))`;
  - `coby`: `write_err` writes to its standard error;
  - `cobc`/cbrt: `cb_write_err`.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.eprintf-stderr` (file case
`21-standard-library-semantics/eprintf_stderr_ok.cb`, its standard error
checked by `impl/tests/print_output.rs`), `conf.eprintf-format-checked`,
`conf.write-err-private`.

## Prior-art status

See D-0040.

## Revisit conditions

See D-0040.
