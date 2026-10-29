# D-0040 — Standard Error: `eprintf`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0031, D-0038, D-0039, rule.stdlib.format
Affects: rule.stdlib.format, rule.stdlib.prelude, diag.format-invalid

## Problem

A program had one output stream. An error message went to standard
output with the program's results, so `tool > out.txt` hid the error
and put it in the file. D-0039 left standard error as a revisit
condition.

## Constraints

- No new tokens; D-0038's checking applies unchanged.
- No stream type: nothing to open, close or pass around.
- D-0031 stands: the exit status is `main`'s value, and every
  destructor runs.

## Candidate mechanisms

1. **`eprintf(f, …)`**, `printf` to standard error. **Selected.**
2. `fprintf(stream, f, …)` with `std::stdout`/`std::stderr` values: a
   stream type for two streams.
3. An `eprint`-style unformatted call: a second way to write, which
   D-0039 removed for standard output.

With it, the owner considered `std::exit(u8)` again (D-0031's revisit
condition) and kept D-0031's decision: a program exits with `main`'s
value, errors reaching `main` through `?` and `return`.

## Selected design

- `eprintf(f, a1, …, an)`: the text `printf` would write, written to
  standard error. The same formats, checks and diagnostics.
- It writes through a private extern of `std`, `write_err`, the
  counterpart of `write`. Both streams are written at once, so their
  text interleaves in the order the calls ran.

## Rejected alternatives

- **`fprintf`:** a stream type and two constant values for one choice
  between two streams; files are whole-file (D-0034), so there is no
  third stream to pass.
- **`exit`:** see D-0031; unchanged.

## Semantic rationale

`[Eprintf]` is `[Printf]` with `write_err` in place of `write`: an
`[Extern-Call]` inside `std`, its result discarded.

## Usability

    eprintf("%s: not found\n", &path);
    return 1;

## Explainability

"`eprintf` is `printf` to standard error."

## Implementation-feasibility

`modres` rewrites `eprintf(f, …)` to `std`'s private
`eprint_str($fmt(f, …))`; `coby` writes `write_err`'s bytes to its
standard error, `cobc` calls `cb_write_err`.

## Compatibility impact

Extension. A program's own `eprintf` takes precedence (D-0024).

## Prior-art status

C `fprintf(stderr, …)`; Rust `eprintln!`/`eprint!`; Go
`fmt.Fprintf(os.Stderr, …)`.

## Invariant traceability

None new: `write_err` reads the bytes of a `str` inside `std`, as
`write` does for `printf`.

## Revisit conditions

- A third output stream (a file handle, a pipe).
- Exiting from deep in a call chain (D-0031's candidate 3).
