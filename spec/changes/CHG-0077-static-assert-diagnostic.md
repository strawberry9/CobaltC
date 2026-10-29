# CHG-0077 — A diagnostic of its own for a `static_assert` that cannot be computed

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0067
Affects: rule.module.static-assert

## Problem / motivation

D-0067: the diagnostics for a `static_assert` whose condition cannot be
computed before the program runs did not say so.

## Decision

D-0067: `diag.static-assert-not-constant`.

## What changed

- **`spec/17` 2.6.0:** `[Static-Assert-Not-Constant]` and the prose after
  it.
- **`spec/registry/diagnostics.md` 1.29.0:** the new entry;
  `diag.const-not-constant` covers local constants.
- **`spec/conformance.md` 3.60.0:** the cases below.
- **`spec/02-schema.md` 1.0.50:** §5's "in use" ranges.
- **Implementations:** the checker (`static_assert`'s condition typed and
  checked there).

## Compatibility classification

Clarification: the same programs are rejected; the diagnostic's id
changes.

## Conformance changes

**Changed:** `conf.static-assert-not-constant` (was
`diag.const-not-constant`) and `conf.static-assert-not-bool` (was
`diag.type-mismatch`) expect `diag.static-assert-not-constant`.

## Revisit conditions

None.
