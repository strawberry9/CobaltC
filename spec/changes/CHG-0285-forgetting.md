# CHG-0285 — The flow analysis's forgotten facts escape their root

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0236)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0236
Affects: `spec/14` (1.27.0), `spec/OVERVIEW.md`; `impl/model`; `spec/conformance.md`

## What changed

- `spec/14` §6: `escaped(x)` also records a forgotten fact; the rows for block exit, `[Ref-Binding-Last-Use]`,
  `r = &_m x.π`, `r = e` and a moved reference binding forget; the paragraph "Forgetting". §1: the prose after
  `[Ref-Binding-Last-Use]`.
- Cases: `conf.ref-copied-into-struct-then-binding-ended`, `conf.ref-copied-into-vec-then-block-exit`,
  `conf.ref-copied-into-struct-then-reassigned`.

## What changed for programs

Nothing: `escaped` decides only between *proven* and *unknown*, and both implementations already checked every
access the change makes unknown.
