# D-0039 — One Way to Write Output: `printf`, `%v`, `sprintf`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0024, D-0028, D-0038, rule.stdlib.print, rule.stdlib.format
Affects: rule.stdlib.print, rule.stdlib.format, rule.stdlib.prelude

## Problem

After D-0038 a program had two ways to write output: `print(x)`, one
value as it is, and `printf(f, …)`. They overlap: `printf("%d", x)`
and `print(x)` write the same text, and a line mixing text and values
was either several `print` calls or one `printf`. Two spellings for one
job is one more thing to learn and to choose between in every program.
What `print` alone offered was "write this value, whatever its type,
in the form that reads back", which no C conversion does (`%g` drops
digits).

## Constraints

- No new tokens.
- Nothing `print` could write becomes unwritable.
- `printf`'s C conversions keep C's text.

## Candidate mechanisms

`print`: **private to `std`, every program migrated** (selected), kept
as a shorthand, or kept but not taught. A `String` result: **`sprintf`
beside `String::appendf`** (selected), or `appendf` only. Other
streams: `fprintf`/`eprintf` now, or **later** (selected).

## Selected design

- `%v`: a conversion taking any printable type (`str`, every integer,
  `f32`, `f64`, `bool`, `&String`) and writing `print`'s text for it —
  integers in decimal, floats in the shortest form that reads back.
  It takes a width and `-`, nothing else.
- `sprintf(f, …) : String`: the text `printf` would write, as a new
  `String`.
- `print` stays in `std` as the function `printf` is defined through,
  but is not exported: `print(x)` in a program is unbound, and
  `std::print` is not visible. A program may declare its own `print`.
- Every example, test, showcase program and guide section writes
  output with `printf`.

## Rejected alternatives

- **Keep `print` as a shorthand:** `printf("%v\n", x)` costs six
  characters over `print(x); print("\n");`, and removes the choice.
- **`appendf` only:** `String s = String::new(); String::appendf(&mut
  s, …)` for every built string is two statements where C programmers
  expect one call.
- **`fprintf`/`eprintf`:** standard error needs a stream model this
  decision does not need; left as a revisit condition.

## Semantic rationale

`[Printf] ≡ print(format(…))`, `[Appendf] ≡ String::append(s,
format(…))`, `[Sprintf] ≡ String::from_str(format(…))`; `%v` is
`text(v)` of `rule.stdlib.print`. No rule of aliasing, destruction or
typing changes; `print` only loses its `export`.

## Usability

    printf("%v items, mean %v\n", n, mean);
    String label = sprintf("%s-%03d", &prefix, id);

## Explainability

"Output is `printf`; `%v` writes any value as CobaltC does."

## Implementation-feasibility

The front end already rewrote `printf` to `std::print($fmt(…))`;
`sprintf` is rewritten to `std::String::from_str($fmt(…))`. `%v` is a
fourth kind in `impl/src/fmt.rs`, rendering through the same
`format_float` both implementations use for `print`.

## Compatibility impact

Breaking for programs calling `print` (all of the repository's were
migrated mechanically: `print(x)` → `printf("%v", x)`, a literal →
`printf` of the literal with `%` doubled). A program's own `printf`,
`sprintf` or `print` takes precedence (D-0024).

## Prior-art status

Go `%v` (the value in its default format); Rust `{}`; C `sprintf`
(here returning a new `String`, so there is no buffer to overrun).

## Invariant traceability

`inv.string.utf8-validity`: `sprintf` builds its `String` from
formatted text, which is UTF-8 (D-0038).

## Revisit conditions

- Standard error (`eprintf`, or `fprintf` with a stream type).
- `%v` of composite values (`Vec`, `Option`, structs).
