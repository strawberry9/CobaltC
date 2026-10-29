# D-0129 — Room in advance: `Vec::reserve`, `String::reserve`

Status: ACCEPTED (2026-10-01, the owner: "put reserve into std"; meaning and scope chosen by the owner: room for `n` more, like Rust; on `Vec` and `String`, used by `std`'s own builders)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0041 (`std` written in CobaltC), D-0085 (`Vec::from_slice`), D-0113 (functions straight from a `String`)
Affects: `spec/21` §1 `rule.stdlib.vec` and §2 `rule.stdlib.string` (3.51.0), `spec/conformance.md` (3.120.0), the guide §21, `impl/src/prelude.rs`

## Problem

A `Vec` grows by doubling: `push` on a full `Vec` calls `Vec::grow`,
which allocates twice the capacity (4 the first time), copies the
elements and frees the old cells. A program that knows how many
elements are coming cannot say so, so filling a `Vec` of `n` elements
always allocates and copies about log₂ `n` times. `std` is in the same
position: `String::from_str`, `String::clone` and `Vec::from_slice` know
their final length before the first byte or element and still grow step
by step. Making a short `String` cost two allocations, two copies and
two frees where one allocation would do.

## Candidate mechanisms

1. **Leave it.** Doubling is amortized; the cost is a constant factor.
2. **`Vec::reserve(&mut v, n)` and `String::reserve(&mut s, n)`**: room
   for `n` more elements (bytes) without growing again — afterwards the
   capacity is at least the length plus `n`. Selected.
3. **`reserve(&mut v, n)` meaning a total capacity of `n`** (C++'s
   `std::vector::reserve`). At a call site that already holds elements
   the programmer adds the length by hand. Not adopted (the owner chose
   candidate 2's meaning).
4. **`Vec::with_capacity(n)` / `String::with_capacity(n)`** as well:
   `new` and `reserve` in one call. One more name each for what two
   calls say; not adopted now (*Revisit*).

## Decision

    export fn Vec::reserve<T>(ref<Vec<T>, exclusive> v, usize n)
    export fn String::reserve(ref<String, exclusive> s, usize n)

- **Meaning.** Afterwards `cap >= len + n`. When that already holds,
  nothing happens: the `Vec` is not touched beyond reading its length
  and capacity, and its cells stay where they are. Otherwise the cells
  move once, to new storage of capacity `max(len + n, 2 × cap)` —
  doubling, as `push` grows, so that a loop of small reserves stays
  amortized rather than growing by one each time.
- **Never shrinks; never changes the length or the elements.** The
  elements are moved as `Vec::grow` moves them (`copy_raw`, then the old
  cells deallocated); a reference into the old cells is stale afterwards,
  exactly as after a `push` that grows.
- **Faults.** `len + n` overflowing `usize` is `diag.arith-overflow`
  (the addition is checked, as all arithmetic is), before anything is
  allocated.
  Allocation failing: `fault(alloc_failure)`, as for `push`.
- **`String::reserve(&mut s, n)`** is `Vec::reserve(&mut s.bytes, n)`:
  room for `n` more bytes. A `String`'s bytes are private to `std`, so
  without it a program could not reserve for text at all.
- **`std` uses it.** `String::from_str`, `String::clone` (through
  `append_string`) and `Vec::from_slice` reserve their final length
  before filling; their results are unchanged, and each now allocates
  once.
- **Capacity stays unobservable.** No function reports a capacity, so
  `reserve` is a promise about speed, not about anything a program can
  test, beyond where an allocation failure can happen. That is why the
  growth rule above is stated but programs are not meant to depend on
  it.

## Compatibility

Additive: two new names in `std`, shadowed by a program's own. The
builders' results are unchanged.

## Revisit

`with_capacity` constructors, if programs show `new` followed by
`reserve` often enough to be one intent of their own (the admission
test, Master Instructions §9). A `shrink_to_fit`, if a program ever
needs to give memory back.
