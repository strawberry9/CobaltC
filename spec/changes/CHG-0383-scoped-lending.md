# CHG-0383 — Scoped lending: a program that spawns threads may be decided

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-28)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0289, D-0281, D-0284, D-0286, D-0288, D-0213, D-0204
Affects: `spec/14-control-flow.md` 1.37.0 (§6 decided programs, scoped lending, the spawn rows); `impl/src/typecheck.rs`,
`impl/cobc/src/lower/program.rs`, `impl/cobc/src/lower/intrinsics.rs`, `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`, `impl/model/src/statics.rs`

## What changed

In the static pass (`typecheck.rs`):

1. `keeps_refs_in_data` and `tier_d_type`: a `handle<T>` or a `guard<T>` whose `T` keeps no references is
   frame-held, as a reference is. In data, a handle keeps only what its result does (one that lends is never in
   data unseen: items 3 and 4); a guard is tier D as before.
2. `spawn_facts_tri_of`: a handle bound by `let` or an assignment holds, besides the spawn's visible borrows and
   lent reference bindings, what the closure it runs captured (`closure_facts_of`, or a closure binding's facts).
3. `tier_spawn_rule` (at every `spawn`): the run decides when a spawn that lends is not bound by a `let` or an
   assignment (`bound_spawns`), when a lent argument forms no fact, when the thread runs a `fn` value, and when a
   spawned closure captures a guard (`holds_guard`).
4. `flow_consume`: a handle or a guard holding facts moved anywhere but `join` or `drop` (`joining`) leaves the
   function to the run.
5. `handle_lend_through`: a reference binding given a new referent while a handle holds facts through it leaves
   the function to the run.

In `cobc`:

6. `pathless_blocker` no longer keeps a program's paths because it spawns a thread.
7. A thread's trampoline marks the thread lent (`cb_thread_lent`, under `CB_PATHLESS`) when the thread is passed
   an exclusive reference or its closure captured one, so that a pathless thread's fault is contained exactly when
   a tracked one's is (spec/18).
8. In a decided program, a thread's resource argument is handed over as a call's is (`cb_forget_obj`) and its
   resource result is given an object where it lands (`cb_new`), not sent and received.

In `cbrt`:

9. `cb_lock_pathless`, and `cb_lock` under `CB_PATHLESS`: the mutex claimed by address, no lock path.
10. `cb_call_self` under `CB_PATHLESS` borrows nothing. It borrowed a path of a closure's box at every call, which
   `cb_call_done` (a no-op there) never retired: a decided program calling a `fn` value in a loop ran in quadratic
   time, since Update 143 (D-0288).

In the model (`cobmodel`):

11. The spawn row as spec/14 1.37.0 has it: a bound spawn escapes nothing, and its handle holds the closure's facts.

## What changed for programs

Threaded programs whose spawns lend to handles their frame holds, and programs that lock mutexes, are decided.
One program was rejected earlier than before: `stress/verify/vsweep/model_enum/p002049.cb` lends `y` to two threads
exclusively, once through a closure's capture of `r1`; it is refuted at the second spawn, where it was refuted
at a later read. Its model header follows.

## Conformance

`conf.spawned-closure-capture-blocks-owner-write` (static): a closure that captured a reference, run by a thread
whose handle the frame holds, blocks a write through the reference's referent while the handle lives.
