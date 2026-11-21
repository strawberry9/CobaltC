# D-0092 — `read_line` consumes a `\r` before its `\n`

Status: ACCEPTED (2026-09-28, the owner's decision after the Windows-portability review)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0029, D-0050, D-0054
Affects: `spec/21` §2b, §2e, the implementations, the guide

## Problem

`read_line` — on standard input and on a `File` — ends a line at `\n`
and keeps a `\r` before it, so a line typed on a Windows console reads
as `yes\r` where the same line on Linux reads as `yes`: `line == "yes"`
and `parse` of a read line succeed on one platform and fail on the
other. Everywhere else the specification keeps the platform
unobservable through the library (module paths, files, arguments); the
guide could only warn about this one.

## Candidate mechanisms

1. **Keep the line byte-exact.** The simplest rule, but every portable
   interactive program must trim the `\r` by hand, and forgetting is
   invisible until the program runs on Windows.
2. **Consume the `\r` on Windows only.** Makes the platform *more*
   observable, not less: the same input bytes give different lines.
3. **A `\r` immediately before the terminating `\n` is consumed with
   it, on every platform.** A CRLF file read on Linux gives the same
   lines as on Windows; the platform stops being observable through
   `read_line`. What C#, Java and Rust's `lines()` settled on.
   Selected.

## Selected design

- A **line** is every byte up to the next `\n` (byte 10), which is
  consumed and not included, or up to the end of input. One `\r`
  (byte 13) immediately before the consumed `\n` is consumed with it
  and not included.
- Any other `\r` is part of the line, and a last line ended by the end
  of input keeps a final `\r`: no `\n` was consumed, so no `\r` is.
- `read_line()` and `File::read_line` agree, as before.
- A program that wants the raw bytes still has `read`, `File::read`
  and `read_bytes`, which remain byte-exact.

## Compatibility impact

Changes the value `read_line` returns for input whose lines end
`\r\n`: previously `…\r`, now `…`. No known program relied on the kept
`\r` — the guide described it only as a hazard ("a line typed on
Windows ends in a `\r` that makes it `Invalid`").

## Revisit conditions

None.
