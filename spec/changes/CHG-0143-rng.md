# CHG-0143 — `Rng`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0120)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0120
Affects: `spec/21` §0 and §2g (3.48.0), `spec/conformance.md` (3.116.0), the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21`:** one row; `[Rng]` with splitmix64, xoshiro256**, the
  rejection `below`, `unit_f64`, and check values from seeds 42 and 0.
- **`std`:** the struct and four functions, written in CobaltC.
- **Rows:** `conf.rng-check-values`, `conf.rng-below-range`,
  `conf.rng-below-zero-faults`, `conf.rng-unit-range`.
- **Guide:** §21's table row and a paragraph with an example.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a program's own `Rng` shadows `std`'s (D-0024).
