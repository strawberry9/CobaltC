# CHG-0295 — The closure row marks a captured reference binding escaped

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0243)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0243
Affects: `spec/14` 1.28.0 (§6 transfer table); one conformance case

## What changed

`escaped(r) := T` for a captured local `r` of reference type; a new row: a closure without `move` anywhere but
a `let` or an assignment marks every captured local escaped.

## What changed for programs

Nothing accepted or rejected changes; an implementation may no longer omit the check of a write to a
reference binding a live closure has captured (both tools never did).
