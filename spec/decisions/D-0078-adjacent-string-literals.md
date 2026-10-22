# D-0078 — Adjacent string literals are one literal

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: `spec/22` §1 (`str-literal`), rule.stdlib.format
Affects: `spec/22` §1

## Problem

A `str-literal` must close on its own line, and nothing joins two
literals. A program that embeds more than a line of text, such as a
table of test data, a small script its own interpreter runs, or a usage
message, has to write it as one line full of `\n`. That line often
runs to hundreds of columns. `printf`'s format must be a single literal
too, so a long format could not be split either.

## Candidate mechanisms

1. **Adjacent literals are one literal** (`"ab" "cd"` is `"abcd"`),
   joined before parsing, as in C. Selected. No new token or form: a
   C programmer already reads it this way. Everywhere a literal is
   required (a format string, `extern "…";`, a module file name), a
   joined one is still a literal.
2. **Multi-line literals** (a newline allowed inside `"…"`). The
   program's indentation would become part of the text, and a missing
   `"` would swallow the rest of the file before any error.
3. **A raw or block literal form** (`"""…"""` or similar). A new token
   and new lexical rules for a convenience that option 1 already gives.
4. **`+` on `str`.** `str` has only `==` and `!=` (`spec/12` §4), and a
   `+` of two literals would be a run-time value, not a literal, so it
   could not be a format.

## Selected design

- **Lexical (`spec/22` §1):** two `str-literal`s with only whitespace
  and comments between them are one `str-literal`. Its text is theirs
  in order. This repeats, so `"a" "b" "c"` is `"abc"`.
- Byte literals (`b"…"`) are not joined. An array's length comes from
  its one literal, and nothing asked for joining them.

```
const str USAGE = "usage: tool [-v] FILE\n"
                  "  -v  say what is done\n";
printf("%v items, %v bytes, "
       "%v skipped\n", n, size, skipped);
```

## Compatibility impact

None. Two adjacent literals were a syntax error before.

## Revisit conditions

If byte literals need the same.
