# D-0047 — Slices, `$`, and Indexing a `Vec`

Status: ACCEPTED (2026-09-26, owner-proposed: `&array[1..4]`, D's `$`, `a[0..$]`; details delegated)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0042, D-0046, rule.agg.index, rule.alias.borrow, rule.temporal.elision
Affects: type.slice (new), rule.agg.slice (new), rule.agg.index-vec (new), rule.agg.dollar (new), rule.temporal.elision, rule.control.foreach, spec/22 §1–§3

## Problem

A function taking a run of numbers was tied to one container: a
`ref<Vec<i32>, shared>` could not take an `array<i32, 5>`, and an
array parameter fixed its length. Part of an array or `Vec` could not
be passed without copying it. And a `Vec`'s element was reached only
by `*Vec::index_shared(&v, i)`.

## Constraints

- A slice is a borrow: while it lives, what it views cannot be pushed
  to, popped from or reallocated (no dangling view).
- No inference of borrow modes: the form is written (D-0042, D-0046).

## Candidate mechanisms

Spelling: **`&a[lo .. hi]` with D's `$` for the length** (selected,
the owner's), or functions (`Vec::slice(&v, 1, 4)`). Whole slice:
**`&a[0..$]`** (selected, the owner's) or D's `a[]`. A slice without `&`
(D's value slices) or **with** (selected: the mode is visible). Vec
indexing `v[i]`: **yes** (selected).

## Selected design

- Tokens `..` and `$`; type-name `slice`; type `slice<T, m>`.
- `&a[lo .. hi]` / `&mut a[lo .. hi]` of an array, a `Vec` or a slice
  (directly or through a reference): elements `lo` up to `hi`
  exclusive, bounds checked; the source borrowed in the slice's mode
  for its whole life; a slice of a slice views the same source.
- `$` inside `[…]` is the length of what those brackets index; it is a
  `usize`, so `a[$ - 1]` of an empty `a` is an overflow fault.
- `s[i]`, `v[i]` index slices and `Vec`s, bounds-checked; `v[i]` is
  `*Vec::index_exclusive(&mut v, i)` where it is written and
  `*Vec::index_shared(&v, i)` elsewhere. A resource element is borrowed
  in place, never moved out.
- `slice_len(s)`; `foreach` over slices in their own mode; a slice
  parameter counts as a reference for `rule.temporal.elision`, and a
  slice of a local does not escape.

## Rejected alternatives

- **D's value slices without `&`:** the mode (and that it is a borrow)
  would be invisible.
- **Python's negative indices (`a[-3:]`):** indices are unsigned;
  `$ - 3` is checked arithmetic instead.
- **A slice holding a reference to its first element only:** `pop`
  would still be allowed while it lives and could empty a slot it
  views; a slice borrows its whole source.

## Semantic rationale

A slice is a borrow of its source plus two numbers: every aliasing and
temporal rule applies to it as to `&source`. An element access is a
place reached through that borrow.

## Usability

    fn sum(slice<i32, shared> s) : i32 { … }
    sum(&arr[1..4]);  sum(&v[0..$]);  sum(&v[$ - 3 .. $]);
    sort(&mut a[0..$]);

## Explainability

"`&a[i..j]` borrows elements `i` to `j`; `$` is the length."

## Implementation-feasibility

Both tools represent a slice as the borrow of its source (a reference,
tracked as any other), a start, and a length; `cobc` keeps a pointer
to the first element beside it (stable, since the source cannot be
reallocated while borrowed). `$` is a stack of lengths during an
index's lowering.

## Compatibility impact

`slice` becomes a reserved type-name and `$`, `..` tokens. One program
in the repository used `slice` as a name (`showcase/tier3/parallel_sort.cb`'s
function copying a range of a `Vec`), now `copy_range`.

## Prior-art status

D (`a[i .. j]`, `$`), Rust (`&v[i..j]`, `&mut`), Go (`a[i:j]`), C++20
`std::span`.

## Invariant traceability

`inv.alias-validity`, `inv.temporal-validity`, `inv.spatial-validity`
(bounds checked at formation and at each index).

## Revisit conditions

- `split_at` (two disjoint exclusive halves): needs borrows that track
  ranges, not whole sources.
- Slices of a `String`, cut on character boundaries.
- Ranges in `foreach` (`foreach (i in 0 .. n)`), now that `..` exists.
