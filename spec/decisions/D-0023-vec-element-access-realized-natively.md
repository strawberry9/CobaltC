# D-0023 — `Vec` Element Access Keeps Its Model; Implementations Realize It Natively

Status: ACCEPTED (2026-09-23, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0017, D-0018, D-0022, rule.trust.rawptr, rule.stdlib.prelude, `spec/21-standard-library-semantics.md` §0–§1, conf.e2e-vec-realloc-stale-ref, conf.vec-ref-then-push-rejected
Affects: D-0022 (its remark that a cheaper model of `Vec` element identity was declined, corrected below; its decision is unchanged), `impl/COBC-PLAN.md` §10

## Problem

A compiled program spends almost all of its time in `Vec` element
access. On a sieve of 10,000,000 `bool`s plus a Collatz search, `cobc`
took 210 s and 4.6 GB where the same program in Rust took 0.34 s. The cost
was in the prelude: every `Vec::index_shared`/`Vec::index_exclusive`
call ran its body step by step (a frame, the parameter's binding, two
checked reads through `*v`, `[Reclaim]`, `[Borrow]`, the scopes that
retire them), and every element ever reclaimed kept its object until
`[Release]` — also `Vec::drop`'s own `drop(reclaim(…))` of each element,
one object per element just before the buffer is freed.

D-0022 recorded "a cheaper model of element identity for `Vec`" as
declined, on the ground that the reclaimed element objects are what
make a reference held across a reallocating `push` stale. This record
decides how an element reference relates to its `Vec`, and so whether
that remark still stands.

## Constraints

- `conf.e2e-vec-realloc-stale-ref`: a shared element reference held
  across a reallocating `push` is `diag.stale-binding` at its next use,
  dynamically.
- `conf.vec-ref-then-push-rejected`: the same shape inside one function
  is rejected statically.
- `spec/21` §0: the library bodies "are normative for their observable
  behavior; an implementation may realize them differently provided
  every conformance case in `spec/conformance.md` holds."
- The owner's preference for no new tokens and no new surface.

## Candidate mechanisms

A. **Keep the model; realize it natively.** An element reference stays
   a root path on the element's own reclaimed object (`[Reclaim]`), so
   it goes stale only when the buffer is released; holding one across a
   `push` that does not reallocate stays well-defined. Implementations
   use `spec/21` §0's latitude for speed. **Selected.**
B. **Element references as derived paths of the `Vec` object.** Any
   write through the `Vec` while an element reference is held would be
   `diag.aliasing-conflict`, reallocating or not. Stricter and closer to
   Rust; it changes `conf.e2e-vec-realloc-stale-ref`'s expected
   diagnostic and needs `CHG`s to `spec/20`, `spec/21` and
   `spec/conformance.md`.
C. **By-value `Vec::get`/`Vec::set` for plain `T`** beside `index_*`.
   Two new library names; programs gain only when rewritten to use
   them; each needs a trust argument against `CHG-0019`'s requirement
   that no reclaimed object be live under a raw access.

## Selected design

Candidate A. No rule changes. Two facts make it sufficient:

1. **The bodies need not run as written.** `spec/21` §0 already
   licenses a different realization with the same outcomes: the same
   faults at the same locations, in the same order, and the same
   output.
2. **An element object nobody can observe need not persist.** For an
   element type that is not a resource, holds no reference and is not a
   `fn` value, a reclaimed object with no path but its root carries
   nothing a later `[Reclaim]` would not re-establish identically:
   `init = valid`, no authority, no obligation, no held references.
   Ending it early and establishing a fresh one on the next `[Reclaim]`
   of the same cells is not observable. `[Release]` still ends every
   object that is live, so a reference held across a reallocation is
   still stale.

What `cobc`/`cbrt` do under this record, for the prelude's own
declarations only (a program may declare a function of the same name,
and that one is compiled as written):

- `Vec::index_shared`/`Vec::index_exclusive`, every `T`: the checked
  read of `v.len`, the bounds fault, and one runtime call performing
  `[Reclaim]`, `[Borrow]` and the return. The body's second read of `v`
  (`v.ptr`) goes through the same path with no state change in between,
  so it cannot fail where the first passed, and it is skipped.
- For a plain `T`, the element object that call establishes ends when
  its last derived path ends, unless a `[Reclaim]` written in the program has
  since re-attached it and holds its root.
- `Vec::push` for a plain, non-zero-sized `T`: the checked read of
  `v.len`, the prelude's `grow` when full, the element's
  `[Rawptr-Write]`, and the checked write of `v.len`.
- `Vec::drop` for a plain, non-zero-sized `T`: only the elements that
  already have a reclaimed object are reclaimed and read, in index
  order. Every other element would get a fresh object with no path, and
  its read cannot fail. Then the buffer is deallocated as the body does.

## Rejected alternatives

- **B** is a language change, and speed does not require it. It would
  also turn today's well-defined "hold an element reference across a
  non-reallocating `push`" into a fault. It remains open on its own
  language merits: whether the language should report a write through
  the `Vec` while an element reference is held, whether or not the
  write reallocates.
- **C** adds surface and helps only programs rewritten to use it; A
  helps every program unchanged.

## Measurements

Release builds, `cobc` before → after this record (the `cc` step is `-O1`):

| Program | Before | After |
|---|---|---|
| Sieve of 10,000,000 + Collatz below 1,000,000 | 210 s, 4.6 GB | 33 s, 18 MB |
| Sieve of 1,000,000 | 16.2 s, 581 MB | 3.1 s, 3 MB |
| 1,000,000 `Vec::push` of `bool` | 4.7 s, 617 MB | 0.39 s, 3 MB |
| `impl/cobaltc_examples/12_collatz.cb` | 0.15 s | 0.07 s |

The equivalent Rust sieve-plus-Collatz program runs in 0.34 s, or
0.66 s with overflow checks on. What remains is not specific to `Vec`:
each statement's scopes and each borrow's path record, which D-0018's
use-time checks need.

## Semantic rationale

The element model is unchanged, so every existing program means what
it meant. Staleness still comes from `[Release]` → `[Object-End]`, not
from aliasing with the `Vec`.

## Usability

No program changes. The diagnostics, their locations (a fault inside
the prelude still reports no location), and their order are the same
in `coby` and `cobc`.

## Explainability

Unchanged.

## Implementation-feasibility

Implemented in `impl/cobc/src/lower.rs` (`native_vec_index`,
`native_vec_push`, `native_vec_drop`) and `impl/cbrt/src/lib.rs`
(`cb_elem_borrow`, `cb_vec_drop_plain`, ephemeral element objects in
`retire_token`). Verified against `coby`: every spec row, the file
suite, the examples, 35 showcases and 90 guide examples, plus targeted
programs for stale-after-realloc, stale-after-pop, conflicting element
references, out-of-bounds, a `Vec` of plain structs, a `Vec<Vec<i32>>`
(resource elements keep the prelude body), and a program declaring its
own `Vec::index_shared`.

## Compatibility impact

None.

## Prior-art status

Not applicable (an implementation latitude already in `spec/21` §0).

## Invariant traceability

`inv.temporal-validity`, `inv.alias-validity`: unchanged; every check
that can fail is still performed.

## Revisit conditions

- Candidate B brought forward on its language merits.
- A conformance case whose outcome depends on the identity, rather
  than the state, of a plain reclaimed element object.

## Observed in passing

A program may declare `fn Vec::index_shared<T>(…)` (or any prelude
`Type::name`), and both `coby` and `cobc` accept it and call the
program's function instead of the prelude's. Whether that should be
`diag.duplicate-item`, or allowed shadowing, is not decided here.
