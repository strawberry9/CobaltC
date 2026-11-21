# D-0304 — A text literal is typed by its position in the five positions that remained

Status: ACCEPTED (2026-11-01, with D-0303: the owner's authorisation of
`private/D-0303-attributes-deprecated-removed-proposal.md`, decided under "my dislike of being able to do the same
thing two different ways"); implemented by `CHG-0402`
Kind: Decision (`spec/02-schema.md`); a typing rule
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 3), §9
Depends on: D-0006, D-0134 (`[Str-Literal-View]`), D-0149 (`[Str-Literal-String]`, whose revisit condition on
closures this is), D-0155 (tails), D-0162 (a type parameter fixed as `String`), D-0225 (a variant's payload),
D-0302 (`[Ref-String-View]`, `[Str-Value-View]`)
Affects: `spec/21` §2a and §2h (4.74.0); `spec/12` `rule.type.expected` (1.27.0); `spec/conformance.md`; both
checkers (`impl/src/typecheck.rs`, `impl/selfhost/front/typecheck.cb`) and both literal passes (`impl/src/views.rs`,
`impl/selfhost/front/views.cb`); the guide's text chapter; the book (ch06, ch08, ch11, ch15)

## Problem

`[Str-Literal-String]` typed a literal as a `String` by the position it stands in: a local's written type, a
parameter, a struct literal's field, a function's result (through D-0155's tails) and a type parameter the call
fixes as `String` (D-0162). Measured on 2026-11-01 with `coby --check`, five positions still refused it:

| Position | Example | Before |
|---|---|---|
| the value assigned to a place | `s = "x";`, `p.name = "x";`, `v[0] = "b";`, `*r = "w";` | "the place is `String`, but the value assigned is `str`" |
| a variant constructor's payload, once the expected type fixes it | `Option<String> o = Some("x");`, `return Ok("x");`, `Vec::push(&mut cells, Some("42"))`, a program's own `A("x")` | "`Some` holds a `String`, but this is `str`" |
| a generic struct literal's field the expected type fixes | `Pair<String> p = Pair { .first = "a", … };` | "field `first` of `Pair` is `String`, but the value is `str`" |
| an array literal's elements | `array<String, 2> a = ["x", "y"];` | "an array's elements have one type: `String`, and this one is `str`" |
| a closure declared `: String` | `[](bool c) : String { if (c) { return "a"; } "b" }` | "this `return` gives `str`, but the closure's result is `String`" |

In each of them `String::from_str("…")` was the only spelling, which kept a second way of writing owned text alive
(281 literal `from_str` sites in the tracked `.cb` trees after Update 157, 132 of them in variant payloads).

## Selected design

`[Str-Literal-String]` and `[Str-Literal-View]` gain the five positions; `[Ref-String-View]` and `[Str-Value-View]`
take the same list (they are stated as "the positions of `[Str-Literal-View]`"). D-0155's tails apply in each. What
stays out: `auto` (the literal is a `str`), a `str` binding (D-0006: `s = t;` is `diag.type-mismatch`, and the copy
is `String::from_view(t)`), a type parameter nothing fixes, and a generic function whose instantiations would type
one literal two ways (D-0162).

## Candidates not adopted

- Fewer positions (without the assignment, whose declaration is not on the literal's line): leaves `s = "x";`
  needing a call where `String s = "x";` does not.
- Typing every `str`-giving expression by the position (a `str` binding included): an implicit conversion of a
  value, D-0006.

## Compatibility impact

Additive: every newly accepted program was a type error.

## Implementation notes (CHG-0402)

- A closure's result is a declared position and is rewritten by the literal pass, as a function's is.
- The other four need the type the checker finds for the place, the payload, the field or the element. At each,
  a literal (or a literal tail) in a position whose type comes out `String` or `StringView` is recorded for the
  rewrite D-0162 and D-0225 already apply (`String::from_str(L)`, `StringView::of(L)`), the value is taken as that
  type, and the program is checked again as rewritten, so every later pass sees an ordinary call. `&s` and a `str`
  value assigned to a `StringView` place are viewed (D-0302) by the same record.
- A body the checker reaches only after the program is checked (`check_more`) takes no rewrite, as D-0302 notes;
  `std` writes those positions out.
