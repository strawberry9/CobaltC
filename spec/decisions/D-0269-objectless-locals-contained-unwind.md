# D-0269 — Objectless locals when a contained fault unwinds a thread

Status: ACCEPTED (2026-10-19, the owner: "first do D-0269 as per your recommendation"; the draft of 2026-10-18 from the overnight run's finding)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0200 (objectless locals), D-0204 `[Fault-Contain]`, `spec/18` `[Fault-Unwind]`, D-0008
Affects: `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h` (`cb_threaded`, `struct cb_note`, `cb_notes_push`, `cb_notes_pop`); `impl/cobc/src/lower/frames.rs` (`objectless_bound`, `notes_finish`), `functions.rs`, `closures.rs`, `passes.rs`; `CHG-0341`

## Problem

`cobc` gives a container local no runtime object where every use lends or moves it (D-0200, phase 1):
its buffer is freed by a C statement at the block's end. A fault unwinds by the runtime, which destroys
the frames' registered objects; the C code after the fault never runs, so the objectless local's buffer
is not freed. In the main thread this is invisible: the process ends with the fault. In a spawned thread
whose fault is contained (D-0204 `[Fault-Contain]`: `try_join`, or `join` raising it later) the thread
ends, the program goes on, and the buffer is lost, once per contained failure: a pool that contains a
failing job a thousand times leaks a thousand buffers. `spec/18` `[Fault-Contain]` has the thread's frames
unwind "as `[Fault-Unwind]` folds them", every destructor running, `Vec::drop` among them; `coby` does so.
Found by the fuzzer's fault-unwinding mode (seed 14, under LeakSanitizer) and by round 10's pool
(`run_job`'s `Vec<u64>` for its two faulting jobs: 48 bytes in two allocations).

## Options

1. Register objectless locals for the unwind, one runtime call at the binding and one at the end, in
   every function: keeps D-0200's proofs, costs two calls per container local where D-0200 removed one
   object and its tracking.
2. As 1, but only in functions a thread can run (reachable from a `spawn` through a call graph, and
   closures passed to `spawn`): programs without threads pay nothing.
3. Accept and document: a contained failure under `cobc` leaks the objectless locals of the unwound
   frames; advise `--all-checks` for a program that contains failures in long-running threads.
4. Give up objectless locals in frames with a fault point: every function has one, so D-0200 undone.

## Decision

Option 2's effect by another route. A call graph over the program would reach `std`'s functions too,
and a `std` function's lowering would then depend on the program around it, which the object cache's
premise forbids (one C text per `std` unit in every program, `CHG-0340`'s test). Instead:

- A function (or closure body) that binds objectless locals declares a table of them at its start, one
  entry per local (`struct cb_note`: the local's address, the address of its moved flag or null, its
  type, whether it is bound now). The binding sets its entry live; the local's own drop, wherever the
  lowering places it (the frame's end, a `return`, a `break`), clears it.
- The table is registered with the runtime (`cb_notes_push`) at entry and withdrawn (`cb_notes_pop`) at
  every exit of the body's root frame, under a flag read once at entry: `cb_threaded`, 1 from the first
  `spawn` on. A fault in the main thread ends the program, so only a spawned thread's contained fault
  needs the table, and a spawned thread exists only after `cb_threaded` is set (the store is ordered
  before the thread starts). A program without threads pays a byte's read and a branch per call of a
  function that binds an objectless local; a threaded program pays a push and a pop on the thread's own
  state (no hold on the runtime) per such call, not per local.
- `[Fault-Contain]`'s unwind, after folding the thread's frames and scopes, destroys the live, unmoved
  entries of every table the thread registered, innermost first, by `cb_drop_local`'s path (the type's
  destructor by its id, which frees a `Vec`'s or `String`'s buffer), and clears the tables.

The per-function table rather than a per-local call is what keeps the cost off the locals: D-0200's
saving was the object, its paths and its tokens, and the table adds nothing to a local. `exit` and a
fault in the main thread unwind no tables (the process ends). Library mode's foreign threads (D-0257)
have no contained faults and are unchanged.

## Results

The fuzzer's probe (`stress/verify/fuzz/probe/objectless_leak.cb`, 16 bytes in one allocation) and
`conf.contained-fault-frees-objectless-locals` (six jobs, three faulting: 120 bytes in six allocations
before) report nothing under LeakSanitizer; round 10's `pool.cb` likewise. The output of each is the one
`coby` gives. `cobc`'s test `contained_fault_frees_objectless_locals` (`driver.rs`) keeps it so under
Clang's sanitizer. The object cache's identity test is unchanged (the table is part of the unit's text
in every program alike). Re-timed: the benchmarks' instruction counts are unchanged; a guide program that
binds a container local in its loop counts a few per cent more instructions (the entry's stores at each
binding: `e11_counting` +3.7 %, `e01_confined` +7 % under GCC, none under Clang), wall times within the
run-to-run spread.

## Revisit conditions

A measured cost of the entry read in a hot function that binds objectless locals; or a frame that a
contained unwind folds whose locals the table misses (a new objectless form bound outside
`objectless_bound`).
