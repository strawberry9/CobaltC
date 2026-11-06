# CHG-0289 — A guard's value holds its lock path; a lock path is not lent

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0240)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0240
Affects: `spec/04` (`refs-in`), `spec/19` (1.16.0); `spec/conformance.md`

## What changed

- `spec/04`: `refs-in` of a slice and of a guard value. `spec/19`: `[Thread-Lend]` lends no lock path; a note
  under `[Lock]`; a paragraph on a guard moved to another thread.
- Cases: `conf.guard-moved-to-thread`, `conf.guard-moved-to-thread-and-dropped`.

## What changed for programs

Nothing: a clarification of what both implementations did.
