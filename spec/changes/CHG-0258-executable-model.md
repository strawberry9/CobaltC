# CHG-0258 — The executable model; six clarifications it found; the *modelled* evidence word

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-12; D-0219)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0219
Affects: `spec/05` 1.8.0, `spec/14` 1.25.0, `spec/16` 1.24.0, `spec/19` 1.15.0, `spec/20` 1.17.0,
`spec/conformance.md` 3.213.0, `spec/README.md`, `spec/02-schema.md` 1.0.77; `impl/model/` (new), `impl/Cargo.toml`
(the workspace member), `impl/battery.sh`, `impl/tools/suites.sh`, `impl/tools/manifest.py`; thirteen new cases under
`impl/conformance/`; the guide's "The guarantee"

## What changed

- `spec/16` `[Field-Access]`: a projection exists for the one access made through it. `[Index-Vec]`: the written
  place covers `*e[k]` written and `&mut *e[k]`.
- `spec/05` `[Object-End]`: `Dead` leaves out the references a block's exit spares; `spec/14` `[Block-Exit]`: those
  are its result's.
- `spec/14` `[Ref-Binding-Last-Use]`: a reference passed whole to `spawn` is borrowed through; §6: the row for a
  binding ended at its last use.
- `spec/19` `[Recv]`: the received value's unheld paths are re-stamped into the receiver's statement.
- `spec/20` `[Copy-Raw]`: a copied reference is held by a reclaimed object over its new cells.
- `spec/conformance.md`: `conf.projection-one-access`, `conf.returned-reference-parameter`,
  `conf.ref-lent-to-thread-lives-to-join`, `conf.channel-lent-ref-received`, `conf.vec-of-refs-grown`,
  `conf.vec-of-exclusive-refs-write-element`.
- `spec/README.md`: the evidence word *modelled*.
- Tools: `impl/model` (`cobmodel`); battery stage 1 and `impl/tools/suites.sh` compare `impl/model/sample` with both
  implementations; `impl/tools/manifest.py` adds *modelled* to the tier when that run's comparison was clean.
- Implementations (D-0219 "Tool defects found"): `cobc` calls an `__arrive` twin of a function's dispatcher, which
  tests each reference argument clash-free as it arrives (`cb_direct_ok`, `cb_paths_arrive`), where an argument is
  an existing reference passed on; the static pass refutes a conflicting read through `*r` as it did a write; both
  tools look for a deadlock when a thread ends, and let `[Deadlock]` mark the main thread before
  `[Channel-Deadlock]` (a lone main thread's channel wait stays `[Channel-Deadlock]`); `coby` reborrows an exclusive
  reference for a shared parameter at the call (D-0049) and locates a fault of a block's exit. Cases `conf.reference-passed-on-while-reborrowed`,
  `conf.read-through-reborrowed-reference`, `conf.deadlock-after-last-thread-ends`,
  `conf.deadlock-main-end-of-chain`, `conf.deadlock-at-block-exit-located`,
  `conf.shared-param-given-reborrowed-exclusive`.

## What changed for programs

- Under `cobc`, a call that passed an existing exclusive reference on whole while a reborrow of it lived, and
  wrote through it, now faults `diag.aliasing-conflict` as under `coby` and the rules: a soundness repair (the
  write went past a live shared reference).
- Under both tools, a read through `*r` while an exclusive reborrow of `*r` lives is rejected statically, as the
  flow analysis always required; before, it faulted when it ran, and was accepted when it never ran.
- A program in which every thread came to wait only when a thread ended reported `diag.deadlock` where it could
  hang; one whose main thread was an end of a chain of waits reports `diag.deadlock` where it could report
  `diag.channel-deadlock`; a fault of a block's exit under `coby` has its location; under `coby`, a conflict with a
  shared parameter's reborrow faults at the call, as under `cobc`.
- Nothing else: the six clarifications state what both implementations already did, and each of their cases
  passed under both before the text changed. Findings 7 to 10 of D-0219 are not changed here (since decided: D-0221,
  D-0222).
