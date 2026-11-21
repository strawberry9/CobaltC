# CHG-0164 — `std` divided into submodules; the floating-point functions are `std::math`'s

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0136)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0136, CHG-0163
Affects: `spec/21` (4.2.0) §0, every section head, §3d (new); `spec/06` (1.16.0); `spec/conformance.md`; the guide; `impl/std/*.cb` (was `impl/src/prelude.rs`'s source), `impl/src/modres.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`std`'s submodules** (`spec/21` §0): `std::core`, `std::collections`, `std::text`, `std::io`, `std::sys`, `std::memory`, `std::sync`, `std::random`, `std::math`, each re-exported by `std`'s root (CHG-0163). Every `std::` path and `import std;` are unchanged; `std::collections::Vec` is `std::Vec`; `import std::text;` imports one subject. Each section of `spec/21` names its submodule.
- **One privacy:** a private item or field of any of `std`'s submodules is visible in all of `std`.
- **`rule.arith.float-fns`** moved from `spec/06` to `spec/21` §3d, id kept: `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`, `atan2`, `powf` are exported functions of `std::math`, realized natively and typed as before; they are no longer intrinsics.
- **Previous semantics:** `std` was one module; the floating-point functions were intrinsics, in scope without an import, and a program's root item of one of their names was keyed apart from them (D-0055).
- **Rows:** `conf.std-submodule-path`, `conf.std-import-one-subject`, `conf.std-import-one-subject-only`, `conf.std-message-short-path`, `conf.float-fn-needs-import`, `conf.float-fn-name-own-item`.

## Compatibility classification

Breaking (source) for a program that calls a floating-point function without `import std;`; additive otherwise.
