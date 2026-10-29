# D-0086 — Shifts bind tighter than the bitwise operators

Status: ACCEPTED (2026-09-28, owner's decision on the fourth stress round's finding)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: rule.arith.shift, rule.arith.bit
Affects: `spec/22` §2 (the expression grammar)

## Problem

`spec/22` placed `<<` and `>>` below `|`, `^` and `&`:
`compare > shift > | > ^ > & > + - > * / %`. Every language a CobaltC
programmer is likely to know (C, C++, Java, Go, Rust) binds shifts
tighter than the bitwise operators, so the idioms of bit packing and
masking parse differently here, and none of the differences is a type
error:

| Written | Meant (and meant everywhere else) | CobaltC read it as |
|---|---|---|
| `hi << 8 \| lo` | `(hi << 8) \| lo` | `hi << (8 \| lo)` |
| `x >> 4 & 0xF` | `(x >> 4) & 0xF` | `x >> (4 & 0xF)` |
| `flags & 1 << n` | `flags & (1 << n)` | `(flags & 1) << n` |

The results are wrong only for some operands (`8 | lo` is `8` whenever
`lo`'s low bits are clear), so tests pass while the program is wrong.
The stress rounds parenthesized every such expression defensively; a
showcase program carried a comment warning of the rule.

## Candidate mechanisms

1. **Move shifts between `&` and `+ -`** (C's and Rust's position for
   this pair). An expression means what its reader expects. Code that
   relied on the old order would change meaning silently; none exists
   in the repository (every mix is parenthesized, checked by a scan of
   the conformance cases, examples, showcase, guide and `std`).
2. **Keep the order, reject an unparenthesized mix** of a shift with
   `&`, `^` or `|`. Nothing changes meaning; every mix needs
   parentheses; a new rejection rule.
3. **Keep the order.** The trap stays and no check can see it.

Selected: 1. Only the shift level moves; comparison keeps binding
looser than the bitwise operators (unlike C, as before), so
`x & 1 == 0` is still `(x & 1) == 0`.

## Selected design

    compare     ::= bitor (('==' | '!=' | '<' | '<=' | '>' | '>=') bitor)?
    bitor       ::= bitxor ('|' bitxor)*
    bitxor      ::= bitand ('^' bitand)*
    bitand      ::= shift ('&' shift)*
    shift       ::= additive (('<<' | '>>') additive)*

`a << b + 1` is `a << (b + 1)`, as before and as in C.

## Compatibility impact

A change of meaning for an unparenthesized mix of a shift with `&`,
`^` or `|`; none exists in the repository. Every other expression
parses as before.

## Revisit conditions

None.
