# CHG-0097 — A closure's result type may be written

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner decision)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0081
Affects: rule.fn.closure, `[T-Closure]`, `spec/22` §2

## Problem / motivation

D-0081: a closure's result type could not be written.

## Decision

D-0081.

## What changed

- **`spec/15` 1.8.0, `spec/12` 1.14.0, `spec/22` 2.29.0:** the grammar and `[T-Closure]`.
- **Implementations:**
  - the parser, modres (the type is resolved like a parameter's) and the checker;
  - `cobc` lowers the body at the written type, with no probing pass;
  - `coby` takes the type from the checker, as for every closure (D-0073).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.closure-result-type-written`, `conf.closure-result-type-conflict`.

## Revisit conditions

None.
