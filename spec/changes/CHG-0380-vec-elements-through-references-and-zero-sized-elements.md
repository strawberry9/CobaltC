# CHG-0380 — An element of a `Vec` reached through a reference, and a `Vec` of a zero-sized type, in the static pass

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0217, D-0282, D-0284
Affects: `spec/14-control-flow.md` 1.34.0 (`rule.control.frame-references`, element accesses), `spec/conformance.md`
3.277.0; `impl/src/typecheck.rs`

## What changed

Two repairs to `rule.control.frame-references` as implemented. Each let a program be decided, and so run with no
record of references (D-0284), while it held two exclusive references to one place; `coby` and the tracked build
faulted on both:

1. **An element through a reference.** An access to `v[k]` where `v` is a reference to a `Vec` (a parameter
   `ref<Vec<T>, exclusive> v`, auto-dereferenced), or `(*r)[k]`, was not placed: a borrow of it still inherited
   the facts of `*v`, but the next access was checked against nothing. Two `&mut v[0]` through a reference
   parameter were both accepted, and the decided build printed a view of the `String` that writing through the
   second had destroyed. The access is now to `*v` at element `k` (to `*r` and its fields for `r.f[k]`), as an
   access to a local `Vec` is to the local.
2. **A `Vec` of a zero-sized type.** Every element of a `Vec<Z>`, `sizeof(Z) = 0`, is the one object anchored at
   its buffer, one place (D-0217, `conf.zero-sized-vec-elements-one-place`). The static pass compared their
   literal indices and took `v[0]` and `v[1]` to be two places, so `&mut v[0]` and `&mut v[1]` held together
   were decided and ran with no check (`stress/review2_oct11/z07.cb`, called a false fault in the tracked
   runtimes until now: they were right). The elements of such a `Vec` now all have one index; where the
   element type is a type parameter the index is not known, and the run decides. The diagnostic says "every
   element of a `Vec` of a zero-sized type is one place".

The elements of an `array<Z, N>` stay N places (D-0217): only a `Vec`'s elements are reclaimed objects.

## What changed for programs

A program that held two exclusive references to one element of a `Vec` through a reference, or to two elements
of a `Vec` of a zero-sized type, is now rejected before it runs; `coby` faulted on each at run time. No correct
program changes outcome.

## Conformance

`conf.vec-element-through-ref-conflict-rejected` (static), `conf.zero-sized-vec-elements-frame-refs-rejected`
(static), `conf.zero-sized-array-elements-frame-refs` (positive).
