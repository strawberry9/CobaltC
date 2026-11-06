# D-0048 — `swap`, `replace`, `Vec::swap`, `slice_swap`

Status: ACCEPTED (2026-09-26, owner-approved: "if you are confident these are useful, go ahead")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0033, D-0047, rule.stdlib.vec, rule.agg.slice
Affects: rule.stdlib.swap (new), rule.stdlib.prelude

## Problem

Locals can be exchanged with a temporary (a moved-from variable takes
a new value, D-0033), but a value reached through a reference cannot
be moved out (`diag.move-out-of-field`). So nothing could exchange two
fields of a borrowed struct, replace a value held by reference while
keeping the old one, or reorder a slice of resources: a sort over
`slice<String, exclusive>` was not expressible.

## Selected design

- `swap<T>(ref<T, exclusive> a, ref<T, exclusive> b)`.
- `replace<T>(ref<T, exclusive> r, T v) : T`.
- `Vec::swap(&mut v, i, j)` and `slice_swap(s, i, j)`: bounds-checked,
  nothing when `i == j` (where `swap(&mut v[i], &mut v[j])` would be
  two exclusive borrows of one place).
- In `std`, over one std-only intrinsic, `swap_places`, that exchanges
  the two places' contents. Two different elements of a `Vec` or a
  slice are disjoint places, so both `&mut v[i]` and `&mut v[j]` can be
  held at once (in `cobc`, an element's borrow now carries its index).

## Rejected alternatives

- **Moving out of a reference, leaving it empty until refilled:** a
  place in an undefined state is what the language rules out.
- **`swap` only:** exchanging two elements of one collection needs the
  `i == j` case handled; Rust has `mem::swap` and `slice::swap` for the
  same reason.

## Semantic rationale

`[Swap-Places]` exchanges values between two places; ownership stays
with places, so each resource is destroyed exactly once, by whichever
place holds it last.

## Usability

    swap(&mut p.left, &mut p.right);
    String old = replace(&mut name, String::from_str("new"));
    slice_swap(s, j - 1, j);

## Explainability

"`swap` exchanges two values you hold by `&mut`."

## Implementation-feasibility

`coby` reads and writes the two places; `cobc` exchanges their bytes
(and a reference slot's token, for a value holding references).

## Compatibility impact

Extension: new names in `std`; a program's own `swap` takes
precedence (D-0024).

## Prior-art status

Rust `mem::swap`, `mem::replace`, `slice::swap`; C++ `std::swap`,
`std::exchange`.

## Invariant traceability

`inv.resource-authority`: no value is duplicated or lost.

## Revisit conditions

None.
