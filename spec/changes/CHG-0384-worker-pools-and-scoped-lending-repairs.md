# CHG-0384 — Worker pools are decided; scoped lending repairs

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-28)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0289, D-0281, D-0284, D-0013, D-0240
Affects: `spec/14-control-flow.md` 1.38.0 (§6 scoped lending); `spec/conformance.md` 3.280.0;
`impl/src/typecheck.rs`, `impl/src/interp/calls.rs`, `impl/model/src/statics.rs`

## What changed

In the static pass (`typecheck.rs`):

1. **A guard given as an argument.** `tier_spawn_rule` leaves the function to the run when a thread is given a guard
   as an argument (the callee's or the closure literal's parameter holds one, or the argument is a binding that
   does), as it did for a guard a spawned closure captured. Update 144's release suites found it:
   `conf.guard-moved-to-thread` was decided and lowered pathless, and the thread's access through the guard,
   which faults `diag.aliasing-conflict` (D-0240), passed unchecked.
2. **Worker pools.** `Vec::push(&mut hs, spawn(…))`, `hs` a local of the frame, binds the spawn as `let` does:
   `hs` holds the spawn's facts besides what it held (`add_deriv_tri`), and so does a handle holding facts pushed
   there. A handle taken out of `hs` holds what `hs` holds (`handle_holder_of`): by a `let` of `hs`, by a consuming
   `foreach` (`Vec::drain`, `Drain::take`), by `Vec::pop`, `Vec::remove` or `Vec::swap_remove` whose result is
   bound (`mark_taken`, `taken_handles`), through `Option::unwrap`/`expect`, or by a pattern's binder. A move of
   a holder whose facts follow it is not a move out of sight (`handing`); any other move of a holder of handles
   holding facts is (`flow_consume`, now for any type holding a handle). An exclusive borrow of such a holder
   anywhere but the operations that keep its handles in the frame (`push`, `len`, `is_empty`, `clear`, `truncate`,
   `reserve`, and the bound takes above) leaves the function to the run (`handle_borrow_ok`).
3. **Holders that end at their scope's end.** An access that facts of value U or a path overlap of `Maybe` may
   meet counted a holder only while it was mentioned later (`tier_report::live_after`). A handle, a guard or a
   `Vec` of handles is not ended at its last use (`ends_at_last_use`): its end waits or releases. It now holds its
   facts to its scope's end (`holder_live_after`). Before this, a handle dropped on one branch only (`if (c) {
   drop(h); } x = 10;`) left a program decided whose thread could still read `x` while it was written: a hole of
   CHG-0383.
4. **`r = pass(r)`.** An assignment `r = e` cleared `r`'s facts before `e`'s were found, so a value made from `r`
   itself (`name = StringView::sub(name, 0, k)`) inherited only `*r`, its own new referent, and its origin was
   unknown: the run's. The facts `r` held before the assignment now stand for `*r` there (`prev_facts`): the new
   value lies within what the old one reached.
5. **`Option::unwrap_or`.** `Option::unwrap_or(o, d)` and `Result::unwrap_or` hold what `o` or `d` holds; their
   generic result was taken for a reference read out of data (tier D). An `if` or a `match` whose every branch is
   static data (`if (b) { "byte literal" } else { "string literal" }`) gives static data, not an unknown source.

In `coby` (`interp/calls.rs`):

6. A generic call given as another generic call's argument (`Arc::new(Channel::new(4))` for an
   `Arc<Channel<u32>>`) was evaluated with no expected type, so its own type argument fell to a default and the
   storage it made was sized for that; destroying the `Arc` then faulted `diag.destroy-while-aliased`. When the
   arguments before it leave its parameter's type open, the outer call's expected type now fixes it, as the static
   pass already did. Found by the D-0281 phase 4 study, whose rewrites of worker pools onto `Arc<Channel<T>>`
   `coby` could not run; `cobc` ran them.

In the model (`cobmodel`):

7. `Vec::push(&mut hs, spawn(…))` onto a `Vec` of handles adds the spawn's facts to `hs`'s, and a handle binding
   pushed there gives `hs` its facts; a handle popped out of it by a `match` holds them; a handle or a `Vec` of
   handles is live to its scope's end. Four model-enumerated programs and `impl/model/sample/hand-e01.cb`, which
   wrote what a handle in a `Vec` was lent, are refuted before the run by both, and their headers follow.

## What changed for programs

Worker pools whose handles a local `Vec` holds and whose threads are lent shared references (`std::sync`'s
barrier example, `showcase/tier13/first_answer.cb`) are decided. A pool whose handles are taken out by
`Vec::pop` and whose lent place is written after the loop stays with the run: the analysis does not know the
`Vec` is empty there. A program that drops a lending handle on some paths only and then writes what it lent is
no longer decided.

## Conformance

- `conf.pool-vec-lend-blocks-owner-write` (static): two threads lent `x` pushed onto a `Vec` of handles; one taken
  out and joined; writing `x` conflicts with the other's lend.
- `conf.generic-call-argument-takes-outer-expected-type`: `Arc::new(Channel::new(4))` and
  `Arc::new(Vec::filled(4, None))` run and end cleanly under both tools.
- `conf.handle-moved-into-vec-then-referent-written` is static (it was dynamic under D-0244): the handle moved
  into `hs` hands its fact on `x` to `hs`, and the write meets it before the run. Found by the release suites
  `stress/suites/runs/u144b`, which ran the case under the old expectation.
- Driver test `worker_pools_in_a_vec_leave_a_program_decided`: a pool decided (gcc and clang, output checked); a
  partly popped pool, a handle dropped on one branch, and a `Vec` of handles given away keep paths.
