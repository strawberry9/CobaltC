# CHG-0330 — `inv.resource-authority`: a finished thread's result is an owner case

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0265
Affects: `spec/03-invariants.md` (`inv.resource-authority`), `spec/23-preservation.md` §6

## What changed

`inv.resource-authority`'s proposition listed four ways an obligation is owned: the holder's frame is
open, the object is a temporary of an open statement scope, it is a sub-resource of a live object, or it
is reclaimed. Between `[Thread-Body-Done]` and the `[Join]` or `[Handle-Destructor]` that claims the
result, a resource in a finished thread's result is in none of them: its parameter frame has exited
(`block'(f', r_b)`, which keeps `objs-in(r_b)` and drops their bindings), and the thread's scope stack is
empty. The obligation is still tracked — the worker's authority token is unconsumed, and the handle,
itself a resource under obligation, will claim or discard the result — so the proposition now has a
fifth case: `r ∈ objs-in(Σ.threads(ℓ').value)` for a thread `ℓ'` whose value is set and not `taken`,
with `ℓ'`'s handle in `Σ.destruction-obligations`. `spec/23` §6 states the case in (R) and adds the
`[Thread-Body-Done]` row.

## What changed for programs

Nothing: no rule changes, and both implementations already keep the result's objects alive until the
join or the handle's drop (`conf.conc.join-moves-result`, `conf.conc.handle-drop-discards-result`).

## Found by

D-0265's threaded fragment: with (R) as the four cases, no state after `threadDone` satisfied it, so
the step could not be added; the fifth case is what the Lean model needed, and it is what the rules
already do.
