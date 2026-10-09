# D-0067 — A diagnostic of its own for a `static_assert` that cannot be computed

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §19
Depends on: D-0052, D-0065, D-0066
Affects: rule.module.static-assert, `diag.static-assert-not-constant` (new)

## Problem

`static_assert(x == 1)` with a local `x` was rejected before the
program ran, but with `diag.const-not-constant`, whose text was written
for `const` declarations: it spoke of "a `const`'s type" and "the
`const` declaration" and suggested computing the value in a function.
It did not mention `static_assert`, the local variable, `assert`, or a
local `const`. A condition that was not a `bool` gave the generic
`diag.type-mismatch` with its list of unrelated typing rules. The owner
read both as the check failing late.

## Constraints

- A diagnostic says which rule failed and what to do instead (Master
  Instructions §19).
- No change to which programs are accepted.

## Candidate mechanisms

1. **`diag.static-assert-not-constant`**, for a condition that is not a
   constant expression or not a `bool`, whose repair names `assert` and
   a local `const`. **Selected.**
2. Rewrite `diag.const-not-constant`'s text to cover both uses.

## Selected design

Candidate 1. A type error inside the condition (`1 == "a"`) is still
that error, and a message that is not a string literal is still
`diag.type-mismatch`. `diag.const-not-constant`'s text is also brought
up to date for local constants (D-0066).

## Rejected alternatives

- **2:** one text for two situations stays vague about both; a
  `static_assert` has its own remedies.

## Semantic rationale

None changed: the same programs are rejected, at the same phase.

## Usability

    i32 x = 1;
    static_assert(x == 1);   // ✗ diag.static-assert-not-constant:
                             //   use assert(x == 1), or const i32 X = 1;

## Implementation-feasibility

The checker types the condition itself and reports the new id when its
type is not `bool` or it is not a constant expression.

## Compatibility impact

None for programs; the id reported for these rejections changes.

## Prior-art status

- **C++:** "static assertion expression is not an integral constant
  expression" is its own error.
- **Rust:** `const` contexts report "cannot call non-const fn" and
  "attempt to use a non-constant value in a constant" separately.

## Revisit conditions

None.
