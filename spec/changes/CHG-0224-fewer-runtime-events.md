# CHG-0224 — Objectless `String`s by binding, builders, raw returns, native `find_str`, lazy statement scopes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0196)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0196
Affects: `spec/conformance.md` 3.189.0; `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`

## What changed

- **`cobc`:**
  - `objectless_strings` works by binding (`HashSet<usize>` of initializers), with `BUILDERS` and move-out;
    `cur_let`.
  - Exclusive borrows of objectless locals; the move-out in `read_place`.
  - `raw_ret_ok`, `__raw` forms (`pending_raw_ret`, `raw_ret`, `raw_call`).
  - `native_map_find_str`.
- **`cbrt`:**
  - Pending statement scopes (`TState::pend`, `ts`/`ts_raw`, `make_pending`).
  - `cb_borrow_unstamped` and `borrow_range_in` accept token 0.
- **Conformance:** `conf.objectless-strings-by-binding`, `conf.string-builders-raw-return`.

## What did not change

No rule, diagnostic or observable behavior; `coby` is unchanged.
