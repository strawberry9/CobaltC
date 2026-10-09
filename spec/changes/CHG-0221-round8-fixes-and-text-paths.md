# CHG-0221 — Round-8 fixes, text and queue paths, `swap_remove`, `Response::text`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0193)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0193
Affects: `spec/21` (4.46.0); `spec/conformance.md` (3.186.0); `impl/src/interp.rs`, `impl/cobc/src/lower.rs`, `impl/cbrt/`, `impl/std/`

## What changed

- **`spec/21`:** §1 `Vec::swap_remove`; §2h `char_count`; §2p `Response::text` (tables, listings, history).
- **`std`:** the new functions; `StringView::parse` parses the view's bytes in place (`parse_bytes`, shared with
  `String::parse`) instead of copying them into a `String` first.
- **`coby`:** `closure_cid_types` and `closure_ftys`; resource-owning closures detected through captured `fn`
  values and moved as handles on `move` capture.
- **`cbrt`:** deep `cb_fn_copy`; recursive `fn_owns_resource`; `lock_write_spinning`; `cb_rotl8` … `cb_rotr64`
  in `cbrt.h`.
- **`cobc`:** `closure_base`/`closure_id`; `native_view_split_whitespace`, `native_view_trim`, `native_ascii_case`,
  `native_drain_take`, `native_pq`, native `chars`, `native_parse_int`; `bind_reader_slice`; rotations at the operand's width.
- **`spec/conformance.md`:** `conf.closure-names-across-functions`, `conf.closure-captures-closure`,
  `conf.closure-captured-owner-destructors`, `conf.closure-combinator-loop`, `conf.text-split-trim-case`,
  `conf.vec-swap-remove`, `conf.priority-queue-order`, `conf.text-chars-count`, `conf.text-parse-int-edges`.

## What did not change

No rule or diagnostic. The closure fixes make both tools do what spec/15 and D-0089 already required.
