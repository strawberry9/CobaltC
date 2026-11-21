# CHG-0248 — Supervised tasks: cancellation points, the order at an unwind, a failed thread's result, the member of a cycle

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0210)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0204, D-0210
Affects: `spec/19` 1.12.0, `spec/conformance.md` 3.204.0, `spec/02` 1.0.72; `spec/decisions/D-0204-supervised-tasks.md`
(a "Precisions" section); `impl/tests/conformance.rs`; `impl/conformance/19-concurrency/thread_failure_handle_end_in_unwind_ok.cb`,
`thread_cancelled_holding_guard_rejected.cb`, `failed_handle_in_main_unwind_rejected.cb`, `mutex_poisoned_second_waiter_ok.cb`,
`lock_join_cycle_ok.cb`, `channel_wait_sleeper_ok.cb`; `impl/cobc/src/lower.rs` (two fixes found on the way, D-0210 "Results")

## What changed

- `spec/19` §1: two paragraphs after `[Join-Failed]`'s and `[Cancelled-Wait]`'s prose — "Order at an unwind"
  and "Cancellation points"; §4: which member of a cycle `[Deadlock]` faults, and a sleeping thread's wait.
- `spec/conformance.md` §12: the six rows.
- `cobc`: the native `StringView::sub`/`split` fall back to the written bodies when `StringView::boundary` is
  not a pure direct predicate (`--all-checks`); the `fn`-value thunk restores the fault location after the call.

## What did not change

No rule, diagnostic or accepted program: the paragraphs restate what `[Fault-Contain]`, `[Handle-Destructor]`,
`[Join-Failed]`, `[Cancelled-Wait]`, `[Guard-Drop]`, `[Lock-Poisoned]` and `[Deadlock]` already give, with the
order and the list written out. Both tools behaved so before.
