# CHG-0360 — `x?` moves a binding that holds a resource

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0273, D-0060, D-0130
Affects: `spec/18-error-failure-semantics.md` (§2, 1.9.0); `spec/conformance.md` (two rows, 3.263.0);
`impl/src/typecheck.rs` (`Propagate`); `impl/selfhost/front/typecheck.cb` (`check_propagate`)

## What changed

D-0273. Spec/18 §2 said that a `Result` that is a place (`x?`) is read and stays. For a binding whose type
holds a resource (`Result<String, E>`, `Option<Vec<T>>`, a `Result` whose error is a `String`) that rule
refused the program, since a resource is never copied out of a place: the payload had to be moved out
through a temporary (`own(x)?`). Now such a binding is used by value, as an argument is: `x?` moves it,
and it is consumed whichever variant it holds; a later use is a use after a move (`diag.stale-binding`).
A binding that holds no resource is read and stays, as before; a field or element holding a resource is
still not moved out (`diag.move-out-of-field`).

Only the static checker changed. Both runtimes already moved the binding: `coby` takes the `Result` by
value at `?`, and `cobc` lowers `?` to the `match` that [Propagate] gives.

## What changed for programs

A program that wrote `x?` on such a binding was rejected and is now accepted. No accepted program changes
meaning, except that a binding used after its `x?` is now rejected where its type holds a resource; such a
program was rejected before as well, at the `x?`.

## Found by

The CobaltC type checker (`impl/selfhost/front/typecheck.cb`), which needed `own(r)?` at a dozen sites;
2026-10-20.
