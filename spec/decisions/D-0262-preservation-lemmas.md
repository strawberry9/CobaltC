# D-0262 — Preservation lemmas: the meta-theory, rule by rule, kept by speccheck

Status: ACCEPTED (2026-10-17, the owner: "go ahead with the verification plan's remaining steps … autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20, §24
Depends on: D-0003, D-0004, D-0005, D-0018, D-0019, D-0107, D-0206, D-0207, D-0261
Affects: `spec/23-preservation.md` (new); `spec/02` §5 (1.0.107); `spec/03` (1.6.2);
`impl/tools/speccheck.py`; `CHG-0326`

## Problem

`spec/03` states each invariant with coarse lifecycle fields (establishment, preservation, invalidation),
and the audits (`spec/AUDIT.md`, `AUDIT-2.md`) derived the examples by hand. No artifact says, for a given
rule, why the state it produces still satisfies each invariant, so a rule change could break preservation
with nothing in the corpus to fail. The plan toward proofs (`private/verification-plan-2026-10-17.md`,
A1) asked for the written meta-theory first: cheap, readable, and the place where the next review finds
a hole; the mechanised fragment (A3) comes after it has been read for a while.

## Options

- **A. Proof sketches in prose beside each rule.** Scattered; nothing checks that every rule has one.
- **B. One artifact of lemma tables per invariant over the rules that write the components the invariant
  ranges over, with the writer set computed from the rules' text by `speccheck`.** The frame case (a rule
  that writes none of the components preserves the invariant trivially) covers the rest, and an unlisted
  writer fails the corpus check. **Selected.**
- **C. A mechanised proof now.** Months; and it needs the statement B provides first.

## Decision

`spec/23-preservation.md`: for `inv.identity`, `inv.origin-stability` and `inv.resource-authority`, the
state lemma per writing rule; for the per-access invariants (`inv.spatial-validity`,
`inv.temporal-validity`, `inv.initialization-validity`, `inv.alias-validity`, `inv.concurrency-validity`,
`inv.trust-transition`), the auxiliary state lemma the access premise relies on (a valid path's object is
alive, `held-by` is the set it is defined to be, a locked mutex has its guard, …), proved per writing
rule, and the access lemma listing the rules that carry the premise; establishment in `Σ_0`. Each section
declares the components of `Σ` it ranges over; `speccheck` reads every rule block of `spec/05`–`21` for
the components it assigns (`Σ[ c := … ]`, `c(…) := …`, `c.f := …`) and reports an unlisted writer as an
error and a listed rule that writes none of the components as a warning. The artifact is non-normative:
it proves what `spec/03` states; where the two disagree, the rule and `spec/03` are the authority and the
lemma is wrong.

## Results

53 of the 675 rule blocks write a component; the tables name 198 rule entries over them, and `speccheck`
reports no unlisted writer. Writing the lemmas settled two points the earlier verify work had met: a state may
hold two conflicting paths (a call's pending then bound arguments, D-0107), so `inv.alias-validity` is
preserved as an access property with an auxiliary lemma on ancestors; and `[Relocate-Out]` leaves an
`uninit` sub-range inside a `valid` object, the one exception to the initialization state lemma, which
the rules that follow it (`[Match]`'s consume, the binding's end) never read.

## Revisit conditions

The Lean 4 mechanisation of the model's fragment (A3), which would supersede the prose rows it covers; a
reviewer's counterexample to any row, which is a defect of the rule or of `spec/03`, recorded as such.
