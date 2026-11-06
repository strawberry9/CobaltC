# CHG-0244 — The flow analysis defines the accepted set; further analyses only discharge checks; `coby --check` is the reference

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (6))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0110, D-0188, D-0203, D-0206
Affects: `spec/14` 1.23.0, `spec/IMPLEMENTATION-NOTES.md` §4

## What changed

- `rule.control.flow-analysis`: "This analysis defines the language; any other only optimizes": its facts,
  lattice, CFG and transfer functions fix the accepted set; a further analysis may discharge a check
  statically and leave it out where it cannot fail, but may not accept a program the rule rejects or reject
  one it accepts; where two static diagnostics apply, the first in source order is reported (D-0110).
- `spec/IMPLEMENTATION-NOTES.md` §4: `coby --check` accepts exactly the programs the static rules accept, so
  a third implementation is measured against it on any program; it may discharge more checks, not accept or
  reject differently.

## What did not change

The analysis, every transfer function and every diagnostic. `cobc`'s dual bodies (D-0188), frozen
parameters (D-0205) and confinement (D-0189–D-0192) are the further analyses the sentence permits.
