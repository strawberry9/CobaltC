# D-0070 — A slice borrows its range, not its whole source

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0018, D-0022, D-0047
Affects: rule.agg.slice (`[Slice-Form]`, `[Slice-Index]`), `spec/04` `overlap`

## Problem

A slice borrowed its whole source for its life (D-0047), so two
exclusive slices of one array or `Vec` always conflicted, even over
disjoint ranges. The standard way to work on two parts of a sequence
at once — divide and conquer, an in-place merge, a partition, swapping
halves — needs exactly that: Rust's `split_at_mut`. D-0047 deferred it
("needs borrows that track ranges, not whole sources").

## Constraints

- No new tokens; no new function where the existing syntax can say it.
- Every access still checked; a slice never reaches an element outside
  its range.

## Candidate mechanisms

1. **A slice's borrow targets its elements `start … start + len`**; two
   slices conflict only where their ranges meet. `&mut v[0 .. m]` and
   `&mut v[m .. $]` live together. **Selected.**
2. A `split_at(s, m)` function returning both halves (it would need a
   pair type, which the language does not have, and still the range
   rule underneath).
3. Keep whole-source borrows.

## Selected design

Candidate 1.

- **Range.** `&m e[lo .. hi]` borrows the elements `start + lo` up to
  `start + hi` of its source (`start` the offset of `e` in its own
  source when `e` is a slice), and the clash check at its formation is
  over that range. An empty range overlaps nothing.
- **What overlaps a range.** The source itself (a borrow of the whole
  array or `Vec`, the `Vec` passed to `Vec::push`, a read of it whole),
  any slice or element borrow whose elements meet the range. An element
  reached through a slice is its element of the source (`[Slice-Index]`,
  already), so it is compared by its position there.
- **Unchanged.** Everything that reaches the whole source still
  conflicts with every live slice of it.

## Rejected alternatives

- **2:** a function to spell what two `&mut …[…]` already say.
- **3:** leaves divide-and-conquer on one buffer unwritable.

## Semantic rationale

`spec/04`'s `overlap` is already the intersection of targets; D-0047's
slice simply targeted more than it needed. A slice reaches only its
range (bounds are checked at every index), so targeting exactly that
range is sound.

## Usability

    fn sort_halves(slice<i32, exclusive> s)
    {
        usize mid = slice_len(s) / 2;
        slice<i32, exclusive> left = &mut s[0 .. mid];
        slice<i32, exclusive> right = &mut s[mid .. $];
        …                                  // both usable, each alone in its range
    }

## Implementation-feasibility

`coby`: a slice's reference carries a range step (`Proj::Range`) after
its source path, ignored by navigation and compared by `overlap`, where
a range meets an index inside it or a range that intersects it; an
element through a slice is compared at its absolute index. `cbrt`: a
range projection in the borrow the slice holds (`cb_borrow_range`), an
element through the slice normalised to its absolute index in
`check_access`.

## Compatibility impact

Extension: programs rejected for two slices over disjoint ranges now
run; none that ran changes.

## Prior-art status

- **Rust:** `split_at_mut`, `chunks_mut`; the borrow checker does not
  track ranges itself, so the halves come from an `unsafe` function.
- **Ada:** slices of an array are independent objects of its components.

## Revisit conditions

- Ranges of a `String`'s bytes (a `StringView`) under the same rule.
