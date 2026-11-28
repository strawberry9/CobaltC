# D-0175 — `while (true)` without a `break` never completes

Status: ACCEPTED (2026-10-05, the owner: "accept option 1 as D-0175 and implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: `rule.control.while` (`spec/14` §4), `[T-Never]` (`spec/12`), D-0115 (`while (p = e)`, defined through `while (true)`)
Affects: `spec/12` (1.22.0), `spec/14` (1.22.0), `spec/conformance.md` (3.167.0), the guide §14, `src/typecheck.rs`, `cobc/src/lower.rs`, `impl/std/*.cb`; `CHG-0203`

## Problem

`[T-While]` gives every loop the type `unit`. A function whose body ends
in a loop that only `return`s or faults out of it was therefore
rejected — "`f` returns `i32`, but its body gives `void`" — and every
such function carried a value after the loop that could never be
reached. `std` had fourteen of them (`k`, `None`, `Err(-3)`, a whole dummy
`RsaPrivateKey` built after `rsa_generate_key`'s loop), each a line a
reader must recognise as dead. `while (true)` appears 20 times in `std`
and 15 times in the showcases.

## Candidate mechanisms

1. **A `while (true)` that no `break` leaves has the type `never`.**
   Selected: no new syntax and no reserved word; every existing program
   keeps its meaning; `never` is already the type of `return`,
   `break`, `continue` and `fault`, accepted at any expected type
   (`[T-Never]`). The static pass already tracks which `break`s leave
   which loop (for definite assignment).
2. **A `loop { … }` construct.** The same effect with a new reserved
   word and a second spelling of what `while (true)` says; against the
   minimal doctrine (§9). Not adopted.
3. **No change.** The dead trailing value stays the idiom. Not adopted.
4. **`break value` (a loop that yields a value).** Not needed by any
   program in the repository; a separate decision if one is.

## Selected design

**`spec/12` `[T-While-Forever]`:** `while (true) b` with `true` the
literal, where no `break` in `b` leaves this loop (a `break` inside a
nested loop leaves that loop), has the type `never`. Every other loop
keeps `[T-While]`'s `unit`. Only the literal counts: a condition that is
always true at run time is not looked into.

**`spec/14` §4:** the prose notes that such a loop has no normal
completion: the code after it is unreachable, and the static pass's
flow facts there assert nothing (as after `return`).

`for` and `foreach` loops are not affected (`for (;;)` has no empty
condition in CobaltC). `while (p = e)`, defined as `while (true)` with a
`_ : break` arm, always has its `break` and stays `unit`.

## Compatibility impact

Additive: every program accepted before is accepted with the same
meaning; a function ending in such a loop is now accepted without a
trailing value. A dead trailing value is still accepted.

## Revisit conditions

- `break value`, if a program needs a loop that yields a value.
