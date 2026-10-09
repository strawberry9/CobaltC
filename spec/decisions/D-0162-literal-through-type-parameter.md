# D-0162 — A text literal is a `String` where a type parameter is fixed as `String`

Status: ACCEPTED (2026-10-04, the owner: "fix the unwrap_or String friction as D-0162")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0079 (a literal argument is typed by its call), D-0149 (`[Str-Literal-String]`, whose revisit condition this is), D-0155
Affects: `spec/21` §2a (4.27.0), `spec/12` `rule.type.expected` (1.21.0), `spec/conformance.md` (3.152.0), the guide §21, `impl/src/typecheck.rs`, `impl/src/views.rs`, `impl/src/lib.rs`; `CHG-0190`

## Problem

D-0149 typed a literal as a `String` where a `String` is declared, and
listed one revisit condition: a type parameter solved as `String`.
Writing the crypto roadmap's test programs met it three times:

    String name = Result::unwrap_or(arg(0), "anonymous");   // was a type error
    Vec::push(&mut names, "x");                              // for a Vec<String>
    HashMap::insert(&mut m, "k", 7);                         // for a HashMap<String, i32>

Each needed `String::from_str`, where the same literal as a declared
`String` local did not.

## Candidate mechanisms

1. **Leave it.** The most common `std` call shapes keep the ceremony.
2. **Type the literal by the solved parameter**, as D-0079 types a
   numeric literal argument: the literal is not evidence for the type
   parameter; it is typed after the other arguments and the expected
   type have fixed what they can, and if its parameter is then `String`
   it is `String::from_str(L)`. Selected.

## Selected design

In `[Str-Literal-String]`'s positions, an argument whose parameter is a
type parameter (or mentions one) counts once the other arguments and the
expected type have fixed the parameter to exactly `String`. Where
nothing fixes it, the literal is a `str` as before (`pick(true, "x",
"y")` is `str`). A `str` variable is never converted (D-0006).

**Realization:** the checker defers a text literal argument as it defers
a numeric one; when its parameter comes out `String` it records the
literal and accepts it. If any were recorded, the program's tree is
rewritten (each such literal becomes `String::from_str(L)`, in
`impl/src/views.rs`), rebuilt and checked again, so the interpreter and
the compiler only ever see an ordinary call. A literal in a generic
function whose instantiations disagree (`String` for one, `str` for
another) cannot be both and stays a type error.

## Compatibility impact

Additive: only programs that were type errors are accepted.

## Revisit conditions

- A variant constructor with an expected type (`Option<String> o =
  Some("x");`), which the call rule does not reach, if programs ask.
