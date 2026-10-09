# CHG-0056 — `swap`, `replace`, `Vec::swap`, `slice_swap`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0048
Affects: rule.stdlib.swap (new), the cases listed below

## Problem / motivation

D-0048.

## What changed

- **`spec/21` 3.17.0:** §3b `rule.stdlib.swap`.
- **`spec/conformance.md` 3.43.0:** the cases below.
- **`spec/02-schema.md` 1.0.32:** §5's "in use" ranges.
- **Implementations:** `std` (the four functions); the std-only
  intrinsic `swap_places` in typecheck, `coby` and `cobc`; `cobc`
  element borrows of a `Vec` or a slice carry the element's index, so
  two elements are disjoint (as in `coby`).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.swap-locals`, `conf.swap-fields-through-ref`,
`conf.replace-returns-old`, `conf.vec-swap`, `conf.slice-swap-sort-strings`,
`conf.swap-disjoint-elements`, `conf.swap-places-std-only`.

## Prior-art status

See D-0048.

## Revisit conditions

None.
