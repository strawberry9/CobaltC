# D-0313 — The proofs' assurance: each result's axioms checked, two more kernels, the fragment widened

Status: ACCEPTED (2026-11-07; the owner approved the proofs-assurance proposal of 2026-11-07, and delegated the
widening of the fragment to channels, atomics and `[Match]` with the rest of the agenda after Update 170 "and you
make the decisions for all of this"); implemented by `CHG-0414`
Kind: Decision (`spec/02-schema.md`); assurance, non-normative (no rule, invariant, type or state entity changes)
Governed by: `CobaltC_Master_Instructions.md` §12, §20
Depends on: D-0265 (the mechanised fragment), D-0270 (the elision lemmas, the executable machine, the replay),
D-0299 (`spec/ASSURANCE.md`), D-0300 (the statics core), D-0253 (`wake-count`), D-0219 (channels)
Affects: `proofs/` (`Statements.lean`, `State.lean`, `Step.lean`, `Preservation.lean`, `Elision.lean`, the
lakefile, `README.md`, `COVERAGE.md`); `impl/tools/proofcover.py`; `impl/tools/gate.py` (`proofs-kernels`);
`spec/ASSURANCE.md`

## 1. What was missing

`spec/ASSURANCE.md` calls the fragment's results *proof*: "a theorem checked by a proof assistant, `lake build`
with no `sorry`". Three things that claim rests on were not checked by anything:

1. **What each theorem rests on.** "No `sorry`" was a grep of the build's log for Lean's warning. A result that
   came to rest on a declared `axiom`, or on a `sorry` in a lemma that warned once and was cached, would not be
   seen. Lean says what a theorem rests on (`#print axioms`); nothing asked it.
2. **Which kernel said yes.** `lake build` runs one kernel, Lean's own, as the elaborator calls it. Lean's
   `leanchecker` replays the compiled files through the kernel again, and nanoda is a second kernel written
   independently (in Rust) from the export format. A defect in one kernel, or in what the elaborator hands it,
   is unlikely to be the same defect in another.
3. **What a reader must compare with the text.** The results are spread over five files, their statements
   written for the proofs. A reader checking that they say what `spec/23` means had to find them.

And four of `spec/23`'s rules were outside the fragment for no reason that still held: `[Match]` (argued as a
composition of steps, though its test of the scrutinee is an access of its own), `[Atomic-Op]` and
`[Atomic-Notify]` (atomics write `sync` as a count, never a lock state, so they cannot break (C1) or (C2)), and
`[Recv]` (the value leaves the channel by `[Relocate-Out]`, a step already, and its paths, lent into the channel,
are of no thread).

## 2. Decision

1. **`proofs/Cobalt/Statements.lean`** restates each of the 18 results with its full statement, as an `example`
   the build checks against the theorem, and checks the axioms each rests on under `#guard_msgs` against
   `#print axioms`. The build fails if a result comes to rest on any axiom but those listed for it. Every one
   rests on Lean's own three at most (`propext`, `Classical.choice`, `Quot.sound`; three rest on none).
2. **`proofcover.py`** reads those blocks and writes their table into `proofs/COVERAGE.md`; `--check` fails on any
   axiom but Lean's three, on a block whose name and expectation disagree, and on a `#print axioms` no
   `#guard_msgs` checks.
3. **The release unit `proofs-kernels`** builds the development, runs `leanchecker` over every root module, and
   runs nanoda over `leanexport`'s export with Lean's three axioms and `Lean.trustCompiler` permitted. nanoda is
   not told to refuse an unpermitted axiom, because the export declares core's `sorryAx` whether or not anything
   uses it; which axioms each result rests on is item 1's check. The unit is green only on nanoda's "no
   typechecker errors".
4. **Four steps more.** `SyncState` gains `wake n`; `Step` gains `atomicOp` and `matchOn` (with `[Read]`'s
   premises, changing nothing), `atomicNotify` (`sync(o) := wake n` where `o` is unlocked or already a count) and
   `recv` (the received value's unheld paths re-stamped into the receiver's current scope). Every invariant is
   preserved by each, and the elision lemmas hold of each. 38 of the 53 rule rows `proofcover.py` counts are steps
   (34 before); of the 70 labels `spec/23` names, 39 (35).

## 3. Not decided

- **The executable machine** (`Exec.lean`'s `Act`) and so the replays do not take the four steps: the model
  checker's traces say where they leave the fragment (`out channels`, `out atomics`), and the trace format has no
  action for them. Adding them is the model's trace work as well as the machine's; nothing here depends on it.
- **The channel's queue** is not state of the fragment: `[Recv]` is the arrival of what `[Relocate-Out]` moved out
  of it. Foreign calls, cells and addresses, and the constructions that only move references between slots stay
  outside, as `proofs/README.md` says.
