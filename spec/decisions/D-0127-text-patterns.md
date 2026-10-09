# D-0127 — Text literal patterns

Status: ACCEPTED (2026-09-30, the owner: "proceed with implementing match on str")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0057 (literal patterns; amended), D-0056, D-0058, D-0109, D-0115, D-0053 (StringView), rule.agg.match
Affects: `spec/16` `rule.agg.match` (1.21.0), `spec/22` `pattern` and §2 (7), §5 (2.43.0), `spec/conformance.md`, the guide §16, the shared parser and checker, both tools

## Problem

A choice among texts — a command word, a header name, a token — was an
`if` chain of `String::eq_str(&s, "get")` or `s == "get"` calls, while
the same choice among integers, bytes or `bool`s was a `match` with
literal arms (D-0057), checked for a missing `_` and a repeated case.
Text is the commonest thing programs dispatch on. D-0057 closed the
pattern language at its four forms, so adding a form needs a record.

## Candidate mechanisms

1. **Leave it.** `if` chains, or an enum the text is parsed into first.
2. **Text literals as patterns** — a `string-literal` is a pattern like
   an integer literal: it stands alone against a `str`, `String` or
   `StringView` scrutinee, or ends a nested pattern (`Some("quit")`,
   `Ok("x")`), and matches when the level's text has exactly its
   bytes. Coverage and reachability as for integers: `_` (or a binder)
   is needed, a repeated text is an unreachable arm. Selected.
3. As 2, with prefix or glob forms. Declined: `==` semantics only, as
   for every other literal.

## Selected design

`lit ::= … | string-literal` in `rule.agg.match` and `pattern` in
`spec/22`. A text literal is of every text type (`str`, `String`,
`StringView`) and of no other; an integer or `bool` literal is of no
text type (`[Pattern-Type]`). The level's text is compared byte for
byte where it is: a `String` scrutinee is read, not moved, and stays
usable after the `match`; a binder arm after the literals takes the
whole value as D-0058's binder does (a `String` is moved out, so the
binding is consumed); `match (r)` on a reference to text at the top
level is rejected as it is for an integer (`match (*r)` reads it), and
below the top level a reference payload is looked through (D-0109).
A text literal also starts a `pattern '=' expr` condition (D-0115):
`if ("quit" = cmd)`, `while ("go" = next(&mut q))`.

Not chosen: a constant of type `str` as a pattern by name. A constant
in a pattern must fold to an integer or `bool` literal (D-0058) and
that rule is unchanged; a text constant is compared with `==`.

## Compatibility impact

Additive: a text literal was a syntax error in pattern position.

## Invariant traceability

None changed: `[Match]` reads the scrutinee under `¬clash(a, shared)`
as before; a text comparison is a read.

## Revisit conditions

- A `str` constant by name in a pattern, with a program that shows the
  `==` chain it replaces.
