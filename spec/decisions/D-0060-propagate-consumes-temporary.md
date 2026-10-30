# D-0060 — `?` consumes a temporary operand

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0049, rule.fail.propagate, rule.agg.match
Affects: rule.fail.propagate

## Problem

`[Propagate]` was exactly `match (r) { Ok(v) : v, … }`. For a `Result`
owning no resource, `Ok(v)` copies `v` out and a temporary `r` stays
until its statement ends (D-0049). If `v` is a reference, `r` holds a
second copy of it: `match (peek(p)?) { … : { take(p); } }` was an
aliasing conflict, although the same `match` over a `peek` returning the
reference directly was accepted. Real-world testing met it in a parser.

## Candidate mechanisms

1. **A temporary operand ends at the `?`**: its payload has been taken
   and nothing can name the rest. **Selected.**
2. Unchanged; a `peek` returning a value, not a reference, is the way.

## Selected design

Candidate 1: `[Propagate]`'s `match` ends a temporary `r` as soon as the
arm has bound its payload. A `Result` that is a place (`x?`) is read as
before and stays usable; a resource-bearing one is consumed as before.

## Rejected alternatives

- **2:** sound, but a rule whose only effect was to reject code that is
  sound once `?` is seen to consume its operand.

## Semantic rationale

`?` takes its operand apart; a temporary taken apart has no remaining
use. The references `?` produces are unaffected.

## Implementation-feasibility

`coby`'s `eval_propagate` takes the value out of a temporary operand
(`result_to_value_resource_aware`); `cobc`'s `lower_match`, for a `?`,
ends the temporary after the match (`cb_end_moved_out`).

## Compatibility impact

Extension: only programs that were rejected change.

## Prior-art status

- **Rust:** `?` moves its operand into `Try::branch`.

## Revisit conditions

None.
