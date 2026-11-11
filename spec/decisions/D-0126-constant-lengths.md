# D-0126 — Array lengths as constant expressions

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 7; D-0074's candidate 2)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0074, D-0105, D-0037 (literal expressions), rule.module.const
Affects: `spec/22` §3 (2.42.0), `spec/17` §1a (2.11.0), `spec/16` `type.array` (1.20.0), `spec/registry/diagnostics.md`, `spec/conformance.md`, the guide §16, the shared parser

## Problem

A length took a literal or a constant that was one (D-0074), so
`array<u8, N * 2>`, a table sized from another constant, or a length
computed from a module's constant was refused, and lookup tables were
built at run time. D-0074 named this as its revisit.

## Candidate mechanisms

1. **Leave it.** Programs write the number twice.
2. **Constant expressions** — integer literals, constants (by name,
   qualified too), and `+ - * / % & | ^ << >> ~ ( )` over them, folded
   before the program runs, in a constant's initializer as in a
   length; a constant may name one declared after it (a fixpoint).
   Selected: the grammar's own operators, nothing new to learn.

## Selected design

`length ::= const-expr` (`spec/22`); folded by the parser with the
grammar's precedence; outside parentheses `>` and `>>` end a length
(so `Vec<array<u8, N * 2>>` parses; a shift in a length is written in
parentheses). A length that is not such an expression, is negative,
divides by zero or overflows is a syntax error saying which. Types and
both tools see only the folded number, as before.

## Compatibility impact

Additive: every length that parsed before parses to the same number.
