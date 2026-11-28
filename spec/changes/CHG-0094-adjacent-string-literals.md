# CHG-0094 — Adjacent string literals are one literal

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0078
Affects: `spec/22` §1

## Problem / motivation

D-0078: long texts had to be written on one line.

## Decision

D-0078.

## What changed

- **`spec/22` 2.28.0:** §1, adjacent `str-literal`s.
- **`spec/conformance.md` 3.76.0:** the rows below.
- **Implementations:** the shared lexer joins the tokens, so both tools
  see one literal.

## Compatibility classification

Extension: a syntax error becomes a valid program.

## Conformance changes

**Added:** `conf.adjacent-string-literals`,
`22-surface-syntax/adjacent_string_literals_ok.cb`.

## Revisit conditions

None.
