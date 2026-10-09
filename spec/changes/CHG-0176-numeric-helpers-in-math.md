# CHG-0176 — `min`, `max`, `abs`, `pow` move to `std::math`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0148)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0148
Affects: `spec/21` (4.13.0) §0; the guide §21 and its intro table; `impl/std/core.cb`, `impl/std/math.cb`

## What changed

- **`spec/21` §0:** the submodule table's `std::core` row loses `min`, `max`, `abs`, `pow` and the `std::math` row gains them; their signature row moves from the "numbers, bytes, checksums, random numbers" table to the `std::math` table, now headed "numbers".
- **`std`:** the four functions move, unchanged, from `impl/std/core.cb` to `impl/std/math.cb`.
- **Rows:** none; no case names the submodule path of these functions.

## Compatibility classification

Additive through `std` (`import std;`, `std::min`, unqualified names unchanged); breaking (source) only for the full path `std::core::min` etc., which nothing checked in writes.
