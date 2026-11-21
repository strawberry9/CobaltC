# CHG-0301 — coby: a `move` closure's copy of a capture is a checked read

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; found by D-0246's enumeration)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.fn.closure` `[Closure-Form-Move]` (each capture stored as `store` does), `spec/05` `[Read]`
Affects: `impl/src/interp/closures.rs`; one conformance case

## What changed

Forming a `move` closure, coby read a capture it copies with no conflict check, and moved one that owns a
resource with no `solitary` check. It now checks the read (`clash(·, shared)`, `diag.aliasing-conflict`) and
the move (`diag.move-while-aliased`), as `[Read]` and `store` do and as cobc always did.

## What changed for programs

In coby, capturing `x` by `move` while another path holds an exclusive reference to `x` faults
(`conf.move-capture-beside-exclusive-ref`); it copied `x` and went on.
