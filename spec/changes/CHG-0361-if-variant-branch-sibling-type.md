# CHG-0361 — `cobc` types `if (c) { None } else { e }` from its `else` branch

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0049
Affects: `spec/conformance.md` (one row, 3.264.0); `impl/cobc/src/lower/matching.rs` (`lower_if`);
`impl/cobc/src/lower/values.rs` (`is_untyped_variant_block`)

## What changed

No rule changed. With no expected type (a `match` scrutinee, an `auto` binding's value, a generic call's
argument), an `if` whose `then` branch is a payloadless variant of a generic enum, `{ None }`, has the type of
its `else` branch: the static pass gives it that type, and `coby` ran such programs. `cobc` lowered the `then`
branch first, with nothing to fix `Option`'s type argument, and stopped with an internal error ("cannot infer
type arguments of `std::Option`"). It now takes the `else` branch's type first in that case, as it already did
for a literal `then` branch (D-0049). The new row `conf.if-variant-branch-takes-sibling-type` covers the three
contexts.

## What changed for programs

A program `cobc` refused with that internal error compiles and runs as under `coby`.

## Found by

Speeding up the CobaltC type checker (`impl/selfhost/front/typecheck.cb`): a cache lookup written
`match (if (key == 0) { None } else { HashMap::swap_remove(&mut m, &key) })`; 2026-10-20.
