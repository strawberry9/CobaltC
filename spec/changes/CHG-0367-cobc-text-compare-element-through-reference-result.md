# CHG-0367 — `cobc` compares a `String` element reached through a call's reference result where it stands

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0108 (`[Cmp-Text]`), CHG-0363, CHG-0366
Affects: `impl/cobc/src/lower/calls.rs` (`lower_borrowed_operand`); `impl/selfhost/back/gen.cb` (the same)

## What changed

No rule changed. CHG-0363 made a field reached through a call's reference result (`Box::get(a).name`) a place
of a comparison; an element of such a field (`Box::get(p).segs[0]`) was still lowered as a value, a move out
of the element, and the program faulted `diag.move-while-aliased` where `coby` compared the text. The walk to
the operand's root now passes through indexing as well as fields. CHG-0366's row,
`conf.text-compare-element-through-reference-result`, covers it under both tools.

## What changed for programs

A program that faulted there under `cobc` runs as under `coby`.

## Found by

The code generator's port (D-0274), whose compiled checker and code generator faulted on the pattern in
their own source, 2026-10-21.
