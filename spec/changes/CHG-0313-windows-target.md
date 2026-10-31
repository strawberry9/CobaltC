# CHG-0313 — `cobc --target`, the Windows target

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0255
Affects: `spec/IMPLEMENTATION-NOTES.md` §5; `impl/cobc`, `impl/cbrt`, `impl/src/fileio.rs`,
`impl/conformance/run.py` and one conformance case's header

## What changed

`cobc` compiles for `x86_64-pc-windows-gnu` as well as `x86_64-unknown-linux-gnu`, and builds as `cobc.exe`. The
implementation notes list the targets and the Windows calling convention. No rule changes: the targets share
every implementation-defined choice that the rules name.

## What changed for programs

None on Linux, except that a thread's over-aligned argument is now aligned (it was by luck). On Windows,
programs can be compiled at all.
