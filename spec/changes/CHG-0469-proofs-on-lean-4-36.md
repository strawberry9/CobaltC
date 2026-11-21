# CHG-0469 — The proofs are checked by Lean 4.36.0 (implementation only)

Status: implemented (2026-11-18); Update 198
Affects: `proofs/lean-toolchain`, `proofs/Cobalt/Statements.lean`, `proofs/COVERAGE.md`, `proofs/README.md`

## Why

`CHG-0465` named the Lean release that checks the proofs (4.35.0) when 4.36.0, published during a run of the
gate, changed the axioms three results rest on, and left the move to 4.36.0 as a change of its own. The owner
put it into this update (2026-11-18).

No rule changes, and no proof changes.

## What changes

- `proofs/lean-toolchain` names `leanprover/lean4:v4.36.0`.
- Three `#guard_msgs` blocks of `Cobalt/Statements.lean` list `Classical.choice` beside `propext` and
  `Quot.sound`: `read_no_clash`, `exec_sound` and `Statics.run_sound`, which 4.35.0 proved without it. The
  proofs are the same text; the release's own library reaches the axiom where the earlier one did not. Every
  result now rests on Lean's three axioms and no other (the fifteen other results listed all three before).
  Bringing the three back off `Classical.choice` was considered and not done: it is one of the three axioms
  the development allows itself (`proofcover --check` holds every result to them), fifteen of the eighteen
  results rest on it already, and a proof bent to avoid it in one release has no reason to stay off it in
  the next.
- `proofs/COVERAGE.md` regenerated; `proofs/README.md` says which release and what a later move takes.

## Checked

- `lake build` under 4.36.0: built, no error, no `sorry`. It prints 32 warnings 4.35.0 did not (the
  deprecated names `if_pos` and `if_neg`, bound names not referred to); they are not errors and the proofs'
  text is left as it was.
- `impl/tools/proofcover.py --check`: 53 rules, 18 results, standard axioms only, 0 errors.
- The two other kernels, as the release unit `proofs-kernels` runs them, under 4.36.0: `leanchecker` over the
  nine modules, no error; `nanoda` over the export, 61,238 declarations, none refused.
