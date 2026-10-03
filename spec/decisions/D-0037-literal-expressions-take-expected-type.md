# D-0037 — Literal Expressions Take the Expected Type

Status: ACCEPTED (2026-09-25, owner-chosen: "yes i like: u64 x = 1024 * 1024;")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0006, D-0012, D-0013, D-0025, D-0036, rule.type.expected, rule.arith.literal
Affects: rule.type.expected, rule.arith.literal, rule.module.const

## Problem

`u64 x = 1024 * 1024;` was a type mismatch: each literal took its
type from the other operand, neither had one, so both defaulted to
`i32` and the product was an `i32`. `u64 y = 65536 * 65536;` was an
`i32` overflow. So was `const u64 MIB = 1024 * 1024;` (D-0036 recorded
it as a revisit condition). C programmers write exactly these.

## Constraints

- No implicit conversion (D-0006): a value never changes type.
- Nothing inferred from later statements (D-0012).
- Every existing well-typed program keeps its meaning.

## Selected design

A **literal expression** is an unsuffixed numeric literal, or `-`,
`~`, parentheses, or an arithmetic or bitwise operator applied only to
literal expressions (a shift's amount aside). At an operator whose
operands are all literal expressions, an expected number type is the
expected type of each operand (the left one of a shift). So the whole
literal expression is computed in the expected type, and each checked
operation is checked in it. With no expected type (`auto d = 65536 *
65536;`) the default is `i32` as before.

## Rejected alternatives

- **Keep the rule:** `1024: u64 * 1024` everywhere a C program writes
  `1024 * 1024`.
- **Compute in unbounded precision and check the result's range**
  (Go's untyped constants): a second arithmetic, and `255 + 1 - 1` as a
  `u8` would be accepted though its intermediate overflows the type it
  is said to be computed in.
- **Propagate into any operand** (`u64 x = y * 2` with `y : u32`): an
  implicit conversion of `y`; D-0006.

## Semantic rationale

Only literals change: a literal's type was already taken from its
context (`[Literal-Type-From-Context]`); this adds one context, the
operator whose operands are all literals. No value changes type and no
program that was accepted changes meaning: every newly accepted
program was a type mismatch or an `i32` overflow before.

## Usability

    u64 sixteen_gib = 16 * 1024 * 1024 * 1024;
    const u64 MIB = 1024 * 1024;
    i64 n = -(1 << 40) + 3;

## Explainability

"An expression of literals only takes the declared type" — one
sentence.

## Implementation-feasibility

`is_literal_expr` in the shared AST; the static pass checks such an
operator's operands with the expected type; `coby` evaluates them with
it; `cobc` already passed an arithmetic operator's expected type to its
operands; the constant folder types a literal expression with the
constant's type.

## Compatibility impact

Extension: only programs that were rejected (or overflowed as `i32`)
are affected.

## Prior-art status

- **Rust:** `let x: u64 = 1024 * 1024;` infers `u64` for both literals
  (full inference).
- **C:** `uint64_t x = 1024 * 1024;` computes in `int` and converts
  (and `65536 * 65536` overflows `int`) — the trap this avoids.
- **Go:** untyped constant arithmetic in arbitrary precision.

## Invariant traceability

`inv.arith.range-validity`: each operation is checked in the type it
is computed in, which is now the declared one.

## Revisit conditions

- A need for untyped constant arithmetic in arbitrary precision.
