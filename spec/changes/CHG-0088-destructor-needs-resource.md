# CHG-0088 — A type with a destructor is a resource

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0075
Affects: `spec/07` §1, `spec/registry/diagnostics.md`

## Problem / motivation

D-0075: a destructor declared for a plain type never ran.

## Decision

D-0075.

## What changed

- **`spec/07`:** §1's rule.
- **`spec/registry/diagnostics.md` 1.33.0:**
  `diag.destructor-on-plain-type`.
- **`spec/conformance.md` 3.71.0:** the case below.
- **`spec/02-schema.md` 1.0.61:** §5's "in use" ranges.
- **Implementations:** the checker's destructor pass tests the type's
  resource-ness with plain type arguments.
- **The guide:** §07.

## Compatibility classification

Tightening.

## Conformance changes

**Added:** `conf.destructor-on-plain-type-rejected`.

## Revisit conditions

None.
