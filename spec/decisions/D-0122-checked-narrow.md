# D-0122 — `checked_narrow`

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 2)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §4, §9
Depends on: rule.arith.convert (`[Narrow-Overflow]`), rule.arith.alt (the `checked_` family)
Affects: `spec/06` §4 (1.15.0), `spec/21` §0, `spec/conformance.md`, the guide §06, both tools

## Problem

`narrow<T>(x)` faults when `x` does not fit — right for a programmer's
mistake, wrong for a value that came from outside: a length field in
a file, a count in a network header. Programs guarded by hand,
`if (x > max) { … } else { narrow<u32>(x) }`, per width, or faulted on
bad input. §4 of the master instructions says external data is a claim
to be validated, and the arithmetic family already answers `Option`
for the same question (`checked_add`).

## Candidate mechanisms

1. **Leave the guard to programs.**
2. **`checked_narrow<T>(x) : Option<T>`**, the missing member of the
   `checked_` family: `Some(x)` when `x` fits `T`, else `None`, never a
   fault. Selected.

## Selected design

`[Checked-Narrow]` in `spec/06`: an intrinsic between integer types, the
same fit test as `[Narrow-Overflow]`'s. A literal out of range gives
`None`, not a static rejection, since asking is the point.

## Compatibility impact

Additive; one intrinsic name.
