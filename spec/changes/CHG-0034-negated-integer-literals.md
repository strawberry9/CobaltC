# CHG-0034 — `[Literal-Negated]`, and `[T-Neg]` and `[Div-Overflow]` as Implemented

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-24, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0025, rule.arith.literal, rule.arith.neg, rule.arith.div, rule.type.typing
Affects: rule.arith.literal, rule.arith.neg, conf.i32-div-min-neg-one, the §1 cases listed below

## Problem / motivation

D-0025 has the details. In short, `spec/06` read `-2147483648` as a
minus applied to the out-of-range literal `2147483648`, and so gave no
way to write a signed type's minimum as a literal. `spec/examples.md`
and both implementations accepted it anyway.

## Decision

D-0025: a minus applied directly to an integer literal is range-checked
as one value.

## What changed

- **`spec/06` 1.5.0:**
  - `[Literal-Negated]` added to `rule.arith.literal`;
  - `rule.arith.neg`'s note says `[Neg]` governs every operand but a
    literal the minus is applied to directly.
- **`spec/conformance.md` 3.19.0:** seven §1 cases added.
  `conf.i32-div-min-neg-one`'s derivation no longer says `min(i32)`
  cannot be written as a literal.
- **Implementations** (`coby`'s checker is shared by `cobc`):
  - `[Literal-Negated]` for `i128` accepted only the minimum itself, so
    `i128 x = -1;` was `diag.literal-out-of-range`. Fixed in the checker
    and in `coby`'s evaluator.
  - `[T-Neg]`: unary minus on an unsigned operand was accepted. It is
    now `diag.type-mismatch`, statically. Before, `coby` produced an
    out-of-range value, and `cobc` wrapped, or faulted on `-0`.
  - `[Div-Overflow]` with both operands literal was reported as
    `diag.arith-overflow (static)`. It is now `diag.div-overflow
    (static)`, and a literal `min % -1` is refuted as well.

## Affected entities

`rule.arith.literal` (clause added); `rule.arith.neg` (note).

## Previous semantics

    -L   =   -(L)   with L range-checked alone by [Literal-In-Range]

so `-2147483648` was `[Literal-Out-Of-Range]`.

## New semantics

    [Literal-Negated]   L an integer literal with determined type τ; `-` applied directly to L
                        signed(τ)    -val(L) ∈ represented-domain(τ)
                        ⊢ -L : τ;   ⟨-L, Σ⟩ → ⟨-val(L), Σ⟩

## Affected invariants

`inv.arith.range-validity`: unchanged in force. The range check is made
on the value the expression denotes.

## Dependency impact

`rule.arith.literal` now depends on `rule.type.typing` (`[T-Neg]`'s
signedness premise) and D-0025.

## Compatibility classification

- **Specification:** extension. `-L` whose magnitude is `max(τ) + 1` is
  newly well-formed. Every other program means what it meant.
- **Implementations:**
  - newly rejected: unary minus on an unsigned operand, which `[T-Neg]`
    always rejected;
  - newly accepted: negated `i128` literals other than the minimum;
  - reported differently: a literal `min / -1` or `min % -1`.

## Migration implications

None in this repository: no program negated an unsigned value or
relied on the old diagnostics.

## Example changes

None. `spec/examples.md` already wrote `-2147483648`.

## Conformance changes

**Added:**
- `conf.negated-literal-min`
- `conf.negated-literal-signed-mins`
- `conf.negated-literal-below-min`
- `conf.negated-literal-parenthesized`
- `conf.negated-literal-unsigned-rejected`
- `conf.neg-unsigned-rejected`
- `conf.div-min-neg-one-static`

## Future implementation implications

- **Only directly.** An implementation recognizes `-L` in the syntax
  tree. It must not fold `-(L)` or `- - L` into it.
- **128-bit.** `2^127` has no positive `i128` value, so the negation of
  that one magnitude must not go through a host `i128` negation.

## Prior-art status

See D-0025.

## Revisit conditions

See D-0025.
