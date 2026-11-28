# CHG-0203 — `while (true)` without a `break` has the type `never`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0175)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0175
Affects: `spec/12` (1.22.0); `spec/14` (1.22.0); `spec/conformance.md` (3.167.0); the guide; `src/typecheck.rs`, `cobc/src/lower.rs`; `impl/std/` (fourteen dead values removed)

## What changed

- **`spec/12`:** `[T-While-Forever]`.
- **`spec/14` §4:** a `while (true)` no `break` leaves has no normal completion.
- **Rows:** `conf.while-true-never`, `conf.while-true-break-needs-value`, `conf.while-true-literal-only`.

## Compatibility classification

Additive.
