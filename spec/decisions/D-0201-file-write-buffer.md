# D-0201 — `File` writes are buffered

Status: ACCEPTED (2026-10-09; the owner chose option 2, buffering with fixed flush points)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0054 (file handles), D-0059 (`File::printf`, revisit condition "buffered writing"), D-0178
(standard output buffered, revisit condition "a buffered `File` writer"), D-0181 (`File::flush`)
Affects: `spec/21` §2e (`[File-Write]`, the "Buffering" note), `impl/src/fileio.rs` (shared by both tools), the
guide (§21); `CHG-0229`

## Problem

Every `File::write`, `write_text` and `printf` is one system call: `spec/21` says "each `write` is handed to the
environment before it returns". A program writing a file line by line pays for that on every line.

Measured on extsort (round 8, 50,000 lines into chunk files): 100,050 `write` calls, with system time 0.25 s of a
1.03 s run, about a quarter. Python, and C's `stdio`, buffer file writes by default. D-0178 made the same choice for
standard output (7.6 s down to 0.77 s for 2,000,000 lines).

## Candidate mechanisms

1. **Unchanged.** Every write reaches the operating system before it returns. Simple. The cost stays, and the guide
   would have to advise building one `String` per file, the opposite of the plain way to write the program.
2. **Buffer each open `File`'s writes, with fixed flush points (recommended).** Following D-0178's
   `[Print-Buffer]`, a rule `[File-Buffer]` fixes the latest moments at which bytes written to a `File` reach the
   operating system:
   - any other operation on the same handle: a read, `read_line`, `seek`, `len`, `position`, `flush` or `close`, or
     its destruction;
   - **any other file-system operation of the program, on any path:** opening a file, `read_file`, `write_file`,
     `file_info`, removing, renaming, copying, directory operations. So a program that writes a file and then reads
     it back, by any route, always sees every byte;
   - before a child process starts;
   - at the program's end, however it ends (return, `exit`, fault).

   A write larger than the buffer goes straight through.

   Within one program, nothing observable changes. Another process watching the file while it is being written (a
   `tail -f`), or a kill from outside, sees the bytes later or loses the unflushed ones, as with standard output.
   `File::flush` (D-0181) already exists and then also empties the buffer, so a program that wants another process
   to see its bytes at once has a way.
3. **A separate buffered writer** (`BufferedFile`, or `File::buffered(f)`). Purely additive. The default stays
   slow, and it is more surface for the same intent. D-0178 rejected the same mechanism for standard output.

## Recommendation

Option 2. In both tools it lives in `impl/src/fileio.rs`: a per-entry buffer of 8 KiB, which is not part of the
rule. Every file-system entry point empties all buffers first, and so does each tool's process start and program
end. Expected gain is about a quarter of extsort's time, and as much in any program that writes a log or a report
line by line.

## Result

Implemented as `CHG-0229` (`spec/21` 4.50.0). extsort: 100,050 `write` calls became 100, and its time went from 1.01 s
to 0.61 s (Python 0.18 s).

## Compatibility impact

Additive within a program: the same bytes, in the same order relative to every operation the program itself can
make. Different only in timing for an outside observer of the file while the program runs, and for bytes still
buffered when the process is killed from outside. The "Buffering" note of `spec/21` §2e changes from "writing is
not [buffered]" to `[File-Buffer]`.

## Revisit conditions

- A program that needs every write visible at once to another process, with too many writes for a `flush` each,
  would argue for a per-handle mode.
