# CHG-0067 — `File::printf`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0059
Affects: rule.stdlib.file-handle

## Problem / motivation

D-0059 (friction found by real-world testing, `impl/STATUS.md`).

## Decision

File::printf(&mut f, format, …) writes formatted text to a file.

## What changed

spec/21 3.22.0 ([File-Printf], File::write_text in the std source, §0 table; [File-Read] reads max bytes, fewer only at the end, as the implementation now does), spec/conformance.md 3.52.0. Implementations: modres rewrites File::printf to File::write_text over $fmt; std gains File::write_text; fileio read_some reads past one read-ahead fill up to cap. The guide (§21).

## Compatibility classification

Extension.

## Conformance changes

**Added:** conf.file-printf, conf.file-printf-format.

## Prior-art status

See D-0059.

## Revisit conditions

See D-0059.
