# CHG-0343 — The fragment made faithful, executable, covered, and replayed against the model (D-0270)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19, with D-0270)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0270, D-0265, D-0219
Affects: `proofs/Cobalt/State.lean`, `Step.lean`, `Preservation.lean`, `Corollaries.lean` (amended), `Elision.lean`, `Exec.lean`, `Replay.lean`, `COVERAGE.md`, `README.md`, `lakefile.lean` (new or rewritten); `impl/tools/proofcover.py`, `impl/tools/replay_model.py` (new); `impl/model/src/machine.rs`, `main.rs` (`cobmodel trace`); `impl/battery.sh` (stage 1); `spec/02` §5 (1.0.123)

## What changed

- `proofs/`: `clash` requires neither path lock-derived; `solitary`, `tempOf`, `owns` defined;
  `[Destroy]`/`[Object-End]` require `solitary` (and `[Object-End]` the ender's ownership), `[Binding-Form]`
  and `[Temp-Root]` a temporary of the thread's current scope within a statement, `[Relocate-In]` and
  `[Rawptr-Move-In]` the thread's own object; `[Authority-Transfer]` is `transferTo`, forming the new root
  itself, of an obligation, with the source path alone; `noGuard` over live guards. `Preservation.lean`'s
  cases adjusted (the transfer case composes the temp-root and transfer lemmas); `bound_parameter_clashes`
  takes the parameter path's non-lock-derivation. `Elision.lean`: `StepBy`, `Standing`, `ApartObj`,
  `Apart`, `object_stays`, `path_stays`, `running_stays`, `standing_step`, `OthersRun`, `standing_run`,
  `conflict_diagnosed`, `write_conflict_diagnosed`, `read_conflict_diagnosed`, `unreached_stays`.
  `Exec.lean`: the decidable premises with their soundness lemmas, `Act`, `check`, `apply`, `exec`,
  `exec_sound`, `exec_reachable`. `Replay.lean`: the trace reader (the line forms in its header), the
  identity maps, the state flattened into arrays after each step (the same state, read in constant
  time), the two gap readings, `--time`. `COVERAGE.md` generated; `README.md` rewritten for the new parts.
- `impl/tools/proofcover.py` (the coverage table, `--check`); `impl/tools/replay_model.py` (a directory
  traced and replayed, `--check`); `cobmodel trace FILE [--out TRACE] [--max N]`: the machine's primitives
  write the fragment's actions (`bind` as establish and binding, a pending argument's binding as the
  rule's `bindParam`, borrows, lends, takings, projections, reads, writes with the references they drop
  and add, settlings, the frame and statement scopes, spawns with the handle object, thread ends,
  joins, handle and guard ends, locks, contained faults, moves as `[Authority-Transfer]`, and `out` where
  the program leaves the fragment); the model's own outcomes are unchanged (every sample's `MODEL:`
  header still holds).
- `impl/battery.sh` stage 1: `proofcover --check`, `lake build` with no `sorry`, and the sample replay.

## What changed for programs

Nothing: no rule changed. The fragment's premises added are the rules' own, written where the fragment
had read a rule more loosely than its text.
