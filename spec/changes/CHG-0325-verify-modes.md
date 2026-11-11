# CHG-0325 — The verify modes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0261
Affects: `spec/IMPLEMENTATION-NOTES.md` §5; `impl/src/interp/verify.rs`, `impl/cbrt/src/lib.rs`,
`impl/battery.sh`, `README.md`

## What changed

Nothing normative. §5 of the implementation notes lists `COBALTC_VERIFY=N`, under which `coby` re-derives
its holder index from the whole state at alias checks and checks locks and ownership at statements' ends,
and `cbrt` re-derives the agreement of its path and object tables; the battery runs both conformance
suites under it.

## What changed for programs

Nothing: the modes change no program's behaviour.

## Found by

D-0261.
