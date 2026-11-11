# CHG-0242 — The schema's Rule entry carries a `disposition` only where CFN gives it one

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (4))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206
Affects: `spec/02` 1.0.68

## What changed

- §2, Rule: "with a `disposition` tag" became "carrying a `disposition` tag where it has an
  invariant-reliant precondition and none otherwise (`spec/01` §5), and an `outcome` tag where its result
  is not determined by its premises".
- §5's "in use" ranges: D-0001 through D-0206, CHG-0001 through CHG-0244.

## What did not change

No rule gained or lost a tag.
