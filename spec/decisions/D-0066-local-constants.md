# D-0066 — Local constants, and fields and elements of constants

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0036, D-0052, D-0058
Affects: rule.module.const, `spec/22` `statement`

## Problem

A constant could be declared only at a module's top level. A value a
function computes from literals (`u8 b = 200; auto d = b + 1;`) was
known before the program ran, yet `static_assert(d == 201)` was
rejected: `d` is a local variable, and a variable is not a constant
expression. The only ways to check it before running were to move the
value to the module's top level, away from the code it belongs to, or
to fall back to a run-time `assert`. Separately, a field of a constant
struct (`SCREEN.w`) or an element of a constant array was not a
constant expression, even though its value is fixed.

## Constraints

- No new tokens; the fewest new rules.
- Whether `static_assert` accepts a condition must be visible from the
  declarations it names, not from what the rest of the function does.

## Candidate mechanisms

1. **`const τ N = e;` as a statement**, scoped like a local.
   **Selected.**
2. A local variable counts as a constant when its initializer is a
   constant expression and nothing later writes, borrows exclusively or
   moves it.
3. Constants at module level only.

And, independently:

4. **A field of a constant expression, and an element of one at a
   constant index, is a constant expression.** **Selected.**

## Selected design

Candidates 1 and 4. A local constant is a module constant in all but
the scope of its name: from its statement to the end of its block,
shadowing and shadowed as a local variable is. Its initializer may name
other constants — module or local — but not a local variable or a type
parameter of the function.

## Rejected alternatives

- **2:** constness would be invisible and non-local: a later
  `d = 5;`, or `f(&mut d)` twenty lines on, would make an earlier
  `static_assert` stop compiling, with the error far from the edit; and
  the exact rule would have to be specified and agreed by every
  implementation.
- **3:** a function's own constants belong in the function.

## Semantic rationale

A constant has no object and one value; where its name is written is a
matter of scope only. Hoisting a local constant to its module with a
name no program can write gives it every property of a module constant
without a second set of rules.

## Usability

    fn checksum(slice<u8, shared> data) : u32
    {
        const u32 PRIME = 16777619;
        const u32 SEED = 2166136261;
        static_assert(PRIME % 2 == 1);
        …
    }

    static_assert(SCREEN.w * SCREEN.h == 307200);

## Implementation-feasibility

The parser keeps the scopes of the function it parses: each local
constant becomes a module constant `N$k`, and each use of `N` within its
scope is renamed to it; a local variable or type parameter in its
initializer is `diag.const-not-constant`. Everything after the parser
sees a module constant. The checker's constant expressions take fields
and elements.

## Compatibility impact

Extension.

## Prior-art status

- **C23:** `constexpr` objects at block scope.
- **C++:** `constexpr` local variables.
- **Rust:** `const` items inside function bodies, scoped to the block.

## Revisit conditions

- A local constant that depends on the function's type parameters.
