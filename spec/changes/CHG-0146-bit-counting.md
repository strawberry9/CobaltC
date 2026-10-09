# CHG-0146 — `count_ones`, `leading_zeros`, `trailing_zeros`, `rotate_left`, `rotate_right`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0123)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0123
Affects: `spec/06` (1.15.0), `spec/21` §0, `spec/conformance.md`, the guide §06, both tools, `cbrt.h`

## What changed

- **`spec/06`:** `[Count-Ones]`, `[Leading-Zeros]`, `[Trailing-Zeros]`, `[Rotate]`.
- **Tools:** the intrinsics; `cbrt.h` gains inline helpers over the C builtins, with 128-bit forms.
- **Rows:** `conf.bit-counts`, `conf.bit-rotate`, `conf.bit-count-non-integer-rejected`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
