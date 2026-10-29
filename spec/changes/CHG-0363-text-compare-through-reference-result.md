# CHG-0363 — `cobc` compares a `String` field reached through a call's reference result where it stands

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0108 (`[Cmp-Text]`), D-0271 (`[Cmp-Key]`)
Affects: `spec/conformance.md` (one row); `impl/cobc/src/lower/calls.rs` (`lower_borrowed_operand`)

## What changed

No rule changed. `[Cmp-Text]` reads each `String` operand where it is, neither moved. `cobc` read an operand as a
place only when it was written as one (a binding, a field or element of one, a dereference); a field reached
through a call's reference result (`Box::get(a).name == Box::get(b).name`) was lowered as a value, and reading
a `String` field as a value is a move out of the field, so the program faulted `diag.move-out-of-field` where
`coby` compared the text. Such an operand is now a place through the returned reference. The new row
`conf.text-compare-through-reference-result` covers `==` and `<`.

## What changed for programs

A program that faulted there under `cobc` runs as under `coby`.

## Found by

Porting the code generator to CobaltC (D-0274): the port's own `unify` compared two type names through
`Box::get`, and the compiled code generator faulted on its first run; 2026-10-21.
