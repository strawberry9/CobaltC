# D-0178 — Standard output is buffered; `flush_stdout`

Status: ACCEPTED (2026-10-06, the owner delegated the choice: "proceed with your recommendations in all cases")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0039 (`printf`), D-0040 (`eprintf`), D-0050 (reading standard input), D-0143 (processes), D-0150 (`exit`)
Affects: `spec/21` §0, §2a `rule.stdlib.print` `[Print-Buffer]`, §2f (4.42.0), `spec/conformance.md` (3.170.0), the guide (chapter P, §21), `impl/src/outbuf.rs`, `impl/cbrt`, `impl/std/io.cb`; `CHG-0206`

## Problem

Both implementations wrote every `printf` straight to the operating
system and flushed it: one `write(2)` per call. That kept standard
output and standard error in program order, but it made printing
line by line the slowest part of many programs. In the measurements
of 2026-10-05 (`stress/perfguide/e08`), 2,000,000 `printf` lines to a
file took 7.6 s, and the guide's performance chapter told programs to
build one `String` and print it once instead (2.8 s), which is the
opposite of the plain way to write the program.

## Candidate mechanisms

1. **Unchanged: write and flush at every call.** Simple, with every
   byte visible at once; the cost above stays, and the guide keeps
   advising against the obvious code. Not adopted.
2. **Buffer standard output, with fixed flush points.** Before
   standard error, standard input, a child sharing standard output,
   the program's end (return, `exit`, fault), and at each line on a
   terminal. Within the program, the order of the two streams is
   unchanged, and an interactive program behaves as before. A program
   watched through a pipe while it runs (progress into `tee`, a log)
   shows its output in pieces, with no way to ask for it sooner.
3. **Option 2 and `flush_stdout()` in `std::io`.** Selected. The one
   case option 2 leaves without an answer gets a function that names
   one common intent (Master Instructions §9's admission test).
4. **Buffering chosen by the program** (a mode switch, or a buffered
   writer type). More surface, and the slow choice would remain the
   default that most programs use. Not adopted.

## Selected design

`[Print-Buffer]` (`spec/21` §2a) fixes the latest moments at which the
bytes written to standard output reach the operating system: before
any byte to standard error, before any read of standard input, before
a child that writes to the program's standard output starts
(`Command::status`), at the program's end however it ends, when
`flush_stdout()` returns, and, when standard output is a terminal, at
the end of each line. Standard error is not buffered. The buffer's
size is not part of the rule; both implementations use 8 KiB.

`flush_stdout()` is an ordinary exported function of `std::io`, safe,
returning nothing (as `printf`'s failure is not observable,
`rule.stdlib.print`), written over a primitive private to `std`.

Measured with `cobc`, 2,000,000 lines to a file: `printf` per line 7.6 s
before, 0.77 s after; one `String` printed once 2.8 s, unchanged.

## Compatibility impact

Additive: every program writes the same bytes, in the same order
relative to standard error and standard input. Different only in timing,
for a program whose standard output is a pipe or a file and which
someone watches while it runs, and for output still buffered when the
process is killed from outside. A program's own C code writing to
standard output through `stdio` has its own buffer, as it did before.

## Revisit conditions

- A buffered `File` writer (D-0059's revisit condition) would use the
  same rule's shape.
- A program that needs standard output unbuffered throughout would
  argue for a mode; none so far.
