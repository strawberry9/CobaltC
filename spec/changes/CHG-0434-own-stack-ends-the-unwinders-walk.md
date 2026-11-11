# CHG-0434 — The program's own stack ends an unwinder's walk (RT-001)

Status: implemented (2026-11-11), on the owner's report RT-001; Update 179
Affects: `impl/cbrt/src/lib.rs` (`run_on_own_stack`, `own_stack_entry`, the new `cbrt_own_stack_trampoline`);
`impl/cobc/tests/driver.rs` (`runtime_panic_walk_ends_at_own_stack`). No rule changes. No program's outcome
changes: a panic inside the runtime aborted the program before and does now.

## What was wrong

On Linux a program runs on a stack of its own (256 MB, mapped by the runtime). `run_on_own_stack` moved onto it
with `mov rsp, top` and a `call` of `own_stack_entry`, in inline assembly inside `cb_run_program`. The `call`
left a return address in `cb_run_program`, so a walk of the stack went on into that function's frame, whose
unwind information describes it on the *thread's* stack: the walk read past the top of the mapping.

A Rust panic inside the runtime (an internal invariant; the report's case is C code popping the runtime's
frames until none is left) therefore ended in `failed to initiate panic, error 3` (a failed search), and with
`RUST_BACKTRACE=1` the backtrace's own walk crashed (SIGSEGV, exit 139) after printing the real frames. A
debugger's backtrace ran on into junk frames the same way.

`own_stack_entry` also wrapped the program in `catch_unwind`. The program's entry is an `extern "C"` function,
which cannot unwind, so the compiler had removed the catch: it was never a handler.

## What changes

The stack is entered through `cbrt_own_stack_trampoline`, a function in assembly whose call frame information
marks its return address undefined (`.cfi_undefined rip`; on aarch64 `.cfi_undefined x30`). A walk ends there,
at the end of the stack. The dead `catch_unwind` is removed, with a comment saying why none belongs there:
unwinding through the generated C would not restore the runtime's state, so an internal failure aborts.

## Measured

The report's reproduction, built by `cobc` under GCC and Clang: plain, `failed to initiate panic, error 5`
(end of stack), exit 134; with `RUST_BACKTRACE=1`, the whole backtrace, ending at
`cbrt_own_stack_trampoline`, then the same abort, exit 134 (was 139). `gdb`'s `bt` ends at the same frame.
Threads and `diag.stack-exhausted` behave as before under both C compilers and `coby`. The runtime builds with
Rust 1.77.2 and for Windows (where no own stack is used).

## Not done

- **aarch64 is assembled, not run**: no aarch64 machine was at hand. The library builds for
  `aarch64-unknown-linux-gnu` and holds the trampoline.
- A panic still cannot be caught or recovered from; that is intended.
- An executable linked without unwind tables (the report's `ccc`) has nothing to walk; nothing here changes it.
