# CHG-0156 — A local holding references ends after its last use

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-02; D-0133)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0133
Affects: `spec/14` (1.21.0), `spec/conformance.md`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/14` §1 `[Ref-Binding-Last-Use]`:** the binding's type may be any type that holds a reference and no resource and no `fn` value, not only `ref`, `slice` and `StringView`.
- **Implementations:** the checker, `coby` and `cobc` end such a binding after its last use as they end a reference binding.
- **Rows:** `conf.ref-holding-binding-ends-at-last-use`.

## Compatibility classification

Additive.
