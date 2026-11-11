# CHG-0366 — `coby` compares a `String` element reached through a generic call's reference result where it is

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0108 (`[Cmp-Text]`), CHG-0363
Affects: `spec/conformance.md` (one row); `impl/src/typecheck.rs` (`static_place_type`),
`impl/selfhost/front/typecheck.cb` (the same)

## What changed

No rule changed. `[Cmp-Text]` reads each `String` operand where it is. `coby`'s checker takes an operand for a
place by its static place type, and a generic call's reference result (`Box::get(p)`) kept its declared
`ref<T, m>`, so a field or element through it (`Box::get(p).segs[0]`) had no place type: the comparison read
it as a value, and the checker rejected it `diag.move-out-of-field`. The call's type arguments are now read
off its arguments' shapes, as for its other uses. The new row `conf.text-compare-element-through-reference-result`.

## What changed for programs

A program the checker rejected there is accepted and compares the text, under both tools.

## Found by

Writing D-0275's port in the CobaltC front end (`Box::get(p).segs[0] == *m`), 2026-10-21.
