# CHG-0051 — Digit separators

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-proposed)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0043
Affects: spec/22 §1, the cases listed below

## Problem / motivation

D-0043: long numeric literals are hard to read.

## What changed

- **`spec/22` 2.15.0:** `digits(d)` in `int-literal`, `float-literal`
  and `exponent`, and a paragraph on where a `_` may go.
- **`spec/conformance.md` 3.37.0:** the cases below.
- **`spec/02-schema.md` 1.0.27:** §5's "in use" ranges.
- **Implementations:** the shared lexer (`digit_run`); a misplaced `_`
  is a lexical error, "a `_` in a number goes between two digits".

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.digit-separators`, `conf.parse-rejects-digit-separator`;
file cases `22-surface-syntax/digit_separator_doubled_parse_error.cb`,
`digit_separator_trailing_parse_error.cb`,
`digit_separator_after_prefix_parse_error.cb`,
`digit_separator_beside_point_parse_error.cb`,
`digit_separator_exponent_parse_error.cb`.

## Prior-art status

See D-0043.

## Revisit conditions

See D-0043.
