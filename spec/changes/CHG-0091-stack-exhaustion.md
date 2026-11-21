# CHG-0091 — Stack exhaustion is `diag.stack-exhausted`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0077
Affects: rule.fn.call, `spec/registry/diagnostics.md`

## Problem / motivation

D-0077: deep recursion crashed both tools.

## Decision

D-0077.

## What changed

- **`spec/15` 1.6.0:** `[Call-Stack-Exhausted]`, the guaranteed minimum
  depth.
- **`spec/registry/diagnostics.md` 1.34.0:** `diag.stack-exhausted`.
- **`spec/conformance.md` 3.74.0:** the cases below.
- **`spec/02-schema.md` 1.0.64:** §5's "in use" ranges.
- **Implementations:**
  - **`coby`:** `set_stack_budget` for the runner thread and each program
    thread; a check at every function and closure call.
  - **`cbrt`:** `cb_run_main` (the program on a 256 MiB thread), program
    threads of 64 MiB, the per-thread floor from `pthread_getattr_np`
    (Linux), and `cb_stack_exhausted`.
  - **`cobc`:** `cb_stack_check()` at the top of every function and
    closure body; the generated `main` starts `cb_program` through
    `cb_run_main`.

## Compatibility classification

Fix: crashes become a defined fault.

## Conformance changes

**Added:** `conf.recursion-ten-thousand-deep`,
`conf.recursion-stack-exhausted`.

## Revisit conditions

None.
