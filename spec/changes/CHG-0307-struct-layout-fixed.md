# CHG-0307 — `[Layout-Struct]` fixes the offsets

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0250
Affects: `spec/16` (1.30.0), `spec/IMPLEMENTATION-NOTES.md` §1, `spec/conformance.md`; two conformance cases

## What changed

`[Layout-Struct]` gives each offset (C's rule) instead of admitting any assignment that meets its
side-conditions; "struct field offsets" leaves the implementation-defined choices.

## What changed for programs

Nothing: both tools already placed fields so. The layout is now a guarantee a program's C code can rely on.
