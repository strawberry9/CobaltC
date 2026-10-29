# CHG-0108 — A generic item as a value needs its type arguments fixed

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §20
Depends on: D-0093
Affects: rule.type.typing, the implementations

## Problem / motivation

No rule typed a generic item used as a value; coby ran `auto f = id;`
polymorphically while cobc exited 3 — two conforming implementations
deciding differently from the same text (D-0093).

## Decision

D-0093.

## What changed

- **`spec/12` 1.16.0:** §5 gains `[T-Item-Generic]` (instantiation by
  explicit arguments or the expected fn type) and
  `[T-Item-Value-Uninferable]` (a binding with neither is rejected,
  `diag.cannot-infer-type-parameter`).
- **Implementations:** the shared checker (`src/typecheck.rs`) types
  the instantiated cases (`generic_item_value`) and rejects the
  uninferable binding at `Let`; one fix serves `coby` and `cobc`.
  `cobc`'s message for the argument-position instantiations its
  lowering cannot yet solve stops claiming an internal error
  (exit 3 `unsupported`, unchanged disposition).
- **Guide:** no change — the guide never showed the rejected form.

## Compatibility classification

Breaking in principle: a program binding a generic item with nothing
to fix it, previously accepted (and run polymorphically) by coby
alone, is now rejected. No known program did this.

## Conformance changes

**Added:** `conf.generic-item-value-uninferable`
(`impl/conformance/12-type-system/generic_item_value_uninferable_rejected.cb`),
`conf.generic-item-value-wrong-expected`
(`impl/conformance/12-type-system/generic_item_value_wrong_expected_rejected.cb`).
**Unchanged (already pinned the positive forms):**
`generic_fn_item_as_value_ok.cb`.

## Revisit conditions

D-0093's.
