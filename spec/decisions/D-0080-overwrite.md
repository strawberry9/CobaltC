# D-0080 — `overwrite`: replace a value and destroy the old one

Status: ACCEPTED (2026-09-28, owner decision)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0008, D-0033, D-0048, `[Write-Resource-Overwrite-Rejected]`
Affects: `spec/21` §0, §3b; `spec/registry/diagnostics.md`

## Problem

Assigning over a live resource is rejected so that no destruction happens
out of sight (D-0008, D-0033). The accepted way to do it was
`drop(replace(&mut x, v))`, which is long and indirect for a common need,
such as advancing a big number (`r = r - b`) or resetting a buffer. It
was the most frequent friction in the third round of stress programs, six
times in one program.

## Candidate mechanisms

1. **A std function `overwrite(&mut place, v)`** that stores `v` and
   destroys the old value. Selected. No new tokens. The destruction is
   still a visible call. It pairs with `replace`, which hands the old
   value back.
2. **Let `=` destroy the old value.** Rejected by D-0033 for D-0008's
   reason: a destructor would run with nothing on the line saying so.
3. **A new operator** (`:=`) with the same meaning. A new token for what
   a function does.
4. **Accept `=` where the old value provably owns nothing.** Narrow, and
   hard for a reader to predict.

Names considered: `put` (reads as output to a C programmer, like `puts`),
`assign` and `reassign` (these describe `=` and hide the destruction),
`set` (too generic). `overwrite` shares its word with
`diag.overwrite-of-live-resource`, whose repair now names it.

## Selected design

`export fn overwrite<T>(ref<T, exclusive> r, T v) { drop(replace(r, v)); }`
in `std` (`spec/21` §3b). The old value's destructor runs during the call.

## Compatibility impact

None: a new function.

## Revisit conditions

None.
