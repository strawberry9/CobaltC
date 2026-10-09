# CHG-0217 — `Vec` kept unchecked in more places

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-08; D-0189)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0189
Affects: `spec/conformance.md` (3.182.0); `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`; the guide

## What changed

- **`cobc`:** `CONFINING_CALLS` and their unchecked forms (`nc_variant`,
  `lower_confining_call`); a native `contains`; field dual bodies
  (`field_dual_eligible`, `field_confined_uses`, `lower_fn_field_dual`);
  `foreach (x in v)` over an exclusive `Vec` reference lowered as
  `$each_at_mut`; temporary search keys passed by address; native
  `dedup`; `retain`/`position` with a capture-free closure literal
  (`lower_pred_literal`); `sort_by`'s literal through a parameter.
- **`cbrt`:** `cb_write` returns at once for no path (0), as `cb_read` does.
- **`spec/conformance.md`:** `conf.vec-struct-field-loops`,
  `conf.vec-struct-field-held-element`, `conf.vec-local-calls`,
  `conf.vec-search-keys`, `conf.vec-sort-by-through-parameter`,
  `conf.vec-predicate-literals`, `conf.vec-retain-held-element`,
  `conf.vec-dedup-held-element`, `conf.vec-position-held-element`.

## What did not change

No rule, diagnostic or observable behavior. `coby` is unchanged.
