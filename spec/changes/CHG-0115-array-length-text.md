# CHG-0115 — spec/17's array-length text follows D-0074

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29; editorial correction, found by the round-6 stress probes)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0074
Affects: `spec/17` §1a

## Problem / motivation

D-0074 let an array length be the name of a constant whose initializer
is an integer literal (`spec/22` `length`), but listed only `spec/22` as
affected; `spec/17` §1a still said an array length "takes an integer
literal, not a constant". The two texts contradicted each other.

## What changed

- **`spec/17`:** the bullet states D-0074's rule and its limits (no
  qualified name, no computed constant).
- **Implementations:** the parser's message for `array<τ, m::N>` names
  the qualified constant and the rule, instead of only `m`.

## Compatibility classification

Editorial: the grammar and both implementations already followed
D-0074.
