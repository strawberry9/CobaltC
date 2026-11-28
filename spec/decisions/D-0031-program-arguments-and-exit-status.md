# D-0031 — Program Arguments and Exit Status

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0008, D-0020, D-0024, D-0029, rule.fn.program, rule.trust.extern-call, rule.stdlib.string
Affects: rule.fn.program, rule.stdlib.args (new), rule.stdlib.prelude, spec/22 §3

## Problem

A CobaltC program can read standard input (D-0029) and print
(D-0028), but it cannot see the arguments it was started with, and it
cannot tell its environment whether it succeeded: every program that
runs to completion exits with status 0. D-0029 left both to this
decision. It also named the difficulty: an exit status must say what
happens to destructors still pending.

Separately, `rule.fn.program` requires `fn main()`, but neither
implementation checked `main`'s parameters or return type.

## Constraints

- No new tokens.
- Every destructor D-0008 would run on ordinary exit still runs.
- A program that reads its arguments needs no `unsafe`, and what it
  gets is a checked value (`inv.trust-transition`): on Linux an
  argument is bytes, not necessarily UTF-8.
- Existing programs keep working, including those declaring their own
  `arg`, `arg_count` or `arg_bytes`.
- The interpreter and the compiler give the same output for the same
  program and arguments (the owner, 2026-09-25: `coby` supports both).

## Candidate mechanisms

Exit status:

1. **`main` may return `u8`.** **Selected.**
2. **`std::exit(u8) : never`, skipping destructors** (C's `exit`,
   Rust's `process::exit`).
3. **`std::exit(u8) : never`, unwinding** the calling thread as a fault
   does, so destructors run.
4. **`main` returns `i32`**, as in C.

Arguments:

5. **`arg_count()` and `arg(i)`**, written in CobaltC on one `extern`
   primitive, `arg_bytes`. **Selected.**
6. **`args() : Result<Vec<String>, Utf8Error>`**, all arguments at once.
7. **`fn main(Vec<String> args)`**, a second `main` signature.

## Selected design

Candidates 1 and 5, with four choices made:

- **(a) `fn main() : u8`** is allowed beside `fn main()` and
  `fn main() : void`. `main`'s value is the program's exit status; a
  `main` without a result gives 0. `main` has no parameters. A root
  `main` with parameters or another return type is `diag.no-main`, as
  a generic or `extern` one already is.
- **(b) `arg_bytes(usize i, rawptr<u8> buf, usize len) : isize`** is
  an `extern` in `std`, like `read`: it copies up to `len` bytes of
  argument `i` into `buf` and returns the argument's whole length, or
  −1 when there is no argument `i`. Programs can use it for arguments
  that are not UTF-8.
- **(c) `arg_count() : usize` and `arg(usize i) : Result<String,
  Utf8Error>`** are written in CobaltC on `arg_bytes`. `arg(i)` with
  `i ≥ arg_count()` faults `diag.index-out-of-bounds`, as a `Vec`
  index does.
- **(d) Argument 0 is the first argument after the program**, not the
  program's name. The name differs between implementations (a `.cb`
  path for `coby`, an executable for `cobc`), so including it would
  make the same program print different things.

## Rejected alternatives

- **2:** skips destructors D-0008 promises, so a file written through a
  buffer loses its last bytes.
- **3:** usable deep in a call chain, but it needs a new termination
  rule beside `[Fault-Unwind]`, and called from another thread it
  abandons the other threads' resources. `?` and `return` already carry
  a failure back to `main`. It can be added later if a program needs it.
- **4:** Linux keeps only the low 8 bits, so how an `i32` is cut down
  would have to be implementation-defined; `u8` is exactly what the
  environment keeps.
- **6:** one argument that is not UTF-8 makes every argument
  unreadable. It is a short loop over `arg`, and can be added later.
- **7:** an argument that is not UTF-8 would have to fault before
  `main` begins, and every program would build a `Vec` at startup.
- **The program's name as argument 0:** see (d). A separate function
  can provide it later.
- **`main` returning `Result<void, E>`:** printing `E` needs a way to
  print any type, which the language does not have (D-0028).

## Semantic rationale

`main` returning a value changes only `[Terminate-Ok]`: termination is
`ok(s)`. The status is known only after `main`'s final `[Block-Exit]`,
which has run every destructor and joined every thread, so no rule of
destruction or concurrency changes. `arg_bytes` is one more instance of
`[Extern-Call]`, whose result is a claim; `arg` turns that claim into a
typed value through the checked validator `String::from_utf8`, the
`[Trust-Transition]` pattern `read_line` follows.

## Usability

    import std;

    fn main() : u8
    {
        if (arg_count() != 1)
        {
            print("usage: greet NAME\n");
            return 2;
        }
        match (arg(0))
        {
            Ok(name) :
            {
                print("hello, ");
                print(&name);
                print("\n");
                0
            },
            Err(_) :
            {
                print("the name is not UTF-8\n");
                1
            },
        }
    }

    $ cobc --run greet.cb Ada          # hello, Ada        (status 0)
    $ coby greet.cb                    # usage: greet NAME (status 2)

## Explainability

"`arg(i)` is argument `i`, counting from 0 after the program's name;
`main` can return a `u8`, which is the exit status" is one sentence.

## Implementation-feasibility

`std` gains about 50 lines. Both tools check `main`'s signature in the
front end they share. The compiler passes `argc`/`argv` to its runtime
and `main`'s value to `exit`; the interpreter takes the arguments after
the file name and exits with `main`'s value. `cobc --run file.cb a b`
passes `a b` to the program.

## Compatibility impact

Extension, except that a root `main` with parameters or a return type
other than `void`/`u8` is now rejected, as `rule.fn.program` always
required. No program in the repository has one. A program's own `arg`,
`arg_count` or `arg_bytes` takes precedence over `std`'s (D-0024).

## Prior-art status

- **C:** `int main(int argc, char **argv)`; `argv[0]` is the name.
- **Rust:** `std::env::args()` (panics on non-UTF-8) and `args_os()`;
  `main` may return `ExitCode`, built from a `u8`.
- **Zig:** `pub fn main() u8`; `std.process.argsAlloc`.
- **Go:** `os.Args` (with the name); `os.Exit(int)` skips deferred
  calls.
- **Python:** `sys.argv` (with the name); `sys.exit` raises
  `SystemExit`, so `finally` blocks run.

## Invariant traceability

`inv.trust-transition`: argument bytes from `arg_bytes` are claims until
`String::from_utf8` validates them. `inv.string.utf8-validity`: a
`String` from `arg` is valid UTF-8. D-0008: every destructor runs
before the status is reported, since `main` returns through its
`[Block-Exit]`.

## Revisit conditions

- A program needing to exit from deep in a call chain, or from another
  thread (candidate 3).
- A need for the program's own name.
- Environment variables (would suggest the same `extern`-plus-wrapper
  pattern).
