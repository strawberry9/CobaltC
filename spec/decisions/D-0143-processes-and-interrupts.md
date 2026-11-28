# D-0143 — `std::process`: child processes and interrupt requests

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §6 and §10.4–§10.5, §10.7: "No shell in `Command`; exit status is `i32` with signals negative; a dropped running `Child` is not killed", "Interrupts are a polled flag (`watch_interrupts`, `interrupt_requested`), never a callback; `std::exit` stays out", "Conformance cases that need a Unix program carry `// platform: unix`")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0039 (no `std::exit`), D-0054 (`File`'s reading model), D-0063 (channels), D-0117 (`text` of an error), D-0134 (`std`'s naming conventions), D-0136 (`std`'s submodules), D-0141 (the `platform:` header, the shared primitive shape)
Affects: `spec/21` §0, §2k (new) (4.9.0), `spec/conformance.md` (3.133.0), the guide §21, `impl/std/process.cb` (new), `impl/std/std.cb`, `impl/src/procio.rs` (new), `impl/src/prelude.rs`, `impl/src/interp.rs`, `impl/cbrt`, `impl/cobc/src/lower.rs`

## Problem

A deployed program runs other programs — `git`, a compressor, a
converter, a health check — and needs their output and status. A
server must stop cleanly when its operator presses Ctrl-C or the system
asks it to (SIGTERM): finish the request in hand, close files, and
leave. `std` had neither.

## Candidate mechanisms

1. **`system(text)` through a shell.** One string, quoted by hand: the
   classic injection hole, and a different shell on every platform.
   Rejected.
2. **A `Command` value built in CobaltC and run three ways** (to the end
   with output captured; to the end with this program's streams; started
   with pipes), the program and each argument passed as given. Selected:
   the shape Rust, Go and Python's `subprocess` (without `shell=True`)
   converged on.
3. **Interrupt callbacks** (a function the runtime calls on a signal).
   A signal arrives between any two instructions of any thread; a
   CobaltC function called there could observe and break every
   invariant the language guarantees, in both tools. Rejected for a
   polled flag.

## Selected design

`spec/21` §2k, `rule.stdlib.process`:

- `Command::new(program)`, `arg`, `current_dir`, `env`, `clear_env`;
  `Command::output` (stdin empty, both streams captured, read at once so
  neither pipe fills), `output_with_input`, `status` (this program's
  streams), `spawn` (all three piped) giving a `Child` (`id`,
  `write_input`, `close_input`, `read_output`, `read_output_line`,
  `read_error`, `wait`, `try_wait`, `kill`). `Output { status, stdout,
  stderr }`.
- **Every failure is a `FileError`** (D-0134: "every input and output
  failure is a `FileError`"): `NotFound` for a program that is not there,
  `Denied`, `Io`, `Utf8` for a line of output that is not UTF-8. A first
  version had its own `ProcessError` with exactly those four variants;
  the battery's showcase run found the cost — `Err(NotFound)` written as
  a value in a program importing `std` became `[Resolve-Ambiguous]`,
  since two imported enums then had the variant — so the convention is
  kept and the duplicate type dropped.
- **No shell, ever.** A program that wants `sh -c` runs `/bin/sh` and
  says so in its own code.
- **A status is an `i32`**: the exit code, or minus the signal that
  ended the child on Unix (`-9` for SIGKILL), so one number answers
  "how did it end" on every platform.
- **`Child` is a `resource struct`; dropping it closes its pipes and
  does not end it** (a finished child is reaped); this is Rust's
  behaviour and the least surprising for a background job. `wait` closes
  the child's standard input first, so a child reading to its end ends.
- `Child::read_output` and `read_error` mirror `File::read` (append up
  to `max`, at most a MiB, 0 at the end); `read_output_line` mirrors
  `File::read_line` with a read-ahead buffer.
- **Interrupts are a flag.** `watch_interrupts()` installs a handler for
  SIGINT and SIGTERM (`signal` and `siginterrupt`, so a blocking system
  call the signal arrives in returns rather than resumes), or
  `SetConsoleCtrlHandler` on Windows, that only sets an atomic flag;
  `interrupt_requested()` reads it. A blocking `std` call an interrupt
  arrives in may return early: a listener's `accept` while interrupts are
  watched polls, and gives `Err(TimedOut)` once the flag is set; a
  socket's `read` gives `Err(TimedOut)` (D-0144). `std::exit` stayed out
  here (D-0031's choice): a program left by returning from `main`, until
  D-0150 added `exit` to this submodule.
- **Realization:** `impl/src/procio.rs`, shared by both tools, over
  Rust's `std::process`; children in a process-wide table, each entry
  locked on its own so one thread's wait does not hold up another's;
  a finished `output` run kept until `std` copies it out. `proc_op` has
  D-0141's shape; `coby` releases its interpreter lock while any of it
  runs, so other CobaltC threads run while one waits for a child.
- **A broken pipe is `Err(Io)`, never the program's end.** On Unix a
  write to a pipe whose reader is gone raises SIGPIPE, which ends a C
  program at once; Rust's start-up ignores the signal, so `coby` already
  reported the write's failure. The compiled program's runtime (`cbrt`)
  now ignores it too at start-up, so `Child::write_input` after the
  child's end and `output_with_input` to a program that exits without
  reading agree across the tools (found by a review on 2026-10-04; case
  `process_closed_pipe_ok.cb`). The same disposition makes a compiled
  program whose standard output is a closed pipe (`prog | head`) fail
  its writes rather than die, as `coby` did already.
- **The command crosses as bytes** (`Command::encode`): each string as an
  8-byte length and its bytes, so an argument may hold any text; one
  holding a NUL byte cannot be passed to a program and is `Err(Io)`.
- **Conformance** needs programs certain to exist: the cases run
  `/bin/echo`, `/bin/cat`, `/bin/sh`, `/bin/sleep` and carry `//
  platform: unix` (D-0141's header). Sending an interrupt is not tested
  in a case (it would interrupt the test runner's process when cases run
  in it); `stress/process` signals itself through a child.

Rejected along the way: `Command::args(slice)` (one `arg` per call says
the same; add it when programs ask); a `Child::stdout` stream value
(there are no traits to make one readable like a `File`, so the reads
are `Child`'s own functions).

## Compatibility impact

Additive: a new submodule `std::process` and the names `Command`,
`Output`, `Child`, `watch_interrupts`, `interrupt_requested` in `std`
(no new enum variant names, so no unqualified variant becomes
ambiguous), shadowed by a program's own items of
those names.

## Revisit conditions

- `std::exit`: revisited and added by D-0150.
- Pipelines (one child's output into another's input) without a shell,
  and redirecting a child's stream to a `File`, if programs ask.
- Signals other than interrupts (SIGHUP for reload), if servers ask.
- Inheriting only some streams (`spawn` with standard error inherited),
  if programs ask.
