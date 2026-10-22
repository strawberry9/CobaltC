# D-0270 — The elision lemmas mechanised, the fragment kept honest, and the proven machine run against the model

Status: ACCEPTED (2026-10-19, the owner: "keep it [the object cache]. add the identity unit test" and then "do the proof items 1, 2 and 3 … autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20, §24
Depends on: D-0265 (the mechanised fragment), D-0262 (`spec/23`), D-0219 (the model checker), D-0188, D-0209, D-0200 (the elisions), D-0207 (the corpus checker)
Affects: `proofs/` (`Cobalt/Elision.lean`, `Cobalt/Exec.lean`, `Replay.lean`, `COVERAGE.md`, `README.md`; `State.lean`, `Step.lean`, `Preservation.lean`, `Corollaries.lean` amended); `impl/tools/proofcover.py`, `impl/tools/replay_model.py`; `impl/model` (`cobmodel trace`); `impl/battery.sh`; `CHG-0343`; `CHG-0342` (found on the way)

## Problem

The Lean development (D-0265) proved the invariants preserved over a fragment, and had found one defect
of the specification's text (`CHG-0330`), but nothing of the implementations'; it was outside the battery,
so a rule change could invalidate a lemma unnoticed; its coverage of `spec/23`'s rule rows was a sentence
in its README; it talked to nothing that runs; and the one place where the compiler trusts a prose proof
and checks nothing — the elisions of D-0188, D-0209 and D-0200, which make `cobc` fast and whose failure
is a silent soundness hole — was outside it. The owner asked, after the assessment of 2026-10-19, for
three things: keep the development honest, prove the elision lemmas, and connect the proven machine to
the model checker.

## Options

1. Prose only: leave `[Standing-Local]` as `spec/08` §4 states it, with its case list.
2. Mechanise `[Standing-Local]` and D-0200's lemma over the fragment as it was.
3. Mechanise them, making the fragment faithful wherever the lemmas need a premise the rules have and
   the fragment lacked; put the build and a coverage table in the battery; make the step relation
   executable and replay the model checker's runs through it.

## Decision

Option 3.

**Honesty.** `impl/tools/proofcover.py` writes `proofs/COVERAGE.md`: each rule label a row of `spec/23`
names, with the Lean step whose docstring cites it or the reason it is outside the fragment; `--check`
fails on a row with neither, on a listed-outside rule that has a step, and on a Lean citation of a label
the specification does not define. Battery stage 1 runs it, builds the development (`lake build`, no
`sorry`), and replays the model's sample programs (below).

**The elision lemmas** (`Cobalt/Elision.lean`). `StepBy ℓ σ σ'` exposes a step's performer. Over a
standing path of ℓ (`Standing`: ℓ's own, valid, not lock-derived, not pending, over fresh storage that is
no guard, ℓ running) whose object and scope another thread ℓ' leaves alone (`Apart`: owned by no frame or
scope of ℓ', no handle, in no finished result), `path_stays`, `object_stays` and `running_stays` give
`standing_step`: after any step of ℓ', the path is valid, the same object, projection, mode, scope and
frame, ℓ's, not lock-derived, not pending, its object alive and as it was; `standing_run` carries it over
a run of other threads' steps from a reachable state. `conflict_diagnosed` (and its read and write forms)
is the second half: a standing path is a `clash` witness for every access through a path that overlaps
it in an unpermitted mode, so such an access is refused by the step's own premise — `spec/08` §4's
closing sentence, "every conflict involving such a path is still diagnosed, at the other thread's
access". That, not the preservation of `¬clash(a)` the prose states, is what the elision rests on: another
thread may bind a pending argument that overlaps `a` (D-0022 finds the conflict at the first use), and
that use is the access refused. `unreached_stays` is D-0200's lemma: an object no valid path reaches is
reached by no other thread's step, and stays alive while the other thread leaves it alone.

Reading `[Standing-Local]`'s hypotheses against the compiler before writing a line of Lean found
`CHG-0342`: H2, "checked against the other arguments of its call", is exactly the window in which `cobc`
vouched for an argument at its borrow rather than at the entry, and a sibling argument's lend to a
thread passed unseen. Repaired and cased before the proofs were written.

**The fragment made faithful** where the lemmas needed it (each a premise the rules have): `clash`
exempts lock-derived paths on either side (`sync-exempt`); `[Destroy]` and `[Object-End]` require
`solitary`; `[Authority-Transfer]` forms the new binding's root path itself (`transferTo`; before, the
root was a separate step no rule provides, which also made cross-thread moves impossible to replay) and
moves an obligation; a binding or a temporary root is of a temporary of the thread's own scope
(`tempOf`), within a statement; `[Relocate-In]`, `[Rawptr-Move-In]` and `[Object-End]` move or end the
thread's own object (`owns`); `noGuard` concerns live guards. Every earlier theorem still holds.

**The executable machine** (`Cobalt/Exec.lean`). `Act` names a step and its data; `check` decides the
premises by searching the tokens, identities and threads below the bounds the invariants give (`Fresh`,
and `ThreadsBelow`, which the replayer keeps); `apply` performs the step; `exec_sound` proves an admitted
action a `Step`, and `exec_reachable` that a replayed run stays within the reachable states. `cobmodel
trace FILE` (D-0219's model, instrumented) writes one run — the lowest-numbered enabled thread stepping
each time — as the fragment's actions with the model's own identities; `lake exe replay TRACE` maps the
identities as the machine allocates them, checks and performs each action, and reports the first refusal
or the point where the model left the fragment (vectors, channels, atomics, slices, closures, statics,
arrays: what came before was checked). Two readings of the rules the fragment does not yet take are
applied as counted gaps, not steps, and stated in `README.md`: `[Object-End]`'s `Dead` (a path whose
only holder ended, or which a write released after its statement, is invalid) and `[Stmt-Exit]`'s result
exemption (a statement result's unheld paths pass to the parent scope). `impl/tools/replay_model.py`
runs a directory and tallies.

## Results

Over the model checker's 315 sample programs: 178 runs replay whole, 137 stop where the model leaves the
fragment (vectors 82, channels 30, closures 14, arrays 4, swaps 3, jobs 3, atomics 1), none is refused;
7,181 actions admitted, 205 gaps applied. The replay's first runs refused three shapes, each a fragment
defect repaired by this decision, not a model defect: a spawn's pending argument bound as a plain store
(the rule binds it: `bindParam`), a block's exit refused for a guard already dropped (a dead guard is no
guard), and a lock path's write refused as a clash with the mutex's shared paths (`sync-exempt`
exempts the lock path's own accesses). `proofs/COVERAGE.md`: 34 of `spec/23`'s 53 rule rows are steps.
The development builds in under a minute from clean; `lake build` also builds the replay executable.

## Revisit conditions

A refusal in the battery's replay step: a model step the fragment's rules do not admit, to be settled
as a fragment gap, a model defect, or a text defect. The two counted gaps are the next fragment
milestone (`endObjects` with `[Object-End]`'s `Dead`; `stmtExit` with the result's references spared),
which Preservation's `endObjects` lemmas would need to follow. The 137 runs outside the fragment name
the next steps to model: vectors first.
