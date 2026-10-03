# D-0087 — A use of a binding that may have been moved is rejected

Status: ACCEPTED (2026-09-28, the owner's decision on the fourth stress round's findings)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: rule.control.flow-analysis, rule.init.definite-assignment, D-0049
Affects: `spec/14` §6 (the discharge table)

## Problem

The flow analysis refuted a use of `x` only when `valid(x) = F` on every
path. A use where `x` was moved on some paths only (`?`) was left to the
run-time check:

    Vec<i32> v = …;
    while (i < 3) { takes(v); i += 1; }     // the second pass finds `v` moved

    if (c) { takes(v); }
    Vec::len(&v)                             // stale when `c` held

Both are rejected by Rust at compile time; here they ran until the
failing path was taken, so a test that did not take it passed. A read
that may be uninitialized, the same shape of question about the same
binding, was already rejected (`spec/11` §3, the one exception to
"unknown is dynamic").

## Candidate mechanisms

1. **Keep "unknown is dynamic".** Sound, but a moved-in-a-loop mistake
   is found only by the run that takes the second iteration.
2. **Reject `valid(x) = ?` for a use of `x`**, as `init(x) = ?` is
   rejected. Selected.

## Selected design

`temporally-valid(a_x)` is refuted, for a use of the binding `x` (its
lookup, a borrow or read of it or a part of it, a closure capture of
it), when `valid(x) = F` or `valid(x) = ?`. A binding moved on some path
is used again only after a whole-object write gives it a value on every
path (`[Assign-Reestablish]`).

Other conditions keep "unknown is dynamic".

## Compatibility impact

A tightening: a program whose maybe-moved use was never reached, or
whose moving path is never taken, is now rejected. Four conformance
cases that pinned the old boundary now expect `(static)`; no positive
program in the repository or the stress set is affected.

## Revisit conditions

None.
