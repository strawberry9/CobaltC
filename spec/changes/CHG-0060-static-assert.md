# CHG-0060 — `static_assert`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0052
Affects: rule.module.static-assert (new)

## Problem / motivation

D-0052.

## What changed

- **`spec/17` 2.3.0:** §1b `rule.module.static-assert`.
- **`spec/21` 3.19.0:** §0's table.
- **`spec/registry/diagnostics.md` 1.24.0:** `diag.static-assert-failed`.
- **`spec/conformance.md` 3.46.0:** the cases below.
- **`spec/02-schema.md`:** §5's "in use" ranges.
- **Implementations:** typecheck (typing, constancy, recording per
  instantiation); `lib.rs` (`check_static_asserts`, with the
  interpreter's `eval_static_assert`); the message's rendering
  (`diagnostics::detail`); `coby` and `cobc` do nothing at run time. The
  guide (§17).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.static-assert-holds`, `conf.static-assert-fails`,
`conf.static-assert-uncalled`, `conf.static-assert-generic`,
`conf.static-assert-not-constant`, `conf.static-assert-overflow`,
`conf.static-assert-not-bool`.

## Prior-art status

See D-0052.

## Revisit conditions

None.
