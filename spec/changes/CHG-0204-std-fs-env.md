# CHG-0204 — `std::fs` and `std::env`; `std::sys` retired

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-05; D-0176)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0176
Affects: `spec/21` (4.40.0) §0, §2c, §2e, §2e′; `spec/conformance.md` (3.168.0); `CobaltC_Master_Instructions.md` §9; the guide; `impl/std/`, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0:** the submodule list and table (`std::fs`, `std::env`; `std::io` the standard streams; `std::time` with the clocks; `std::http` added to the list); the rule that each item lives with the items it works with.
- **`spec/21` §2c, §2e, §2e′:** each section names its new submodule.
- **Rows:** `conf.std-subjects-fs-env`, `conf.std-io-has-no-files`.
- **Master Instructions §9:** the placement rule.

## Compatibility classification

Additive for programs that import `std`; a program importing `std::sys`, or `std::io` alone for files, names the new submodule.
