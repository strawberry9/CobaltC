# D-0240 — A guard's value holds its lock path's token; a lock path is not lent

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); a clarification (`spec/STABILITY.md` §2)
Governed by: `CobaltC_Master_Instructions.md` §8 (formal precision), §20, §21
Depends on: D-0213, D-0215 (`held-by` derived), D-0233 (lock paths derived from the locking reference)
Affects: `spec/04` §2 (`refs-in`), `spec/19` §1 (`[Thread-Lend]`), §2 (`[Lock]`, prose); `spec/conformance.md`
(CHG-0289)

## Problem

The fourth review (F-08): a guard's value is a lock path's token, but `refs-in` counted only `ref` values, so
`held-by(a_g)` was `∅` by its derived definition and `{o_g}` by `[Lock]`'s assignment, and whether
`[Thread-Lend]` lends a guard moved to `spawn` depended on which sentence one read. Probed 2026-10-15: both
tools let a guard move to a spawned thread; the thread's access through it faults (`[Thread-Affine]`: the lock
path stays the locking thread's), and a thread that only drops it releases the mutex.

## Options

(i) a guard value's `refs-in` is its token; `[Thread-Lend]` lends no lock path; (ii) forbid moving a guard to
`spawn` statically; (iii) lend lock paths. (ii) rejects the hand-off of an unlock, which both tools run
correctly; (iii) would let another thread write the interior while the locking thread may still be inside it.

## Decision

(i), which is what both tools do.

## Results

- `spec/04` `refs-in`; `spec/19` `[Thread-Lend]`, `[Lock]`, and a paragraph after `[Guard-Drop]`'s prose.
- Cases: `conf.guard-moved-to-thread` (one outcome over 40 schedules), `conf.guard-moved-to-thread-and-dropped`.
