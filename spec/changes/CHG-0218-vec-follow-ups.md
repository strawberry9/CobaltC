# CHG-0218 — The `Vec` items D-0189 left open

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-08; D-0190)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0190
Affects: `spec/conformance.md` (3.183.0); `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`

## What changed

- **`cbrt`:** `cb_paths_disjoint` admits a single path over a reclaimed object.
- **`cobc`:** the field dual body's struct path direct when nothing under it
  is borrowed and its other fields are plain (`field_confined_uses` reports
  it); a `foreach` holder confined before the loop's condition; capturing
  predicate literals in `lower_pred_literal` (`pending_pred_closure`).
- **`spec/conformance.md`:** `conf.vec-struct-in-vec-loops`,
  `conf.vec-struct-in-vec-slice-held`, `conf.vec-struct-in-vec-inner-held`,
  `conf.vec-foreach-over-parameter`, `conf.vec-capturing-predicates`,
  `conf.vec-capturing-predicate-held-element`.

## What did not change

No rule, diagnostic or observable behavior. `coby` is unchanged.
