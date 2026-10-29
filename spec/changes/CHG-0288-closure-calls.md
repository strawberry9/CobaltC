# CHG-0288 — Capture paths; a `move` closure that writes a capture is borrowed exclusively

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0239)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0239
Affects: `spec/15` (1.15.0), `spec/08` (`[Read-Fn-Through-Shared]`), `spec/OVERVIEW.md`; `coby`
(`interp.rs`, `interp/closures.rs`, `interp/calls.rs`, `interp/refs.rs`); `cobc` (`lower.rs`,
`lower/closures.rs`, `lower/literal_calls.rs`, `lower/exprs.rs`, `lower/calls.rs`); `cbrt`; `spec/conformance.md`

## What changed

- `spec/15` §6: `[Closure-Call]` with `writes-self` and `m_self`, capture paths; prose. `spec/08`:
  `[Read-Fn-Through-Shared]` also for a self-writing closure.
- `coby`: `ClosureInfo.writes_self`; a call of such a closure checks an exclusive access of its place;
  `fn_through_shared` faults on one. `cobc`: the same fact, its box marked `closure = 3`, an exclusive
  `cb_write` of the place a marked box is called through; `cbrt`'s `fn_exclusive_capture` counts a marked box,
  and `cb_fn_shared` names the reason.
- Cases: `conf.move-closure-writing-capture`, `conf.move-closure-writing-capture-while-shared`,
  `conf.move-closure-writing-capture-through-shared`.

## What changed for programs

A soundness repair (`spec/STABILITY.md` §2): a `move` closure that writes a value it captured, called while a
shared reference to it lives, faults `diag.aliasing-conflict`; called through a shared reference, it faults
`diag.borrow-exceeds-source` ("changes a value it captured by `move`"). Called from its own binding with no
other live reference, it runs as before.
