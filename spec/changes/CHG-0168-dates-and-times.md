# CHG-0168 — `std::time`: `DateTime`, `Weekday`, `unix_ms`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0140)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0140
Affects: `spec/21` (4.6.0) §0, §2e′, §2j (new); `spec/registry/diagnostics.md` (1.44.0); `spec/conformance.md` (3.130.0); the guide §21; `impl/std/time.cb` (new), `impl/std/std.cb`, `impl/src/prelude.rs`, `impl/src/fileio.rs`

## What changed

- **`spec/21` §2j (new):** `rule.stdlib.time` with `[From-Unix]`, `[To-Unix]`, `[Invalid-DateTime]`, `[Weekday]`, `[To-Iso]`, `[From-Iso]`, `[Unix-Ms]` and the CobaltC source.
- **`spec/21` §0:** the submodule table and listing gain `std::time`; a `std::time` table.
- **`spec/21` §2e′ `rule.stdlib.env`:** `[Clocks]` lists `unix_ms`; `clock_read(2)`.
- **`spec/registry/diagnostics.md`:** `diag.invalid-datetime`.
- **`std` (`impl/std/time.cb`, new):** `DateTime`, `Weekday` and their functions, `unix_ms`, written in CobaltC; both tools run them as written. `clock_read(2)` in `impl/src/fileio.rs`, shared by both tools.
- **Rows:** `conf.datetime-from-unix`, `conf.datetime-to-unix-limits`, `conf.datetime-weekday`, `conf.datetime-iso`, `conf.datetime-new`, `conf.datetime-invalid`, `conf.datetime-sort`, `conf.unix-ms`.

## Compatibility classification

Additive: a new submodule `std::time` and its names, shadowed by a program's own.
