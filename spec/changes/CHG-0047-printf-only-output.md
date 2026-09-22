# CHG-0047 — One way to write output: `printf`, `%v`, `sprintf`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0039
Affects: rule.stdlib.print, rule.stdlib.format, rule.stdlib.prelude, diag.format-invalid, the cases listed below

## Problem / motivation

D-0039: `print` and `printf` were two ways to write output.

## What changed

- **`spec/21` 3.10.0:** `print` is not exported; `rule.stdlib.print`
  defines the text `printf`, `%v` and `String::append` write;
  `rule.stdlib.format` gains `%v` and `[Sprintf]`.
- **`spec/12` 1.10.1, `spec/17` 2.2.2, `spec/examples.md` 3.13.1:**
  non-normative wording.
- **`spec/conformance.md` 3.33.0:** the cases below; every fragment
  calling `print` now calls `printf`.
- **`spec/02-schema.md` 1.0.23:** §5's "in use" ranges.
- **`spec/registry/diagnostics.md` 1.16.0:** `diag.format-invalid` covers
  `sprintf` and `%v`.
- **Implementations** (shared front end unless named):
  - `fmt.rs`: the `v` conversion (width and `-` only), rendering
    through `format_float`, which moved here from `value.rs` and cbrt;
  - `modres`: `print` not exported; `sprintf` rewritten to
    `std::String::from_str($fmt(…))`;
  - typecheck: `%v` takes any printable type;
  - `coby`, `cobc`/cbrt: the `v` kind in `$fmt` and `cb_format`.
- **Programs:** every example, conformance case, test, showcase
  program, benchmark and guide section migrated from `print` to
  `printf`.

## Compatibility classification

Breaking: a program calling `print` is rejected (`diag.unbound-name`).

## Conformance changes

**Added:** `conf.printf-value`, `conf.printf-value-precision-rejected`,
`conf.sprintf-returns-string`, `conf.print-unbound`,
`conf.std-print-private`, `conf.print-own-declaration-ok`.

**Changed (fragment only, same outcome):** every row whose fragment
called `print`; `conf.std-own-declaration-wins` declares its own
`printf`.

## Prior-art status

See D-0039.

## Revisit conditions

See D-0039.
