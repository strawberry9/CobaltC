# D-0051 — Converting between f32 and f64, a float's bits, and a float's limits

Status: ACCEPTED (2026-09-26, owner: "fix the language gap where there is no conversion between f32 and f64, and the to_float issue, and anything else related to these")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0006, D-0026, D-0027, rule.arith.convert, rule.arith.limits, [Arith-Float]
Affects: rule.arith.convert (`[Widen-Float]`, `[Float-To-Float]`, `[Reinterpret-Float]`), rule.arith.limits

## Problem

Integer conversions were complete (`widen`, `narrow`, `narrow_wrapping`,
`reinterpret`, `to_float`, `to_int`), but:

1. no conversion took one float type to the other: an `f32` could not
   become an `f64`, nor an `f64` an `f32`;
2. `to_float` accepted only an integer operand;
3. a float's bits were reachable only through `unsafe` raw pointers
   (the tier-4 showcase `float_anatomy` does exactly that), and an
   integer's bits could not become a float at all without them;
4. `min_value`/`max_value` rejected the float types
   (`[Limits-Not-Integer]`).

## Selected design

No new names: the existing conversions take floats where their meaning
carries over.

- **`widen<f64>(x : f32)`** — exact: every `f32` value (NaN and the
  infinities included) is an `f64` value. `widen` from a float type to
  itself is the value. `widen<f32>` of an `f64` is rejected: not every
  `f64` is an `f32` (`[T-Convert]`).
- **`to_float<τ'>(x)` for a float `x`** — the IEEE-754 conversion, as
  `[Arith-Float]` rounds: the nearest `τ'` value, ties to even; beyond
  `τ'`'s finite range, an infinity of `x`'s sign; a NaN stays a NaN.
  So `to_float<f32>(x : f64)` is the rounding conversion, and
  `to_float<f64>(x : f32)` equals `widen<f64>(x)`.
- **`reinterpret<τ'>(x)` between a float and an integer of its width**
  (`f32` ⇄ `u32`/`i32`, `f64` ⇄ `u64`/`i64`) — the same bits: a float's
  IEEE-754 image as an integer, and any integer's bits as a float.
- **`min_value<f32>()`, `max_value<f64>()`, …** — a float's largest
  finite value and its negation. `[Limits-Not-Integer]` becomes
  `[Limits-Not-Number]`.

## Rejected alternatives

- **`narrow<f32>(x : f64)`** (exact or `diag.narrowing-overflow`): an
  `f64` is almost never exactly an `f32`, so it would fault on nearly
  every value; `narrow` keeps its integer meaning.
- **Fault when `to_float<f32>` overflows**: float arithmetic, and
  `to_float` from an integer (`to_float<f32>` of a large `u128`),
  already give an infinity; one rule for all float results is simpler
  than a special case here. Only a *literal* out of range stays
  rejected (`[Literal-Out-Of-Range]`), since that is a typo.
- **New names** (`to_f64`, `bits`, `from_bits`, `f64::MAX`): the owner
  prefers the fewest new tokens, and each existing name already says
  what it does here.
- **Also `min_positive`, `epsilon`, infinity and NaN constants**: not
  conversions; `1.0 / 0.0` and `0.0 / 0.0` already give them, and the
  others can be added when a program needs them.

## Semantic rationale

`[Widen]`'s meaning is "every value is preserved", which holds for
`f32` into `f64`; `to_float`'s is "the nearest value of the target",
which holds for any number source; `reinterpret`'s is "the same cell
image", which holds for a float and an integer of one width.

## Usability

    f64 d = widen<f64>(sample);          // f32 -> f64, exact
    f32 s = to_float<f32>(d * 0.5);      // f64 -> f32, rounded
    u64 bits = reinterpret<u64>(d);      // no unsafe
    f64 top = max_value<f64>();

## Explainability

"`widen` when nothing can be lost, `to_float` when it rounds,
`reinterpret` for the bits."

## Implementation-feasibility

`coby`: Rust's `as` between floats (IEEE-754, round to nearest even,
overflow to infinity) and `to_bits`/`from_bits`. `cobc`: C's
conversions between `float` and `double` (Annex F, which gcc and clang
implement on these targets) and a union compound literal for the bits;
the limits as hex float literals, exact.

## Compatibility impact

Extension; one row changes (`conf.limits-not-integer` now uses `bool`,
since `max_value<f64>()` is valid).

## Prior-art status

Rust: `f64::from(f32)`, `as f32`, `f64::to_bits`/`from_bits`,
`f64::MAX`/`MIN`. C: implicit float conversions, `memcpy` for bits,
`FLT_MAX`/`DBL_MAX`.

## Invariant traceability

`inv.arith.range-validity`: every result is a value of its type.

## Revisit conditions

If programs need the other float constants (`min_positive`, `epsilon`),
add them as limits.
