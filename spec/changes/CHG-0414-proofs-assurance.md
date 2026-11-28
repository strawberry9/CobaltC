# CHG-0414 — The proofs' assurance: each result's axioms checked, two more kernels, the fragment widened

Status: implemented (2026-11-07), D-0313
Affects: `proofs/Cobalt/Statements.lean` (new), `proofs/Cobalt/State.lean` (`SyncState.wake`),
`proofs/Cobalt/Step.lean` (`notify`, `recvRestamp`; `atomicOp`, `atomicNotify`, `matchOn`, `recv`),
`proofs/Cobalt/Preservation.lean` (`C1_notify`, `C2_notify`, `recvRestamp_some`, `recvRestamp_of`,
`recvRestamp_invariants`, four cases of `preservation`), `proofs/Cobalt/Elision.lean` (`StepBy`'s four
constructors; `object_stays`, `path_stays`, `running_stays`, `unreached_stays`), `proofs/lakefile.lean`,
`proofs/README.md`, `proofs/COVERAGE.md`; `impl/tools/proofcover.py`; `impl/tools/gate.py`; `spec/ASSURANCE.md`

## What changes

1. **`Statements.lean`**: the 18 results restated (`reachable_invariants`, `access_reaches_live`, `heldBy_exact`,
   `obligation_owned`, `sub_obligation_owned`, `reclaimed_obligation_owned`, `read_no_clash`, `one_lock_path`,
   `pending_unheld`, `bound_parameter_clashes`, `exec_sound`, `exec_reachable`, `standing_run`,
   `conflict_diagnosed`, `write_conflict_diagnosed`, `read_conflict_diagnosed`, `unreached_stays`, `run_sound`),
   each with a `#guard_msgs in #print axioms` block. A tampered expectation fails the build (checked by hand).
2. **`proofcover.py`**: the axioms table in `COVERAGE.md` ("The axioms each result rests on"); `--check` errors as
   D-0313 §2.2 says; its summary line says "standard axioms only" or "AN AXIOM BEYOND LEAN'S OWN". `[Match]`,
   `[Recv]`, `[Atomic-Op]` and `[Atomic-Notify]` leave its table of rules outside the fragment.
3. **`impl/gate.sh proofs-kernels`** (release profile, inputs `proofs/`): `lake build`, `lake env leanchecker`
   over the lakefile's roots, `lake env leanexport` of them and `nanoda_bin` (0.4.19) over the export; the export
   (about 360 MB) is removed after. About 2 min 30 s: leanchecker 11 s, the export 50 s, nanoda 1 min 40 s; nanoda
   checked 60,601 declarations with no typechecker errors (its pretty printer reports that it cannot print the axioms,
   which is not a checking error).
4. **The four steps**, D-0313 §2.4; `lake build` is clean of `sorry`, and the new lemmas raise no warning.

## Conformance

No case: nothing a program can observe changes. The gate's `model` unit builds the proofs (with
`Statements.lean`), and `proofcover --check` runs in `records`.
