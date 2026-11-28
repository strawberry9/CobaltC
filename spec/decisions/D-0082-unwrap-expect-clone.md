# D-0082 — `unwrap`, `expect` and `Vec::clone`

Status: ACCEPTED (2026-09-28, owner decision)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0033, D-0065, rule.stdlib
Affects: `spec/21` §0; `spec/registry/diagnostics.md`

## Problem

Programs that know a value is there wrote a small generic helper over
`fault` to take it out of an `Option` or `Result`, because `std` offered
only `unwrap_or`. Copying a `Vec` also needed a hand-written loop, while
`String` had `String::clone`.

## Candidate mechanisms

1. **`Option::unwrap`/`expect`, `Result::unwrap`/`expect`, faulting
   with a new `diag.unwrap-failed`**, and `expect` carrying the program's
   message. Selected. It names the failure for what it is.
2. **The same, faulting with `diag.assert-failed`.** No new diagnostic,
   but the report would call a missing value a failed assertion.
3. For copying: **`Vec::clone` for element types that own nothing**,
   and **`Vec::clone_by(v, copy)`** for any element type. Selected. A
   general `clone` would need a notion of cloning every type, which the
   language does not have. `clone_by(&names, String::clone)` is short.

## Selected design

- `Option::unwrap(o) : T`, `Option::expect(o, str m) : T`,
  `Result::unwrap(r) : T`, `Result::expect(r, str m) : T`. On
  `None`/`Err` they fault `diag.unwrap-failed` (dynamic) at the call,
  with `m`, or with which call it was for `unwrap`. An `Err`'s payload is
  destroyed.
- `Vec::clone(&v)`: each element copied in order. A resource element
  type is `diag.type-mismatch` at the call, and the message names
  `clone_by`. `Vec::clone_by(&v, copy)`: each element made by `copy`.

## Compatibility impact

None: new functions and a new diagnostic.

## Revisit conditions

If the language gains a general notion of copying a value.
