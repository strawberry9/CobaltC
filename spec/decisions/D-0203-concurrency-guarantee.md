# D-0203 — The concurrency guarantee, stated

Status: ACCEPTED (2026-10-10; the owner asked for a stated memory model and guarantee)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §17
Depends on: D-0004, D-0018, D-0063, D-0188, D-0202
Affects: `spec/19` 1.10.0 (§4 `rule.conc.guarantee`, a sentence in §1), `spec/03` 1.4.0, `spec/conformance.md`
3.197.0, `impl/src/interp.rs` (`coby`'s spawn); `CHG-0232`

## Problem

Concurrency is to be a main selling point of CobaltC. What makes it one is already in the rules, but spread across
them:
- `[Thread-Step]` interleaves whole steps, sequentially consistent;
- `clash` is checked at every access in every thread;
- the mutex is the only way to share mutable state;
- `inv.concurrency-validity` ends in one clause, "data races are unreachable".

No rule says plainly what a program can rely on, or what an implementation must not do as it proves checks
unnecessary in advance (D-0200) or runs threads in parallel (D-0202).

## Decision

A new rule in `spec/19`, `rule.conc.guarantee`, states four clauses. They are consequences of the existing rules,
and add none:

1. **`[Conc-Interleaving]`:** every execution is an interleaving of whole steps, sequentially consistent.
2. **`[Conc-No-Race]`:** two accesses of different threads to overlapping cells, at least one a write, are either
   ordered by a join, a lock or a send and receive, or the later one is a diagnostic, whatever the timing.
3. **`[Conc-Defined]`:** every step of every interleaving is defined (outside `unsafe`).
4. **`[Conc-Checks-Omitted]`:** an implementation may leave out a check only where it cannot fail in any
   interleaving.

The rule names what stays open: timing may decide between permitted outcomes (a finished thread holds nothing),
and deadlock is detected only in `[Channel-Deadlock]`.

## What writing it down found

`[Spawn]` binds the new thread's parameters in the spawn step itself, so a reference passed is held from the spawn.
`coby` bound them only when the new thread first ran. In between, the spawning thread's conflicting access went
unseen: a fault in some runs, none in others, against `[Conc-No-Race]`.

`coby` now holds the arguments from the spawn, in an object the thread ends with its body, as `cbrt`'s argument
block does. `spec/19` §1 says so in a sentence. The case `conf.conc-race-detected` pins it down.

## Compatibility impact

None for a program that follows the rules. A program whose conflicting access `coby` let through when the new
thread had not started yet now faults there, as the compiled program did.
