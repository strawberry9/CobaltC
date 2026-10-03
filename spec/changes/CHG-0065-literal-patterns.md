# CHG-0065 — Literal patterns

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0057
Affects: rule.agg.match, `spec/22` `pattern`

## Problem / motivation

D-0057: no checked multi-way branch on values.

## Decision

D-0057: integer, byte and `bool` literals as patterns, at the top level
and at the end of a nested pattern; the pattern language closed at four
forms.

## What changed

- **`spec/16` 1.10.0:** `rule.agg.match`'s grammar (`lit`), the closed
  list, `[Pattern-Type]` for literals, `[Match-Non-Exhaustive]` for
  `bool` and integer levels, the prose.
- **`spec/22` 2.19.0:** `pattern`; §5 restriction 2.
- **`spec/conformance.md` 3.51.0:** the cases below.
- **`spec/02-schema.md` 1.0.41:** §5's "in use" ranges.
- **Implementations:** `ast::Arm::lit`; the parser (a literal, and a
  message for a float); the checker (typing, `pattern_elems`,
  `pattern_missing` over `bool` and integer levels, a reference to an
  integer rejected); `coby` (`lit_equals`, a scalar `match`); `cobc`
  (a literal test in an arm's condition, `lower_match_scalar`). The
  guide (§16); `showcase/tier7` uses `Ok(0)`, `Ok(true)`.

## Affected entities

`rule.agg.match`.

## Previous semantics

A `match` scrutinee was an enum; patterns named variants only.

## New semantics

`spec/16` §4 as amended.

## Affected invariants

None changed.

## Compatibility classification

Extension.

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.match-literals`
- `conf.match-literal-non-exhaustive`
- `conf.match-bool-non-exhaustive`
- `conf.match-literal-duplicate`
- `conf.match-literal-wrong-type`
- `conf.match-literal-out-of-range`
- `conf.match-literal-on-enum`
- `conf.match-literal-through-reference`

and the file case `match_float_pattern_rejected.cb` (a parse error,
which `spec/conformance.md`'s rows do not express).

**Changed:** `12-type-system/type_mismatch_rejected.cb` (variant arms
against a reference to an `i32`) is rejected statically now, by the
check that a reference to an integer is not matched; it was dynamic.

The lexer takes `N: τ` as a suffixed literal only when `τ` names a
numeric type, since a literal pattern's arm writes `3 : x + 1`.

## Future implementation implications

None.

## Prior-art status

See D-0057.

## Revisit conditions

See D-0057.
