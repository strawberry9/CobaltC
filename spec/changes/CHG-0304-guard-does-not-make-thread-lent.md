# CHG-0304 — cbrt: a guard's lock path does not make a thread lent

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16; found by D-0246's round four)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0240 (a lock path is not lent), `spec/18` contained, D-0204
Affects: `impl/cbrt/src/lib.rs` (`cb_thread_start`); two conformance cases

## What changed

`cb_thread_start` decided whether a new thread was lent an exclusive reference by any exclusive token in its
arguments or its closure's captures; a guard's lock path is exclusive but is not lent (D-0240), and no longer
counts.

## What changed for programs

In cobc, a thread whose closure owns a guard and faults on it is contained, as in coby and the text: the
spawner goes on, and the failure is raised again at `join` or the handle's end
(`conf.thread-holding-guard-contained`). Also from CHG-0302: `conf.spawn-resource-closure-moves` now
expects its `diag.stale-binding` statically.
