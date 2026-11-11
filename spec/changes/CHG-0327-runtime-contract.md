# CHG-0327 — The runtime's contract table

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0263
Affects: `spec/IMPLEMENTATION-NOTES.md` §6; `impl/cbrt/include/cbrt.h`; `impl/tools/runtime_contract.py`;
`impl/battery.sh`

## What changed

Nothing normative. `cbrt.h`'s declarations state the rule steps each entry point performs and what it
assumes; `impl/tools/runtime_contract.py` renders them as §6 of the implementation notes and checks them
in the battery.

## What changed for programs

Nothing.

## Found by

D-0263.
