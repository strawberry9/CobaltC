# CHG-0111 — What follows a false trusted claim

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §12, §20
Depends on: D-0096
Affects: `spec/01` §5, rule.trust.unsafe, term.conformance, the guide

## Problem / motivation

No text said what follows when a `discharge: trusted` side-condition is
false, so conformance was undefined for such an execution (D-0096).

## Decision

D-0096, option B.

## What changed

- **`spec/01` 1.1.0:** §5's `trusted-unchecked` states that the rules
  describe executions whose trusted side-conditions hold, and constrain
  nothing from a step whose side-condition is false; explicitly not a
  fourth `outcome` value.
- **`spec/20` 1.10.0:** `rule.trust.unsafe` gains
  `[Trusted-Claim-False]` and the informative encouragement to report a
  detected false claim.
- **`spec/00` 1.1.0:** `term.conformance` admits every behavior from
  such a step on.
- **Guide:** §20 gains "When a promise is false" (an unexecuted
  fragment illustrates it: its behavior is unconstrained by the rule
  it shows).
- **Implementations:** none.

## Compatibility classification

Clarifying: no execution the rules described changes.

## Conformance changes

None (a case cannot pin unconstrained behavior).

## Revisit conditions

D-0096's.
