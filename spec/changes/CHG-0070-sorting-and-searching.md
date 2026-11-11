# CHG-0070 — Sorting and searching

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0062
Affects: rule.stdlib.sort (new), rule.stdlib.prelude

## Problem / motivation

D-0062: no sort in `std`.

## Decision

D-0062: `Vec::sort`, `Vec::sort_by`, `Vec::binary_search`,
`Vec::binary_search_by`; stable.

## What changed

- **`spec/21` 3.24.0:** §1b (new) `rule.stdlib.sort` (`[Sort]`,
  `[Sort-By]`, `[Binary-Search]`, `[Sort-Not-Key]`, the `std` source);
  §0's table.
- **`spec/conformance.md` 3.54.0:** the cases below.
- **`spec/02-schema.md` 1.0.44:** §5's "in use" ranges.
- **Implementations:** `std` gains the four functions, `key_before`,
  `Vec::sorted_order`, `Vec::apply_order`; the intrinsic `key_less`
  (std-only) in the checker, `coby` and `cobc` (`cb_bytes_less`); the
  checker checks `Vec::sort`'s and `Vec::binary_search`'s `T` as
  `HashMap`'s `K`; `cobc` realizes `Vec::sort` of integers and `bool`s
  natively (`native_vec_sort`, `cb_sort_plain`). The guide (§21).

## Affected entities

`rule.stdlib.sort` (new); `rule.stdlib.prelude`.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.sort-search`, `conf.sort-not-key-type`,
`conf.sort-by-comparator-type`, `conf.sort-element-borrowed`.

## Prior-art status

See D-0062.

## Revisit conditions

See D-0062.
