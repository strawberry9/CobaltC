# CHG-0084 — Moves through a reference, and spawn arguments in flight

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, fixes requested by the owner)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: rule.resauth.transfer, rule.conc.spawn
Affects: implementations only; `spec/conformance.md`

## Problem / motivation

Three implementation faults, found by writing larger programs, where the
tools did not do what the specification says:

1. **`coby`** performed a move of a resource reached through a
   reference, `String y = *(&x);` or `*r` returned from a function taking
   `ref<String, exclusive> r`. It then reported `diag.stale-binding` at
   the owner's next use. `[Authority-Transfer-Aliased]` makes the move
   itself the fault: the owner's path still reaches the object.
2. **`cobc`** stopped with "internal error in cobc: move out of a
   projection" on the same programs.
3. **`cbrt`**: a reference or slice argument to `spawn` could be retired
   while travelling to the new thread. If the spawning statement ended
   before the thread received it, a thread writing through an exclusive
   slice then faulted with `diag.stale-binding`, intermittently and only
   in compiled programs.

## Decision

The implementations follow the specification. No rule changes.

## What changed

- **`coby`:** a resource moved from a place reached through a reference
  (a binding's store, a result, a field's store, a spawn argument) is
  `diag.move-while-aliased`.
- **`cobc`:** the same move lowers to that fault.
- **`cbrt`:** a datum in flight (`cb_send_datum` to `cb_recv_datum`)
  holds its paths.
- **`spec/conformance.md` 3.67.0:** the cases below.
- **`spec/02-schema.md` 1.0.57:** §5's "in use" ranges.

## Compatibility classification

Clarification. Programs the specification rejects are now rejected where
it says, and a program it accepts no longer faults.

## Conformance changes

**Added:** `conf.move-through-reference-rejected`,
`conf.move-through-param-rejected`, `conf.spawn-slice-arguments`.

## Revisit conditions

None.
