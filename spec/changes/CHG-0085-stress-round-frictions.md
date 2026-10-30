# CHG-0085 — Frictions from the stress-test round

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §19, §21
Depends on: D-0072
Affects: rule.agg.array-construct, `spec/22`, `spec/21` §0,
`spec/registry/diagnostics.md`

## Problem / motivation

D-0072:
- diagnostics that named nothing;
- no array repeat literal;
- no trailing comma in array literals;
- no `checked_neg`;
- a repair text that did not mention `replace`.

## Decision

D-0072.

## What changed

- **`spec/16` 1.14.0:** `[Array-Repeat]`, `[Array-Repeat-Not-Plain]`.
- **`spec/22` 2.26.0:** `[e; N]`, and a trailing comma in an array
  literal.
- **`spec/21` 3.30.0:** `checked_neg`.
- **`spec/registry/diagnostics.md` 1.32.0:** the repair of
  `diag.overwrite-of-live-resource`, and `[Array-Repeat-Not-Plain]`
  under `diag.type-mismatch`.
- **`spec/conformance.md` 3.68.0:** the cases below.
- **`spec/02-schema.md` 1.0.58:** §5's "in use" ranges.
- **Implementations:**
  - `ExprKind::ArrayRepeat`, in the parser, the checker, `coby`,
    `cobc`, and every walker;
  - `checked_neg` rewritten by `modres`;
  - messages on `unbound-name`, `name-not-visible` and `type-mismatch`;
  - the `T[N] x` syntax message;
  - `Display for Type`.
- **The guide:** arrays (§16) and checked arithmetic.

## Compatibility classification

Extension.

## Conformance changes

**Added:**
- `conf.checked-neg` and `conf.checked-neg-unsigned`;
- `conf.array-repeat`, `conf.array-repeat-inferred`,
  `conf.array-repeat-reference-rejected`,
  `conf.array-repeat-resource-rejected` and
  `conf.array-trailing-comma`;
- `conf.syntax-c-array-declaration`.

## Revisit conditions

See D-0072.
