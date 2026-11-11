# CHG-0089 — Temporaries and threads in `coby`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, fix)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: D-0073
Affects: implementations only; `spec/conformance.md`

## Problem / motivation

`merge(&mut total, &join(h))` borrowed a thread's result as a temporary
argument (D-0073). `coby` left the temporary keyed to the thread that
made it. At the statement's end it could not destroy it
(`diag.no-destroy-authority`), which then surfaced as an aliasing
conflict. Binding the result first (`auto r = join(h);`) worked, since
a binding adopts what it binds. `cobc` was correct.

Behind it was a second fault. `coby` runs its threads on one
interpreter and switches between statements, including the statements
of a call made in the middle of another statement. It kept one stack of
statement temporaries (and of `$` lengths) for all threads. So a thread
switched out mid-statement could have its temporaries ended by another
thread's statement, intermittently.

## Decision

A temporary borrowed as an argument is adopted by the borrowing thread,
as a binding would adopt it. Statement temporaries and `$` lengths are
kept per thread.

## What changed

- **`spec/conformance.md` 3.72.0:** the case below.
- **`spec/02-schema.md` 1.0.62:** §5's "in use" ranges.
- **Implementations (`coby`):**
  - it re-keys the temporary's owner when it borrows it;
  - `stmt_temps` and `dollar` are per thread;
  - the write-context flag is kept across a thread switch.

## Compatibility classification

Fix.

## Conformance changes

**Added:** `conf.temp-borrow-joined-result`,
`conf.temporaries-across-thread-switches`.

## Revisit conditions

None.
