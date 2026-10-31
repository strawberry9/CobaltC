# CHG-0166 — `HashSet::union`, `intersection` and `difference`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0138)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0138
Affects: `spec/21` (4.4.0) §0, §2g; `spec/conformance.md` (3.128.0); the guide §21; `impl/std/collections.cb`

## What changed

- **`spec/21` §2g `rule.stdlib.hashmap`:** `[Set-Ops]`; the three functions in the listing; the order paragraph names them.
- **`spec/21` §0:** a row in the `std::collections` table.
- **`std` (`impl/std/collections.cb`):** `HashSet::union`, `HashSet::intersection`, `HashSet::difference`, written in CobaltC; both tools run them as written.
- **Rows:** `conf.hashset-union`, `conf.hashset-intersection`, `conf.hashset-difference`.

## Compatibility classification

Additive: three new functions of `std`'s `HashSet`.
