# D-0283 — A conflict at a call that only values decide is checked when the call is made

Status: ACCEPTED (2026-10-26, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions"). D-0281 phase 2, step 4c's
open question.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281 (tier S and tier D), D-0282 (`rule.control.frame-references`, access summaries), D-0018
(conflicts checked at use), D-0049 (an exclusive argument for a shared parameter is reborrowed at the call),
D-0070 (slices), `[Swap-Places]`
Affects: `spec/14` §6 (1.31.0), `spec/conformance.md` (3.274.0), the shared front end (`impl/src/tier.rs`,
`impl/src/typecheck.rs`), `coby` (`impl/src/interp/point_checks.rs`, `calls.rs`, `access.rs`), `cobc`
(`lower/user_calls.rs`), `cbrt` (`cb_point_check`), the book's chapter 7, the guide's §08 and §14, `CHG-0374`

## Problem

D-0282 refutes before the run every conflict of frame-held references that the program text decides. Two
kinds it leaves to the run: a pair of places that meet only for some values (`set_and_read(&mut v[i], &v[j])`,
two computed ranges of one slice), and a pair that meets through a fact that holds on some paths only (a
reference that points at `x` or at `y` depending on a branch). Until now the run found these where the callee
first made the conflicting access, from the tracking state: inside the callee, `*a = 1` faulted.

D-0281's phase 2 (step 4c) lowers tier-S functions with no tracking state. Inside the callee its parameters
are then bare pointers, with no record of where they came from or what else is live: the callee can no longer
find the conflict. The only place left to find it, with no state kept, is the call, where both places are in
hand. So the language has to say where the fault is, and whether it depends on what the callee then does.

## Options

- **A. At the call, from the summary.** The call faults when an access its callee's summary declares through
  one argument would conflict with another argument or a live reference of the caller. The check is the
  comparison of the two places. The fault is located at the call, and comes whether or not the body would
  have made the access on this run.
- **B. At the use, as before.** Keep a checked twin of each callee, chosen at the call when the places
  overlap, that finds the conflict at its first access. Behaviour unchanged, but every function involved is
  compiled twice, and the twin needs tracking state an untracked program does not keep.
- **C. Before the run.** Reject the call unless the program shows the places apart (Rust's `split_at_mut`).
  It removes the run-time check, but it rejects correct programs CobaltC accepts today and changes the
  language's model.

## Decision

A. `rule.control.frame-references` (`spec/14` §6): a pair that may overlap, and a certain overlap through a
fact `?`, are checked when the call is made. Once the arguments are evaluated and bound (D-0049's reborrow
made), and before the body runs, each access the callee's summary declares through an argument such a pair
names is checked as an access through that argument's path at the call: the `¬clash` premise of `[Read]` or
`[Write]`, the argument's own path and its ancestors excluded, the other arguments counted. A clash faults,
`diag.aliasing-conflict`, at the call. Two exemptions follow from what the places are: a path whose range is
empty reaches nothing, and the two arguments of `swap`/`swap_places` are one place or disjoint
(`[Swap-Places]`). Conflicts the run finds otherwise (through tier-D references kept in data, through `fn`
values) stay at the access.

Reasons:

- It is the only option that meets D-0281's cost contract for a point check: one comparison at a known place,
  with no state.
- A fault at the call points at the expression that formed the overlapping pair, which is where the program
  is wrong.
- The outcome no longer depends on the path the callee's body takes: a change to the callee's body changes
  its callers' faults only through its summary, which is already part of what the call-site rule relies on.
- What it gives up is small: the callee's own first conflicting access was a later report of the same pair,
  and for an exclusive argument any access through the other one conflicts, so the callee faulted on nearly
  every path anyway (see Verification).

## Consequences

- Both tools record each point check in the shared static pass (the call, the argument, the projection, the
  mode). `coby` checks them after binding the parameters (`run_point_checks`); `cobc` emits
  `cb_point_check` before the call, which checks through the argument's path with the call's other argument
  paths counted as held. Untracked tier-S lowering (step 4c) will compare the two places directly.
- The message names the callee and the argument: "`set_and_read` may write through argument 1, and a live
  reference reaches the same place: the call is checked when it is made (D-0283)".
- Conformance: two cases (`conf.state-and-own-field-input-dynamic`, `conf.overlapping-slice-arguments-dynamic`)
  now fault at the call; two (`conf.vec-grow-fault-intact-dynamic`, `conf.fault-unwind-callee-tail-conflict-
  dynamic`) pass the shared reference in a struct (tier D), so they still test a fault raised inside the
  callee; four new rows: the eager fault, the distinct case, the self-swap and the empty view.
- The self-hosted toolchain, frozen at Update 135 (D-0278), still finds these conflicts at the callee's use
  until it is ported at the phase boundary.

## Verification

- A differential run of Update 137's `coby` against this one (`stress/d0281/cost/pointdiff.sh`) over 23,285
  programs: every hand-written program in the tree (conformance, showcase, examples, book, guide, the stress
  programs) and the generated programs (the model's, the plan and atomics corpora, the soundness and
  verification probes) that a text filter left as able to form such a pair. 62 have a point check at a call.
  Three change outcome, each in the fault's location and message only (the book's and the guide's examples of
  two computed indexes, `conf.state-and-own-field-input-dynamic`): the same exit status, the same output. No
  program that ran to completion faults now, and none of the 15 generated programs the model gives an outcome
  for changes.
- The first version faulted on two forms Update 137 accepts, found by that run and by probes written for this
  decision: an empty view of a string appended to it (six programs in the corpus) and `swap(&mut v[i], &mut
  v[j])` with `i == j`. Both are exempt as stated above, with conformance rows of their own.
- The changed and new conformance cases under `coby` and `cobc` with GCC and Clang: the same output, location
  and message in all three.
