# CHG-0064 — Nested patterns

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0056
Affects: rule.agg.match, `spec/22` `arm`

## Problem / motivation

D-0056: a nested enum took one `match` per level.

## Decision

D-0056: variant patterns nested to any depth, arms in order, unreachable
arms rejected, the missing pattern named.

## What changed

- **`spec/16` 1.9.0:** `rule.agg.match`'s grammar (`pat`, `sub`),
  `[Pattern-Type]`, the arm taken and its binder at any depth in
  `[Match]` and `[Match-By-Ref]`, `[Match-Move-Through-Ref]` for any
  binder, `[Match-Non-Exhaustive]` over chains, `[Match-Unreachable]`
  (new), the prose.
- **`spec/22` 2.18.0:** `arm ::= pattern ':' expr`; §5 restriction 2.
- **`spec/registry/diagnostics.md` 1.26.0:** `diag.unreachable-arm`;
  `diag.non-exhaustive-match`'s entry.
- **`spec/conformance.md` 3.50.0:** the cases below.
- **`spec/02-schema.md` 1.0.40:** §5's "in use" ranges.
- **Implementations:** `ast::Arm` gains `nested` and `chain()`; the
  parser; `modres` (an identifier naming a variant); the checker
  (pattern typing, `pattern_missing` for coverage and reachability, the
  binder's type at depth); `coby` (`arm_matches`, binding through k
  payload projections, by value and by reference); `cobc` (`arm_levels`,
  an arm's tag tests, the binder's slot and `cb_borrow` path). The
  guide (§16).

## Affected entities

`rule.agg.match`.

## Previous semantics

One variant per arm; the arm for the value's variant was taken, else
`_`, wherever it stood; a second arm for a variant was never taken.

## New semantics

`spec/16` §4 as amended.

## Affected invariants

None changed.

## Compatibility classification

Source-breaking for unreachable arms (rejected now) and for a binder
spelled as a variant's name; neither occurs in the repository.

## Migration implications

Remove an unreachable arm; rename such a binder.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.match-nested`
- `conf.match-nested-non-exhaustive`
- `conf.match-duplicate-arm`
- `conf.match-covered-by-nested`
- `conf.match-nested-wrong-enum`
- `conf.match-nested-generic-payload`
- `conf.match-nested-move-through-ref`
- `conf.match-nested-move-consumes`

## Future implementation implications

None.

## Prior-art status

See D-0056.

## Revisit conditions

See D-0056.
