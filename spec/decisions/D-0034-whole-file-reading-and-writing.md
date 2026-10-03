# D-0034 — Files: `read_file` and `write_file`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0024, D-0029, D-0031, D-0032, rule.stdlib.string, rule.fn.program
Affects: rule.stdlib.file (new), rule.stdlib.prelude

## Problem

A program can take a file name as an argument (D-0031) and parse text
(D-0032), but cannot open the file. The Tier 5 showcases had to take
their data as arguments.

## Constraints

- No new tokens.
- A program that reads or writes a file needs no `unsafe`, and what it
  reads is a checked value (`inv.trust-transition`,
  `inv.string.utf8-validity`).
- The interpreter and the compiler behave alike (the owner,
  2026-09-25: both tools read and write files; unlike standard input,
  a file gives the same bytes to both).

## Candidate mechanisms

1. **Whole-file functions**: `read_file(&path) : Result<String,
   FileError>`, `write_file(&path, &text) : Result<void, FileError>`.
   **Selected.**
2. **A `File` handle**, a resource whose destructor closes it, with
   `File::open`, `File::read_line`, `File::write`.
3. **Both**, `read_file` written on the handle.

Errors:

4. **`FileError { NotFound, Denied, Io, Utf8(Utf8Error) }`.**
   **Selected.**
5. `ReadError { Io, Utf8 }` reused.
6. `Io(i32)` carrying the operating system's error number.

## Selected design

Candidates 1 and 4, with writing included, in both tools:

- `read_file` returns the whole file as a `String`, or why not:
  `NotFound`, `Denied`, `Utf8(e)` with the offset of the first byte
  that is not UTF-8, or `Io` for anything else.
- `write_file` makes the file hold exactly `text`, creating it or
  replacing its contents; `NotFound` when a directory on the path does
  not exist.
- The path is `ref<String, shared>`, as `parse` takes its text: paths
  usually come from `arg`. How a path names a file is the
  implementation's, documented: both pass it to the operating system,
  which resolves a relative path against the working directory.
- `FileError`'s `Io` and `Utf8` share names with `ReadError`'s. A
  `match` names them unqualified (the scrutinee fixes the enum); code
  constructing one writes `FileError::Io` (`spec/17`), as `std`'s own
  code now does for both enums.

## Rejected alternatives

- **2, 3:** a new resource type and several functions, for streaming
  files too large to hold, which no program here needs. It can be
  added later beside these.
- **5:** a missing file and a refused one look alike, though programs
  react to them differently.
- **6:** platform-specific numbers in portable programs.
- **Paths as `str`:** paths come from arguments, which are `String`s.
- **Reading only:** a tool that reads a file usually writes one.

## Semantic rationale

`read_file` is the trust-transition pattern `read_line` follows: bytes
from the environment become a `String` only through
`String::from_utf8`. `write_file` writes a `String`'s bytes. Neither
changes any rule of the core; files are part of the environment, as
arguments and standard input are (`rule.fn.program`).

## Usability

    String path = Result::unwrap_or(arg(0), String::new());
    match (read_file(&path))
    {
        Ok(text) : { … },
        Err(e) : match (e)
        {
            NotFound : { print("no such file\n"); },
            _ : { print("cannot read it\n"); },
        },
    }

    String out = String::from_str("totals.txt");
    Result::unwrap_or(write_file(&out, &report), ());

## Explainability

"`read_file(&path)` gives the file's text or says why not;
`write_file(&path, &text)` replaces the file with `text`" is one
sentence.

## Implementation-feasibility

`std` gains about 90 lines of CobaltC over two primitives private to
`std`; one Rust source file (`impl/src/fileio.rs`) does the file access
for both tools (`cbrt` includes it). Implementing it found and fixed a
`cobc` bug: a parameter of type `void` (a generic function
instantiated at `T = void`) produced invalid C.

## Compatibility impact

Extension. `std`'s own code names `ReadError`'s variants qualified. A
program's own `read_file`, `write_file` or `FileError` takes precedence
(D-0024).

## Prior-art status

- **Rust:** `std::fs::read_to_string`, `std::fs::write`; `io::Error`
  with `ErrorKind::{NotFound, PermissionDenied, …}`; `File` for
  streaming.
- **Go:** `os.ReadFile`, `os.WriteFile`; `errors.Is(err, fs.ErrNotExist)`.
- **Python:** `open(p).read()`; `FileNotFoundError`, `PermissionError`.
- **Zig:** `std.fs.cwd().readFileAlloc`, `writeFile`.

## Invariant traceability

`inv.trust-transition`, `inv.string.utf8-validity`: a file's bytes
become a `String` only through `String::from_utf8`.

## Revisit conditions

- Files too large to hold, or reading as they grow: a `File` handle.
- Appending, directories, removing files.
- Bytes that are not text (`Vec<u8>` reading and writing).
