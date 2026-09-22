# D-0025 — A Minus Applied Directly to an Integer Literal Is Range-Checked as One Value

Status: ACCEPTED (2026-09-24, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0002, rule.arith.literal, rule.arith.neg, rule.type.expected
Affects: rule.arith.literal, rule.arith.neg, conf.i32-div-min-neg-one

## Problem

A signed type's minimum could not be written as a literal under the
letter of the rules. `spec/22` §1 makes literals unsigned digit strings,
and `spec/06` said `-1: i32` is `-(1: i32)`. So `-2147483648` was
`-(2147483648: i32)`, and `2147483648` is `[Literal-Out-Of-Range]` for
`i32` before any negation. `spec/conformance.md` said so
(`conf.i32-div-min-neg-one` writes `min(i32)` as `-2147483647 - 1`).

The specification disagreed with itself and with both implementations:
- `spec/examples.md` writes `div(-2147483648, -1)` and
  `neg(-2147483648)`;
- `coby` and `cobc` accept `-2147483648` (a fix recorded in
  `impl/STATUS.md`), range-checking the negated value.

No rule said which reading is right. CobaltC also has no named
constants for a type's limits (no `i32::MIN`), so the literal is the
only way to write a minimum.

## Constraints

- No new tokens (the owner's standing preference).
- Literals stay unsigned digit strings in the grammar (`spec/22` §1).
- Every range check the rules make today stays.

## Candidate mechanisms

1. **Range-check `-L` as one value.** When `-` applies directly to an
   integer literal, the literal's range check is made on `-val(L)`.
   **Selected.**
2. **Keep the letter of the rule.** Reject `-2147483648`, and fix
   `spec/examples.md` and both implementations to match. The minimum is
   written `-2147483647 - 1`.
3. **Named limits.** `i32::MIN` and `i32::MAX`, or `std` functions.

## Selected design

Candidate 1, as a clause of `rule.arith.literal`:

    [Literal-Negated]   L an integer literal with determined type τ; `-` applied directly to L
                        (whitespace allowed, no parentheses)
                        ⊢ -L : τ   requires signed(τ) (as [T-Neg])   and   -val(L) ∈ represented-domain(τ)

- **Only directly.** `-(2147483648)` is `[Neg]` applied to a
  parenthesized literal, which is checked on its own and is out of
  range. The clause is syntactic, not constant folding.
- **Typing unchanged.** `L`'s type comes from its suffix, the expected
  type, or the default (`[Literal-Type-*]`), as before. The suffix
  belongs to the literal: `-2147483648: i32` is covered.
- **Unsigned.** `-L` of an unsigned type is ill-typed (`[T-Neg]`), like
  any unary minus on an unsigned operand, including `-0: u8`.
- **Nothing else changes.** `(-2147483648) / -1` is still
  `[Div-Overflow]`, and `-(-2147483648)` is still `[Neg-Overflow]`.

## Rejected alternatives

- **2:** it rejects working code and gives nothing in return but
  `-2147483647 - 1`, the idiom C needs only because of the pitfall this
  decision removes.
- **3:** worth having eventually, but it is a separate decision.
  `i32::MAX` needs associated constants on primitive types, which is a
  new concept and new syntax. A later `std` addition could supply limits
  without new syntax. Either way the literal must be well defined, so
  the clause is needed regardless.

## Semantic rationale

The only literals the clause affects are the ones whose magnitude is
exactly one past the type's maximum. Every other negated literal has the
same value under both readings. So the clause changes no value. It only
accepts the one literal that denotes `min(τ)`.

## Usability

A program writes `i32 lo = -2147483648;`, the spelling C, Java and Rust
programmers know. No workaround is needed.

## Explainability

"A minus written directly before a number is part of the number" is one
sentence, and the only visible difference from ordinary negation is the
minimum.

## Implementation-feasibility

Both implementations already range-check the negated value, in the
checker and in `coby`'s evaluator. The work found three bugs next to it,
which `CHG-0034` fixes:
- for `i128`, only the minimum was accepted, so `i128 x = -1;` was
  `diag.literal-out-of-range`;
- `-2147483648 / -1` was reported `diag.arith-overflow (static)` instead
  of `diag.div-overflow (static)`, and a literal `min % -1` was not
  refuted statically;
- unary minus on an unsigned operand was accepted, against `[T-Neg]`.
  The two tools then disagreed at run time (`coby` produced an
  out-of-range value, and `cobc` wrapped, or faulted on `-0`).

## Compatibility impact

- **Accepted:** `-2147483648` and each signed type's minimum. It was
  ill-formed under the letter of the rules, though the implementations
  accepted it.
- **Rejected:** unary minus on an unsigned operand, which `[T-Neg]`
  always rejected but the implementations did not.

## Prior-art status

- **Java** (JLS §3.10.1): `2147483648` may appear only as the operand
  of unary minus. The same rule as this one.
- **Rust:** `-128i8` is negation of a literal, but the literal range
  check (`overflowing_literals`) accepts the magnitude when it is
  negated.
- **C:** `-2147483648` is the negation of a literal of a wider type,
  which is why `limits.h` writes `INT_MIN` as `(-2147483647 - 1)`.
- **Go:** constant expressions have arbitrary precision, so the question
  does not arise.

## Invariant traceability

`inv.arith.range-validity`: every literal value is still checked against
its type's represented domain. The check is made on the value the
expression denotes.

## Revisit conditions

- Named limits (candidate 3) being added.
- Hexadecimal or other literal forms, where a negated bit pattern could
  mean something different.
