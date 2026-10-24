# CHG-0178 — `exit(status)` in `std::process`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0150)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0150
Affects: `spec/15` (1.12.0) `rule.fn.program`; `spec/18` (1.5.2) §1 prose; `spec/21` (4.15.0) §0, §2k; `spec/conformance.md` (3.140.0); the guide §21; `impl/src/modres.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/lib.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/15` `rule.fn.program`:** `[Terminate-Exit]`: `exit(s)` unwinds the calling thread's frames (destructors run) and the program terminates `ok(s)`; other threads take no further steps; a destructor's fault during the unwind is the outcome.
- **`spec/18` §1:** `[Fault-Unwind]`'s prose names `[Terminate-Exit]` as the same unwind ending in `ok(s)`.
- **`spec/21` §2k, §0:** `[Exit]`; `exit(u8 status) : never` in the `std::process` tables, realized natively.
- **Both tools:** `exit` is a native item of `std::process`; `coby` unwinds through its fault path and reports `ok(s)`; `cobc` emits `cb_exit(s)`, the runtime's fault unwind without a report.
- **Rows:** `conf.exit-runs-destructors`, `conf.exit-from-thread`.

## Compatibility classification

Additive: a new exported function of `std::process`, shadowed by a program's own.
