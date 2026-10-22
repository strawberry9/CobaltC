# D-0256 — A freestanding mode (CobaltC without an operating system)

Status: DEFERRED (2026-10-17; proposed, then set aside on the owner's question whether it would be usable;
reassessed the same day after D-0257 and D-0258 were in use, and left deferred: see Results)
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0121, D-0202, D-0249, D-0250, D-0251, D-0255, D-0257, D-0258
Affects: nothing yet; the proposal is `private/D-0256-freestanding-proposal.md`

## Problem

Every program `cobc` builds is a hosted program: it starts at C's `main`, runs on a thread of the runtime,
reports faults on standard error with the source line read from the `.cb` file, and ends with `exit`. The
runtime needs Rust's standard library and libc. So CobaltC cannot produce a kernel, a bootloader, firmware or
an embedded image, which `CobaltC_Master_Instructions.md` §5 names as a systems language's work.

## Options

A target `x86_64-unknown-none` with a `no_std` build of the runtime, platform hooks for memory, output and
halting, a `cb_start` entry in place of `main`, and threads rejected (the proposal); a second runtime in C; a
mode without dynamic checks (against §5 and the language's premise).

## Decision

Deferred. As proposed it would compile but be hard to use: the checks need a heap, so the earliest and
lowest code (boot, a kernel's memory manager, small microcontrollers) stays in C; interrupts and several
CPUs have no meaning in `spec/19`'s model, and rejecting threads leaves single-loop programs; the pieces a
kernel needs first — an address as a pointer, `static` storage, assembly files, linker sections — are
missing; and the runtime's memory and time are not bounded up front. The cost is several rounds. Library mode
(D-0257) and the low-level pieces (D-0258) come first, with a measurement of the runtime's footprint on
core-only programs before any freestanding target.

## Results

Reassessed on 2026-10-17 with D-0257 and D-0258 in use and the runtime measured
(`stress/freestanding_measure`). The design is sound in its structure: the checks need no unwinder (a fault
folds the runtime's own frames and runs destructors, then exits, which freestanding becomes a halt hook); the
generated C needs only `memcmp`, `memset`-class helpers and `setjmp` for library entries; Rust 1.77.2 has
`x86_64-unknown-none`. Its memory choice would change: a core-only program costs the runtime 57 allocations
and 123 KB at start, 121 KB of it live for the program's life, in a 1.9 MB process, and a small `std` program
allocates 617 KB in all, so the runtime would take one region handed to `cb_start` (the proposal's option 2b)
rather than allocator hooks. The same figure rules out small microcontrollers for good: the dynamic checks
have a floor of a few hundred kilobytes, and the hosted runtime is 666 KB of code. The checks' bookkeeping
is not reentrant, so interrupt handlers would stay in C or assembly, talking to the program through static
atomics and volatile memory, until `spec/19` says what an interrupt is. The cost is the largest runtime
change since D-0202: 8,200 lines using threads, thread locals, mutexes, condition variables, hash maps and
process exit in about seventy places, plus the hosted modules, for two or three sessions and a second
runtime build to keep across two targets and two platforms.

Left deferred because no one would use it. A kernel, bootloader or firmware author chooses between C, Rust
and Zig: Rust gives the guarantees CobaltC checks at run time statically and at no cost, with a mature
`no_std` ecosystem; Zig is freestanding by design with optional checks; neither carries a table floor, a
runtime call on most checked accesses, no interrupt handlers, one CPU and one architecture. A freestanding
target would be a credibility exhibit, not a use, and it adds nothing to what distinguishes CobaltC — a
total, executable, spec-defined semantics that both tools share, that `--explore` and the model enumerate,
and that a C program can call as a library (D-0257). That proposition is strongest where a runtime is
acceptable: user-space systems code, services, tools, libraries C calls, teaching. With raw pointers,
addresses, volatile access, C layout, `align`, `bitstruct`, static atomics, assembly files and C interop,
CobaltC already faces hardware from user space, through a mapped device region, which is the hosted systems
position the Master Instructions' §5 can honestly claim. The sessions D-0256 would take are better spent on
what decides adoption in that position: the cost of the checks on hot paths, the guide and the diagnostics,
real programs and libraries, Windows, editor and debugger support.

## Revisit conditions

A concrete user who needs a freestanding image and accepts the floor above; `spec/19` saying what an
interrupt is; a hardware-facing demonstration wanted sooner is a user-space program over a mapped device
region, which needs no new target.
