# D-0150 — `exit(status)`: ending the program from anywhere, destructors run

Status: ACCEPTED (2026-10-04, the owner: "accept and implement std::exit as D-0150", after the release-readiness assessment named it the one absence programmers keep asking about)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0008 (destructors at scope exit), D-0009 (faults are fatal), D-0031 (exit status from `main`; its candidates 2 and 3), D-0136 (`std`'s submodules), D-0143 (`std::process`)
Affects: `spec/15` `rule.fn.program` (1.12.0), `spec/18` §1 prose (1.5.2), `spec/21` §0, §2k (4.15.0), `spec/conformance.md` (3.140.0), the guide §21, `impl/src/modres.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/lib.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`; `CHG-0178`

## Problem

D-0031 made a program's exit status the value of `fn main() : u8` and
chose not to add `exit`: a program ends by returning from `main`. That
is clean when the decision to stop is taken in `main`, and awkward
everywhere else. A command-line tool that finds its arguments wrong
three calls deep, a server whose configuration file is unreadable, a
worker thread that detects a state the program must not continue in:
each has to thread a `Result` back to `main`, or fault, which reports a
diagnostic the program did not want. Every language a CobaltC programmer
comes from has `exit`, and the assessment of 2026-10-04 found it the one
absence programmers would keep asking about.

## Candidate mechanisms

1. **Keep returning from `main`** (D-0031's choice). Rejected now, for
   the reason above.
2. **`exit(s)` skipping destructors**, C's `exit`, Rust's
   `process::exit`. Fast and simple, and it leaves files unflushed,
   locks held and child processes unreaped, which is exactly what the
   language's destructors exist to prevent. Rejected.
3. **`exit(s)` unwinding the calling thread as a fault does**, so every
   destructor D-0008 would run on ordinary exit runs, and then the
   program ends with status `s`. The machinery exists in both tools:
   `[Fault-Unwind]` already folds the thread's frames and ends the
   program; this is the same unwind ending in `ok(s)`. Selected.
4. **A `Result`-returning `main` or a `Never`-typed `fault`-like
   intrinsic with a status.** More tokens for the same effect. Rejected.

## Selected design

`spec/21` §2k `[Exit]` and `rule.fn.program` `[Terminate-Exit]`:

- **`exit(u8 status) : never`**, an exported function of `std::process`
  (so `import std;` gives `exit`, and `std::process::exit` is its full
  name). Of type `never`, it stands where any value is expected:
  `None : exit(2)` in a match arm, or as a statement.
- **The calling thread unwinds**: its statement scopes and frames are
  folded innermost first and every destructor runs, exactly as under
  `[Fault-Unwind]`; standard output is flushed; the program ends with
  status `status`. Other threads take no further steps and their frames
  are not unwound, as under a fault: the process is ending. A destructor
  that faults during this unwind makes that fault the outcome, reported,
  with status 1, as it would under a fault.
- **No diagnostic is reported**: `exit` is an ordinary ending, `ok(s)`,
  distinguishable from every fault by the program's environment.
- **Realized natively** by both tools, since no CobaltC body can end the
  program: `modres` enters it as a native item of `std::process` (as
  `printf` and `assert` are), the checker types it `never` with one
  `u8` argument, `coby` raises an internal flow that unwinds like a
  fault and is turned into `ok(s)` at the top (a spawned thread's
  `exit` reaches `main` through the same shared outcome a thread's fault
  does), and `cobc` lowers it to `cb_exit(s)`, which in `cbrt` is the
  fault path without a report.

Not adopted: `exit` without destructors (candidate 2), even as a second
function; an `abort`; running other threads' destructors (a second
unwind could itself fault, as `[Fault-Unwind]` says).

## Compatibility impact

Additive: a new exported function `exit` in `std::process`, shadowed by
a program's own item of that name. D-0031's candidates 2 and 3 are
closed by this record. D-0143 listed `std::exit` as a revisit condition;
this is that revisit.

## Revisit conditions

- An `abort` that skips destructors, if a program is found that must
  end without running them (a destructor that hangs).
- A hook run on exit (`at_exit`), if programs ask; today a destructor on
  a value owned by `main` does the same.
