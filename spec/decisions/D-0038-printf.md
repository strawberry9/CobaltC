# D-0038 — Formatted Output: `printf` and `String::appendf`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0010, D-0028, D-0032, rule.stdlib.print, rule.stdlib.text
Affects: rule.stdlib.format (new), rule.stdlib.prelude, rule.stdlib.print, diag.format-invalid (new)

## Problem

`print` writes a value as it is. Fixed decimals (`3.14`), widths
(`   42`), zero padding and other bases had to be built by hand: the
Tier 5 `wc` right-aligned its columns itself, and `stats` could print
only shortest round-trip floats. C programs use `printf`. C's is unsafe:
nothing checks a format against its arguments, so `printf("%s", 42)`
compiles and crashes.

## Constraints

- No new tokens, and no general variadic functions.
- A format and its arguments are checked when the program is compiled.
- The same text from both implementations, and C's text where C
  defines it.

## Candidate mechanisms

Syntax: **C's `%` specifiers** (selected) or `{}` specifiers (Rust,
Python). Functions: **`printf` and `String::appendf`** (selected),
`printf` and `format(…) : String`, or `printf` only. Format: **a string
literal, checked when compiled** (selected), or also a run-time format
checked when the call runs.

## Selected design

- `printf(f, args…)` and `String::appendf(&mut s, f, args…)`, each
  call typed on its own with any number of arguments (as `spawn` is);
  `f` a string literal.
- C's specifiers `%[- 0 + space #][width][.precision]conv` with the
  conversions `d i u x X o b f F e E g G s` and `%%`, C's text for each,
  with four differences: `%d` takes every integer type (no length
  modifiers); `%x`/`%o`/`%b` print a value's bits in its own width;
  `%#o` writes `0o` (CobaltC's octal prefix); widths and precisions
  count characters, so UTF-8 aligns and is never split. `%s` takes a
  `str`, a `&String` or a `bool`.
- Rejected when compiled: a format that is not a literal or does not
  parse, or a flag its conversion does not take (`diag.format-invalid`);
  an argument of another kind, or the wrong number of arguments
  (`diag.type-mismatch`).
- Not included: `%c` (a byte above 127 in a `String` would break its
  UTF-8), `%p`, `%n`, `*` widths.

## Rejected alternatives

- **`{}` specifiers:** more uniform, but not what `printf` means to the
  programmers who ask for it.
- **`format(…) : String`:** a new `String` per call where
  `appendf` adds to one; `String::new()` then `appendf` is the same.
- **Run-time formats:** a class of errors moves from compile time to a
  run-time fault; a program choosing a format at run time chooses
  between literal-format calls.
- **General variadic functions:** a language mechanism for one library
  function.

## Semantic rationale

`printf(f, …)` is `print` of a `str`, and `appendf(s, f, …)` is
`String::append(s, …)` of one (`[Printf]`, `[Appendf]`): the text is
computed first, from copies of the arguments' values, so no rule of
aliasing or destruction changes. (`String::appendf(&mut s, "%s", &s)`
appends a copy of `s`; C's `sprintf(buf, "%s", buf)` is undefined.)

## Usability

    printf("%-10s %6d %8.2f %#06x\n", &name, count, ratio, mask);
    String::appendf(&mut line, "%3d%%", percent);

## Explainability

"`printf` is C's, checked when compiled" — one sentence.

## Implementation-feasibility

The front end rewrites each call to `print`/`String::append` of
`$fmt(f, …)`, a name no program can write, and checks it;
`impl/src/fmt.rs` parses and formats for the front end, `coby`, and
`cobc`'s runtime (`cb_format`, a per-thread buffer that `print` or
`append` copies from at once).

## Compatibility impact

Extension. A program's own `printf` takes precedence (D-0024).

## Prior-art status

C `printf`/`snprintf`; GCC's and Clang's `-Wformat` check literal
formats at compile time (as warnings); Rust `format!`/`println!`
(compile-time checked, `{}`); Zig `std.fmt` (comptime-checked); Go
`fmt.Printf` (checked by `go vet`).

## Invariant traceability

`inv.string.utf8-validity`: `appendf` adds only UTF-8 (ASCII digits,
signs and exponents; `str`/`String` text; widths never split a
character).

## Revisit conditions

- `%c` for ASCII bytes, with a check.
- Run-time formats.
- Formatting composite values.
