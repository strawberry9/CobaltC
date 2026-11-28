# CHG-0105 — Built-in bounds on a function's type parameters

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9
Depends on: D-0090
Affects: rule.type.kind, rule.type.bound, the grammar, the implementations

## Problem / motivation

A generic body could do nothing with a value of its type parameter
beyond moving it (D-0090).

## Decision

D-0090.

## What changed

- **`spec/12` 1.15.0:** `rule.type.bound` (`[T-Bound-Op]`,
  `[T-Bound-Literal]`, `[Bound-Unsatisfied]`, `[Bound-Missing]`);
  `rule.type.kind` refers to it.
- **`spec/22` 2.31.0:** `type-param`, `bound`.
- **`spec/registry/diagnostics.md` 1.37.0:** `diag.unbounded-type-parameter`
  names bounds as the repair.
- **Implementations:** `src/ast.rs` `Bound`, `FnDecl::type_bounds`;
  `src/parser.rs` `parse_bounded_type_params`; `src/typecheck.rs`
  (`marker_bounds`, `check_bounded_binary`, the literal and call-site
  checks; generic bodies are checked before their instantiations);
  `number_literals` in coby and cobc; coby infers a type parameter from
  a `slice<T, m>` argument (`unify_type_shape`), which a `number` body's
  literal needs.
- **Guide:** §12's "Bounds", "monomorphic" explained.

## Compatibility classification

Additive.

## Conformance changes

**Added:** `conf.bound-ordered`, `conf.bound-number-literal-float`,
`conf.bound-missing-rejected`, `conf.bound-unsatisfied-rejected`,
`conf.bound-weaker-caller-rejected`, and four `.cb` cases under
`impl/conformance/12-type-system/`.

## Revisit conditions

None.
