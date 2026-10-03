# CHG-0055 — Slices, `$`, and indexing a `Vec`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-proposed)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0047
Affects: type.slice, rule.agg.slice, rule.agg.index-vec, rule.agg.dollar, rule.temporal.elision, rule.control.foreach, spec/22, the cases listed below

## Problem / motivation

D-0047.

## What changed

- **`spec/16` 1.6.0:** §3a `type.slice`, `rule.agg.slice`,
  `rule.agg.index-vec`, `rule.agg.dollar`.
- **`spec/22` 2.17.0:** tokens `..`, `$`; type-name `slice`; the slice
  form; `$`.
- **`spec/10` 1.1.0, `spec/14` 1.8.0, `spec/21` 3.16.0:** slices as
  references for elision; `foreach` over slices; `slice_len`.
- **`spec/registry/diagnostics.md` 1.22.0:** `diag.index-out-of-bounds`
  names `[Slice-Out-Of-Bounds]`.
- **`spec/conformance.md` 3.42.0:** the cases below.
- **`spec/02-schema.md` 1.0.31:** §5's "in use" ranges.
- **Implementations:** lexer (`..`, `$`, `slice`); parser (the slice
  form, `$` in brackets); typecheck (types, bounds of literal array
  slices, writes through shared slices, borrows for escape/elision,
  no move out of a `Vec` element); `coby` (slice values, `Vec` and
  slice indexing with the access mode, `$`); `cobc` (a slice struct
  with a tracked reference slot, `cb_borrow` of the source, element
  access through the slice's token); `src/each.rs` (`foreach`).

## Compatibility classification

Extension; `slice`, `$` and `..` newly reserved.

## Conformance changes

**Added:** `conf.slice-array`, `conf.slice-vec`, `conf.slice-of-slice`,
`conf.slice-dollar`, `conf.slice-param-any-source`,
`conf.slice-exclusive-write`, `conf.slice-foreach`,
`conf.vec-index`, `conf.vec-index-write`, `conf.slice-bounds-rejected`,
`conf.slice-bounds-dynamic`, `conf.slice-push-while-borrowed-rejected`,
`conf.slice-write-through-shared-rejected`,
`conf.slice-escape-rejected`, `conf.vec-index-move-out-rejected`,
`conf.dollar-empty-overflow`, `conf.slice-in-vec`, `conf.slice-in-struct`,
`conf.slice-compare-rejected`; file cases
`22-surface-syntax/slice_without_borrow_parse_error.cb`,
`dollar_outside_brackets_parse_error.cb`.

## Prior-art status

See D-0047.

## Revisit conditions

See D-0047.
