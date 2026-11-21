# CHG-0057 — Literal branches, shared reborrow at calls, overwriting an empty value, match consumes what it moves

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0049
Affects: rule.type.identity, rule.type.expected, rule.fn.bind-param, rule.value-object.write, rule.control.flow-analysis, rule.agg.match

## Problem / motivation

D-0049.

## What changed

- **`spec/12` 1.12.0:** `rule.type.identity` (the shared-reborrow
  exception), `rule.type.expected` (literal branches).
- **`spec/15` 1.5.0:** `rule.fn.bind-param`'s reborrow row.
- **`spec/05` 1.3.0:** `live-resource-at` requires `owns`.
- **`spec/14` 1.9.0:** the `¬live-resource-at` row; a `match` consumes
  its scrutinee binding only in an arm that moves a resource payload.
- **`spec/16` 1.7.0:** `[Match]`'s consume step and prose.
- **`spec/conformance.md`:** the cases below.
- **`spec/02-schema.md`:** §5's "in use" ranges.
- **Implementations:** typecheck (`is_literal_branch`, `weakens_to`,
  `always_owns`, per-arm consumption); `coby` (branch hints for `if`,
  `weaken_arg` at parameter binding, `value_owns`, per-arm
  consumption); `cobc` (branch probing, `weaken_arg` at calls and
  `spawn`, slices formed for a call end at its return, `cb_owns` for a
  resource target that may own nothing, per-arm consumption); `cbrt`
  (`cb_owns`, the descriptor's `owner` flag).

## Compatibility classification

Relaxation. One observable ordering change: a temporary resource
scrutinee that no arm moves out of ends at the end of its statement,
after the arm, rather than before it.

## Conformance changes

**Added:** `conf.if-literal-branch-takes-sibling-type`,
`conf.match-literal-arm-takes-later-type`,
`conf.exclusive-ref-arg-for-shared-param`,
`conf.exclusive-slice-arg-for-shared-param`,
`conf.overwrite-none-of-resource-option`,
`conf.overwrite-some-of-resource-option-dynamic`,
`conf.overwrite-owner-type-still-static`,
`conf.match-wildcard-keeps-scrutinee`,
`conf.match-move-arm-consumes`.

## Prior-art status

See D-0049.

## Revisit conditions

None.
