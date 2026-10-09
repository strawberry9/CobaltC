# CHG-0102 — A use of a binding that may have been moved is rejected

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12
Depends on: D-0087
Affects: rule.control.flow-analysis, the checker

## Problem / motivation

A move inside a loop body, or in one branch, followed by a use was
caught only when the run took the moving path (D-0087).

## Decision

D-0087.

## What changed

- **`spec/14` 1.14.0:** the discharge table refutes
  `temporally-valid(a_x)` also when `valid(x) = ?`; the note on
  exceptions to "unknown is dynamic" names it.
- **Implementations:** `src/typecheck.rs` (`check_path`, a closure's
  captures): `diag.stale-binding` says the binding *may* have been moved,
  on some path or in an earlier pass of a loop. `tests/conformance.rs`:
  `conf_match_move_arm_consumes` and the moved-binding half of the merge
  and loop test expect the static rejection.

## Compatibility classification

Tightening.

## Conformance changes

**Changed** (`(dynamic)` → `(static)`):
`conf.match-nested-move-consumes`
(`16/match_nested_move_consumes_rejected.cb`), `conf.match-move-arm-consumes`;
`14/flow_move_in_loop_unknown_dynamic.cb`,
`14/flow_move_in_one_branch_unknown_dynamic.cb` and
`21/vec_len_after_move_in_loop_dynamic.cb`, renamed
`…_maybe_moved_rejected.cb`, `…_maybe_moved_rejected.cb` and
`vec_len_after_move_in_loop_rejected.cb`.
**Added:** `14/moved_then_given_a_value_on_every_path_ok.cb`.

## Revisit conditions

None.
