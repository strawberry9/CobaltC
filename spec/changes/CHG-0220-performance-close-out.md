# CHG-0220 — Performance close-out

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0192)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0192
Affects: `spec/conformance.md` (3.185.0); `impl/cobc/src/lower.rs`, `impl/cbrt/`

## What changed

- **`cbrt`:** reader-writer runtime lock (`enter_read`, `fault_lock`,
  `rt_ro`); locking off once only the main thread runs (`go_solo`);
  `cb_write`'s initialized-target path under the shared hold; `cb_allocate`
  without the lock; `cb_fn_mark_pure`; `cb_elem_access_slow` records the
  location it is given before forming the element's borrow (a native
  predicate loop's clash is reported at the call, as `coby` reports it).
- **`cobc`:** native `Vec` operations for movable elements (`movable_elem`,
  `native_vec_pop_res`, `native_vec_clear_res`, `search_ops`); `_once`
  variants and `lower_vec_checked_once`; pure search keys; pure capturing
  closures; `lower_from_fn_literal`; dual bodies' `__bare` form and
  `lower_dual_bare_call`; `used_as_value` ignores bound names.
- **`spec/conformance.md`:** `conf.vec-resource-elements`,
  `conf.vec-string-search`, `conf.vec-resource-predicates`,
  `conf.vec-string-clone`, `conf.vec-from-fn-literal`,
  `conf.vec-capturing-closure-values`, `conf.vec-search-key-places`,
  `conf.dual-body-bare-call`, `conf.runtime-lock-after-join`.

## What did not change

No rule, diagnostic or observable behavior. `coby` is unchanged.
