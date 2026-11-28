# D-0061 — Directories, the environment and the clocks

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0034, D-0054, rule.stdlib.file, rule.fn.program
Affects: rule.stdlib.fs (new), rule.stdlib.env (new), rule.stdlib.prelude

## Problem

A program could read and write a file but not make, list, rename or
remove one: the archiver of the 2026-09-27 real-world testing extracted
to `x_`-prefixed names beside each other for want of a directory, no
program could clean up after itself, and the showcase runner had to give
tools a fresh directory. Nor could a program read an environment
variable, ask the working directory, or measure time.

## Constraints

- No new tokens; the interpreter and the compiler alike; `coby`
  portable to Windows (`std::fs`, `std::env`, `std::time` only).
- `FileError` unchanged: a new variant would break every exhaustive
  `match` on it already written.
- Outputs a program can make deterministic are deterministic (a
  directory's listing is sorted).

## Candidate mechanisms

1. **Free functions in `std`** over paths as `String`s, as
   `read_file` is. **Selected.**
2. A `Dir` resource (an open directory handle, read entry by entry).
3. A `Path` type.

Existence:

4. **`make_dir` returns `Ok(true)`/`Ok(false)`** (made / a directory
   already there). **Selected.**
5. A `FileError::Exists` variant.

Kinds:

6. **`path_kind(p) : Result<PathKind, FileError>`,
   `PathKind { File(u64), Dir, Other }`.** **Selected.**
7. `exists`, `is_dir`, `file_len` separately.

Environment and time:

8. **`env_var(&name) : Result<Option<String>, Utf8Error>`,
   `current_dir()`, `monotonic_ns() : u64`, `unix_seconds() : i64`.**
   **Selected.**
9. A `Duration`/`Instant` type.

## Selected design

Candidates 1, 4, 6 and 8: `make_dir`, `remove_file`, `remove_dir` (empty
only), `rename` (replacing a file at the target), `list_dir` (names in
byte order, never `.` or `..`), `path_kind`, `env_var`, `current_dir`,
`monotonic_ns`, `unix_seconds`. A name or value that is not UTF-8 is
`Utf8(e)` with its offset, as `String::from_utf8` reports it.

## Rejected alternatives

- **2:** a resource for a listing that fits in a `Vec<String>`.
- **3:** a type for what a `String` already carries.
- **5:** breaks existing exhaustive matches; "made or already there" is
  the question a program asks.
- **7:** three calls where one `match` with nested patterns answers all.
- **9:** a type for one subtraction of `u64`s.
- No `set_current_dir`: a working directory shared by every thread,
  changed under them.

## Semantic rationale

All of it is the program's environment (`rule.fn.program`), like its
arguments and files: nondeterministic in what the environment answers,
nowhere else.

## Usability

    match (path_kind(&p))
    {
        Ok(File(n)) : printf("%v bytes\n", n),
        Ok(Dir) : printf("a directory\n"),
        Ok(Other) : printf("something else\n"),
        Err(NotFound) : printf("nothing there\n"),
        Err(_) : printf("cannot tell\n"),
    }

    u64 t0 = monotonic_ns();
    work();
    printf("%v ms\n", (monotonic_ns() - t0) / 1_000_000);

## Implementation-feasibility

Three primitives private to `std` (`fs_op`, `fs_query`, `clock_read`)
over `impl/src/fileio.rs`, shared by both tools as `file_op` is; the
functions are `std` source.

## Compatibility impact

Extension. A program's own `rename`, `make_dir`, … takes precedence
(D-0024).

## Prior-art status

- **Rust:** `std::fs::{create_dir, remove_file, remove_dir, rename,
  read_dir, metadata}`, `std::env::{var, current_dir}`,
  `std::time::{Instant, SystemTime}`.
- **Go:** `os.Mkdir`, `os.ReadDir` (sorted), `os.Stat`, `os.Getenv`,
  `time.Now`.
- **C/POSIX:** `mkdir`, `unlink`, `rmdir`, `rename`, `opendir`, `stat`,
  `getenv`, `clock_gettime`.

## Revisit conditions

- Recursive removal and making (`make_dirs`), permissions, symbolic
  links, file times.
- Setting environment variables; a working directory per thread.
