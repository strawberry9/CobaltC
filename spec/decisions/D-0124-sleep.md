# D-0124 — `sleep_ms`

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 5)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9
Depends on: D-0061 (the clocks, the same host boundary), spec/19 (threads)
Affects: `spec/21` §0 (3.49.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`, `impl/src/fileio.rs` (shared), `cbrt`

## Problem

Threads, mutexes and channels exist and nothing sleeps: a simulation,
a server loop or a retry spun on `monotonic_ns()`.

## Candidate mechanisms

1. **Busy loops.** Burn a core to wait.
2. **`sleep_ms(u64 ms)` in `std`**, over a std-private
   `extern fn sleep_ns`, one host primitive like the clocks'. Selected.

## Selected design

`[Sleep]`: the calling thread does nothing for at least `ms`
milliseconds; how much longer is implementation-defined; other threads
run meanwhile (the interpreter releases its lock for the duration); a
guard held is kept; no fault. Seconds are a program's arithmetic.

## Compatibility impact

Additive.
