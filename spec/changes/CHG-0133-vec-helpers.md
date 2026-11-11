# CHG-0133 — `Vec::filled`, `from_fn`, `append`, `extend_from`, `position`, `index_of`, `contains`, `reverse`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0112)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0112
Affects: `spec/21` §0 (3.42.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`, `impl/src/typecheck.rs`

## What changed

- **`spec/21` §0:** one row for the eight functions and what each does.
- **`std`:** the eight functions, written in CobaltC (`prelude.rs`).
- **Checker:** `Vec::filled` and `Vec::extend_from` with a resource
  element type are `diag.type-mismatch` at the call, the message naming
  `Vec::from_fn` and `Vec::append` respectively (as `Vec::clone` and
  `Vec::from_slice` are handled, D-0082, D-0085).
- **Rows:** `conf.vec-filled`, `conf.vec-filled-resource-rejected`,
  `conf.vec-from-fn`, `conf.vec-append`, `conf.vec-extend-from`,
  `conf.vec-extend-from-resource-rejected`, `conf.vec-position`,
  `conf.vec-index-of`, `conf.vec-contains`,
  `conf.vec-contains-not-eq-rejected`, `conf.vec-reverse`.
- **Guide:** §21's `Vec` table and its example; the two showcase loops
  the functions replace (`tinyos.cb`, `tinyos/fs.cb`) use `extend_from`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; new names in `std` are shadowed by a program's own (D-0024).
