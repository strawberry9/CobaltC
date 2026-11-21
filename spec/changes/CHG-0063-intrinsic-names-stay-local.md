# CHG-0063 — An item of an intrinsic's name is not found through an enclosing module

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0055
Affects: rule.module.resolve

## Problem / motivation

D-0055: a program's root-level item named as an intrinsic changed what
`std`'s code, and any nested or file-backed module's code, called.

## Decision

D-0055: `[Resolve-Unqualified]` clause (3) does not apply to an
intrinsic's name.

## What changed

- **`spec/17` 2.4.0:** clause (3)'s condition, and a paragraph after
  the rules.
- **`spec/21` 3.21.1:** the scope paragraph's note (non-normative).
- **`spec/conformance.md` 3.49.0:** the cases below.
- **`spec/02-schema.md` 1.0.39:** §5's "in use" ranges.
- **Implementations:** `modres.rs` (`is_intrinsic_name`; clause (3)
  skips such a name; `qualify` keys a root item of such a name
  `name$`, so it no longer shares its key with the intrinsic).

## Affected entities

`rule.module.resolve`.

## Previous semantics

Clause (3) found an item of an intrinsic's name from every module
nested in its own, `std` included.

## New semantics

It finds it only through clauses (1) and (2).

## Affected invariants

None.

## Compatibility classification

Source-breaking for a nested module calling a root-level item named
as an intrinsic (it now reaches the intrinsic); programs declaring such
an item and using `std`, which failed, now run.

## Migration implications

Declare the function in a module and `import` it.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.item-intrinsic-not-inherited`
- `conf.file-module-intrinsic-not-inherited` (with the fixture
  `fixtures/sizes.cb`)
- `conf.item-intrinsic-nested-rejected`

`conf.item-shadows-intrinsic` is unchanged.

## Future implementation implications

None.

## Prior-art status

See D-0055.

## Revisit conditions

See D-0055.
