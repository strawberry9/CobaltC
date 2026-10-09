# CHG-0053 — `Box<T>`; recursive types without indirection rejected

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-approved)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0045
Affects: rule.stdlib.box (new), rule.agg.layout, diag.recursive-type (new), the cases listed below

## Problem / motivation

D-0045: a recursive type crashed both implementations, and `std` had no
single-owner heap indirection.

## What changed

- **`spec/16` 1.4.0:** `[Type-Recursive]` in `rule.agg.layout`.
- **`spec/21` 3.15.0:** §3a `rule.stdlib.box`.
- **`spec/registry/diagnostics.md` 1.21.0:** `diag.recursive-type`.
- **`spec/conformance.md` 3.40.0:** the cases below.
- **`spec/02-schema.md` 1.0.29:** §5's "in use" ranges.
- **Implementations:** typecheck (`recursive_type`, first in
  `check_program`); `Box` in the prelude.

## Compatibility classification

Extension (the rejected programs crashed both tools before).

## Conformance changes

**Added:** `conf.recursive-type-rejected`,
`conf.recursive-type-array-rejected`, `conf.box-recursive-type-ok`,
`conf.box-get-mut`, `conf.box-into-inner`.

## Prior-art status

See D-0045.

## Revisit conditions

See D-0045.
