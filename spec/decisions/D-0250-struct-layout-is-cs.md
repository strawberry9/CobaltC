# D-0250 — A struct's layout is C's

Status: ACCEPTED (2026-10-16, the owner, asked which structs get the guaranteed layout: "Every struct")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0019, D-0146 (`unsafe extern fn`), `rule.agg.layout`, `rule.trust.extern-call`
Affects: `spec/16` §1 (1.30.0), `spec/IMPLEMENTATION-NOTES.md` §1, `spec/conformance.md`; `CHG-0307`

## Problem

`[Layout-Struct]` admitted any offsets that kept declaration order and natural alignment, with the least size,
and left the choice to the implementation. A program passing a struct to C through `rawptr<S>` (`stat`, a
device's descriptor, a network header) therefore relied on `cobc` happening to place fields where C does.

## Options

1. **Every struct.** `[Layout-Struct]` fixes each offset to the least aligned one after the previous field: C's
   rule. No new syntax; both tools already did it, so nothing changes for programs. Declaration order was
   already fixed, so the freedom given up was only where padding goes.
2. **Only an `extern struct`.** A new marker; other structs could later be reordered or packed. A token for a
   freedom the specification had already given up.

Selected: 1.

## Decision

`[Layout-Struct]`: `off(f1) = 0`, each later field at the least multiple of its alignment after the previous
field's end, the size the least multiple of the alignment after the last field. Where the platform's C ABI gives
each member type the size and alignment `spec/06` §7 gives it (x86-64 System V and Windows x64 do), the layout
is the C struct's with the same members in the same order. An enum's discriminant width stays the
implementation's documented choice.

## Results

`conf.struct-layout-offsets` checks sizes and alignments under both tools; `conf.struct-c-layout` (`cobc` only)
has C code read and write a CobaltC struct through a pointer by `offsetof`, and compares `sizeof`/`_Alignof`.
