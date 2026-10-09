# D-0132 — A `bitstruct` declared in a function

Status: ACCEPTED (2026-10-02, the owner's choice of the recommendation)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §17
Depends on: D-0069 (function-local types), D-0118 (`bitstruct`)
Revisits: D-0118, point 6 ("Item level only")
Affects: `spec/16` §2a (1.22.0), `spec/17` §1c (2.13.0), `spec/22` §2 (2.44.0), `spec/conformance.md`, the guide §16, `impl/src/parser.rs`, `impl/src/derive.rs`

## Problem

D-0069 let a `struct` or an `enum` be declared as a statement of a
block, under the principle "one set of rules for a type wherever it is
declared". D-0118, deciding `bitstruct` later, said "item level only"
without a reason: a bitstruct used by one function (a packed register
or header that one decoder reads) had to be declared at the module's
top level, away from its use, as structs had to before D-0069.

## Candidate mechanisms

1. **Keep item level only.**
2. **A `bitstruct` is a local type too**, under `rule.module.local-type`
   as a local `struct` is. Selected.

## Decision

`bitstruct Name : uN { … }` may be a statement of a block. It is a local
type (`spec/17` §1c): its name is in scope from the declaration to the
end of the block, it is a type of the module under a name no program
can write, it cannot be exported, and its declaration sees module items
and the local types and constants in scope, not the function's
variables or type parameters. Every rule of D-0118 holds unchanged.

**Its functions.** `Name::bits` and `Name::from_bits` are derived for a
local bitstruct as for one at item level: they are part of what a
bitstruct is (D-0118 point 6), unlike the `clone` a local `struct` does
not derive (`spec/16` §4, D-0069), which a bitstruct, being plain, has
no use for. A function declared beside it in the block, `fn Name::f`,
is allowed as for any local type; one named `bits` or `from_bits` is
`diag.duplicate-item`, as at item level.

No new tokens and no new syntax: the existing declaration, in one more
place.

## Compatibility

Additive: a program the grammar rejected is accepted.

## Revisit

An enum with codes (D-0106) is still declared at module level only; the
same reasoning would apply to it, and it is not decided here.
