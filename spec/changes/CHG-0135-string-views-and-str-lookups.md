# CHG-0135 — `String::find`, `split`, `trim`, …; `HashMap::get_str`, `contains_str`, `remove_str`, …

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0113)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0113
Affects: `spec/21` §0 and §2g (3.43.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0:** one row for the eight `String::` view functions and
  one for the six `_str` lookups; §2g gains `[Lookup-Str]`.
- **`std`:** the fourteen functions, written in CobaltC (`prelude.rs`),
  over `&s[0..$]` and a private `HashMap::find_str`.
- **Rows:** `conf.string-view-functions`, `conf.string-split`,
  `conf.map-lookup-by-str`, `conf.map-remove-str`,
  `conf.set-lookup-by-str`, `conf.map-get-str-non-string-key-rejected`.
- **Guide:** §21's `StringView` and hash-table tables and one example.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; new names in `std` are shadowed by a program's own (D-0024).
