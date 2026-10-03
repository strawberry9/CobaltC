# CHG-0120 — Math functions; ASCII helpers

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0101
Affects: spec/06 `rule.arith.float-fns` (1.12.0), spec/21 §0

## What changed

- **spec/06:** `[Float-Transcendental]`, `[Float-Transcendental-2]`,
  `[Float-Transcendental-Operand]`; the sentence that excluded them.
- **spec/21:** the functions in §0's table.
- **Implementations:** intrinsics `ln` … `powf` in the checker, `coby`
  (Rust's `f64`/`f32` methods, the platform's library) and `cobc` (C's
  `<math.h>`); the ASCII functions in `std`'s own code.
- **Rows:** `conf.float-transcendental`, `conf.float-transcendental-mixed-rejected`,
  `conf.ascii-helpers`, `conf.string-to-ascii-case`.

## Compatibility classification

Additive.
