# D-0241 — The model checks what the text lets an implementation omit

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); evidence (D-0219), no normative change
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0219 (the model), D-0236, D-0235
Affects: `impl/model` (`statics.rs`, `machine.rs`, `explore.rs`, `main.rs`, `print.rs`, `enumerate.rs`, README,
`sample/`), `impl/battery.sh` stage 1, `impl/tools/suites.sh`, `spec/OVERVIEW.md` §11 (CHG-0291)

## Problem

The fourth review (F-05) observed that the model evaluates `clash` at every access, so it models a maximally
checking implementation. The discharge table's third outcome, *proven* — "no runtime check" — was invisible to
it, and so was F-01, a proof the state refutes: the model and both tools faulted where the text let an
implementation go on. Its alphabet also had no `swap`, the shape of F-02.

## Options

(i) a mode that records, from the static half, each statement's bindings whose accesses the table proves, and
reports an access among them that `clash` refutes as a distinct outcome; (ii) a mode that actually omits those
checks and compares outputs; (iii) leave the model as it is. (ii) can only show a false proof when the
omission changes the output; (i) shows every one.

## Decision

(i): `cobmodel check --as-licensed`, outcome `model.proven-but-false`, exit status 1 when any program has one;
`--pre-d0236` reads the transfer table without D-0236's forgetting, as evidence for the finding. The static
half implements D-0236; the machine implements D-0237's take-over rule; `swap` and `replace` (with D-0235's
premises) and a thread helper `swap_first` join the fragment and the alphabet. Battery stage 1 and
`suites.sh` run the licensed check over the sample after the comparison.

## Results

- Over the 261 sample programs: `--as-licensed` reports none; `--as-licensed --pre-d0236` reports eleven: nine
  enumerated three-statement programs that were in the sample before this round, so the model's alphabet could
  reach F-01 once it could see a proof, and the two new hand-written ones. The review's Programs A and C, the block-exit variant, `replace`, a plain swap and the
  thread-swap are six new sample programs (`hand-r01`..`hand-r06`); `cobmodel compare` over them shows no tool
  outcome outside the model's set.
- The enumeration (size 3, and 2,000 random six-statement programs) under `--as-licensed`: see CHG-0291.
- Still not modelled: closures, slices, a handle in a container (README).
