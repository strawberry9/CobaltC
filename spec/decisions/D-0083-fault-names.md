# D-0083 — `fault` takes only run-time diagnostics

Status: ACCEPTED (2026-09-28, owner decision)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: rule.stdlib (`fault`), `spec/registry/diagnostics.md`
Affects: `spec/21` §0

## Problem

`fault(d)` raised `diag.d` for any identifier `d`. Written in a program,
`fault(assertion_failed)` produced `diag.assertion-failed`, a diagnostic
the registry does not have. `fault` is also rightly used outside `std`:
a program that implements its own collection raises the standard faults
(`showcase/tier4/vec_from_scratch.cb`, `fault(index_out_of_bounds)`).

## Candidate mechanisms

1. **Restrict `fault` to `std`.** This closes the hole but takes the
   standard faults away from code that implements containers, and
   `assert` does not replace them: it reports `diag.assert-failed`.
2. **Keep `fault` available; its name must be a registered diagnostic of
   phase `dynamic` or `both`.** Selected. No unregistered id can appear,
   and library-like code keeps the standard faults.

## Selected design

- `fault(d)` and `fault(d, m)`: `d` is a diagnostic of phase `dynamic`
  or `both` in `spec/registry/diagnostics.md`, written with `_` for `-`.
  `m`, a `str`, is shown after the location, as `assert`'s message is.
- Any other `d` is `diag.unbound-name` (static). The message names the
  form and gives examples (`assert_failed`, `index_out_of_bounds`,
  `unwrap_failed`, `alloc_failure`).

## Compatibility impact

Programs naming an unregistered fault are now rejected. The conformance
suite and the guide used `fault(assertion_failed)`; they now use
`fault(assert_failed)` or `Result::expect`.

## Revisit conditions

None.
