# CHG-0315 — Compound assignment to a `Vec` element indexed by a call

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0035
Affects: `spec/conformance.md` (one row); `impl/cobc/src/lower/stmts.rs`, `impl/cobc/src/lower/exprs.rs`

## What changed

`[Compound-Assign]` is unchanged. `cobc` stopped with an internal error ("borrow of a temporary") on
`v[f(x)] op= e` where `v` is a local `Vec` and `e` a literal or a local, as in `counts[narrow<usize>(n)] += 1`.
The target holds a call, so it is evaluated once through a hidden exclusive reference; `cobc` reaches an
element of such a vector as a place without an access path, and its borrow took that place for a
temporary. It now borrows it as the element's address, its bounds checked, as it already did for the same
element passed as a call's argument. The new row `conf.compound-assign-call-index` covers a local `Vec`, an
element's field, a nested `Vec` and a `Vec` behind a reference parameter. `coby` was already right.

## What changed for programs

Programs that `cobc` refused now compile and run as under `coby`.

## Found by

`stress/round10/treiber.cb`, a test of `AtomicPtr` (round 10, 2026-10-17).
