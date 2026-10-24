# D-0273 — `x?` moves a binding that holds a resource

Status: ACCEPTED (2026-10-20, the owner chose option 2: "go with B")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0009 (two failure channels), D-0049 (a match by value consumes what an arm moves out),
D-0060 (`?` ends a temporary operand), D-0130 (`?` on an `Option`)
Affects: `spec/18` §2 (`rule.fail.propagate`); `spec/conformance.md`; `impl/src/typecheck.rs`;
`impl/selfhost/front/typecheck.cb`; `CHG-0360`

## Problem

`spec/18` defines `e?` as `match (e) { Ok(v) : v, Err(err) : return Err(err) }`, then adds that a
`Result` which is a place (`x?`) "is read as before and stays". Reading a binding that holds a resource
would copy it, and a resource is never copied, so `x?` on a `Result<String, String>` binding was
rejected (`diag.read-of-resource`), while the `match` the rule expands to was accepted and moved the
payload out. A result kept in a binding first (to check something else before propagating it) could not
use `?` at all: the type checker written in CobaltC needed this a dozen times and wrote `own(x)?`, an
identity function that makes the value a temporary.

## Options

1. Keep the rule; the diagnostic names the `match` and applying `?` where the value is made.
2. `x?` moves `x` when `x` holds a resource: the binding is used by value, as an argument is, and is
   consumed whichever variant it holds. A binding that holds no resource is read and stays, as now.
3. As 2, but consume `x` only when the moved-out part is a resource, as a written `match` does: with
   `Result<i32, String>` holding `Ok`, `x` would stay valid.

## Decision

Option 2 (the owner's choice). It is the language's rule for a whole binding used by value (`f(x)`), so
it adds nothing to learn, and `?` is accepted wherever the `match` it stands for is. Option 3 follows the
`match` more closely but makes whether `x` is still usable depend on which variant it held, which a
reader cannot see at the `?`. A field or element holding a resource stays rejected, as moving one out is.

## Compatibility impact

A program `diag.read-of-resource` rejected at `x?` is accepted. Nothing accepted before changes meaning:
`x?` on a binding holding no resource is read as before. Both runtimes already moved such a binding's
payload out (`coby` takes a whole resource place by value at `?`; `cobc` lowers `?` to the `match`), so
the change is the static rule alone.

## Results

`conf.propagate-moves-resource-binding` (a `Result` of a `String`, an `Option` of a `Vec`, a resource
error, and a plain binding used twice) and `conf.propagate-moved-binding-reused` (a second `r?` after the
first moved `r`).

## Revisit conditions

None foreseen.
