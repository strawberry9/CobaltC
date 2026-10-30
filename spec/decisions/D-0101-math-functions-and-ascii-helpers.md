# D-0101 — Logarithms, exponentials, trigonometry; ASCII helpers

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 frictions 17 and 23)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0091 (`rule.arith.float-fns`)
Affects: spec/06 `rule.arith.float-fns`, spec/21 §0

## Problem

std had `sqrt`, `floor`, `ceil`, `round`, `trunc` and an integer-exponent
`pow`, and nothing else numeric: any simulation (exponential variates),
statistics (logarithms), geometry (angles) or audio program had to write
its own `ln` from the IEEE bits (`queuesim.cb` did) or declare `extern`
C functions (compiled only). D-0091 had left them out because their
last bit differs between platforms' libraries. And no program could ask
whether a byte is a digit or a letter, or change a letter's case: every
text program wrote `c >= b'0' && c <= b'9'` and friends by hand.

## Candidate mechanisms

Math: **the platform's C library, specified to within 1 ulp**
(selected); correctly rounded implementations carried in the runtime
(one result everywhere, a large body of code to own); or none (D-0091).
ASCII: **small exported `std` functions** (selected); Unicode case
mapping (tables, and locale questions) — a larger decision, not taken.

## Selected design

- `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan` of an `f32` or `f64`,
  and `atan2(y, x)`, `powf(x, y)` of two of one type: C's `log`, `exp`,
  … (`logf`, … at `f32`), within 1 ulp. `coby` calls the same library
  through Rust's methods, so the two implementations agree bit for bit on
  one platform; a result may differ in the last place between platforms.
  `powf` is named apart from `pow`, whose exponent is a `u32`.
- `ascii_is_digit`, `ascii_is_alpha`, `ascii_is_alnum`, `ascii_is_upper`,
  `ascii_is_lower`, `ascii_is_space`, `ascii_to_lower`, `ascii_to_upper`
  on a `u8`; `String::to_ascii_lower` and `String::to_ascii_upper`
  return a changed copy. Only ASCII is touched, so UTF-8 stays valid.

## Compatibility impact

Additive. A program's own function of one of these names takes
precedence over the intrinsic or `std` item (D-0055, D-0024).

## Revisit conditions

If bit-identical results across platforms are needed, carry correctly
rounded implementations in the runtime.
