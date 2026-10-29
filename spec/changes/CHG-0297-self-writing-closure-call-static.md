# CHG-0297 — A self-writing closure's call in the flow analysis

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0245)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0245, D-0239
Affects: `spec/14` 1.28.0 (§6 "A closure call"), `spec/15` 1.16.0 (§6); `impl/src/typecheck.rs`
(`closure_self_writes`, `flow_clash_saying`) — both tools; one conformance case's expectation

## What changed

The static pass (shared by coby and cobc) checks a call of a binding initialized by a `move` closure literal
whose body writes a capture as an exclusive access to that binding.

## What changed for programs

`auto s = &c; c();` for a self-writing `c` is rejected before the program runs
(`diag.aliasing-conflict`, static, "… so it cannot be called here: the call changes what it captured");
it faulted at the call before. A program where the borrow is not live on every path is checked at run time,
as before.
