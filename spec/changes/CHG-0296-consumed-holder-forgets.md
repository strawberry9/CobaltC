# CHG-0296 — A consumed binding's own facts are forgotten

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0244)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0244
Affects: `spec/14` 1.28.0 (§6 consumption row, "Forgetting"); one conformance case

## What changed

The consumption row also sets `deriv(x, ·, ·) := F`, forgetting them (their roots become escaped).

## What changed for programs

An access to what a moved handle's thread was lent, in the block after the move, is checked at run time, as
both tools always checked it; the text had rejected it statically.
