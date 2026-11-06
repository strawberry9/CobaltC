# CHG-0255 — Anchors for zero-sized objects; `overlap` by steps for an empty target

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0217)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0217
Affects: `spec/04` 1.7.0, `spec/05` 1.7.0, `spec/06` 1.19.0, `spec/20` 1.16.0, `spec/IMPLEMENTATION-NOTES.md`,
`spec/conformance.md` 3.211.0; `impl/tests/conformance.rs` (covers line);
`impl/conformance/16-aggregates/zero_sized_addresses_ok.cb`, `zero_sized_vec_elements_one_place_rejected.cb`

## What changed

- `spec/04`: `ObjectRecord.anchor`; `overlap` for an empty target; `steps`, `addr-of`.
- `spec/05`: `[Object-Establish-*]` choose an anchor and require it aligned; a paragraph on zero-sized objects.
- `spec/06` §7: `[Repr-Zero-Sized]`.
- `spec/20`: `reclaimed-at` keys on the anchor; `[Rawptr-Of]` gives `addr-of`; `[Release]`'s `R` takes zero-sized
  objects by anchor.
- `spec/IMPLEMENTATION-NOTES.md` §2: the anchor's freedom and how each tool fixes it.
- `spec/conformance.md`: two rows; `conf.zero-sized-types`'s derivation.

## What did not change

No accepted program or outcome: both tools already behaved as the text now says.
