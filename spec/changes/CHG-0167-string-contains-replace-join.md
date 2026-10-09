# CHG-0167 — `String::contains`, `replace` and `join`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0139)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0139
Affects: `spec/21` (4.5.0) §0, §2h; `spec/conformance.md` (3.129.0); the guide §21; `impl/std/text.cb`

## What changed

- **`spec/21` §2h `rule.stdlib.stringview`:** `[Contains]`, `[Replace]`, `[Join]`; `StringView::contains`, `StringView::replace`, `StringView::join` and `String::join` in the listing; the prose names the overlap and empty-`from` rules.
- **`spec/21` §0:** the `StringView` and `String::` rows of the `std::text` table list the new functions.
- **`std` (`impl/std/text.cb`):** `StringView::contains`, `StringView::replace`, `StringView::join`, `String::contains`, `String::replace`, `String::join`, written in CobaltC; both tools run them as written.
- **Rows:** `conf.string-contains`, `conf.string-replace`, `conf.string-join`.

## Compatibility classification

Additive: six new functions of `std`'s `String` and `StringView`.
