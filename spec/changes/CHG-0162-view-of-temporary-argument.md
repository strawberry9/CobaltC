# CHG-0162 — A `String` a call returns, viewed as an argument

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0135)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0135
Affects: `spec/21` §2h; `spec/09` §2; `spec/conformance.md`; the guide; `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`[View-Form-Temporary-Argument]`** (`spec/21` §2h): as an argument of a call whose result is not a reference, `&s[lo .. hi]` with `s` a `String` that is a call's result or part of a temporary forms a view; the `String` lives to the end of the statement.
- **`spec/09` `[Ref-Form-Temporary-Part-Argument]`** names the view beside the slice.
- **Diagnostic text:** `diag.borrow-of-non-place` for `&f()[lo .. hi]` outside an argument says where it is allowed.
- **Rows:** `conf.view-of-temporary-argument`, `conf.view-of-temporary-outside-argument-rejected`, `conf.view-of-temporary-kept-faults`.

## Compatibility classification

Additive: programs that were rejected now run.
