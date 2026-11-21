# CHG-0104 — A `fn` value owns its closure

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12
Depends on: D-0089
Affects: rule.fn.closure, the implementations

## Problem / motivation

The tools disagreed on copying and ending `fn` values that hold
closures, and neither destroyed a closure held in a struct's field
(D-0089).

## Decision

D-0089.

## What changed

- **`spec/15` 1.10.0:** `[Fn-Value-Read]`, `[Fn-Value-Empty-Call]`, and
  the ownership paragraph.
- **`spec/registry/diagnostics.md` 1.36.0:** `diag.stale-binding` for a
  call of an empty `fn` value.
- **coby:** `Value::Closure` is a counted handle (`value::ClosureRef`);
  the last one dropped queues the closure, which the interpreter
  destroys after the object end or statement that dropped it
  (`end_dropped_closures`); a read by value copies closures
  (`copy_closures`) or moves the ones that own a resource
  (`split_closures`); a move capture's per-call cell is written back to
  the closure (`capture_writeback`).
- **cobc / cbrt:** `cb_fn_read` and `cb_fn_split` copy or move each `fn`
  slot of a value read by value; the call thunk faults on an empty
  handle (`cb_fn_moved`); types holding `fn` values (`holds_fn`) are
  checked locals, and their closures end with them (`drop_in_place`,
  `cb_drop_value` for a discarded or dropped temporary).
- **cbrt:** a fault's report is printed after the unwind's destructors
  have run, as coby prints it (`[Fault-Unwind]`).

## Compatibility classification

A change of behavior for `fn` values holding closures (copies no longer
shared in coby; resource-owning closures move); fixes otherwise.

## Conformance changes

**Added:** `conf.fn-value-copy-independent`,
`conf.fn-value-owning-closure-moves`, `conf.fn-value-emptied-call`, and
`15/fn_value_copy_is_independent_ok.cb`,
`15/fn_value_owning_closure_moves_ok.cb`,
`15/fn_value_emptied_call_faults.cb`.

## Revisit conditions

None.
