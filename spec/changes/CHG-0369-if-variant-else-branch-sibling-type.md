# CHG-0369 — `if (c) { e } else { None }` has the `then` branch's type

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-23)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0049, CHG-0361
Affects: `spec/conformance.md` (one row, 3.269.0); `impl/src/typecheck.rs` (`ExprKind::If`);
`impl/selfhost/front/typecheck.cb` (`check_if`)

## What changed

No rule changed. With no expected type (a `match` scrutinee, an `auto` binding's value, a generic call's
argument), an `if` whose `else` branch is a payloadless variant of a generic enum, `{ None }`, has the type of
its `then` branch: the static pass checks the `else` branch against that type, but the branch itself came out
untyped, and so did the whole `if`. A `match` on such an `if` then bound its payload untyped, and
`a == "x"` on a `String` binder was not compared as text: the program passed the static pass and faulted
`diag.read-of-resource` at run time, under `coby` and `cobc` alike. A branch checked against the other
branch's type that comes out untyped now has that type, whichever branch it is (CHG-0361 covered the `then`
branch in `cobc`). The new row `conf.if-variant-else-branch-takes-sibling-type` covers the three contexts.

## What changed for programs

A program that faulted so runs to its end.

## Found by

Porting `cobc`'s driver into CobaltC (`impl/selfhost/cbc/driver.cb`):
`match (if (c) { Result::ok(arg(0)) } else { None }) { Some(a) : a == "x", ... }`; 2026-10-23.
