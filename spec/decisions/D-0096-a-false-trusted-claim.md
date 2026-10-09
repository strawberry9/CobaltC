# D-0096 — What follows a false trusted claim

Status: ACCEPTED (2026-09-29, owner: "yes, D-0096 with option B")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §13, §20
Depends on: D-0017 (`unsafe`), D-0030 (foreign code)
Affects: `spec/01` §5, `spec/20` `rule.trust.unsafe`, `spec/00` `term.conformance`, the guide

## Problem

Every `trusted-unchecked` rule (`spec/20`: raw reads and writes,
`reclaim`, `release`, foreign calls, …) relies on a
`discharge: trusted` side-condition, and `rule.trust.unsafe` makes the
`unsafe` block "the record of what the author asserts". Nothing said
what follows when an assertion is **false**. `term.conformance` admits
the behaviors "the rules admit", which for such an execution was
undefined — the Master Instructions §20 incompleteness test. It was met
in the wild: `stress/round5/d0095/raw_empty_then_assign.cb` moves a
field out of a live local through a raw pointer (breaking
`[Rawptr-Move-Out]`'s side-condition); coby panics, cobc faults, and
no text said whether either, both or neither conforms.

## Candidate mechanisms

1. **The whole execution is unconstrained** (C's undefined behavior):
   the rules constrain nothing about any execution that ever reaches a
   false claim, including what it did before. Lets an implementation
   assume the claim true everywhere and "time-travel"; CobaltC has no
   optimizer that needs this, and it makes a program's already-produced
   output unaccountable. Rejected.
2. **Unconstrained from the step that relies on the false claim.** The
   execution up to that step is as the rules say; from it on, no rule
   constrains the observable behavior. Matches the operational
   semantics exactly: that step's conclusion is derived from a false
   premise, and every later step from the state it produced. Precise.
   Selected.
3. **Unconstrained only in what depends on the false claim.**
   Attractive, but not specifiable: a false claim about storage can
   reach any later state (a wrong length indexes anywhere, a double
   obligation destroys twice), so "depends" has no precise boundary —
   fails Master Instructions §8 item 5 (formal precision). Rejected.

## Selected design

- The rules describe the executions in which every `discharge: trusted`
  side-condition holds where it is relied on.
- `[Trusted-Claim-False]` (`spec/20`): an execution that reaches a
  step whose trusted side-condition is false is, before that step, as
  the rules say; from that step on, no rule constrains its observable
  behavior — continuing, wrong output, a fault, a crash are all
  admitted.
- `term.conformance` says so: for such an execution the rules admit
  every behavior from that step on.
- **Not an `outcome`.** `spec/01` §5 closes the `outcome` set and has
  "no fourth outcome value for unbounded or undeclared variation". This
  decision adds none: it bounds the rules' *domain* — the executions
  they describe — and admits no set of results at all. The
  `trusted-unchecked` rules stay `ACCEPTED`; their side-conditions are
  premises the author supplies.
- *(Informative)* An implementation that detects a false claim is
  encouraged to stop with a diagnostic rather than crash. Optional and
  not a conformance matter.

## Consequences

- Every guarantee — each proposition of `spec/03`, the safe-program
  guarantee (Master Instructions §12) — holds for executions whose
  trusted claims are true, and unconditionally for a safe program
  (`term.safe-program`), which makes none.
- coby's panic and cobc's fault on the probe above are both
  conforming. Whether coby should fault instead is an
  implementation-quality question (the informative note), open in
  `stress/round5/BUGS-FOUND.md`.
- No conformance case: a case cannot pin "anything". No implementation
  change.

## Compatibility impact

None: no execution the rules previously described changes. The text
states what was implicit — and gives implementations and readers a
rule to point at.

## Revisit conditions

If a trusted operation's side-condition becomes checkable (for
example, `extern` pointer extents from a future FFI contract), that
check moves the operation to `checked`, and this rule no longer covers
it.
