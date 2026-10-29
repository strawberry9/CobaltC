# CHG-0243 — The byte view over a range of cells; a reference's group is one address image

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (5))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206
Affects: `spec/06` 1.18.0, `spec/conformance.md` 3.200.0 (`conf.ref-byte-image-width`),
`impl/src/interp.rs` (`object_addr`, `ref_image`, `rawptr_of`), `spec/IMPLEMENTATION-NOTES.md` §2 (one row's
wording)

## What changed

- `spec/06` §7 "Byte view": `bytes-of-range(S, p, n)` by absolute position; a `ref(a)` cell with its
  `cont` cells inside the range contributes the `AddrWidth/8`-byte image of `min(target(a))` once, byte `j`
  at `p+i+j`; a `cont` outside its group and a `pad` contribute one unspecified byte; so a stored
  reference's byte view has length `sizeof(ref<τ,m>)` and equals `represent(rawptr<τ>, min(target(a)))`,
  and `value-at` over the group reads `a` back.
- `conf.ref-byte-image-width`: `copy_raw` of a `ref<i32, shared>`'s cells into a `u8` array, compared
  byte by byte with the bytes of `rawptr_of` on the same object: `sizeof<ref<i32, shared>>()` bytes, all
  equal.
- `coby`: a reference's byte image is its referent's address (it was zeros: the reference value is kept
  aside, and nothing was encoded); `rawptr_of(r)` on a reference binding reads the reference and answers
  the referent's address (it answered the binding's own cells); an object is given one stable address,
  which `rawptr_of` and every reference image to it share (`object_addr`).

## What did not change

`[Repr-Ref]`, `[Repr-Rawptr]`, `rule.trust.rawptr` and `[Copy-Raw]` are unchanged; the paragraph says what
they already meant. `cobc` needed no change.
