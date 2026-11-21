# CHG-0306 — `Atomic<T>`: sequentially consistent atomics

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0249
Affects: `spec/19` (1.17.0), `spec/21` (4.63.0), `spec/22` (2.51.0), `spec/04` (1.10.0), `spec/08` (1.7.1),
`spec/OVERVIEW.md`, `spec/conformance.md`; `impl/std/sync.cb`, `impl/src/typecheck.rs`,
`impl/src/interp/intrinsics_threads.rs`, `impl/cobc/src/lower/intrinsics.rs`, `impl/cobc/src/lower/passes.rs`,
`impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; thirteen conformance cases

## What changed

`spec/19` §2a `rule.conc.atomic` (`[Atomic-Op]`, `[Atomic-Overflow]`); `spec/21` §3c″ `rule.stdlib.atomic`;
`synchronized` counts two atomic operations on one atomic; `spec/08`'s "one route" for mutation through a
shared handle becomes two; `spec/22` §5 keeps only weaker memory orders among the absences. The std-only
intrinsic `atomic_op` is typed in the shared static pass, performed by `coby` in one step and lowered by `cobc`
to `__atomic` builtins; `cbrt` adds `cb_atomic_point` and `cb_sched_flag` for schedules.

## What changed for programs

`import std;` brings `Atomic<T>`. Nothing that ran before changes. An item named `Atomic` in a program's own
module is the program's, as with any `std` name.
