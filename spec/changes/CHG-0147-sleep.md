# CHG-0147 — `sleep_ms`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0124)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0124
Affects: `spec/21` §0 (3.49.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`, `impl/src/fileio.rs`, `cbrt`, `cbrt.h`

## What changed

- **`spec/21`:** the row and `[Sleep]`.
- **`std`:** `sleep_ms` over the std-private `extern fn sleep_ns`; the interpreter releases its lock while sleeping; the runtime's `cb_sleep_ns`.
- **Rows:** `conf.sleep-at-least`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
