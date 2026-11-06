# D-0091 — Numeric functions and `HashMap::clear`

Status: ACCEPTED (2026-09-28, the owner's decision on the fourth stress round's findings)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0090, D-0041, rule.arith.represent
Affects: `spec/06`, `spec/21` §0, the implementations

## Problem

Real programs wrote their own `min`, `max`, `abs` and integer powers
(a `let_max` helper, `if (d < 0) { -d } else { d }` at every use),
reached `sqrt` only through `extern` and `unsafe`, and emptied a
`HashMap` by replacing it.

## Candidate mechanisms

1. **Leave them to programs.** Every program repeats them.
2. **Intrinsics for all of them.** Needed before D-0090, since a
   generic `T` had no operators.
3. **`min`, `max`, `abs`, `pow` in `std`, written in CobaltC over
   D-0090's bounds; the floating-point functions as intrinsics** (they
   are operations of the machine, as the conversions are); `HashMap::clear`
   in `std`. Selected.

## Selected design

- `min<T: ordered>(T a, T b) : T`, `max<T: ordered>`: `a` when the two
  are equal.
- `abs<T: number>(T x) : T`: checked (`abs` of a signed type's minimum is
  `diag.arith-overflow`); `abs(-0.0)` is `0.0`; a NaN stays a NaN.
- `pow<T: number>(T base, u32 exp) : T`: by squaring; checked, and
  overflowing only when the result does; `pow(x, 0)` is `1`.
- `sqrt`, `floor`, `ceil`, `round` (halfway cases away from zero),
  `trunc` of an `f32` or `f64`, of that type (`rule.arith.float-fns`):
  each exact or correctly rounded, so every implementation gives the
  same bits.
- Transcendental functions are not provided: their last bit differs
  between platforms' libraries. `extern` reaches the platform's.
- `HashMap::clear(m)`: every entry removed (keys and values destroyed),
  the table back to its first size.

A program's own `min`, `max`, `abs`, `pow` or `sqrt` is the one its
unqualified name finds (D-0024, D-0055).

## Compatibility impact

Additive.

## Revisit conditions

None.
