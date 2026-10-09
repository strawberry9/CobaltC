# CHG-0192 — `String == str` and `String == StringView`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0164)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0164
Affects: `spec/06` (1.17.0); `spec/21` (4.29.0) §0, §2h; `spec/conformance.md` (3.154.0); the guide; `impl/std/text.cb`, `impl/src/typecheck.rs`, `impl/src/views.rs`, `impl/src/lib.rs`

## What changed

- **`spec/21`:** `[Text-Mixed-Eq]`; `String::eq_view` in the listing; `spec/06`'s comparison prose; §0's tables.
- **Both tools:** the checker records each mixed comparison and the front end rewrites it to `String::eq_str`/`String::eq_view` before checking again.
- **Rows:** `conf.text-mixed-eq`, `conf.text-mixed-order-rejected`.

## Compatibility classification

Additive.
