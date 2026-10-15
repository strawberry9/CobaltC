# CHG-0076 — Local constants, and fields and elements of constants

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0066
Affects: rule.module.const, `spec/22` `statement`

## Problem / motivation

D-0066: a function's constant values could not be checked by
`static_assert` where they are computed, and fields of constant structs
were not constants.

## Decision

D-0066: `const τ N = e;` as a statement, scoped like a local; a field
or element of a constant expression is one.

## What changed

- **`spec/17` 2.5.0:** §1a (local constants, `[Const-Local]`,
  `[Const-Local-Not-Constant]`; the list of constant expressions).
- **`spec/22` 2.21.0:** `statement`.
- **`spec/conformance.md` 3.59.0:** the cases below.
- **`spec/02-schema.md` 1.0.49:** §5's "in use" ranges.
- **Implementations:** the parser (scopes of the function being parsed;
  hoisting; renaming; `diag.const-not-constant` for a local variable or
  type parameter in the initializer); the checker's `const_expr`
  (fields, elements). The guide (§17).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.const-local`, `conf.const-local-names-variable`,
`conf.const-local-names-type-parameter`, `conf.const-local-out-of-scope`,
`conf.const-field-element`.

## Prior-art status

See D-0066.

## Revisit conditions

See D-0066.
