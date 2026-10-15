# D-0107 — An argument's borrow is pending until the call binds it

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 proposal P1, option A′)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0007 (left-to-right evaluation), D-0018, D-0022 (conflicts checked at use)
Affects: spec/04 `clash`, spec/15 `[Call]`

## Problem

`put(&mut x, read(&x) + 1)` forms `&mut x` for the first argument and
then `&x` for the second. By the letter of `[Borrow]` and `clash`, the
second borrow meets a valid exclusive path to `x` and faults. Both
implementations instead let it run — the first argument's path is not
yet held by anything — and catch a real conflict where the parameters
are used (D-0022). 24 sites in the repository rely on this, such as
`overwrite(&mut acc, Big::mul(&acc, &a))` and
`Vec::push(&mut ed.undo, snapshot(&ed.buf))`.

## Candidate mechanisms

- **A: the borrow counts from the call's binding of the parameter.**
- **A′: as A, with conflicts between parameters found at their first
  use** (D-0022). Selected: it is what the implementations do, and it
  keeps `swap(&mut a, &mut a)` running (`[Swap-Places]` allows `p = q`)
  and `two(&mut v, &mut v)` faulting inside `two` at the first write.
- **B: the implementations follow the letter**: all 24 sites need a
  temporary first.

## Selected design

A path an argument forms is *pending* from its formation until `[Call]`
stores it into its parameter; `clash` does not count pending paths.
After binding, nothing changes: every access is checked against every
held path (D-0018).

## Compatibility impact

None: the rules now describe what both implementations did.

## Revisit conditions

None.
