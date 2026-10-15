# CHG-0107 — `read_line` consumes a `\r` before its `\n`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9
Depends on: D-0092
Affects: rule.stdlib.read, rule.stdlib.file-handle, the implementations

## Problem / motivation

A line typed on a Windows console read as `yes\r`, on Linux as `yes`:
the platform was observable through `read_line`, alone in the library
(D-0092).

## Decision

D-0092.

## What changed

- **`spec/21` 3.37.0:** §2b `rule.stdlib.read`'s line definition and
  `read_line`'s `std` source; §2e's **Lines** bullet and
  `File::read_line`'s `std` source.
- **Implementations:** `std` (`src/prelude.rs`): `read_line()` pops a
  final byte 13 before breaking on byte 10; `File::read_line` pops it
  after popping the 10. One source serves `coby` and `cobc`.
- **Guide:** §21's `read_line` paragraph and result table, the `parse`
  paragraph (its Windows-`\r` warning is obsolete), and `File`'s
  `read_line` row.

## Compatibility classification

Breaking in principle: `read_line` of input whose lines end `\r\n`
returned `…\r`, now `…`. No known program relied on the kept `\r`.

## Conformance changes

**Changed:** `conf.read-line-lines`
(`read_line_lines_ok.cb`: input `ab\r\n\na\rb\r\nc\r` reads as `ab`,
``, `a\rb`, `c\r`).
**Added:** `conf.file-read-line-crlf`
(`impl/conformance/21-standard-library-semantics/file_read_line_crlf_ok.cb`).

## Revisit conditions

None.
