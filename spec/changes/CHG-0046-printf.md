# CHG-0046 — Formatted output: `printf` and `String::appendf`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0038
Affects: rule.stdlib.format (new), rule.stdlib.prelude, rule.stdlib.print, diag.format-invalid (new), diag.type-mismatch, the cases listed below

## Problem / motivation

D-0038: no formatted output.

## What changed

- **`spec/21` 3.9.0:** §2f (new) `rule.stdlib.format`; §0's table and
  scope paragraph; `rule.stdlib.print`'s scope note.
- **`spec/registry/diagnostics.md` 1.15.0:** `diag.format-invalid`;
  `diag.type-mismatch` names `[Format-Arg-Mismatch]`.
- **`spec/conformance.md` 3.32.0:** the cases below.
- **`spec/02-schema.md` 1.0.22:** §5's "in use" ranges.
- **Implementations:** `impl/src/fmt.rs` (parse, render; `cbrt`
  includes it); `modres` rewrites `printf(f, …)` to `print($fmt(f, …))`
  and `String::appendf(s, f, …)` to `String::append(s, $fmt(f, …))`;
  typecheck checks `$fmt`; `coby` and `cobc` (`cb_format`) compute its
  text.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.printf-output` (a file case, its numeric lines checked
against C's `printf`), `conf.printf-arg-mismatch`,
`conf.printf-count-mismatch`, `conf.printf-format-invalid`,
`conf.printf-format-not-literal`, `conf.printf-generic-instantiation`,
`conf.appendf-builds-string`, `conf.appendf-self`.

## Prior-art status

See D-0038.

## Revisit conditions

See D-0038.
