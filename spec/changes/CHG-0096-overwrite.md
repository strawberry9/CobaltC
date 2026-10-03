# CHG-0096 — `overwrite`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner decision)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0080
Affects: `spec/21` §0, §3b; `spec/registry/diagnostics.md`

## Problem / motivation

D-0080: replacing a live resource took `drop(replace(&mut x, v))`.

## Decision

D-0080.

## What changed

- **`spec/21` 3.33.0:** `overwrite` in §0's table and §3b.
- **`spec/registry/diagnostics.md` 1.35.0:** `diag.overwrite-of-live-resource`'s repair names it.
- **Implementations:** `std` (`prelude.rs`), shared by both tools.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.overwrite-destroys-old-value`.

## Revisit conditions

None.
