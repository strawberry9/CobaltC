# CHG-0098 — `unwrap`, `expect` and `Vec::clone`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner decision)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0082
Affects: `spec/21` §0; `spec/registry/diagnostics.md`

## Problem / motivation

D-0082: no way to take a value that must be there, or to copy a `Vec`.

## Decision

D-0082.

## What changed

- **`spec/21` 3.33.0:** §0's table.
- **`spec/registry/diagnostics.md` 1.35.0:** `diag.unwrap-failed`.
- **Implementations:** `std` (`prelude.rs`); the checker rejects `Vec::clone` of a resource element type at the call.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.unwrap-some`, `conf.unwrap-none-faults`, `conf.expect-message`, `conf.vec-clone`, `conf.vec-clone-resource-rejected`.

## Revisit conditions

None.
