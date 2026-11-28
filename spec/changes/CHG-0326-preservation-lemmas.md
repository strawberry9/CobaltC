# CHG-0326 — `spec/23`: the preservation lemmas

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0262
Affects: `spec/23-preservation.md` (new, 1.0.0); `spec/02` §5 (1.0.107); `spec/03` (1.6.2);
`impl/tools/speccheck.py`

## What changed

A new non-normative artifact, `spec/23-preservation.md`, proves per rule that the state-writing rules of
`spec/05`–`21` preserve `spec/03`'s invariants, and `speccheck` keeps its tables complete against the
rules' text. `spec/02` §5 lists it; `spec/03`'s purpose names it. No rule changes.

## What changed for programs

Nothing.

## Found by

D-0262.
