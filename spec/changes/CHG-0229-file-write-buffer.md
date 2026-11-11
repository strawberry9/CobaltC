# CHG-0229 — `File` writes are buffered (`[File-Buffer]`)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0201)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0201
Affects: `spec/21` 4.50.0 (§2e `[File-Buffer]`, `[Flush]`), `spec/conformance.md` 3.194.0; `impl/src/fileio.rs`,
`impl/src/procio.rs`, `impl/src/lib.rs`, `impl/cbrt/src/lib.rs`

## What changed

- **`spec/21` §2e:** the "Buffering" note now says writing is buffered. The new rule `[File-Buffer]` fixes the
  latest points at which written bytes reach the environment:
  - the file's next operation, or its destruction;
  - any other file-system operation of the program;
  - a child process's start;
  - the program's end, however it ends.

  A failure for buffered bytes is reported by the file's next operation. `[Flush]` writes the buffer out before its
  fsync.
- **`impl/src/fileio.rs`, shared by both tools:**
  - an 8 KiB write buffer per open file; a larger write is not buffered;
  - `flush_all`, called by every path-taking entry point, by `procio` before a child starts, and at each tool's end
    of program (coby after `run_main`; cbrt's `flush_at_end` in `cb_exit`, a fault, `cb_terminate_ok`, a runtime
    panic);
  - a failure kept per file (`failed`) for its next operation.
- **Conformance:** `conf.file-write-buffer-visible`.

## What did not change

Every program writes the same bytes, and sees them by every route it has. Only an outside observer of a file while
the program runs, or a kill from outside, can tell. extsort (round 8): 100,050 `write` calls became 100, and its time
went from 1.01 s to 0.61 s.
