# CHG-0124 — A module's exported constant as an array length

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0105
Affects: spec/22 `length`, spec/17 §1a (2.10.0)

## What changed

- **spec/22, spec/17:** a qualified constant name as a length.
- **Implementations:** the shared parser's pre-scan records each
  module's literal-initialized constants under their qualified names and
  whether they are exported; `array_length` resolves `a::b::N` from the
  current module outward.
- **Rows:** `conf.array-length-qualified-constant`,
  `conf.array-length-qualified-private-rejected`.

## Compatibility classification

Additive.
