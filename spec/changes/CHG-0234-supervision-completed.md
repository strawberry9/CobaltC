# CHG-0234 — Supervised tasks completed: lent through a mutex, deterministic deadlock reports, compiled exploration

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0204, the open items after Update 93)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0204, CHG-0233
Affects: `spec/18` 1.6.1, `spec/19` 1.11.1, `spec/conformance.md` 3.198.1; `impl/src/typecheck.rs`, `impl/src/interp.rs`,
`impl/cbrt/src/lib.rs`, `impl/cobc/src/lower.rs`, `impl/cobc/src/main.rs`

## What changed

- **`spec/18` `contained`:** a thread that locks a mutex whose interior type may hold an exclusive reference (a
  channel of them among such mutexes) is lent another's data, as one given such a value at `spawn` is. Both tools
  contained a thread that had received `&mut v` through a channel (`conf.thread-channel-exclusive`). Decided by
  type, so both tools agree (`typecheck::may_hold_excl_ref`).
- **`spec/19` `[Deadlock]` (b):** with no cycle, the threads at the ends of the chains of waits (waiting on a
  channel) fault, not every waiting thread: the failure then reaches the others through `[Join-Failed]` and
  `[Lock-Poisoned]`, so the outcome no longer depends on which thread wakes first (found by `cobc --explore`).
  §4 states what is not decided: a channel wait while another thread is in a call outside the program.
- **`cobc --explore N` and `--schedule-seed S`** (and `COBALTC_SCHEDULE_SEED` in a compiled program's
  environment): a seeded scheduler in `cbrt`, one thread at a time, switching at a spawn, every wait, a mutex
  released, a thread's end, a call outside the program, and one in 32 runtime entries; a thread holding the
  runtime keeps its turn through a call.
- **`cobc`:** a `str`'s view has no path (`type.str`: its bytes are no object's cells); every thread viewing a
  literal had met in the runtime on the literal's address (a third of a busy server's time).
- **`cbrt`:** a pathless datum is sent without entering the runtime; an absent id (0) names the entering
  thread's heap, not heap 0; 256 heaps (ids carry the heap in bits 55 to 62, the generation in 23 bits); a
  binding's new value asks under a local hold whether its root still lives (`cb_rebind`).
- **Both tools, Linux:** a socket wait blocks in `ppoll` with no timeout; `cancel` interrupts it with `SIGURG`,
  blocked in every thread except inside `ppoll`. Other platforms wait in slices as before.

## What did not change

No other rule. A program whose spawned threads receive no exclusive reference through a mutex, and that deadlocks
in no chain of waits, behaves as before.
