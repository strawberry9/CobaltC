# CHG-0106 — Numeric functions and `HashMap::clear`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9
Depends on: D-0091
Affects: rule.arith.float-fns, rule.stdlib.hashmap, the implementations

## Problem / motivation

Programs rewrote `min`, `max`, `abs` and powers, and reached `sqrt`
through `extern` (D-0091).

## Decision

D-0091.

## What changed

- **`spec/06` 1.11.0:** `rule.arith.float-fns`.
- **`spec/21` 3.36.0:** the float intrinsics in §0's table; `min`, `max`,
  `abs`, `pow`; `HashMap::clear`.
- **`spec/12`:** `rule.type.bound`'s dependencies name existing rules.
- **Implementations:** `std` (`src/prelude.rs`): `min`, `max`, `abs`,
  `pow`, `HashMap::clear`; the checker (`FLOAT_INTRINSICS`), coby
  (`try_intrinsic`) and cobc (C's `<math.h>` functions) for the float
  intrinsics; coby types a `number` parameter's literal operand at a
  floating-point instantiation (`0 - x`, `x < 0`), which D-0090's
  bodies need.
- **Guide:** the numeric functions and `HashMap::clear` in the library
  tables.

## Compatibility classification

Additive.

## Conformance changes

**Added:** `conf.min-max`, `conf.abs-min-overflows`,
`conf.pow-checked`, `conf.float-fns`, `conf.float-fn-int-rejected`,
`conf.hashmap-clear`, and `impl/conformance/21-standard-library-semantics/numeric_functions_ok.cb`.

## Revisit conditions

None.
