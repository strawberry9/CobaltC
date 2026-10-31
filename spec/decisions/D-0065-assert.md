# D-0065 — Runtime assertions

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0009, D-0038, D-0052, rule.fail.fault-unwind
Affects: rule.fail.assert (new), rule.stdlib.prelude

## Problem

A program could check a constant before it runs (`static_assert`,
D-0052) but not a condition as it runs. The language guide defined its
own `check(bool ok)` that divides by zero when `ok` is false, and many
conformance cases and showcases end a failed check with `1 / (1 - 1)`:
the failure they report, `diag.div-by-zero`, says nothing about what was
wrong.

## Constraints

- No new tokens; the same in both tools.
- Every other check in CobaltC is always on.
- A failure should say what was assumed and what the values were.

## Candidate mechanisms

1. **`assert(c)` and `assert(c, f, a…)`, a function of `std`, the
   message optional and formatted as `printf`'s.** **Selected.**
2. The message required.
3. Assertions that can be turned off (C's `NDEBUG`).

## Selected design

Candidate 1. A false `c` is a checked fault, `diag.assert-failed`,
reporting the formatted message with the call's location; the program
unwinds as for any fault. The message is optional, as `static_assert`'s
is, and may carry values: `assert(n <= cap, "n = %v exceeds cap %v", n,
cap)`. The condition is always evaluated, once; the message and its
arguments only when the condition is false.

## Rejected alternatives

- **2:** `static_assert` takes an optional message; `assert` does the
  same, so the two read alike. The failure always names `assert` and its
  line, which the division-by-zero workaround never did.
- **3:** a check that can be removed is a check a release build does
  not make; CobaltC makes all of its checks.

## Semantic rationale

`assert` is `if (!(c)) { fail(message) }`: a condition the program
states and the language checks, as it checks an index or an overflow.

## Usability

    assert(Vec::len(&v) == n, "expected %v items, got %v", n, Vec::len(&v));

## Implementation-feasibility

`modres` rewrites the call as `printf`'s is rewritten (D-0038): an `if`
around `$assert_fail($fmt(f, a…))`; `$assert_fail` faults with the text
as the diagnostic's message, the mechanism `static_assert` uses
(`coby`), and `cb_fault_msg` (`cbrt`).

## Compatibility impact

Extension. A program's own `assert` takes precedence (D-0024, D-0055).

## Prior-art status

- **C:** `assert(expr)` (`<assert.h>`), removed under `NDEBUG`.
- **Rust:** `assert!(cond, "fmt", args)`, always on; `debug_assert!`
  only in debug builds.
- **Go:** none; tests use `t.Fatalf`.

## Revisit conditions

- `assert_eq`-style forms that show both values without a format.
