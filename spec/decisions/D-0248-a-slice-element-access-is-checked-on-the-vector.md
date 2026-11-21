# D-0248 — A slice's element access is checked on the vector

Status: ACCEPTED (2026-10-15; delegated by the owner)
Kind: Decision (`spec/02-schema.md`); repair of `[Slice-Index]` (a soundness hole in the text and coby)
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0070, D-0213 (`[Thread-Lend]`), D-0238, D-0242
Affects: `spec/16` §3a `[Slice-Index]`; `impl/src/interp/index.rs` (CHG-0303)

## Problem

D-0238 wrote `[Slice-Index]` for a `Vec` source as an access to the element's own object, with the slice's
temporal validity and `[Thread-Affine]` as premises. Nothing checked the slice's own standing on the vector,
where a reborrow of it lent to a thread lives. So

    auto s = &mut v[0 .. 1];
    auto t = spawn(wait_then_bump, &ch, s);   // the thread waits, holding a reborrow of s
    s[0] = 9;                                 // the spawner writes the element meanwhile

wrote element 0 from both threads with no fault: coby followed the text and printed `10`; cobc faulted, and
so did the model once it modelled slices passed to threads through closures (D-0246, round four). For a
reference the same write is caught, because the lent path lives on the object written.

## Decision

`[Slice-Index]` of a `Vec` source forms the element borrow `a_n` on the vector as `[Index-Vec]` forms `a_k`:
its last step `index(start + n)`, the access's mode, `¬clash(a_n, ·)` with the slice's borrow among its
ancestors, pending until the access (D-0242). An array source needs nothing new: its element place is a
projection on the array, checked at the access.

## Results

`conf.slice-written-while-lent-reborrow` (dynamic, both tools); coby faults where it wrote. Disjoint slices
(`conf.vec-index-beside-slice`, the model's `hand-l01`) run as before.
