# CHG-0294 — A path formed for one access is pending until it

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0242)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0242
Affects: `spec/16` 1.29.0 (§2 "One access", `[Index-Vec]`, `[Slice-Index]`), `spec/04` 1.9.0 (`clash`),
`spec/08` 1.7.0 (§1, §4); three conformance cases

## What changed

The paths a projection, `[Index-Vec]` and `[Slice-Index]` form for a place are pending until the one access
made through the place, which ends them. `[Index-Vec]`'s `a_k` is no longer a temporary of the statement.

## What changed for programs

None in either tool: `v[k] = v[k] + 1`, `h.r = h.r` and `Vec::push(&mut v, v[0])` ran before, and run. The
text, read literally, faulted on them.
