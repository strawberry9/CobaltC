# CHG-0460 — A contained fault's jump no longer unwinds the stack on Windows

Status: implemented (2026-11-16); Update 195
Affects: `impl/cbrt/src/rt_init.c`, `impl/cbrt/src/rt_threads.c`, `impl/cbrt/src/rt.h`,
`.github/workflows/bootstrap.yml`

## Why

GitHub's Windows runner, on Update 194's push: `conf.discarded-noted-results-end-with-statement` ended with exit
status 5 and no message, once in four runs of the suite there. Run by itself on that runner the case ended with
an access violation in about a third of its runs (321 of 1 032; 29 of 80), never under Wine (300 runs), never on
Linux, and never under a debugger.

The case spawns a thread whose function faults; `try_join` contains the fault (`[Fault-Contain]`, D-0204). The
runtime ends the thread's work with `longjmp` to the `setjmp` in the thread's first frame. On 64-bit Windows
`setjmp` records that frame, and `longjmp` then unwinds the stack to it frame by frame (`RtlUnwindEx`), reading
each function's unwind table. For a function with a frame pointer and saved XMM registers, GCC (15.2, the
runner's) writes a table that places those registers above the function's frame: the unwinder reads there. The
case's function is such a function (the stack check takes its frame's address, which keeps the frame pointer; the
function saves `xmm6` to `xmm10`), and it runs as the thread's function, a few hundred bytes below the end of the
thread's stack, so the read passed that end whenever Windows had placed the thread's first frames high enough.
The crash reporter linked into the case on the runner showed it: the fault inside `RtlUnwindEx`, called from
`longjmp`, called from `contained_fault`, reading 0x298 bytes past the end of the stack.

Not a fault of Update 193's runtime in C or of Update 194 alone: any runtime that left a contained fault through
`longjmp` had it with this compiler; the suite had run on Windows three times before it showed.

## What changes

- **The jump is a plain one.** The runtime clears the frame `setjmp` recorded in the buffer the generated code
  hands it (`jump_plainly`, called by `cb_thread_jmp` and `cb_lib_jmp`; 64-bit Windows only), so `longjmp`
  restores the registers and jumps without unwinding. Nothing between a fault and the `setjmp` that contains it
  needs unwinding: the frames are C's, and the runtime gives up its holds and folds the thread's scopes and frames
  before the jump. Generated C and `cbrt.h` are unchanged. The same for a library's `_try` entry points (D-0257).
- **GitHub's Windows job runs the case 60 times** after the suite (`.github/workflows/bootstrap.yml`), so the
  fault, which one run in three showed, cannot return unseen.

No rule changes; no change on Linux.

## Checked

- **On GitHub's Windows runner** (Windows Server, MinGW-w64 GCC 15.2), the case run by itself: before, 321 of
  1 032 runs and 29 of 80 ended with an access violation; with the fix, 0 of 300 (run 43800168801), and the
  suite 1 406 of 1 406 in the same run.
- **The cause seen, not inferred**: a crash reporter linked into the case on the runner printed the fault inside
  the system's unwinder under `longjmp` under `contained_fault`, at an address 0x298 past the end of the thread's
  stack; the function's unwind table read from the executable the runner built (frame register `rbp`, the XMM
  saves at 1360 to 1424).
- **Here**: the case under GCC, under Clang, and compiled for Windows under Wine; the 143 `cbtest` cases; the
  runtime built by GCC, by Clang and by MinGW-w64 without a warning.
- **The gate**: Update 195's run, under `CHG-0462` (the gate profile green on the commit with this change).
