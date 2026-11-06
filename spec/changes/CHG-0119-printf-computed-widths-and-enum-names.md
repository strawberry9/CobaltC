# CHG-0119 — printf computed widths; `%v` of an enum

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0100
Affects: spec/21 §2f (`rule.stdlib.format`)

## What changed

- **spec/21:** `spec` admits `*` for a width or precision; `[Format-Arg-Mismatch]`
  counts one `usize` argument per `*`; `%v` takes an enum.
- **Implementations:** the shared format module (`fmt.rs`, used by the
  checker, `coby` and `cbrt`) parses `*` and reads the width from the
  arguments; `coby` and `cobc` write an enum's variant name.
- **Rows:** `conf.format-star-width`, `conf.format-star-precision`,
  `conf.format-star-not-usize-rejected`, `conf.format-enum-name`.

## Compatibility classification

Additive.
