# D-0050 — The interpreter reads standard input too

Status: ACCEPTED (2026-09-26, owner: "see if coby can support reading standard input like cobc can, but only if coby can continue to be built and execute on Windows")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §21
Depends on: D-0029, D-0019 (the GIL), rule.stdlib.read
Affects: `coby`'s realization of `std::read`; D-0029's constraint "the interpreter will not read input"

## Problem

D-0029 gave `std` standard input (`read`, `read_line`) and, by the
owner's choice then, left `coby` refusing it (exit 3). A program that
reads could therefore only be run compiled, and the conformance cases
that read had no interpreter to compare with.

## Selected design

`coby` realizes `std::read` as `cbrt`'s `cb_read_in` does: standard
output is flushed, then up to `len` bytes are read from Rust's buffered
standard input (`std::io::stdin().lock()`) into the arena at `buf`;
the result is the count, 0 at the end of input, −1 on an error. Only
the Rust standard library is used, so it builds and runs on Windows as
on Linux. While the read waits, `coby` releases its interpreter lock
(D-0019's GIL), as a blocked `join` or `lock` does, so other threads
keep running.

## Rejected alternatives

- **Keep the refusal:** the owner asked for input in both tools once
  it could be done portably; it can.
- **Read through the C library (`ffi`):** `coby` calls C only on
  x86-64 Linux; Rust's `stdin` is everywhere.

## Semantic rationale

`[Read]` (`spec/21`) states the outcome, not the implementation; both
now meet it.

## Usability

    printf 'ada\nlin\n' | coby greet.cb

## Explainability

"Both tools read standard input."

## Implementation-feasibility

A dozen lines in `coby`'s extern dispatch. On a Windows console, Rust
reads the console as UTF-16 and hands over UTF-8; input that is not
valid Unicode there is an I/O error (`Err(Io)`), where a pipe passes
any bytes.

## Compatibility impact

Relaxation: programs that `coby` refused now run.

## Prior-art status

Every interpreter reads standard input.

## Invariant traceability

`inv.trust-transition`: the bytes still become a `String` only through
`String::from_utf8`, as before.

## Revisit conditions

None.
