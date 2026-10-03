# CHG-0080 — A slice borrows its range, not its whole source

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §17, §19, §21
Depends on: D-0070
Affects: rule.agg.slice, rule.agg.index-vec, rule.control.flow-analysis

## Problem / motivation

D-0070: two exclusive slices of disjoint ranges of one source always
conflicted (no `split_at`).

## Decision

D-0070: a slice's borrow targets its range; `v[k]` is checked at
element `k`.

## What changed

- **`spec/16` 1.13.0:** `[Slice-Form]`, the prose after `[Slice-Index]`,
  `[Index-Vec]`.
- **`spec/14` 1.10.0:** `deriv`'s paths and their overlap.
- **`spec/conformance.md` 3.63.0:** the cases below.
- **`spec/02-schema.md` 1.0.53:** §5's "in use" ranges.
- **Implementations:** `coby`: `Proj::Range`, `Value::Ref::range`,
  `overlap`; a slice formed and checked over its range; a `Vec` element
  checked at its index. The checker: `PElem::Range`, literal ranges
  compared, others left to the dynamic check. `cbrt`: `Proj::Range`,
  `normalize`/`ranged_overlap` in `check_access`, `cb_borrow_range`;
  `cobc` forms slices with it. The guide (§16).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.slice-range-disjoint`, `conf.slice-range-threads`,
`conf.slice-range-overlap`, `conf.slice-range-literal-conflict`,
`conf.slice-range-source-whole`.

## Revisit conditions

See D-0070.
