# CHG-0079 — Function-local types

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0069
Affects: rule.module.local-type (new), `spec/22` `statement`

## Problem / motivation

D-0069: a type used by one function had to be a module item.

## Decision

D-0069: local `struct` and `enum`, with their functions in the same
block.

## What changed

- **`spec/17` 2.8.0:** §1c (new) `rule.module.local-type`.
- **`spec/22` 2.23.0:** `statement`.
- **`spec/conformance.md` 3.62.0:** the cases below.
- **`spec/02-schema.md` 1.0.52:** §5's "in use" ranges.
- **Implementations:** the parser (`parse_local_type`, `lookup_type`;
  renaming in types, paths, struct literals, patterns and
  destructuring; `parse_fn` for a function of a local type). The guide
  (§16).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.local-type`, `conf.local-type-type-parameter`,
`conf.local-type-out-of-scope`, `conf.local-type-fn-no-variables`; and
the file cases `local_fn_rejected.cb` and
`local_fn_foreign_type_rejected.cb` (syntax errors, which
`spec/conformance.md`'s rows do not express).

## Revisit conditions

See D-0069.
