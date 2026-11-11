# D-0217 — Zero-sized objects have anchors; their places are told apart by steps

Status: ACCEPTED (2026-10-11; the owner: "action the plan in full autonomously and using the recommendations as written" — `ChatGPT_review/ACTION-PLAN-2026-10-11b.md` WI-6, Q4)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6 (identity, spatial validity), §20 (zero-sized regions)
Depends on: D-0018, D-0019, D-0208
Affects: `spec/04` 1.7.0, `spec/05` 1.7.0, `spec/06` 1.19.0, `spec/20` 1.16.0, `spec/IMPLEMENTATION-NOTES.md`,
`spec/conformance.md` 3.211.0 (CHG-0255); two cases in `impl/conformance/16-aggregates/`

## Problem

The second independent review (F-07): an object of a zero-sized type has an empty extent, and the rules used
`min(addrs)` for its alignment (`[Object-Establish-*]`) and `min(target(a))` for its address (`[Rawptr-Of]`),
both undefined on an empty set; `reclaimed-at` named "the unique" reclaimed object by extent and type, and all
empty extents of one type are equal; `overlap`, by intersection of targets, made two paths to a zero-sized
place never overlap, so two exclusive borrows of one never clashed — which neither tool did.

## Options

- Distinct anchors (the plan's recommendation): every object has an address of its own, a zero-sized one
  distinct among live objects; or unspecified whether two share one, which would leave `rawptr_of` equality
  unspecified and `reclaimed-at` needing another key anyway.
- For overlap: by an address footprint, or by the place's steps. An address footprint cannot give both
  answers the tools give — a struct and its zero-sized last field overlap, its other fields and that field do
  not — since the field's address is one past the others'; steps can.

## Decision

1. Every object has an `anchor` (`spec/04` `state.objects`): `min(extent)`, or for a zero-sized fresh object an
   address no other live object has as its anchor, aligned (`spec/05`); a reclaimed object's anchor is the
   address it was reclaimed at. `reclaimed-at` keys on it too (`spec/20`).
2. `addr-of(a)` (`spec/04`): `min(target)`, or for an empty target the anchor plus the offset of the place's
   steps; `[Rawptr-Of]` gives it. A zero-sized part of an object shares the address where it lies: the elements
   of an `array<Z, 3>` all have its address.
3. `overlap`: when either target is empty, the two paths overlap if they are of one object and one's steps are
   a prefix of the other's (`steps`, `spec/04`). Two elements of an `array<Z, N>` are two places; every slot of a
   `Vec<Z>` is the one object anchored at the buffer, one place.
4. `[Release]` takes a zero-sized object over the range by its anchor (`release(p, 0)` ends the one at `p`).
5. `[Repr-Zero-Sized]` (`spec/06`): the representation is empty and the value unique.

## Results

Both tools already behaved so; no tool changed. `stress/review2_oct11/z07.cb`: two `Z` locals have different
addresses, `p + 1 == p`, an array's zero-sized elements share its address, and `&mut v[0]`, `&mut v[1]` of a
`Vec<Z>` clash, under `coby` and `cobc` with GCC and Clang. `cobc` declares a zero-sized local as an empty C
struct; both compilers keep two such locals apart (observed, recorded in `spec/IMPLEMENTATION-NOTES.md`). Cases:
`conf.zero-sized-addresses`, `conf.zero-sized-vec-elements-one-place`; `conf.zero-sized-types` cites the anchor.
