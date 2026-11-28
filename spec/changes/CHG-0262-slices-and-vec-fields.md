# CHG-0262 — A `Vec`'s own fields meet its live slices; `v[k]` and slices through a reference are checked by range

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; under D-0070)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0070, D-0221, D-0222
Affects: `spec/16`, `spec/conformance.md`; `cbrt` (`impl/cbrt/src/lib.rs`), `cobc` (`impl/cobc/src/lower.rs`),
the static pass (`impl/src/typecheck.rs`); four new cases under `impl/conformance/16-aggregates/`

## Why

D-0219's probe programs `sl2` and `sl3` (a slice of element 0 beside an access to element 1) ended at different
lines under `coby` and `cobc`. Reading them against the text found three defects and one gap in the text:

1. `cobc` checked `v[k]` as a borrow of the whole vector, where `[Index-Vec]` checks element `k`: beside a live
   slice of other elements `cobc` faulted and `coby` ran (`sl3`).
2. Both static passes recorded a slice taken through a reference binding `r` (`&mut r[0 .. 1]`) as a borrow of
   `r` itself, and read `*r` whole in `(*r)[k]` and `(*r).f` (the parentheses lost the place position): valid
   programs (`(*r)[1] = 7` beside a slice of element 0) were rejected statically.
3. `cbrt` let an access to a `Vec`'s own fields pass beside a live slice of it, where `coby` does not. A
   mutator given a reference passed on (`Vec::reserve(r, n)`, `pop`, `clear`, `remove`, `truncate`,
   `extend_from`) changed the vector under a slice taken through `r`, and the slice then read the freed
   buffer: a soundness defect, present in Update 103 (checked with its binaries) and likely since slices of
   ranges (D-0070, 2026-09-27).
4. The text said what reaches the whole source meets every slice (D-0070), but `spec/04`'s `overlap` compares
   targets, and a `Vec`'s fields and its elements do not share one. `coby` already treated the fields as
   reaching the whole storage; the text now says so.

## What changed

- `spec/16` `[Slice-Form]` prose: a `Vec`'s own fields (`ptr`, `len`, `cap`) reach its whole storage and meet
  every live slice of it; through a reference `r`, `&m r[lo .. hi]` and `&m (*r)[lo .. hi]` borrow their range
  of `*r`.
- `cbrt`: a field step and a range step meet (`ranged_overlap_with`), as in `coby`'s `overlap`.
- `cobc`: `e[k]` on a `Vec` (`vec_index_call`) is checked with element `k` appended to the vector's path, after
  the index is evaluated, for any index (a non-literal one is evaluated first, with the place's validity
  checked before it).
- Static pass: a parenthesized place keeps its place position; a slice of a reference binding derives from
  its referent `*r`.
- `spec/conformance.md`: `conf.vec-index-beside-slice`, `conf.vec-grow-through-ref-beside-slice`,
  `conf.vec-pop-through-ref-beside-slice`, `conf.vec-index-exclusive-passed-ref-beside-slice`.

## What changed for programs

A soundness repair (`spec/STABILITY.md` §2) and two extensions:

- Under `cobc`, a `Vec` mutator given a reference passed on while a slice taken through that reference lives
  faults `diag.aliasing-conflict`, as under `coby`; before, it could move the buffer under the slice.
- Under `cobc`, `v[k]` beside a live slice of other elements runs, as under `coby`.
- Under both tools, `(*r)[k]`, `r[k]` and a disjoint `&mut r[lo .. hi]` beside a slice taken through `r` are no
  longer rejected statically.

An explicit `Vec::index_exclusive(r, k)` with `r` passed on still faults beside such a slice (its body reads
`len` through its reference); `(*r)[k]` does not.
