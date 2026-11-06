# CHG-0233 — Supervised tasks: fault containment, cancellation, poisoning, deadlock diagnosis

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0204)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0204, CHG-0230, CHG-0231, CHG-0232
Affects: `spec/18` 1.6.0, `spec/19` 1.11.0, `spec/21` 4.51.0, `spec/03` 1.5.0, `spec/registry/diagnostics.md`
1.50.0, `spec/conformance.md` 3.198.0; `impl/src/interp.rs`, `impl/src/netio.rs`, `impl/src/main.rs`,
`impl/cbrt/src/lib.rs`, `impl/cobc/src/lower.rs`, `impl/std/sync.cb`, `impl/std/collections.cb`

## What changed

- **`spec/18`:** `[Fault-Contain]` and `contained(ℓ, Σ)`; `[Fault-Unwind]` keeps the main thread's fault and a
  thread lent an exclusive reference.
- **`spec/19`:** `[Join-Failed]`; `[Handle-Destructor]` raises a failure nobody took; `rule.conc.cancel`
  (`[Cancel]`, `[Cancelled-Wait]`); `[Lock-Poisoned]`; `[Deadlock]` in §4.
- **`spec/21`:** prelude `cancel`, `join_status`; `std::sync` `try_join`, `is_finished`, `ThreadFailure`;
  `std::collections` `slice_parts` (§3c′).
- **Diagnostics:** `diag.thread-cancelled`, `diag.mutex-poisoned`, `diag.deadlock`.
- **Both tools:** containment (`cbrt`: a `setjmp` in the thread's trampoline, the fault unwinds the thread's
  frames, releases its holds and returns there); socket waits poll in slices so cancellation reaches them;
  deadlock checks at each wait and when every thread waits.
- **`coby`:** `--explore N` and `--schedule-seed S` (a deterministic scheduler).
- **`cbrt`:** at most 128 heaps, threads past that sharing the least-used one (fixes a false
  `diag.aliasing-conflict` with more than 127 threads alive); per-heap totals and page masks for the address
  tables; `spawn`, temporary paths, ranged borrows, reclaimed cells and socket waits without the exclusive lock;
  a listen backlog of 4096.
- **Checker (found on the way):** a literal argument was exempt from the exact-type check whatever its parameter's
  type, so `Option::unwrap(5: u64)` passed (`conf.generic-literal-argument-checked`); a generic call whose argument
  has another type than its parameter now says so instead of only "nothing fixes `T`".
- **Conformance:** `conf.thread-failure-contained`, `-join`, `-handle-end`, `-nested`, `-deep-unwind`,
  `conf.thread-failures-many`, `conf.thread-lent-exclusive`, `conf.thread-cancel-waits`, `-lock`, `-socket`,
  `conf.mutex-poisoned`, `conf.deadlock-all-waiting`, `conf.many-threads-owning-frames`,
  `conf.thread-is-finished`, `conf.slice-parts`.

## What did not change

A program whose threads do not fault, wait forever or get cancelled behaves as before. A fault in the main
thread, or in a thread lent an exclusive reference, still ends the program at once.
