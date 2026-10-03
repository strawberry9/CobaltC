# D-0074 — A constant as an array length

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (item 7), §9
Depends on: D-0036, D-0066, D-0072
Affects: `spec/22` (`length`)

## Problem

An array length had to be a number: `array<u8, W>` and `[0; W]` were
syntax errors, even with `const usize W = 24;` declared. A grid program
wrote `24` and `12` five times over. D-0072 listed this as its revisit
condition.

## Constraints

- No new tokens.
- A length stays a value known when the program is read: the type
  `array<T, N>` is fixed before anything runs.

## Candidate mechanisms

1. **The name of a constant whose initializer is an integer literal.**
   Selected. It covers `const usize W = 24;`, the common case, and the
   parser knows the value directly.
2. **Any constant expression (`W * H`, `N + 1`).** This needs constant
   evaluation before types are formed. `consts` folds after name
   resolution, which already needs the types. That's more machinery than
   the friction warrants now.
3. **Nothing.** The friction stays.

## Selected design

Candidate 1:
- **`length ::= int-literal | identifier`.** The identifier names a
  `const` of an integer type whose initializer is an integer literal.
  The name is looked up among the program's constants by name.
- **Ambiguity:** a name declared as two constants of different values
  must be written as a number (a syntax error says so).
- **Not a length:** a name that is not such a constant is a syntax
  error naming the rule.

## Rejected alternatives

- **2**, for now: see the revisit conditions.
- **3:** the friction met.

## Semantic rationale

A constant with a literal initializer is that literal under another
name (D-0036). The array type is the same as if the number were written.

## Usability

    const usize W = 24;
    const usize H = 12;
    array<array<u8, W>, H> grid = [[0; W]; H];

## Compatibility impact

Extension. Programs that were rejected are accepted.

## Revisit conditions

- A constant expression as a length (`W * H`).
