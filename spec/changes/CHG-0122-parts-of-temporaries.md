# CHG-0122 — Parts of temporaries

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0103
Affects: spec/09 `rule.ref.form`, spec/16 §1

## What changed

- **spec/09:** `[Ref-Form-Temporary-Part-Argument]`; `[Ref-Form-Temporary]`
  excepts it.
- **spec/16:** `[Temp-Field-Move]`; the paragraph on projections of
  temporaries.
- **Implementations:** the checker records approved borrows and slices
  (`temp_borrows`) and field moves (`temp_moves`, the destructuring both
  evaluators run instead); `coby` slices a temporary as an object of the
  statement; `cobc` gives a borrowed temporary root a runtime object.
- **Rows:** `conf.temp-field-borrow-argument`, `conf.temp-array-slice-argument`,
  `conf.temp-field-slice-argument`, `conf.move-out-of-temporary-field`
  (was `-rejected`), `conf.move-out-of-temporary-field-destructor-rejected`,
  `conf.temp-field-move-ends-rest`.

## Compatibility classification

Additive.
