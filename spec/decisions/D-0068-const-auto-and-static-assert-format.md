# D-0068 — `const auto`, and values in a `static_assert`'s message

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0036, D-0052, D-0065, D-0066
Affects: rule.module.const, rule.module.static-assert, `spec/22`
`const-decl` and `statement`

## Problem

Writing a constant beside its computation (D-0066) needed its type
spelled out where a variable's would not: `auto b = a + 1;` but
`const u8 B = A + 1;`. And `assert` takes a format and values
(D-0065) while `static_assert` took only a plain message; a program
written the way `assert` is written (`static_assert(b == 202, "b is
%v", b)`) was rejected with a message that did not say why.

## Constraints

- No new tokens (`auto` is already one); both assertions read alike.

## Candidate mechanisms

1. **`const auto N = e;`**, typed as `auto x = e;`. **Selected.**
2. Types always written for constants.
3. **`static_assert(c, "format", a…)`**, each argument a constant
   expression, formatted before the program runs. **Selected.**
4. A plain message only.

## Selected design

Candidates 1 and 3. A constant whose initializer does not determine its
type (`None`) needs the type written (`diag.type-mismatch`). A
`static_assert`'s message arguments are constant expressions, since the
assertion is computed before the program runs; the message is formatted
only when it fails, as `assert`'s is.

## Rejected alternatives

- **2:** a `const` would be the one declaration whose type cannot be
  inferred from what initializes it.
- **4:** two assertions with different message forms invite exactly the
  mistake the owner made.

## Semantic rationale

`auto` means the same in both declarations. A formatted message is
`printf`'s `format` of constant values: a constant expression itself.

## Usability

    const auto KIB = 1024: u64;
    const auto MIB = KIB * KIB;                  // u64
    static_assert(MIB == 1048576, "MIB is %v", MIB);

## Implementation-feasibility

`consts` synthesizes each `const auto`'s type from its initializer
after name resolution, before anything reads it; constant expressions
are a closed set, so the synthesis is complete for them. The checker
types a `static_assert` message as `$fmt(format, a…)`, and the front end
computes it when the assertion fails.

## Compatibility impact

Extension.

## Prior-art status

- **D:** `enum b = a + 1;` (a manifest constant, type inferred).
- **C23:** `constexpr auto`.
- **Rust:** `const` requires its type; `static_assert` has no equivalent,
  `const _: () = assert!(…)` with `panic!` formatting in const contexts.

## Revisit conditions

None.
