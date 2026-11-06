# CHG-0227 — String performance: native text searches, fused substrings, `unwrap`, a cheaper runtime

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0199)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0199
Affects: `spec/conformance.md` 3.192.0; `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`

## What changed

- **`cobc`:**
  - New native bodies: `native_text_search` (`find`, `contains`, `starts_with`, `ends_with` of `String` and
    `StringView`), `native_string_view`, `native_unwrap` (`Option`/`Result` `unwrap` and `expect`).
  - The literal-view argument (`literal_view_arg`).
  - `lower_substring`, with the helpers `cbn_string_substring` and `cbn_append_substring`.
  - Object hand-offs: `bind_value` adoption, `obj_temps`, `store_part`.
  - Every `cb_elem_access` passes the element's size.
- **`cbrt`:**
  - `borrow_range_in` allocation-free.
  - `rekey_slots`, `cb_copy_datum` and the datum flight use the reusable buffers (`Inflight::Datum` is a pooled
    `Vec<u64>`, 0 = no reference).
  - New: `cb_absorb_refs`, `cb_reclaimed_pages` and `cb_find_bytes`/`cb_char_boundary` (`cbrt.h`).
  - `cb_elem_access(addr, size, …)`.
- **Conformance:** `conf.text-search-edges`, `conf.text-append-view-of-self`, `conf.text-append-empty-view-of-self`,
  `conf.text-substring-not-char-boundary`, `conf.option-unwrap-ref-none`.

## What did not change

No rule, diagnostic or observable behavior; `coby` is unchanged.
