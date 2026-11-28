# CHG-0463 — An object never written ends with nothing (D-0327); a thread's buffers freed at its end

Status: implemented (2026-11-17), D-0327; Update 196
Affects: `spec/14` §1 (`[Block-Exit]`, 1.47.0); `impl/cbrt/src/rt_objs.c`, `rt_init.c`, `rt_threads.c`, `rt.h`,
`fmt.c`, `fmt.h`; `impl/conformance/14-control-flow/unwritten_binding_ends_with_nothing_ok.cb`;
`spec/conformance.md` 3.316.0; `impl/STATUS.md` (known gaps)

## Why

The release profile's units ran on Update 195 for the first time on the tools written in CobaltC and the
runtime in C, and left three known gaps (`CHG-0462`, Checked). This change closes them.

## What changes

**A binding never written (D-0327).** `destroy_obj_tail` (`rt_objs.c`), which every end of an owned object goes
through (a scope's end, a `return`, a fault's unwind, a contained fault's), ends an object whose record says it
was never written (`cb_new_uninit`, no write since) with no destructor and no destruction of its parts: the
record's paths are detached and its identity ends. Before:

- with every check on, `Vec<String> v;` (or any type with a destructor) that met its scope's end or a contained
  fault before its first assignment faulted `diag.use-of-uninitialized` inside the destructor
  (`conf.unassigned-let-takes-first-value` under `COBALTC_ALL_CHECKS=1`);
- in the default form, a type with a destructor of its own had that destructor run on memory never written.

Not from the move off Rust: the last Rust compiler wrote the same C and the Rust runtime did the same.
`[Block-Exit]` says it now (`spec/14` 1.47.0).

**A thread's buffers.** The runtime in C keeps a thread's formatting state (`cb_format`'s parsed formats and
argument buffers, `cb_fmt_space`'s text), the renderer's buffer and the file operations' buffers behind
thread-local pointers; when the runtime was Rust their destructors freed them at the thread's end, and the port
(Update 193) freed none. `thread_locals_free` (`rt_init.c`) frees them when an OS thread of the runtime ends
(`worker`, `rt_threads.c`: a thread that finds 64 idle ones already waiting). The scheduler's parker, made only
under `COBALTC_SCHEDULE_SEED`, stays.

**The fixed point at the bootstrap level** needed no change beyond the unit's limit (Update 195): it holds.

## Checked

- **The binding never written**: `conf.unwritten-binding-ends-with-nothing` and
  `conf.unassigned-let-takes-first-value` in the default form and with every check on, under GCC and under
  Clang, and the first compiled for Windows under Wine: the output the interpreter's last build gives
  (`rust-reference`). Before, on the same compiler: a second `drop 7` under GCC and `drop 0` under Clang where
  nothing was written, `drop 0` before the contained fault, and `diag.use-of-uninitialized` with every check.
- **The thread's buffers**: `stress/net/echo50.cb` (50 clients and their handlers, more OS threads than the 64
  kept idle) under AddressSanitizer with leak detection, as the release profile's `programs` unit runs it: no
  leak reported (42 400 bytes in 20 allocations before).
- The runtime built by GCC, by Clang and by MinGW-w64 without a warning.
- **The gate and the release profile** on the commit with this change (`af81f8a`, run `20261116_144703`), all
  25 units green: the gate profile's 15 in 90 minutes, then the release profile's.
  - `fixpoint`, to its end in the unit for the first time: stage 2 is stage 1, 101 048 122 bytes at level 10, in
    264 minutes with nothing running beside it (5 h 42 min by hand beside the gate's units, `CHG-0462`).
  - `all-checks`: with every check on, 1 422 of the suite's 1 426 cases pass under GCC and none fails; the four
    not run are the slow TLS and HTTP cases, which `slow-cases` runs (1 of 1 425 failed before).
  - `slow-cases`: the suite's 1 426 cases and the 2 129 rows, under GCC and under Clang.
  - `programs`: the stress programs, the sanitizer step (no leak; `net/echo50.cb` leaked before), the crypto
    sweep, the local TLS handshakes, the nine fuzzers and the interoperation checks, in 42 minutes.
  - `runtime` (53 minutes), `verify`, `wine` (1 410 of 1 412, the two 300-thread cases the unit allows) and
    `proofs-kernels` (kept from its last green run, its inputs unchanged).
  - `external` (advisory): the eight DNS-over-TLS exchanges, the HTTP clients, and the server driven by `curl`,
    which reads its port now that the script waits for the build.
