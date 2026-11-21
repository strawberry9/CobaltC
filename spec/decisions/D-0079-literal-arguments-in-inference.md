# D-0079 — A literal argument takes its type from the call

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0012, D-0013, D-0037, rule.fn.generic-call, rule.type.expected
Affects: `spec/15` §5

## Problem

`[Generic-Call-Inferred]` takes each type parameter from what its
arguments synthesize, and an unsuffixed literal synthesizes its default
type. With `i64 x`, `max(0, x)` therefore saw `T` as `i32` from the `0`
and `i64` from `x`, and was rejected. `i64 total = fold(&v, 0, add)`
fixed the accumulator's type as `i32` from the `0` and then rejected
the closure written for `i64`. Elsewhere a literal already takes its
type from its context: an operand from the other operand (D-0037), an
initializer from its declaration, and an argument from its parameter
once that is known. Only a generic parameter, where the literal was
itself the evidence, did otherwise. The workarounds (`0: i64`,
`max<i64>(0, x)`) are noise in exactly the code where generics should
be quiet.

## Candidate mechanisms

1. **Literal arguments are evidence last.** Inference uses the other
   arguments, then the expected type. A literal argument is then
   checked against its parameter's type as solved so far. Only a type
   parameter that nothing else fixes takes a literal's default type.
   Selected. It is D-0037's rule applied to a call: the literal takes
   the type of the things it is combined with.
2. **Unify literals with every candidate** (a literal fits any integer
   type it is in range for). This is more general, but a literal whose
   parameter is fixed by nothing but other literals would still need a
   default, which option 1 already gives, at the cost of a search.
3. **Leave it.** The suffix works, but the rejection is surprising and
   its message cannot easily say why.

## Selected design

- **`spec/15` §5, `[Generic-Call-Inferred]`:** the arguments that fix
  type parameters are those that are not unsuffixed numeric literals (a
  literal, possibly negated or parenthesized). After them comes
  `[Generic-Call-Expected-Type]`, and then an unsuffixed literal
  argument is typed by its parameter's type under the parameters solved
  so far (`rule.type.expected`). A type parameter still unsolved takes
  such an argument's default type (`i32`, `f64`).
- **Order of evaluation is unchanged:** arguments are evaluated left to
  right. A literal has no effects, so the order in which the checker
  and the tools consider evidence is not observable.

```
fn larger<T>(T a, T b) : T { if (a > b) { a } else { b } }

i64 x = -5;
i64 m = larger(0, x);                                  // T = i64 from x
i64 s = fold(&v, 0, [](i64 a, ref<i64, shared> e) { a + *e });   // A = i64
auto k = larger(3, 4);                                 // T = i32: only literals
```

## Compatibility impact

Programs rejected before are accepted. A program that was accepted
keeps its meaning: a literal decided a type parameter only when every
other argument agreed with it, and then the result is the same.

## Revisit conditions

None.
