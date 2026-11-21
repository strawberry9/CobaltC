# CHG-0323 — `Barrier`, `Semaphore` and `Once`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0260
Affects: `spec/19` §2a (1.20.0); `spec/21` §3c‴ (4.68.0); `spec/registry/diagnostics.md` (1.60.0);
`spec/conformance.md` (six rows); `impl/std/sync.cb`, `impl/src/interp.rs`

## What changed

`std::sync` gains `Barrier`, `Semaphore` and `Once` (`rule.stdlib.waiting`), each written over `Atomic`,
`Atomic::wait` and `notify` alone. `Barrier::new(0)` is the new dynamic diagnostic
`diag.barrier-zero-parties` (`[Barrier-Zero-Parties]`). Rows `conf.barrier-phases`, `conf.barrier-short`,
`conf.barrier-zero-parties`, `conf.semaphore-limits`, `conf.once-runs-once`, `conf.once-fault-retried`.

## What changed for programs

Three new types and one new diagnostic; nothing that was accepted changes.

## Found by

D-0260's investigation of what the new atomics make possible in `std` (2026-10-17).
