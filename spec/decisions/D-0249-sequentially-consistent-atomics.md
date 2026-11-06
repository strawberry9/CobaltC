# D-0249 — Sequentially consistent atomics: `Atomic<T>` in `std::sync`

Status: ACCEPTED (2026-10-16, the owner: "draft D-0249 for atomics, option C", then "go with the recommendations and implement it", on `private/D-0249-atomics-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §9, §17
Depends on: D-0004, D-0052 (`static_assert`), D-0063, D-0090 (`integer`), D-0121 (its revisit condition), D-0143, D-0204, D-0213, D-0218
Affects: `spec/19` §2a (1.17.0), `spec/21` §3c″ (4.63.0), `spec/22` §5 (2.51.0), `spec/04` `synchronized` (1.10.0), `spec/08` prose (1.7.1), `spec/conformance.md`; `impl/std/sync.cb`, `impl/src/typecheck.rs`, `impl/src/interp/intrinsics_threads.rs`, `impl/cobc/src/lower/intrinsics.rs`, `impl/cbrt`; `CHG-0306`

## Problem

The only way to share mutable state between threads was `mutex<τ>`, and `spec/22` §5 listed atomics as a
deliberate absence; D-0121 left them "as their own decision". Programs locked a mutex to add one to an integer
(`showcase/tier2/mutex_counter.cb`, `showcase/tier13/supervised_dns.cb`, `stress/round6/bank.cb`,
`stress/round9/bank.cb`, `stress/round9/dnscache.cb`, `stress/round6/bugs/threadscale/many.cb`), and
`supervised_dns.cb` built a stop flag from a whole `Channel<u8>`. Both runtimes used atomics a program could not
reach.

## Options

A. Sequentially consistent atomics only. Fits `spec/19` as it is: the semantics is already an interleaving of
steps, each atomic with respect to `Σ`. B. A with explicit orderings (`Relaxed` … `SeqCst`): full speed, but a
weak memory model in `spec/19`, which neither tool could check. C. A now; weaker orderings, if ever, only inside
`unsafe` as trusted rules, by a later decision. **Selected: C**, with the four recommendations of the proposal:

- Q1: a library type `Atomic<T>` in `std::sync`, not a built-in `atomic<τ>`: no new reserved word.
- Q2: integers only; a flag is an `Atomic<u8>`.
- Q3: 128-bit integers rejected. The proposal named a new `diag.atomic-width`; `Atomic::new`'s
  `static_assert(sizeof<T>() <= 8, …)` gives `diag.static-assert-failed` instead, so no diagnostic is added.
- Q4: `fetch_add`/`fetch_sub` fault on overflow and leave the value unchanged, as `+` does; `_wrapping` forms
  beside them.

## Decision

`spec/19` §2a `rule.conc.atomic`: one rule, `[Atomic-Op]`, for the std-only intrinsic `atomic_op(a, op, x, y)`,
and `[Atomic-Overflow]`. Each operation is one step. The operand is checked as a shared access of the atomic;
the operation forms no access path and reads and writes the integer's cells directly, which no path ever
targets. `synchronized` (`spec/04`) counts two atomic operations on one atomic. `spec/21` §3c″ gives `Atomic<T>`
and its twelve functions: `new`, `load`, `store`, `swap`, `compare_exchange` (`Result<T, T>`), `fetch_add`,
`fetch_sub`, their `_wrapping` forms, `fetch_and`, `fetch_or`, `fetch_xor`.

## Results

- `coby` performs an operation inside one intrinsic call under its interpreter lock, which yields only between
  statements: one step with no further locking. `cobc` lowers to the GCC/Clang `__atomic` builtins with
  `__ATOMIC_SEQ_CST` after `cb_read` checks the reference; a checked add or subtract is a compare-and-exchange
  loop around `__builtin_add_overflow`/`__builtin_sub_overflow` that faults before it stores.
- Under `--explore` and `--schedule-seed`, `cobc`'s runtime passes the turn at atomic operations
  (`cb_atomic_point`, called only while `cb_sched_flag` is set), so a thread spinning on `Atomic::load` cannot keep
  the turn from the thread it waits for.
- Four threads doing 200 000 `fetch_add`s and `compare_exchange` maximum updates each give exact totals under
  both C compilers; a spin-wait on a stop flag explores to one outcome in 30 schedules under both tools.

## Revisit conditions

Weaker orderings, when a measured program on a weakly ordered target shows sequential consistency costing
something that matters: only inside `unsafe`, as trusted rules in `spec/20`. Fences for device memory (D-0121's
other half). Atomic raw pointers, with weaker orderings. `bool` and 128-bit atomics, if programs need them.
