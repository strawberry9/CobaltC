# D-0059 — `File::printf`

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0038, D-0054
Affects: rule.stdlib.file-handle, rule.stdlib.format

## Problem

Writing text to a file took a `String` first: `File::write_str(&mut f,
&String::from_str("…"))` borrows a temporary, which is rejected, and a
formatted line took `sprintf` and then `write_str`. D-0054 listed a
formatted write as a revisit condition; real-world testing met it in
every program that wrote a log or a report.

## Candidate mechanisms

1. **`File::printf(&mut f, format, a…) : Result<void, FileError>`**,
   formatted as `printf` formats, like `String::appendf`. **Selected.**
2. `File::write_str` taking any printable value (`String::append<T>`'s
   way): literals, but no formatting.
3. Unchanged.

## Selected design

Candidate 1. The format is a string literal checked against the
arguments when compiled (`[Format-Invalid]`, `[Format-Arg-Mismatch]`);
the text is written as `File::write` writes (unbuffered); an empty text
writes nothing.

## Rejected alternatives

- **2:** half the need; formatting is the common case.
- **3:** the friction measured.

## Implementation-feasibility

`modres` rewrites `File::printf(f, fmt, a…)` to
`File::write_text(f, $fmt(fmt, a…))`, as it rewrites `String::appendf`;
`File::write_text` is `std` source over `file_op`.

## Compatibility impact

Extension. A program's own `File::printf` cannot exist (`File` is
`std`'s); a program's own `File` type takes precedence (D-0024).

## Prior-art status

- **C:** `fprintf`. **Rust:** `write!(f, …)`. **Go:** `fmt.Fprintf`.

## Revisit conditions

- Buffered writing (a `flush`).
