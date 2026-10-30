# CHG-0305 — coby: dereferencing a reference reads the reference

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16; found by D-0246's enumeration, round 5)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `spec/05` `[Read]`, `[Ref-Deref-Place]`; `rule.fn.closure` `[Closure-Form-Borrow]` and spec/15 §6's capture mode
Affects: `impl/src/interp/refs.rs`; one conformance case; one model sample program

## What changed

`*r` reads the reference `r` before it reaches the place `r` holds. coby read `r`'s value with no conflict
check, so an exclusive borrow of `r` itself went unseen. The static pass rejects the plain form (`&mut r`
then `*r`). But a closure that writes through a captured reference captures it by an exclusive borrow, and
D-0243 marks that reference escaped, so the check is left to run time. coby now checks the read
(`clash(·, shared)` on `r`'s place, with the access's ancestors excluded), as `[Read]` does and as cobc
always did.

## What changed for programs

In coby, `*r` while a live closure holds `r` by an exclusive capture faults with `diag.aliasing-conflict`
(`conf.closure-ref-capture-blocks-deref`); it read through `r` and went on.
