# CHG-0035 — `min_value<T>()`, `max_value<T>()`, and Intrinsic Operand Typing

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-24, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0026, rule.arith.convert, rule.arith.alt, rule.type.kind, rule.stdlib.prelude
Affects: rule.arith.convert, rule.arith.alt, rule.arith.limits (new), rule.type.kind, rule.stdlib.prelude, diag.type-mismatch, diag.cannot-infer-type-parameter, the §1 cases listed below

## Problem / motivation

D-0026 has the details. In short, programs could not name a type's
limits portably or generically. Also, no rule stated the operand typing
of the conversions and alternative operations, and neither
implementation checked it before the program ran.

## Decision

D-0026: the intrinsics `min_value<T>()` and `max_value<T>()` for
integer `T`, with `T` written explicitly; the conversions' and
alternative operations' operand types checked statically, at each
instantiation.

## What changed

- **`spec/06` 1.6.0:**
  - §5a, `rule.arith.limits`: `[Min-Value]`, `[Max-Value]`,
    `[Limits-Not-Integer]`, `[Limits-Uninferable]`;
  - §4, `[T-Convert]`; §5, `[T-Alt]`.
- **`spec/12` 1.8.0:** `rule.type.kind` says an intrinsic applied to a
  type parameter is typed at each instantiation.
- **`spec/21` 3.1.0:** the intrinsics table lists `min_value` and
  `max_value`.
- **`spec/registry/diagnostics.md` 1.9.0:** `diag.type-mismatch` and
  `diag.cannot-infer-type-parameter` name the new rules.
- **`spec/conformance.md` 3.20.0:** eleven §1 cases.
- **Implementations:**
  - both: the intrinsics (`coby` evaluates them, `cobc` emits a C
    constant; the shared checker types them);
  - the shared checker enforces `[T-Convert]` and `[T-Alt]`. Before,
    `coby` faulted `diag.type-mismatch (dynamic)` on a non-integer
    operand, `cobc` stopped with an internal error, and both accepted
    `wrapping_add(1: i32, 2: i64)`.

## Affected entities

`rule.arith.limits` (new); `rule.arith.convert`, `rule.arith.alt`
(typing stated); `rule.type.kind` (text); `rule.stdlib.prelude` (two
intrinsics).

## Previous semantics

- No intrinsic gave a type's limits.
- The conversions' and alternative operations' premises named their
  operand types (`τ integer`, `τ ∈ {f32, f64}`, one `τ`), but no rule
  said what a program that failed them was.

## New semantics

    [Min-Value]            τ integer    Γ ⊢ min_value<τ>() : τ    ⟨min_value<τ>(), Σ⟩ → ⟨min(τ), Σ⟩
    [Max-Value]            τ integer    Γ ⊢ max_value<τ>() : τ    ⟨max_value<τ>(), Σ⟩ → ⟨max(τ), Σ⟩
    [Limits-Not-Integer]   τ not integer                              diag.type-mismatch (static)
    [Limits-Uninferable]   no type argument                           diag.cannot-infer-type-parameter (static)
    [T-Convert]            operand or target of the wrong kind        diag.type-mismatch (static)
    [T-Alt]                operands not of one integer type           diag.type-mismatch (static)

## Affected invariants

`inv.arith.range-validity`: unchanged.

## Dependency impact

`rule.arith.convert` and `rule.arith.alt` now depend on D-0026;
`rule.arith.limits` depends on `rule.type.kind`.

## Compatibility classification

- **Extension:** the two intrinsics.
- **Breaking, in principle:** programs whose conversion or alternative
  operation had operands of the wrong type are rejected statically.
  Those with a non-integer operand already faulted in `coby`; those
  with two different integer types were accepted.

## Migration implications

None in this repository: every program, guide example and showcase
already met `[T-Convert]` and `[T-Alt]`.

## Example changes

None.

## Conformance changes

**Added:**
- `conf.limits-max-i32`
- `conf.limits-values`
- `conf.limits-generic-ok`
- `conf.limits-not-integer`
- `conf.limits-generic-not-integer`
- `conf.limits-uninferable`
- `conf.limits-overflow-dynamic`
- `conf.alt-mixed-types-rejected`
- `conf.alt-non-integer-rejected`
- `conf.alt-generic-non-integer-rejected`
- `conf.convert-wrong-kind-rejected`

## Future implementation implications

- **`u128`'s maximum** does not fit a signed 128-bit host integer.
  Both implementations hold it as its bit pattern, as they hold the
  literal.
- **Literal operands.** `[T-Alt]`'s one-type check must let an
  unsuffixed literal take the other operand's type:
  `wrapping_add(x, 1)` for `x : u8` is well-typed.

## Prior-art status

See D-0026.

## Revisit conditions

See D-0026.
