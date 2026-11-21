# CHG-0095 — A literal argument takes its type from the call

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0079
Affects: rule.fn.generic-call

## Problem / motivation

D-0079: `max(0, x)` and `fold(&v, 0, f)` with `i64` data were rejected.

## Decision

D-0079.

## What changed

- **`spec/15` 1.7.0:** §5, the order of evidence in
  `[Generic-Call-Inferred]`.
- **`spec/conformance.md` 3.77.0:** the rows below.
- **Implementations.** Each tool considers the non-literal arguments
  first, then the expected type, then the literals:
  - the shared checker when checking a call;
  - `coby` when it instantiates a call at run time, re-evaluating a
    literal argument at its parameter's type;
  - `cobc` when it lowers the call, lowering literal arguments last.

## Compatibility classification

Extension: rejected programs become valid; no valid program changes.

## Conformance changes

**Added:** `conf.literal-argument-typed-by-other-argument`,
`conf.literal-argument-typed-by-expected-type`,
`conf.literal-arguments-only-default`,
`conf.literal-argument-out-of-range-for-inferred-type`.

## Revisit conditions

None.
