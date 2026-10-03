# CHG-0101 — Shifts bind tighter than the bitwise operators

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9
Depends on: D-0086
Affects: `spec/22` §2, the parser, the guide

## Problem / motivation

`<<` and `>>` bound looser than `&`, `^` and `|`, the opposite of C,
C++, Java, Go and Rust, so `hi << 8 | lo` was `hi << (8 | lo)`: a
silent wrong result for the common idioms of bit packing and masking
(D-0086).

## Decision

D-0086.

## What changed

- **`spec/22` 2.30.0:** `bitand ::= shift ('&' shift)*`,
  `shift ::= additive (('<<' | '>>') additive)*`; `compare` takes
  `bitor` operands.
- **Implementations:** `src/parser.rs` (`parse_compare`, `parse_shift`,
  `parse_bitand`); cobc and the checker share the parser.
- **Guide:** §13's precedence table and its note.
- **Showcase:** `tier3/tiny_serializer.cb`'s comment and the showcase
  README no longer describe the old order.

## Compatibility classification

A change of meaning for an unparenthesized mix of a shift with `&`,
`^` or `|`. A scan of the conformance cases, examples, showcase, guide
and `std` found none; every other expression parses as before.

## Conformance changes

**Added:** `conf.shift-binds-tighter-than-bitor`,
`conf.shift-binds-tighter-than-bitand`, `conf.shift-looser-than-additive`,
and `impl/conformance/06-arithmetic/shift_precedence_ok.cb`.

## Revisit conditions

None.
