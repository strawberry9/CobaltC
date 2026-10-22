# CHG-0121 — A byte literal takes an unsigned type from its context

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0102
Affects: spec/06 `rule.arith.literal` (1.13.0), spec/22 `byte-char-literal`

## What changed

- **spec/06, spec/22:** the byte literal's type.
- **Implementations:** the lexer marks `b'x'` as a byte literal; the
  shared literal typing (`IntTy::of_literal`) gives it the expected
  unsigned type or `u8`, and the checker, `coby` and `cobc` treat it as a
  bare literal when an operator's other operand fixes the type.
- **Rows:** `conf.byte-literal-u32-context`, `conf.byte-literal-default-u8`,
  `conf.byte-literal-signed-context-rejected`.

## Compatibility classification

Additive.
