# CHG-0151 — `String::chars` and `StringView::chars`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-01; D-0128)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0128
Affects: `spec/21` (3.50.0), `spec/conformance.md` (3.119.0), the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0:** the `StringView` row lists `chars`; the D-0113 row lists `String::chars`.
- **`spec/21` §2h `rule.stdlib.stringview`:** a paragraph on `chars` (one view per character, in order; empty text gives none; the views borrow as views do; one allocation, and why there is no allocation-free `foreach` form, pointing to D-0128); the listing gains `StringView::chars` and `String::chars`.
- **`std` (`impl/src/prelude.rs`):** `StringView::chars`, `String::chars`, written in CobaltC; both tools run them as written.
- **Rows:** `conf.string-chars`, `conf.stringview-chars`, `conf.string-chars-empty`, `conf.string-chars-borrows`.

## Compatibility classification

Additive: two new names in `std`, shadowed by a program's own.
