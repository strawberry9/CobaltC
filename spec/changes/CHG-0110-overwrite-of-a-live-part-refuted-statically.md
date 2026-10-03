# CHG-0110 — Overwriting a live part of an object is refuted statically

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §20
Depends on: D-0095
Affects: rule.control.flow-analysis (`spec/14` §6), rule.value-object.write, the implementations

## Problem / motivation

`[Write-Resource-Overwrite-Rejected]` was refuted statically for a
whole-object write only; a write over a live field, array element or
referent was checked only when it ran (D-0095).

## Decision

D-0095.

## What changed

- **`spec/14` 1.16.0:** §6's side-condition table gains the row
  `¬live-resource-at(a)` for a write to a part of an object: refuted
  when every value of the part's type owns something, the root is a
  valid initialized binding or a valid reference binding, and every
  index step is a literal inside a fixed array.
- **Implementations:** the shared checker (`src/typecheck.rs`
  `flow_write`, `refute_live_part_overwrite`, `deref_ref_valid`): one
  change serves `coby` and `cobc`. The run-time check is unchanged and
  remains the backstop for everything left dynamic.
- **Also fixed (found by this change's whole-repository check):** a
  generic instantiation is recorded under the item's module-qualified
  key (`item_key`), not its bare name; a program's own generic that
  hides a `std` one was checked at std's types, and std's body at
  those types not at all. Implementation defect; no normative text.
- **Guide:** §7's overwrite paragraph gains one sentence: the rule
  covers a field and a write through a reference, rejected before the
  program runs where the old value is visibly still there. The
  diagnostics table already gives the phase as "both".

## Compatibility classification

Breaking in principle: a program overwriting a live part on a path it
never took is now rejected before it runs. No program in the
repository is affected.

## Conformance changes

**Added:**
`conf.overwrite-field-of-live-object`
(`impl/conformance/07-resource-authority/overwrite_field_of_live_object_rejected.cb`),
`conf.overwrite-through-exclusive-reference`
(`…/overwrite_through_exclusive_reference_rejected.cb`),
`conf.overwrite-optional-field-dynamic`
(`…/overwrite_optional_field_dynamic.cb`: an `Option<String>` part stays
dynamic),
`conf.overwrite-computed-index-bounds-first`
(`…/overwrite_computed_index_bounds_first.cb`: bounds before overwrite),
`conf.own-generic-hiding-std-checked-at-own-types`
(`impl/conformance/15-function-semantics/own_generic_hiding_std_checked_at_own_types_ok.cb`).

## Revisit conditions

D-0095's.
