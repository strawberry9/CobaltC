# CHG-0321 — `allocate`'s result typed before its first use

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0036
Affects: `spec/conformance.md` (one row); `impl/src/typecheck.rs`

## What changed

`[Allocate]` is unchanged: `allocate(n, align)` is a `Result<rawptr<u8>, AllocError>` (`spec/21` §3b). The
checker's table of built-in result types had no entry for `allocate`, so a pointer bound by `Ok(p)` in
`match (allocate(…))` had no type until its first use decided one: `rawptr<u8> q = p + i;` and `*(p + i)`
were refused as an `isize` (`[T-Rawptr-Offset]` never applied), while `rawptr<u8> q = p;` first was
accepted. Both tools share the checker. The table now gives `allocate` its type, and the new row
`conf.allocate-result-typed` covers the binding offset, written and read through.

## What changed for programs

Programs that were refused now compile and run; nothing accepted before changes.

## Found by

The unit test of the Tier 14 showcase library (`showcase/tier14/digest`, 2026-10-17).
