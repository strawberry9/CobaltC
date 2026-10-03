# CHG-0059 — Float conversions and limits

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0051
Affects: rule.arith.convert, rule.arith.limits

## Problem / motivation

D-0051.

## What changed

- **`spec/06` 1.10.0:** `[Widen-Float]`, `[Float-To-Float]`,
  `[Reinterpret-Float]`; `[T-Convert]`'s premises; `[Min-Value]`,
  `[Max-Value]` for `f32` and `f64`; `[Limits-Not-Integer]` renamed
  `[Limits-Not-Number]`.
- **`spec/12` 1.12.1**, **`spec/registry/diagnostics.md` 1.23.0:** the
  renamed rule.
- **`spec/21` 3.18.0:** §0's table.
- **`spec/conformance.md` 3.45.0:** the cases below.
- **`spec/02-schema.md`:** §5's "in use" ranges.
- **Implementations:** the checker's conversion and limit premises;
  `coby`'s `convert` and limits; `cobc`'s conversions (C casts between
  `float` and `double`, a union for the bits) and limits (hex float
  literals). The showcase `float_anatomy` checks its raw-pointer read
  against `reinterpret<u64>`. The guide (§06).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.limits-float`, `conf.widen-f32-to-f64`,
`conf.to-float-f64-to-f32-rounds`, `conf.widen-f64-to-f32-rejected`,
`conf.reinterpret-float-bits`, `conf.reinterpret-float-width-rejected`.
**Changed:** `conf.limits-not-integer` (`bool` in place of `f64`).

## Prior-art status

See D-0051.

## Revisit conditions

None.
