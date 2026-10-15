# CHG-0075 — Runtime assertions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0065
Affects: rule.fail.assert (new), rule.stdlib.prelude

## Problem / motivation

D-0065: no runtime check with a message.

## Decision

D-0065: `assert(c)` and `assert(c, f, a…)`, always on; the message
formatted only on failure.

## What changed

- **`spec/18` 1.4.0:** §1a (new) `rule.fail.assert` (`[Assert]`,
  `[Assert-Fail]`, `[Assert-Ill-Formed]`).
- **`spec/21` 3.28.0:** §0's table.
- **`spec/registry/diagnostics.md` 1.28.0:** `diag.assert-failed`.
- **`spec/conformance.md` 3.58.0:** the cases below.
- **`spec/02-schema.md` 1.0.48:** §5's "in use" ranges.
- **Implementations:** `modres` rewrites `assert`; the checker types
  `$assert_fail`; `coby` faults with the message as a detail; `cobc`
  lowers to `cb_fault_msg`, which `cbrt` renders with the message after
  the location (`diagnostics::with_message`, shared with `coby`). The
  guide uses `assert` and `static_assert` in place of its `check`
  helper.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.assert-holds`, `conf.assert-fails`,
`conf.assert-without-message`, `conf.assert-not-bool`.

## Prior-art status

See D-0065.

## Revisit conditions

See D-0065.
