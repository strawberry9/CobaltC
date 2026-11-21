# CHG-0300 — The spawn row's tracked binding is a `ref`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0247)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0247
Affects: `spec/14` 1.28.0 (§6, the spawn row)

## What changed

The row says that a slice binding passed whole to `spawn` forms no `deriv(h, *s, m)` fact.

## What changed for programs

Nothing in either tool; the text no longer reads as keeping such a slice binding alive to its block's exit.
