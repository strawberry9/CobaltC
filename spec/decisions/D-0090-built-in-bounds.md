# D-0090 — Built-in bounds on a function's type parameters

Status: ACCEPTED (2026-09-28, the owner's decision: option 4 of the four presented)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0010, D-0026, rule.type.kind
Affects: `spec/12` §1, `spec/22` §3, `spec/registry/diagnostics.md`, the implementations

## Problem

A type parameter `T` had no operations: a generic body could store,
pass and move a `T`, not compare or add one. Real programs met it
repeatedly: no generic `max` or `sum`, no binary search over numbers,
comparisons always passed in as `fn` values (`Vec::sort_by(less)`, a
heap's `before`), and `min`/`max`/`abs`/`pow` could not be written in
`std`.

## Candidate mechanisms

1. **Keep passing operations as `fn` parameters.** Explicit; verbose;
   some generic code (a generic `max` of two values) needs an argument
   that only restates an operator.
2. **Traits or concepts with user implementations** (`trait Ord`,
   `impl Ord for P`). The most expressive; new keywords, implementation
   lookup and coherence rules, and dispatch in both tools.
3. **Checking only at instantiation (templates).** No new syntax, but a
   generic body is no longer checked where it is written, and errors
   appear at distant uses.
4. **A closed set of built-in bounds**, `<T: ordered>`, each naming the
   built-in types that share a set of operators. Selected.

## Selected design

- `type-param ::= identifier (':' bound)?`, `bound` one of the
  contextual words `eq`, `ordered`, `number`, `integer`, on a function's
  type parameters only. `integer` ⇒ `number` ⇒ `ordered` ⇒ `eq`.
- `eq`: the integer and floating-point types, `bool`, `str`; `==`, `!=`.
  `ordered`: the integer and floating-point types; adds `<` `<=` `>`
  `>=`. `number`: the same types; adds `+ - * / %`, unary `-` and the
  compound assignments, and an unsuffixed integer literal where `T` is
  expected is of type `T` (at a floating-point `T`, that value as a
  float). `integer`: the integer types; adds `& | ^ ~ << >>`.
- The body is checked once against the bound (`[Bound-Missing]`:
  `diag.unbounded-type-parameter`); each call against the types it
  fixes (`[Bound-Unsatisfied]`: `diag.type-mismatch`), a caller's own
  type parameter by a bound implying the callee's. Each instantiation
  is still checked at its concrete types.
- Built-in bounds describe compiler-defined capabilities. They are not
  user-extensible traits: no declaration makes a new bound, and no
  struct or enum satisfies one.

## Compatibility impact

Additive: a program without bounds means what it meant. `eq`,
`ordered`, `number` and `integer` stay ordinary identifiers elsewhere.

## Revisit conditions

If user types ever need to satisfy a bound, option 2 is the extension;
this design does not preclude it.
