# CHG-0219 — Predicates that are functions and `fn` values

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-08; D-0191)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0191
Affects: `spec/conformance.md` (3.184.0); `impl/cobc/src/lower.rs`, `impl/cbrt/`

## What changed

- **`cbrt`:** `cb_fn_item_pure`, `cb_fn_pure`, the set of pure fn items.
- **`cobc`:** `lower_pred_literal` with `pred_ok`/`pred_source` (closure
  literals, named functions, `fn` values tested at the call),
  `binary_search_by`; fn items with pure direct reference parameters made
  with `cb_fn_item_pure`.
- **`spec/conformance.md`:** `conf.vec-predicate-values`,
  `conf.vec-predicate-value-keeps-element`,
  `conf.vec-retain-fn-value-held-element`,
  `conf.vec-binary-search-by-held-element`.

## What did not change

No rule, diagnostic or observable behavior. `coby` is unchanged.
