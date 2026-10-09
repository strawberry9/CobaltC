# CHG-0190 — Text literals through a type parameter

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0162)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0162
Affects: `spec/21` (4.27.0) §2a; `spec/12` (1.21.0); `spec/conformance.md` (3.152.0); the guide §21; `impl/src/typecheck.rs`, `impl/src/views.rs`, `impl/src/lib.rs`

## What changed

- **`spec/21` §2a `[Str-Literal-String]`:** a parameter that a type parameter fixed as `String` is a position.
- **`spec/12` `rule.type.expected`:** the same, beside D-0079's numeric literals.
- **Both tools:** the checker defers and records such literals; the front end rewrites them to `String::from_str(L)` and checks again.
- **Rows:** `conf.str-literal-generic-string`.

## Compatibility classification

Additive.
