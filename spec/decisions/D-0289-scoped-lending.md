# D-0289 — Scoped lending: a program that spawns threads may be decided

Status: ACCEPTED (2026-10-28, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 1, §16 decision 2 (scoped lending).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0071, D-0204 (containment), D-0213 (`[Thread-Lend]`), D-0240, D-0247, D-0281, D-0284, D-0286,
D-0288
Affects: `spec/14-control-flow.md` 1.37.0 (§6 decided programs, scoped lending, the spawn rows);
`impl/src/typecheck.rs`, `impl/cobc/src/lower/program.rs`, `impl/cobc/src/lower/intrinsics.rs`,
`impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/model/src/statics.rs`; `CHG-0383`

## Problem

A program that spawned a thread kept its record of references for its whole run (D-0284 listed "a spawn" among
what is never decided), and so did every program whose types named a thread's handle or a lock's guard: they
were taken to keep references in data. Over the corpus of `stress/d0281/why/` they were the largest group of
programs that kept their paths: 276 named `handle<void>`, 165 `guard<i64>`, and the servers, pipelines and
workers of `showcase/` among them.

## Options

- **A. Keep them tracked.** Every threaded program pays for paths everywhere, whether or not its threads borrow
  anything.
- **B. Scoped lending** (D-0281 §16, decision 2). A handle a binding of the lending frame holds is the borrows
  the spawn lent, as `deriv` facts, until `join`, `drop` or the binding's end; spec/19 already says no thread
  outlives the frame that holds its handle, so the lender's accesses meet the lend before the run. A guard held
  by a frame is the borrow of its mutex it was formed with. What the static pass cannot follow stays with the run.
- **C. Structured concurrency syntax** (a `scope { … }` that joins every thread it spawns). New syntax for what
  B already has: every handle is joined at its frame's end.

## Decision

B. A handle or a guard held by a frame keeps references only as its facts. A handle in data (`Vec<handle<T>>`,
a worker pool) keeps only what its result does: by rules 1 and 2 below, one that lends is never there unseen. A
guard in data is tier D as before. The run checks, and the program is not decided, when

1. a spawn that lends has no binding of the frame to hold its handle (passed, stored or discarded);
2. a handle or a guard holding facts is moved anywhere but `join` or `drop` (returned, passed, captured);
3. a thread runs a `fn` value, whose captures are not known before the run;
4. a spawned closure captures a guard (a lock's path stays its locking thread's, D-0240);
5. a lent argument forms no fact (a slice or a view binding passed whole, D-0247);
6. a reference binding through which a handle holds a lend (`spawn(f, r)`) is given a new referent while the
   handle lives: the fact names `*r`, and the thread still holds the old referent.

A bound spawn's handle also holds what its closure captured (the closure row, D-0288), and escapes nothing.

## Repairs found with it

Lowering threaded programs pathless and decided exposed the runtime's reliance on paths, each repaired in
`CHG-0383`:

1. **A lock.** `cb_lock` found its mutex through the reference's path; with token 0 it faulted
   `diag.stale-binding`. A pathless lock (`cb_lock_pathless`) claims the mutex by address, which is all a guard's
   drop needs.
2. **Containment.** A thread lent an exclusive reference is not contained (spec/18); the runtime found such a lend
   among the paths the thread's block held, and a pathless block holds none, so every thread was contained. The
   trampoline now says so itself (`cb_thread_lent`) when `cobc` sees an exclusive parameter or capture.
3. **Resources in flight.** In a decided program a value passes with no object (D-0286), but a thread's
   trampoline still sent a resource argument and received a resource result: "receive of an object that was not
   sent". It now hands them over as a call does (`cb_forget_obj`, `cb_new`).

4. **A closure call.** `cb_call_self` borrowed a path of the closure's box at every call, and in a pathless
   program `cb_call_done` ends nothing, so the box's paths grew with every call and each check walked them all.
   A decided program that called a `fn` value in a loop (a comparator, since D-0288 made such programs decided
   in Update 143) ran in quadratic time: `stress/perf_oct9/wordpar.cb`, decided now, took 59 s where its tracked
   build takes 6.6 s. Under `CB_PATHLESS` the call borrows nothing; wordpar runs in 0.25 s.

## Consequences

- The workers, pipelines and servers of `showcase/` that lend what they share by reference, lock a mutex, or
  pass a channel are decided: over the 11,022 programs of `stress/d0281/why/`, 590 more than with
  `CHG-0382` alone (9,179): 9,769 in all, where Update 143 had 9,067. One program is refuted earlier (`CHG-0383`).
- A worker pool that keeps its handles in a `Vec` is decided when its workers are given what they work on by value
  (`stress/perf_oct9/wordpar.cb`); one that passes a handle around while it lends stays tracked.
- `cbc` (frozen, D-0278) keeps paths for any spawn until it is ported.

## Verification

See `CHG-0383`.
