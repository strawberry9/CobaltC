# D-0123 — `count_ones`, `leading_zeros`, `trailing_zeros`, `rotate_left`, `rotate_right`

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 3)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §5, §9
Depends on: rule.arith.bitwise, D-0090 (the `integer` bound)
Affects: `spec/06` §3 (1.15.0), `spec/21` §0, `spec/conformance.md`, the guide §06, both tools, `cbrt.h`

## Problem

Two corpus programs wrote `leading_zeros` by hand (a big-integer
normaliser, a sudoku bitmask), `std`'s own generator needed a rotate,
and every bit-set or allocator wants a population count. Each is one
machine instruction and one intent, and a loop in CobaltC hides both.

## Candidate mechanisms

1. **Loops in programs.** Slow where speed matters most, and each a place to be off by one.
2. **Five intrinsics on the integer types**, on the value's cell image
   (so a signed operand behaves as its unsigned image), counts as
   `u32`, rotations by `k mod width`, none faulting. Selected: `cobc`
   gets the builtin, `coby` the loop, the program the name.

## Selected design

`[Count-Ones]`, `[Leading-Zeros]`, `[Trailing-Zeros]`, `[Rotate]` in
`spec/06`'s bitwise rule; `leading_zeros(0)` and `trailing_zeros(0)` are
the width; a rotation's amount is a `u32` taken modulo the width.
Usable on a `T: integer` in generic code.

## Compatibility impact

Additive; five intrinsic names.
