# D-0155 — A text literal in a `match` arm or `if` branch is typed by the position

Status: ACCEPTED (2026-10-04, the owner: "proceed with the entire roadmap using your discretion, friction fix first if that's what you decide", after the friction was reported)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0134 (`[Str-Literal-View]`), D-0149 (`[Str-Literal-String]`)
Affects: `spec/21` §2a (4.20.0), `spec/conformance.md` (3.145.0), `impl/src/views.rs`; `CHG-0183`

## Problem

D-0149 typed a literal as a `String` where a `String` is declared, and
followed a function's result through its `if` and `match` tails. A local
did not get the same treatment: writing the crypto sweeps,

    String shared = match (x25519(&a[0..$], &b[0..$])) { Some(s) : to_hex(&s[0..$]), None : "none", };

was a type error (`one arm gives String and another str`), though the
literal alone in that position was accepted. A rule that holds in one
place and not the next is a friction.

## Selected design

In every position `[Str-Literal-View]` and `[Str-Literal-String]` name
(a parameter, a local's declared type, a struct literal's field, a
function's result), a literal that a block, an `if` branch or a `match`
arm standing in the position gives is in the position too, at any depth
of nesting. The shared pass (`impl/src/views.rs`) looks through those
tails whenever it types a position's expression. No program accepted
before changes meaning.

Candidates not adopted: typing every `str`-giving arm by the other
arms' type (a unification step the checker does not have, and an
implicit conversion of non-literal `str` values, D-0006).

## Compatibility impact

Additive: programs that were type errors are accepted.

## Revisit conditions

None foreseen.
