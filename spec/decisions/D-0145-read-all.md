# D-0145 — `read_all` and `read_all_bytes`: the rest of standard input at once

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §8)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0029 (standard input), D-0050 (`stdin_read` in both tools), D-0134 (`FileError`, the one input/output error), D-0136 (`std::io`)
Affects: `spec/21` §0, §2b `rule.stdlib.read` (4.11.0), `spec/conformance.md` (3.135.0), the guide §21, `impl/std/io.cb`

## Problem

A filter that needs its whole input before it starts (to parse a JSON
document, sort lines, count words) looped `read_line` and glued the
lines back together, losing the difference between `\n` and `\r\n` and
whether the last line had an end. Reading everything is one very common
intent with an obvious name (§9's admission test).

## Selected design

`spec/21` §2b, `[Read-All]`: `read_all_bytes() : Result<Vec<u8>,
FileError>` calls `stdin_read` into the `Vec`'s spare room, 64 KiB at a
time, until it gives 0; `read_all() : Result<String, FileError>` is the
same bytes checked as UTF-8 (`Err(Utf8(e))` otherwise). Both may follow
`read_line` calls, which consume input byte by byte, so nothing is
read ahead and lost. Written in CobaltC; no new primitive.

Rejected: `read_all` returning `Option` at the end (an empty `String` is
the natural answer: there is nothing left), and `read_lines` (a `Vec` of
lines; `String::split(&all, "\n")` says it, with views and no copies).

## Compatibility impact

Additive: two new names in `std`, shadowed by a program's own.

## Revisit conditions

- A buffered standard input (`read_line` reading ahead), if programs
  measured find reading byte by byte slow; `read_all` would then take
  the buffer first.
