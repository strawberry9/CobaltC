# CHG-0328 — Trace events and their comparison

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0264
Affects: `impl/src/interp.rs`, `impl/src/interp/destroy.rs`, `impl/src/lib.rs`, `impl/cbrt/src/lib.rs`,
`impl/tools/trace_compare.py`, `impl/battery.sh`, `README.md`

## What changed

Nothing normative. `COBALTC_TRACE_EVENTS=1` makes both tools write their destructor, fault, exit and end
events; `impl/tools/trace_compare.py` compares the sequences over the single-threaded conformance cases,
in the battery.

## What changed for programs

Nothing: the events are written only under the variable, on standard error.

## Found by

D-0264.
