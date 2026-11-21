# D-0029 — Standard Input: `read` and `read_line`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0017, D-0020, D-0024, D-0028, rule.trust.extern-call, rule.stdlib.string
Affects: rule.stdlib.read (new), rule.stdlib.prelude

## Problem

`std` has no way to read input, so every CobaltC program runs without
any. A program compiled by an implementation that calls C can already
declare `extern fn read(i32 fd, rawptr<u8> buf, usize len) : isize;`
and read standard input inside `unsafe`, but it then has to manage the
buffer, find the line ends and check the UTF-8 itself. Output has a
safe, typed function (`print`, D-0028); input has none.

## Constraints

- No new tokens.
- A program that reads needs no `unsafe`, and what it gets is a checked
  value, not a claim (`inv.trust-transition`).
- An implementation need not provide standard input (the owner,
  2026-09-25: the interpreter will not read input; it says so).
- Existing programs keep working, including those that declare their
  own `extern fn read`.

## Candidate mechanisms

1. **An `extern` `read` in `std`, and `read_line` written in CobaltC on
   top of it.** **Selected.**
2. **`read_line` realized natively** by each implementation, with no
   `extern` beneath it.
3. **Only the `extern` `read`**, with line reading left to programs.
4. **No change**: programs declare libc's `read` themselves.

## Selected design

Candidate 1, with four choices made:

- **(a) `read` mirrors `write`**: `export extern fn read(rawptr<u8>
  buf, usize len) : isize`, always standard input, returning the number
  of bytes read, 0 at the end of input, −1 on an error. Programs can use
  it for binary input as they use `write` for binary output.
- **(b) `read_line() : Result<Option<String>, ReadError>`**, with
  `export enum ReadError { Io, Utf8(Utf8Error) }`. `Ok(Some(line))` is a
  line, `Ok(None)` the end of input, and `Err` an unreadable input or a
  line that is not UTF-8. It works with `?`.
- **(c) A line ends at `\n` only**, which is removed. A `\r` stays in
  the line; a last line without `\n` is still a line.
- **(d) Providing standard input is implementation-defined and
  documented.** An implementation without it stops the program at the
  first `read`, saying so. That is not a diagnostic of this
  specification: the program is not wrong, the implementation cannot
  run that part of it.

`read_line` reads one byte at a time into storage it allocates, since
`[Extern-Call]` may write only outside live objects, and passes the
line to `String::from_utf8`. It is ordinary CobaltC inside `std`, like
`String::from_utf8` itself; an implementation buffers the input beneath
`read` so that this costs no system call per byte.

## Rejected alternatives

- **2:** another native function each implementation must write and
  keep equal, where CobaltC can express the whole thing; and no binary
  input.
- **3:** every program repeats the same buffer, line and UTF-8 code in
  `unsafe`.
- **4:** input stays `unsafe` and untyped (the problem above).
- **Command-line arguments and an exit status:** separate decisions.
  Arguments are input too; an exit status must say what happens to
  destructors still pending.
- **`read_all`:** a short loop over `read_line`; it can be added later.
- **Removing `\r\n` as well:** hides a byte from programs that care,
  and the rule for "a line" stops being one byte.

## Semantic rationale

`read` is one more instance of `[Extern-Call]`, whose result is a
claim; `read_line` turns that claim into a typed value through the
checked validator `String::from_utf8`, the `[Trust-Transition]`
pattern `spec/20` describes. No new rule of the core is needed, only
`[Read]`'s statement of which bytes arrive.

## Usability

    match (read_line())
    {
        Ok(line) : …,      // Some(text), or None at the end
        Err(e) : …,        // Io, or Utf8 with the offset
    }

    fn next_word() : Result<Option<String>, ReadError>
    {
        auto line = read_line()?;
        …
    }

## Explainability

"`read_line` gives the next line without its `\n`, `None` at the end
of input, or an error saying why it could not" is one sentence.

## Implementation-feasibility

`read_line` needed no change to either implementation's core; `std`
gains 60 lines. The compiler maps `std::read` to a runtime function
that reads the runtime's buffered standard input, as it maps
`std::write`. The interpreter refuses `std::read` with
`unsupported: reading input needs cobc; coby does not read standard
input` and exit status 3, as it refuses an `extern` it cannot call.

## Compatibility impact

Extension. A program's own `read`, `read_line` or `ReadError` takes
precedence over `std`'s (D-0024), so no existing program changes
meaning.

## Prior-art status

- **Rust:** `stdin().read_line(&mut s)` → `io::Result<usize>`, keeping
  the `\n`; `lines()` removes `\n` and `\r\n`; invalid UTF-8 is an
  `io::Error`.
- **Go:** `bufio.Scanner`, removing `\n` and a `\r` before it; errors
  through `Err()`.
- **Python:** `input()` removes `\n`; end of input is `EOFError`.
- **C:** `fgets` keeps the `\n`; `NULL` at the end or on an error.

## Invariant traceability

`inv.trust-transition`: bytes from `read` are claims until
`String::from_utf8` validates them. `inv.string.utf8-validity`: a
`String` from `read_line` is valid UTF-8, since it passed that check.

## Revisit conditions

- A need for command-line arguments or an exit status.
- A need to read input faster than a byte per call through `std`.
- Reading from files or other sources (would suggest one reading
  interface for all of them).
