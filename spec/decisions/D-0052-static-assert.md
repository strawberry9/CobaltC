# D-0052 — `static_assert`

Status: ACCEPTED (2026-09-26, owner chose option B)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0026, D-0036, rule.module.const, rule.type.kind
Affects: rule.module.static-assert (new), `spec/21` §0

## Problem

A program could compute a constant `bool` (`const bool OK =
sizeof<H>() == 16;`) but not make the program ill-formed when it is
false; the only way to fail at compile time was an arithmetic trick in
a `const` initializer. Three uses were left without a clean spelling:
checking a struct's layout against a C header, checking relationships
between constants, and bounding a generic function's `T` (CobaltC has
no trait bounds).

## Candidate mechanisms

A. Nothing; document the `const` trick. B. An intrinsic
`static_assert(c)` / `static_assert(c, "message")` used as a statement,
computed when its function is checked, per instantiation of a generic
body. C. B plus a module-level item form. D. Defer.

## Selected design

B. The condition is a constant expression (`spec/17` §1a) of type
`bool`, which may name the body's type parameters (`sizeof<T>()`); the
message, a string literal. The value is computed before the program
runs by the evaluator itself (a constant expression has no effect), so
it is the value it would have at run time. False is
`diag.static-assert-failed` (static), showing the message; a checked
failure while computing it is that failure, statically; a condition
that is not constant is `diag.const-not-constant`. At run time the
call does nothing.

## Rejected alternatives

A: the trick cannot express `==` cleanly. C: module-level syntax for
something any function body can hold. D: the generic-bound use is worth
having now.

## Semantic rationale

Constant expressions already have a value independent of when they are
computed; asserting one is a statement about the program, so it is
checked as the program is: every function, called or not, and every
instantiation.

## Usability

    static_assert(sizeof<Header>() == 16, "Header must match the C struct");
    static_assert(BUF & (BUF - 1) == 0, "BUF must be a power of two");
    fn pack<T>(T x) { static_assert(sizeof<T>() <= 8, "T must fit in 8 bytes"); … }

## Explainability

"`static_assert(c, "why")` stops the build when `c` is false."

## Implementation-feasibility

The static pass types the call and records it with the instantiation's
type arguments; `lib.rs` then computes each recorded condition with the
interpreter before the program runs, for both `coby` and `cobc`. The
message travels in the diagnostic and is rendered after its location.

## Compatibility impact

Extension: one intrinsic name; a program's own `static_assert` function
takes precedence (D-0024).

## Prior-art status

C11 `_Static_assert`, C++ `static_assert`, D `static assert`, Rust
`const _: () = assert!(…)`.

## Invariant traceability

None (a static check only).

## Revisit conditions

A module-level form, if programs want assertions beside the
declarations they check.
