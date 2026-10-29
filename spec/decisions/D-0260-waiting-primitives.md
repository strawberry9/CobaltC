# D-0260 — `Barrier`, `Semaphore` and `Once`

Status: ACCEPTED (2026-10-17, the owner: "draft D-0259 and D-0260, then implement them autonomously using
your recommendations")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (std admission), §17
Depends on: D-0204 (`[Deadlock]`), D-0249, D-0253 (`Atomic::wait`, `notify`), D-0258 (static atomics)
Affects: `spec/19` §2a (1.20.0), `spec/21` §3c‴ (4.68.0), `spec/registry/diagnostics.md` (1.60.0),
`spec/conformance.md`; `impl/std/sync.cb`, `impl/src/interp.rs`; `CHG-0323`

## Problem

D-0253 gave threads `Atomic::wait` and `notify`. The three things programs build on them — a meeting point
for a fixed number of threads, a count of permits that bounds how many threads do something at once or
hands events from one thread to another, and an initialisation done exactly once by whichever thread gets
there first — are each twenty lines that are easy to get subtly wrong (a lost wake-up, a thread of the next
round counted in the old one, an initialiser that faulted leaving the others waiting forever). Each names
one very common intent, which is the admission test for `std` (Master Instructions §9).

## Options

- **A. Leave them to programs**, with the guide showing each once. Every program that needs one rewrites it,
  and the subtle cases are rewritten too.
- **B. `Barrier`, `Semaphore` and `Once` in `std::sync`**, each an atomic or two and a wait loop, no lock and
  no runtime event. Shared by reference, as a `mutex` is.
- **C. B plus a `WaitGroup` or latch.** A latch is a `Semaphore` made with 0 permits whose waiter acquires `n`
  times, or a `Barrier` of the right size, and `join` already waits for threads; declined.
- **D. A read-write lock or a spin lock.** Each needs a guard whose paths the checker treats as lock-derived,
  as the built-in `mutex`'s are: a language decision, not a `std` addition; the owner declined `RwLock` on
  2026-10-04. Out of this decision.

## Decision

**B.** `Barrier::new(parties)` (0 is `diag.barrier-zero-parties`, a new dynamic diagnostic, as
`Channel::new(0)` is `diag.channel-zero-capacity`), `Barrier::wait` → `bool` (true for the last to arrive);
`Semaphore::new(permits)` with 0 allowed, `acquire`, `try_acquire`, `release` (`[Atomic-Overflow]` beyond
`usize`), `available`; `Once::new`, `Once::call(&o, f)` with `f` a `fn()` value or closure run on the
caller's thread, `Once::is_done`. A fault unwinding out of `f` leaves the `Once` as it was and wakes the
waiters, so the next caller runs `f` again: the run is a local resource whose destructor resets the state,
which costs nothing when the run completes. Fairness is not promised (a semaphore's permit goes to whichever
waiter's `compare_exchange` succeeds). A wait none can end is `[Deadlock]` (b), as `Atomic::wait` is, so a
barrier for more threads than exist is `diag.deadlock`, not a hang.

## Results

Written in `impl/std/sync.cb` over `Atomic` alone. The prototypes (`stress/atomics_std/barrier_proto.cb`,
`once_proto.cb`) and the conformance cases (`conf.barrier-phases`, `conf.barrier-short`,
`conf.barrier-zero-parties`, `conf.semaphore-limits`, `conf.once-runs-once`, `conf.once-fault-retried`) run
identically under `coby`, `cobc` with GCC and Clang, and both tools' `--explore`, every positive case to one
outcome. Three unit tests in `std`, which the guide runs. Writing `Once`'s fault test found that a
`fault(d, …)` call inside a closure counted `d` as a captured variable (`CHG-0324`).

## Revisit conditions

A fair semaphore, or a `WaitGroup`, when a program measured on them asks; a condition variable, by its own
decision (it needs a mutex to pair with).
