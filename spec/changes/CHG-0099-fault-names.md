# CHG-0099 — `fault` takes only run-time diagnostics

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner decision)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0083
Affects: `spec/21` §0; `spec/registry/diagnostics.md`

## Problem / motivation

D-0083: `fault` accepted names of diagnostics that do not exist.

## Decision

D-0083.

## What changed

- **`spec/21` 3.33.0:** §0's table and the text after the `Vec` listing.
- **`spec/registry/diagnostics.md` 1.35.0:** `diag.unbound-name` covers `[Fault-Name]`.
- **Implementations:** the checker validates the name against the registry, and both tools carry the optional message (`cb_fault_msg` in `cobc`).
- **Suite and guide:** `fault(assertion_failed)` became `fault(assert_failed)` in seven conformance programs; the guide's file example uses `Result::expect`.

## Compatibility classification

Tightening: programs naming an unregistered fault are rejected.

## Conformance changes

**Added:** `conf.fault-unregistered-name`, `conf.fault-with-message`.

## Revisit conditions

None.
