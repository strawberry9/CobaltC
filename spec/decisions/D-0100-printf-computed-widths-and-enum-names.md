# D-0100 — printf: computed widths, and an enum's variant name

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 frictions 19, 25 and part of 26)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0038 (`rule.stdlib.format`)
Affects: spec/21 §2f

## Problem

A table whose column width comes from the data (the longest name) could
not be printed with `printf`: `%*d` and `%.*s` were rejected, so programs
padded by hand. And `%v` could not print an enum at all, so every program
reporting a `FileError` wrote its own `describe` function with a `match`
(`dirtree.cb`, `kvlog.cb`), and debugging output of a state enum needed
the same.

## Candidate mechanisms

Widths: **`*` as C has it, the argument a `usize`** (selected); or a
width expression inside the format (a new format syntax). Enums: **the
variant's name** (selected); the name and a printable payload in
parentheses (considered first — it needs recursive formatting generated
into compiled code for nested enums and `String` payloads read through
references, for little gain); or a std `text(e)` per error enum.

## Selected design

- `*` in place of a width or precision takes one argument, a `usize`,
  before the value; each `*` adds one to the argument count the format
  takes. A computed width or precision above 4096 (the limit written
  digits have) counts as 4096.
- `%v` of an enum value, or of a reference to one, writes its variant's
  name. The payload is not written.

## Compatibility impact

Additive: formats that were rejected now print.

## Revisit conditions

If payloads in `%v` are wanted, after a general "text of a value" hook
exists in the runtime.
