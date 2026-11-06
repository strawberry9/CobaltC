# CHG-0303 — `[Slice-Index]` checks the element on the vector

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0248)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0248
Affects: `spec/16` 1.29.0 (`[Slice-Index]`); `impl/src/interp/index.rs`; `impl/model` (the machine); one
conformance case

## What changed

The rule's note, coby's `eval_index` (a `Vec`-source slice checks the element's index on the vector with the
access's mode, the slice's token among the ancestors), and the model's machine.

## What changed for programs

In coby, writing or reading an element through a slice while a reborrow of it is lent to a running thread, or
the vector is borrowed whole, faults (`diag.aliasing-conflict`); it went on. cobc already faulted.
