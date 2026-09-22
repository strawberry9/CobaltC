# D-0043 — Digit Separators in Numeric Literals

Status: ACCEPTED (2026-09-26, owner-proposed)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §17
Depends on: D-0033, D-0035, D-0032
Affects: spec/22 §1 (`digits`), the cases listed in CHG-0051

## Problem

Long literals are hard to read and easy to get wrong:
`1000000000` versus `100000000`, `0xFFFF0000` versus `0xFFF0000`.
Radix literals (D-0035) made long hexadecimal and binary masks common.

## Constraints

- No new token: the change is inside the lexing of a number.
- A literal's value never depends on where separators are.
- `parse<T>` stays as strict as D-0032 made it.

## Candidate mechanisms

**`_` between two digits** (selected); `_` anywhere after the first
digit (Rust: `1__0`, `1_`, `0x_1` allowed); `'` (C++14: `1'000`), which
CobaltC already uses for `b'x'`.

## Selected design

`digits(d) ::= d ('_'? d)*` for the integer, fraction and exponent
digits of every literal and for the digits after `0x`, `0o`, `0b`. A
`_` elsewhere — doubled, leading, trailing, beside the prefix, the
decimal point, `e` or the exponent's sign — is a lexical error whose
message says where a `_` may go. Any grouping is allowed.

## Rejected alternatives

- **Rust's looser rule:** `1__000` and `1000_` are more likely typos
  than intent, and accepting them buys nothing.
- **`'`:** taken by byte literals; unfamiliar outside C++.
- **Accepting `_` in `parse<T>`:** text a program reads is data; D-0032
  rejects anything but digits, and "1_000" typed by a user is more
  likely a mistake than a number.

## Semantic rationale

Purely lexical: the token is the literal with its `_`s removed, so no
typing or evaluation rule changes.

## Usability

    u64 ns = 1_000_000_000;
    u32 mask = 0xFFFF_0000;
    f64 avogadro = 6.022_140_76e23;

## Explainability

"A `_` between two digits is ignored."

## Implementation-feasibility

The shared lexer's `digit_run`; both implementations use it.

## Compatibility impact

Extension: every such text was previously a lexical or parse error.

## Prior-art status

Rust, Java 7, C# 7, Python 3.6 (`_`, looser placement); Swift, Kotlin;
C++14 (`'`); Ada (`_` between digits only, as here).

## Invariant traceability

None.

## Revisit conditions

None expected.
