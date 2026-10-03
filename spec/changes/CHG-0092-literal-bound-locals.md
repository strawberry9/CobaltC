# CHG-0092 — Value-range refutation through literal-bound locals

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0071
Affects: rule.control.flow-analysis

## Problem / motivation

`spec/14`'s discharge table already refutes a value-range condition
whose operands are "bound by a local declaration to literals with no
intervening write". Both tools refuted only literal operands, so
`u8 x = 200; u8 y = x + 100;` faulted at run time instead of being
rejected. That is a conformance bug. The phrase "no intervening write"
also left open whether a borrow, a closure capture or a write on one
branch ends the binding, and the spec requires rejecting *exactly* the
refuted instances.

## Decision

A binding is literal-bound when its initializer is an integer literal
(optionally parenthesized or negated) and no path to the use writes,
borrows or captures it. A reference could write later, and a capture
could write from inside the closure, so both end the binding. Joins keep
the binding only when every incoming path agrees on the value, and loop
back edges count as incoming paths.

## What changed

- **`spec/14` 1.12.0:** the definition of "literal-bound", next to the
  discharge table.
- **`spec/conformance.md` 3.75.0:** the rows below.
- **`spec/02-schema.md` 1.0.65:** §5's "in use" ranges.
- **Implementations (shared checker):** `Flow.lits` in `typecheck.rs`,
  set by a `let` with a literal initializer and cleared by writes,
  borrows, captures and scope exit. It is intersected at joins and
  consulted by the arithmetic, divisor, index and shift checks. On a
  loop's speculative fixed-point passes it is not consulted, because
  the back edge has not been joined in yet (as for every other flow
  refutation).
- **Conformance programs that used a literal-bound zero as a
  hand-made assertion:** `17-modules/module_import_exports_ok.cb`
  divides by `a - a`, and `20-trust-boundaries/extern_libc_scalar_calls_ok.cb`
  uses `assert`. `06-arithmetic/u128_overflow_at_max_dynamic.cb` writes
  `big` before the overflowing add, so it stays a dynamic case. The
  showcase `tier3/tinyos` stopped the machine with `u32 zero = 0;
  zero / zero;` on its bus-error path, and now uses `assert(false, …)`
  (D-0065).

## Compatibility classification

Tightening: programs that were bound to fault on the first evaluation of
such an operation are now rejected. A program that only reaches the
operation on a path that never runs is also rejected, as literal
operands already were.

## Conformance changes

**Added:** `conf.literal-bound-local-overflow-static`,
`conf.literal-bound-local-written`, and the
`14-control-flow/flow_literal_bound_local_*.cb` and
`flow_literal_bound_loop_counter_ok.cb` cases.

## Revisit conditions

If value-range reasoning is ever extended beyond literals.
