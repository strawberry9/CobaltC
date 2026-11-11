# D-0253 — Completing atomics

Status: ACCEPTED (2026-10-16, the owner: "go with your recommendations for D-0253 and implement it", after
asking for atomics support to be completed)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (std admission), §17
Depends on: D-0249, D-0204 (`[Deadlock]`, `--explore`), D-0063, D-0252 (`null_rawptr`)
Affects: `spec/19` §2a, §4 (1.18.0), `spec/04` `state.sync` (1.11.0), `spec/21` §3c″ (4.65.0), `spec/22` §5
(2.52.0), `spec/registry/diagnostics.md` (1.56.0), `spec/conformance.md`; `impl/std/sync.cb`,
`impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/interp/intrinsics_threads.rs`,
`impl/cobc/src/lower/intrinsics.rs`, `impl/cobc/src/lower/passes.rs`, `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`, `impl/model`; `CHG-0311`

## Problem

D-0249 gave integer atomics with sequentially consistent operations. Programs that use atomics also need a
flag type, atomic pointers for lock-free structures, a maximum or minimum kept by several threads, a way to
sleep until an atomic changes rather than spin, a fence for raw and foreign memory, and a pause for spinning;
and the question of weaker memory orders, which D-0249 left to a later decision, needed an answer.

## Options and choices

The draft (`private/D-0253-atomics-completion.md`) listed twelve gaps, each with a recommendation; the owner
took them all:

- **Added:** `AtomicBool` (over `Atomic<u8>`); `AtomicPtr<T>` (load, store, swap, compare-and-exchange);
  `Atomic::fetch_max`, `fetch_min`, `fetch_update`; `Atomic::wait`, `notify_one`, `notify_all` (and the
  same on `AtomicBool`); `atomic_fence()`; `spin_hint()`; `cobmodel` support for atomics.
- **Declined:** memory orders weaker than sequential consistency, in safe and `unsafe` code alike (the
  model is interleavings of indivisible steps; a weak model replaces `spec/19`, `--explore` and the model,
  and on x86-64 a read-modify-write costs the same in every order); a weak `compare_exchange` (it differs
  only by failing spuriously); atomics wider than 64 bits (not portable); `static` storage (its own
  decision).

## Decision

`spec/19` §2a: `[Atomic-Wake-Read]`, `[Atomic-Wait]`, `[Atomic-Notify]` over a wake count per atomic in
`state.sync`; `Atomic::wait` reads the count, checks the value and waits for the count to change, so it
returns only after seeing another value and no notification is lost. An implementation may share counts
among atomics (a spurious wake-up, never an early return); `notify_one` wakes at least one waiter. A wait on
an atomic is the end of a chain of waits under `[Deadlock]` (b), the main thread waiting alone included.
`[Atomic-Fence]` changes no state the rules see and orders raw, volatile and foreign accesses as C's
`atomic_thread_fence(memory_order_seq_cst)`; `[Spin-Hint]` is the processor's pause and a schedule point.

## Results

Both runtimes keep 256 wake counts, an atomic's chosen by its address (`cbrt`) or its object and path
(`coby`), so the table never grows. `cbrt` waits through `wait_until`, so cancellation, `[Deadlock]` and
`--explore` apply unchanged; `coby` through `wait_for`. `cobc` compiles `AtomicPtr`'s operations to the same
`__atomic` builtins on the pointer, the fence to `__atomic_thread_fence`, the hint to the CPU's pause plus a
schedule point. Nine conformance cases; a 20,000-move ping-pong through `wait`/`notify` completes under coby,
GCC and Clang, and every new case explores to one outcome in 30 schedules.

## Revisit conditions

Weaker orders: when a measured program on a weakly ordered target (ARM, POWER) is limited by sequentially
consistent loads and stores. `static` storage: by its own decision.
