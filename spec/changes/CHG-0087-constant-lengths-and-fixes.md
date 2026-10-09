# CHG-0087 — Constant array lengths; temporaries' fields and borrows

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §19, §21
Depends on: D-0074, D-0073
Affects: `spec/22`, the implementations

## Problem / motivation

- D-0074: an array length could not name a constant.
- **Moving a resource field out of a temporary** (`mk().v`) is
  `[Store-Binding-Place-Transfer-Sub]` (`spec/05`), a static rejection.
  The checker did not see it. `coby` copied the field out, duplicating
  the resource. `cobc` faulted at run time.
- **Temporary borrows missed by `cobc`:** a temporary borrowed as an
  argument inside a short-circuit condition (`c && f(&g())`) reached
  `cobc` as a rewritten copy of the expression. `cobc` recognised such
  borrows by address, so it failed ("internal error: borrow of a
  temporary").

## Decision

D-0074. The implementations follow the specification.

## What changed

- **`spec/22` 2.27.0:** `length`.
- **`spec/09`:** the prose on a reference held inside an argument's
  result (D-0073).
- **`spec/conformance.md` 3.70.0:** the cases below.
- **`spec/02-schema.md` 1.0.60:** §5's "in use" ranges.
- **Implementations:**
  - **The parser** prescans `const … NAME = <integer>;` and reads a
    length as a number or such a name.
  - **The checker** rejects a resource moved out of a temporary's field,
    with a message on what to do instead.
  - **`coby`** faults when a result would be a part of an object.
  - **Both evaluators** recognise a temporary borrow by its shape (a call
    or an aggregate literal), which the checker admits only as an
    argument.

## Compatibility classification

Extension, and fixes.

## Conformance changes

**Added:**
- `conf.array-length-constant`, `conf.array-length-not-constant`;
- `conf.temp-borrow-escape-stale`, `conf.temp-borrow-in-condition`;
- `conf.move-out-of-temporary-field-rejected`.

## Revisit conditions

See D-0074.
