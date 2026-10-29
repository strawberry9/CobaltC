# D-0238 — A path over a vector's elements: `range` and `index` steps, and `meet`

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); a clarification (`spec/STABILITY.md` §2): what the two implementations
do, stated as rules
Governed by: `CobaltC_Master_Instructions.md` §8 (formal precision), §13, §20, §21
Depends on: D-0070 (slices borrow ranges), D-0217 (`steps`), D-0221, D-0222, CHG-0262
Affects: `spec/04` §1–§2 (`state.access-paths`, `overlap`, `meet`, `steps`), `spec/16` §3a (`[Slice-Form]`,
`[Slice-Index]`, prose), `rule.agg.index-vec` (`[Index-Vec]`, `[Index-Vec-Out-Of-Bounds]`,
`[Index-Vec-Conflict]`), `spec/08` §4 (H4), `spec/OVERVIEW.md` §3; `spec/conformance.md` (CHG-0287)

## Problem

The fourth review (F-03, F-06) found that a slice of a `Vec` had no defined access path. `[Slice-Form]` said
"a borrow of r", r the elements; but a `Vec`'s elements are objects of their own over raw cells, the vector's
extent is its three fields, and `spec/04`'s `overlap` compared targets. So `clash` in `[Slice-Form]`'s own
premise was undefined, and four things rested on prose: CHG-0262's "a `Vec`'s own fields meet every live
slice" (the soundness repair after which `cbrt` and `coby` agreed), `[Standing-Local]`'s H4, D-0222's
`_ = &mut (*v)[i .. i + 1]` in `Vec::index_exclusive`, and `[Index-Vec]`'s "checked as an access to e's
element k". F-06: `[Index-Vec]` defined `e[k]` as `≡` a call whose body reads `v.len`, which by the prose
meets every slice, yet `conf.vec-index-beside-slice` expects `(*r)[1]` to pass beside `&mut (*r)[0 .. 1]` and
`conf.vec-index-exclusive-passed-ref-beside-slice` expects the explicit call to fault; the text could not tell
them apart.

## Options

(i) the record the runtimes already use: a slice of a vector, and `[Index-Vec]`'s element borrow, are paths of
the vector's own object with a last step `range(i, j)` or `index(k)`, and `overlap` compares such steps by a
defined `meet`; (ii) give each slice one path per element object; (iii) leave the prose and add a note. (ii)
has no answer for elements without objects and makes a slice's cost proportional to its length; (iii) is the
§20 failure the review describes.

## Decision

(i). `spec/04`: a path over a vector's elements has the vector's place as `target` and a last *storage step*
`range(i, j)` or `index(k)`; `overlap` of two paths of one object where either holds a storage step is
`meet`: step by step over the shorter list (a prefix meets), equal steps agree, `range`/`index` agree when
they intersect (an empty range agrees with no range or index), and a storage step agrees with the vector's
own field steps (`ptr`, `len`, `cap`). `[Slice-Form]` gives its path record in full. `[Index-Vec]` becomes a
rule of its own: the element borrow `steps(a0) ++ [index(n)]`, checked once, with its bounds read part of that
check; it yields the place `index_exclusive`/`index_shared` would return, but is not a call of either.
`[Slice-Index]` says which object an element access through a slice is of. H4 is restated by the step.

## Rationale

No program changes outcome: probes on 2026-10-15 (an empty slice beside a push through a passed-on reference;
the existing slice cases) confirmed both tools behave as the rules now say, including that an empty slice
meets the vector's fields though it meets no element. `coby`'s `overlap` on projection lists is this `meet`
(`value.rs`), and `cbrt`'s `ranged_overlap_with` realizes it over addresses. The conflicts CHG-0262, D-0222
and `[Index-Vec]` assert become consequences of `clash`.

## Results

- `spec/04`, `spec/16`, `spec/08` H4, `spec/OVERVIEW.md` §3.
- Case: `conf.empty-slice-meets-vec-fields`. The cases CHG-0262 and D-0222 added now follow from the
  definitions: `conf.vec-index-beside-slice`, `conf.vec-index-exclusive-passed-ref-beside-slice`,
  `conf.vec-index-exclusive-while-container-shared`.
