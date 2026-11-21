# CHG-0465 — the proofs name their Lean release (implementation only)

Status: implemented (2026-11-17); Update 196
Affects: `proofs/lean-toolchain`, `proofs/README.md`

## Why

`proofs/lean-toolchain` named `leanprover/lean4:stable`. `elan` reads `stable` afresh at every build, so the
release that checks the proofs was whichever one Lean had published last. On 2026-11-17, during the gate's run
on Update 196's commit, Lean 4.36.0 was published; the `model` unit's `lake build` downloaded it and failed.
Nothing in the tree had changed under it: the unit was green on the same `proofs/` ten hours before.

The failure is in the `#guard_msgs` blocks of `Cobalt/Statements.lean`, which hold each result to the axioms
listed for it. Built by 4.36.0, three results rest on `Classical.choice` beside the axioms listed
(`read_no_clash`, `exec_sound`, `Statics.run_sound`: `[propext, Quot.sound]` under 4.35.0). No proof is
unfinished and no axiom outside Lean's three appears; the list a result is held to is a property of the release
that built it, and the build is right to fail when it changes.

No rule changes.

## What changes

`proofs/lean-toolchain` names `leanprover/lean4:v4.35.0`, the release every result, `proofs/COVERAGE.md` and the
two other kernels (`proofs-kernels`) were checked with. `proofs/README.md` says so, and what a move to a later
release takes.

Not done here: the move to 4.36.0. It is a change of its own (the three blocks and `COVERAGE.md` rewritten, or
the three proofs brought back off `Classical.choice`; `lean4export` and `nanoda` built for that release).

## Checked

- `lake build` in `proofs/` under 4.35.0: built, no error, no `sorry`; `impl/tools/proofcover.py --check`: 53
  rules, 18 results, standard axioms only, 0 errors.
- The failure under 4.36.0 is the gate's own log (run `20261117_004114`, `lake.log`: the download of 4.36.0,
  then the three blocks).
