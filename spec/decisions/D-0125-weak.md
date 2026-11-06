# D-0125 — `Weak<T>`

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 6)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §3, §9
Depends on: rule.stdlib.rc (D-0003's allowance, CHG-0100), D-0080 (`overwrite`), D-0116 (`Weak::clone`)
Affects: `spec/21` §0 and §3 (3.49.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## Problem

A parent pointer in a tree, a cache that must not keep its entries
alive, an observer list: each is a cycle through `Rc`, which leaks by
design, or an index into a `Vec` kept by hand.

## Candidate mechanisms

1. **Indices and `Option<usize>`**, as the corpus does. Works; the
   fact "this index is still valid" lives in the programmer's head.
2. **`Weak<T>`**: a handle that keeps the box, not the value.
   `Rc::downgrade` makes one, `Weak::upgrade` gives a new `Rc` while
   any strong handle exists, else `None`. Selected: the established
   shape, and `Rc` is already CobaltC.

## Selected design

`RcBox<T>` gains a `weak` count and holds its value as an `Option<T>`,
so the value ends (`overwrite` to `None`) when the last strong handle
drops while weak handles remain; the box is deallocated when both
counts are 0, by whichever drop is last. `Rc::get` reads through the
`Option` and can never see `None` through a live `Rc`. `Weak::clone`
counts a weak handle; `Weak::drop` uncounts it. Not thread-safe, as
`Rc` is not.

## Compatibility impact

Additive for programs; the `RcBox` layout changes (its size grows by
`usize` and the `Option`'s discriminant), which no program can observe
except through `sizeof<RcBox<T>>`, a `std`-private type.
