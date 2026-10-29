# D-0210 — Supervised tasks made precise: cancellation points, the order at an unwind, a failed thread's result

Status: ACCEPTED (2026-10-11; the owner: "continue actioning the outstanding items in the Action plan" — WI-7)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §20
Depends on: D-0204, D-0208, D-0209
Affects: `spec/19` 1.12.0, `spec/conformance.md` 3.204.0, `spec/02` 1.0.72 (CHG-0248);
`impl/conformance/19-concurrency/` (six cases); `stress/supervised_oct11/` (local: probes and `--explore` logs)

## Problem

The review (F-07; `ChatGPT_review/ACTION-PLAN-2026-10-11.md` WI-7) found D-0204's model stated but three
things left to be read off the implementation: which waits a `cancel` interrupts; what happens, and in which
order, when a contained fault unwinds a frame that owns a handle whose thread is blocked, has failed, or is
running; and what becomes of the result of a thread that failed or was cancelled. It also asked for adversarial
cases around cancellation, poisoning and cycles of waits, each run under `--explore` so that the outcome is
shown not to depend on the schedule.

## Decision

Both tools already agreed on every probe; the text now says what they do.

1. **Cancellation points** (`spec/19` §1): `[Join]` (and `std::try_join`), `[Lock]`, a channel's `[Send]` and
   `[Recv]` (and `recv_timeout_ms`'s waits), `sleep`, and a socket's wait — the waits `[Cancelled-Wait]`
   names, and no others. A call outside the program (input, a file, a child process) is not a wait of the
   rules: a thread inside one is cancelled at its next wait, or finishes. A cancelled thread holding a guard
   unwinds through `[Guard-Drop]` and poisons the mutex, as a faulting thread would: it is not spared, and the
   half-changed state is reported to the next locker (`conf.thread-cancelled-holding-guard`).
2. **Order at an unwind** (`spec/19` §1): a contained fault `d` unwinding a frame that owns a handle cancels the
   thread first (`[Fault-Contain]`'s `Σ_2`), the handle's destructor waits for it (`[Handle-Destructor]`), and
   the failure it reports — its own or `diag.thread-cancelled` — is discarded (`[Join-Failed]`'s exception):
   the unwinding thread's result is `failed(d)` (`conf.thread-failure-handle-end-in-unwind`). The main
   thread's unwind (`[Fault-Unwind]`) destroys a handle without waiting and discards a failure it holds the
   same way: the program reports the fault that began the unwind (`conf.failed-handle-in-main-unwind`). This is
   the first-fault policy of `conf.fault-unwind-first-fault`, applied to handles.
3. **A failed or cancelled thread has no result**: whatever it had built was destroyed in its unwind; `join`
   raises its failure, `try_join` returns it (`[Join-Failed]`; `conf.thread-failure-deep-unwind` shows the
   destructors running, `conf.try-join-resource-result` the successful case's resource, destroyed once).
4. **Which member of a cycle `[Deadlock]` faults** (`spec/19` §4): the one whose wait closes it. In a mixed
   cycle (a thread holding a mutex joins a thread waiting for that mutex) either wait may come second, so the
   member can depend on timing; the program's outcome need not, because the member's failure reaches the
   others through `[Join-Failed]` and `[Lock-Poisoned]` (`conf.lock-join-cycle`). A pure join cycle cannot form
   without passing a handle through a channel, and is then the same case. The chain with no cycle is
   `conf.deadlock-all-waiting` (D-0204, one outcome).
5. **Six cases**, each deterministic by construction (a channel fixes the order where timing could matter):
   the four above, `conf.mutex-poisoned-second-waiter` (a waiter blocked on a mutex, or arriving later, gets
   `[Lock-Poisoned]`) and `conf.channel-wait-sleeper` (a wait on a channel while the only other thread sleeps
   is no deadlock). The review's "joiner re-raising a fault inside its own destructor during an unwind" is
   `conf.fault-unwind-first-fault` and `conf.thread-failure-handle-end`; "failed thread with an owned resource
   result" is item 3.

Not done: a three-thread join cycle (it needs a handle sent through a channel to close; the mixed two-thread
cycle exercises the same rule and the same `[Join-Failed]`/`[Lock-Poisoned]` propagation), and a separate model
checker (`--explore N` is the bounded exploration, as the plan says).

## Results

Each case gives one outcome under `coby`, `cobc` with GCC and `cobc` with Clang, with and without
`--all-checks`, the same output and the same report; `cobc --explore 40` over each finds one outcome
(`stress/supervised_oct11/explore.log`; `coby` has no schedule exploration — D-0204 gave it to `cobc`, whose
threads are the operating system's). On the way, the compiled suite under
`--all-checks` (D-0209) found two `cobc` defects, fixed with this edition: the native `StringView::sub` and
`split` called `boundary` with nothing sent, right only while `boundary`'s slice is a pure direct parameter
(now they are lowered as written when it is not); and a program function called from `std` through a `fn`
value left its own line as the fault location, so a fault in `std` right after it was reported there — the
thunk now restores the caller's location (`vec_binary_search_by_held_element_rejected` under `--all-checks`
reported line 16, the comparator's, where `coby` reports 32).
