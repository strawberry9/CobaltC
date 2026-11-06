# D-0265 — The mechanised fragment: a Lean 4 development of the alias and temporal lemmas

Status: ACCEPTED (2026-10-17, the owner: "go ahead with the verification plan's remaining steps … autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20, §24
Depends on: D-0004, D-0018, D-0219, D-0262
Affects: `proofs/` (new, outside the Rust workspace); `README.md`; `CHG-0329`

## Problem

`spec/23` argues preservation in prose; the model checker (D-0219) runs a fragment's rules and
enumerates outcomes; neither is a proof a machine has checked. The plan toward proofs (A3) asked for the
fragment's machine to be stated in a proof assistant and its invariants proved by induction on steps —
the item comparable to what Rust has for its core (RustBelt) — as a long track started only once the
written meta-theory existed.

## Options

- **A. Lean 4, core library only, a `proofs/` directory beside `impl/`.** One toolchain download (`elan`);
  no Mathlib, so the development builds in seconds; not a build dependency of the tools; checked by
  whoever changes it. **Selected.**
- **B. Coq (RustBelt's tool).** Iris would be the right library for the full language's separation-logic
  argument; for the fragment's first-order state lemmas it is weight without gain, and the owner's
  machine has neither.
- **C. Encode the lemmas in the model checker's Rust and trust its exhaustive runs.** Not a proof: a bounded
  enumeration.

## Decision

`proofs/`: `Cobalt/State.lean` states `σ` for the fragment (objects, access paths with mode, validity, base
and holders, the current frame and scope, a fresh counter) with `spec/04` §2's predicates (`alive`,
`temporally-valid`, `overlap`, `permitted`, `ancestors`, `clash`); `Cobalt/Step.lean` the step relation
(`[Object-Establish-*]`, `[Binding-Form]` with `[Store-Binding-Temp]`, `[Borrow]`, `[Field-Access]`,
`[Read]`, `[Write]`, `[Destroy]`, `[Object-End]`, `[Block-Enter]`/`[Block-Exit]`, `[Stmt-Enter]`/
`[Stmt-Exit]`), each constructor carrying its rule's premises; `Cobalt/Invariants.lean` `spec/23`'s
auxiliary lemmas (T1: a valid path's object is alive; T2: `held-by` agrees with the contents of the live
objects; A: a base is an earlier path; Fresh: every id is below the counter; R: an object under
obligation is alive and owned by an open frame or scope); `Cobalt/Preservation.lean` the theorem
`reachable σ → Invariants σ`, by cases on the step. `lake build` with no `sorry` is the artifact's
check; it is not part of the battery (Lean is not required to build the tools).

## Results

Proved, in three milestones on 2026-10-17: every step of the fragment preserves T1, T2 (as `spec/04`'s
`held-by` equality over object contents), A, Fresh and R (`inv.resource-authority` for a fragment without
sub-resources, raw storage or threads), so every reachable state satisfies them, and with them every
`read` and `write` the machine admits reaches a live object through a valid path and passes `¬clash` by
its own premise. The fragment's steps: establish, bind, borrow, project, read, write (through any path,
the slot's references replaced by the value's), destroy, plain end, block enter/exit over the frame
stack, statement enter/exit over the scope stack. About 700 lines of Lean, no axiom beyond the core
library, built by Lean 4.35 in under ten seconds.

Fourth milestone, 2026-10-18: threads. σ gains per-thread frame and scope stacks, each obligation's
authority thread, and each thread's result once done and its handle; the steps gain `[Authority-Transfer]`
(across threads too), `[Spawn]`, `[Thread-Body-Done]`, `[Join]` and `[Handle-Destructor]`, and every
step is performed by a running thread. (R) became `inv.resource-authority`'s full statement for the
fragment: exactly one thread holds the authority, and that thread owns the object by its frame, its
scope, or its finished result. The last case is the proof's finding: with the proposition's four cases
no state after `[Thread-Body-Done]` satisfied (R), because the result's objects have left the parameter
frame and the thread's scope stack is empty; the rules keep them (the handle claims or discards them),
so the proposition was short of a case, now its fifth (`CHG-0330`). About 1,450 lines of Lean.

Fifth milestone, 2026-10-18: locks. σ gains `sync` and a path its `lock` flag, an object its `guard`
flag; the steps gain `[Lock]` and `[Guard-Drop]`; the invariants gain `inv.concurrency-validity`'s
state lemma (C) in two directions, with the corollary that at most one lock path of a mutex is valid at a
time; then `[Thread-Lend]` and `[Thread-Affine]`'s taking, a path's thread becoming `transit` and then
the first accessing thread's, with the affinity premise on every access through an exclusive path (a lock
path is never taken, D-0240, which is what keeps (C)); then `[Fault-Contain]`: a contained thread's
mutexes poisoned, its lock paths and its objects ended in one unwind step, its result the failure, with
`contained` as the absence of a lent or taken-over exclusive path. About 2,370 lines.

Seventh milestone, 2026-10-18: pending paths and sub-resources, the two items left. A path gains
`pending` (D-0107, D-0242), which `clash` does not count; an argument's `[Borrow]` and every projection
form one, `[Temp-Root]` roots a temporary, `[Call]`'s `store(binding(p_i))` binds an argument's path to a
parameter object of the parameter frame (the path pending no longer, no `clash` check: D-0022), and a
projection's one access ends it; the invariant (P) says a pending path is held by no object, `write`'s
premise that a stored reference was bound or accessed. σ gains `subs`, the sub-obligations `(o, path)`
with their authority threads; `[Relocate-In]`, `[Relocate-Out]` and `[Destroy-Composite]` are steps, and
`endObjects` discharges a dead object's sub-obligations; the invariant (RS) is `inv.resource-authority`'s
third owner case for the fragment: a sub-obligation's container is itself under obligation (an
aggregate holding a resource is a resource, `spec/12` §3) with the same authority thread, so (R)'s owner
of the container is the sub-obligation's. Corollaries: `pending_unheld`, `bound_parameter_clashes`
(once bound, a conflicting parameter is a `clash` witness at first use) and `sub_obligation_owned`.
Then raw storage (`spec/20` §2), beyond what this decision named: an object's `reclaimed` kind, (R)'s
fourth owner case, and the steps `[Reclaim]` (fresh, or the re-attachment that takes the authority,
CHG-0030), `[Release]` (none of the ended objects foreign-held, D-0214), `[Rawptr-Move-In]` and
`[Rawptr-Move-Out]` (a whole object's references, obligation and sub-obligations to a fresh object of the
other storage kind); `[Join]` and `[Handle-Destructor]` carry the premise that the finished thread holds no
reclaimed obligation, which (R)'s one-thread statement needs once the thread's record is gone. About
3,600 lines; 35 of `spec/23`'s 70 rule labels are steps. The fragment now has every item this decision
named for it; `[Match]` is a composition of its steps; channels, atomics and foreign calls remain outside.

## Revisit conditions

The fragment grows with the model checker's (D-0219 and its extensions); a reviewer's counterexample to
a lemma, which the development would refuse to prove, is a defect of the rule or of `spec/23`.
