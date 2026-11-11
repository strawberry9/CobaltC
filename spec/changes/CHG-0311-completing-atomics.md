# CHG-0311 — Completing atomics

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0253
Affects: `spec/19` (1.18.0), `spec/04` (1.11.0), `spec/21` (4.65.0), `spec/22` (2.52.0),
`spec/registry/diagnostics.md` (1.56.0), `spec/conformance.md`; both tools, `cbrt`, `std/sync.cb`,
`impl/model`; nine conformance cases

## What changed

`AtomicBool`, `AtomicPtr<T>`, `Atomic::fetch_max`, `fetch_min`, `fetch_update`, `wait`, `notify_one`,
`notify_all`, `atomic_fence`, `spin_hint`; the std-only intrinsics `atomic_wait_op` and `cpu_op`, and
`atomic_op` on an `AtomicPtr`'s cell; `state.sync`'s `wake-count`; `[Deadlock]` counts a wait on an atomic.

## What changed for programs

New names only; a program's own items of these names shadow `std`'s.
