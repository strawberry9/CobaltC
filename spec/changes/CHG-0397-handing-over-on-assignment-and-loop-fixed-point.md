# CHG-0397 — Handing over on assignment, and a loop's fixed point however many rounds it takes (D-0300)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0300, D-0291, D-0284, D-0206
Affects: `spec/14-control-flow.md` §6 (1.43.0: "Handing over"), `spec/conformance.md` (3.287.0);
`impl/src/typecheck.rs` (`set_deriv`, `set_deriv_tri`, `hand_over_referent`, `check_while`, `Flow::truncated`);
`impl/selfhost/front/typecheck.cb` (`set_deriv`, `set_deriv_tri`, `hand_over_referent`, `check_while`,
`flow_truncated`); `impl/conformance/14-control-flow/ref_copy_source_reassigned_in_loop_rejected.cb`,
`ref_copy_chain_longer_than_loop_rounds_rejected.cb` (new)

## What changed

1. **Handing over on assignment.** `spec/14` §6 "Handing over" applies when a reference or slice binding is
   assigned a new value, not only when it ends: a fact another binding holds through the old referent (`*r`),
   a copy's among them, becomes a fact `?` on what `r` held, and only then are the facts rooted at `*r`
   cleared. Both static passes call `hand_over_referent_as(r, false)` first in `set_deriv` and
   `set_deriv_tri`; `hand_over_referent` (an end, D-0291) is `hand_over_referent_as(r, true)`.
2. **A loop's fixed point.** `check_while` iterates until the head stops moving. Rounds after the sixteenth
   join the head with what the body gives it, and cut the facts' paths at eight steps (`Flow::truncated`,
   `flow_truncated`: a fact on a longer path is a `?` on its prefix); a head not settled after 4,096 rounds is
   an internal error. A path `hand_over_referent` extends past eight steps is cut the same way.
3. **Two conformance cases**, each `diag.aliasing-conflict (dynamic)`, each a program the earlier `cobc`
   decided and ran with no record of references: `conf.ref-copy-source-reassigned-in-loop` and
   `conf.ref-copy-chain-longer-than-loop-rounds`.

## Verification

- The two cases under `coby` and `cobc --run` (both fault as the rules say); `cobmodel certify` refutes the
  earlier certificates of both (`model.decided-but-false`); `factcheck.py` refutes them
  (`factcheck.decided-but-left`).
- `cargo test --release -p coby`: `cb_conformance_suite` (1,124 cases), `conformance`, `spec_rows`, `cli`,
  `conf_coverage`.
- The static verdict of every program of the model sample, `impl/conformance/` and `showcase/` (1,570) is the
  one the static pass gave before the change.
- The undecided sweep over `stress/d0281/objectless/decided.list`: 8,825 of its 8,854 programs lowered decided,
  the same 28 undecided and one rejected statically as at Update 154.
- The static pass on `impl/selfhost/cbc/main.cb`: 180 MB and 30 s, as before the change (without the path cut
  it took 10 GB).
- `impl/selfhost/build.sh` (cbc) and `compare_c.sh` on the model sample, `impl/conformance/14-control-flow` and
  the showcases: every program's C the same under `cbc` and `cobc` (367 same, 0 differ); the new cases'
  certificates the same.
- The rest: `CHG-0398` §Verification.
