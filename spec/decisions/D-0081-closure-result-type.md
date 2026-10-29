# D-0081 — A closure's result type may be written

Status: ACCEPTED (2026-09-28, owner decision)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0073, rule.fn.closure, `[T-Closure]`
Affects: `spec/15` §6, `spec/12` §5, `spec/22` §2

## Problem

A closure's result type came only from its body, or from the
`fn(…) : R` it was passed as. A body that is a literal could not be given
another type: `[](i64 a) { 0 }` gives `i32`. When a closure was bound with
`auto` and not passed anywhere, nothing could say what it returns.
Writing `: R` after the parameters, as for a function, is what users try
first, and it was a syntax error with an unhelpful message.

## Candidate mechanisms

1. **An optional `: type` after the parameters.** Selected. It is the
   function syntax, with no new token, and the body is checked against it.
2. **Leave it; use a suffix on literals.** Covers literals only, not a
   body whose type is ambiguous in other ways, and it reads as a
   workaround.

## Selected design

`closure ::= 'move'? '[' captures? ']' '(' params? ')' (':' type)? block`.
With `: τd`, the body is checked against `τd` and the closure's result
type is `τd`. Used where a `fn(…) : τ` is expected, `τd` must be `τ`
(`diag.type-mismatch`, naming both).

## Compatibility impact

None: a syntax error becomes valid.

## Revisit conditions

None.
