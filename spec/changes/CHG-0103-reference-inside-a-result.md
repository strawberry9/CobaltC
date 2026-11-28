# CHG-0103 — A result that holds a reference borrows its first reference argument

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12
Depends on: D-0088
Affects: rule.control.flow-analysis, the checker

## Problem / motivation

A reference out of `HashMap::get`, held across an `insert`, faulted
only when the table grew (D-0088).

## Decision

D-0088.

## What changed

- **`spec/14` 1.15.0:** a `deriv` row for a binding or `match` binder of
  a call's reference-holding result.
- **Implementations:** `src/typecheck.rs` `contained_ref_facts`, used for
  declarations, assignments and `match` binders.

## Compatibility classification

Tightening.

## Conformance changes

**Added:** `conf.map-value-held-across-insert-rejected`,
`conf.map-key-written-while-value-held`
(`14/map_key_written_while_value_held_ok.cb`), and
`14/map_value_held_across_insert_rejected.cb`.

## Revisit conditions

None.
