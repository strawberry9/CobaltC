# CHG-0045 — Literal expressions take the expected type

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0037
Affects: rule.type.expected, rule.arith.literal, rule.module.const, the cases listed below

## Problem / motivation

D-0037: `u64 x = 1024 * 1024;` was a type mismatch.

## What changed

- **`spec/12` 1.10.0:** `rule.type.expected` passes an expected number
  type into an operator whose operands are all literal expressions.
- **`spec/06` 1.9.0:** `[Literal-Type-From-Context]` names it.
- **`spec/17` 2.2.1:** §1a's typing note.
- **`spec/conformance.md` 3.31.0:** the cases below.
- **`spec/02-schema.md` 1.0.21:** §5's "in use" ranges.
- **Implementations:** `ast::is_literal_expr`; typecheck
  (`check_binary` takes the expected type); `coby` (`eval_expected`
  on a literal operator, negation, `~`, parentheses); the constant
  folder (`consts.rs`). `cobc` already passed an arithmetic operator's
  expected type to its operands.

## Also in this change

- **`spec/22` 2.13.1** (non-normative): `for (…);`, `while (…);` and
  `if (…);` were already syntax errors (a body is a block); §3 says so,
  both implementations now name the mistake in the message, and three
  file cases (`22-surface-syntax/{for,while,if}_semicolon_body_parse_error.cb`)
  pin it.

## Compatibility classification

Extension: accepts programs that were rejected.

## Conformance changes

**Added:** `conf.literal-expression-typed`,
`conf.literal-expression-default`, `conf.literal-expression-negated`,
`conf.const-literal-expression`.

## Prior-art status

See D-0037.

## Revisit conditions

See D-0037.
