# CHG-0130 — The first static error in the source is reported; Box chains and long expressions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; owner-delegated round-6 frictions: "proceed with all your choices")
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0077 (implementation limits)
Affects: `spec/registry/diagnostics.md` (reporting order), spec/09, `spec/conformance.md`

## Problem / motivation

- A program with several static errors got whichever one the checker
  reached first, which followed the order it visits functions, not the
  text: an error in `main` could be reported before an earlier one.
- Destroying a long `Box`-linked chain recursed once per node: `coby`
  faulted `diag.stack-exhausted` at about 200 000 nodes, compiled code
  at about 300 000.
- A long flat expression (`1 + 1 + …`, 40 000 terms) took `coby` 85 s
  and `cobc` longer than 300 s (both quadratic), and 100 000 nested
  parentheses overflowed `cobc`'s own stack.

## What changed

- **Registry:** when a program has several static errors, the one
  reported is the first in the source (the lowest line; the order among
  one line's errors is unspecified).
- **Implementations** (no rule): a `Box` that is the last resource its
  container destroys hands its referent to the enclosing teardown's
  loop, in both runtimes, so a chain of any length is destroyed in
  constant stack, in the same order; literal-ness of an operator chain
  is decided once per node, and `cobc` no longer lowers a chain once per
  level to learn its type; `cobc` checks and lowers on a thread with a
  large reserved stack, as `coby` evaluates.
- **Rule text:** `[Ref-Form-Temporary-Argument]` names a variant without
  a payload (`f(&None)`), which the checker already admitted and both
  runtimes failed on; outside an argument, `&None` is
  `diag.borrow-of-non-place`.
- **Rows:** `conf.first-error-in-source-order-rejected`,
  `conf.variant-temp-borrow-argument`, `conf.variant-borrow-outside-argument-rejected`.

## Compatibility classification

Corrective (reporting order) and implementation-only otherwise: no
program changes meaning.
