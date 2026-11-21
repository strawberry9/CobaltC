# CHG-0442 — `read_file` reads a file once, to its end

Status: implemented (2026-11-13); Update 184
Affects: `impl/std/fs.cb` (`read_file`), `impl/std/std.cb` (a comment); `spec/21` 4.82.1 (§2e
`rule.stdlib.file`, the §2 table, §0's trusted table); `spec/conformance.md` (3.309.0);
`impl/selfhost/front/prelude.cb` (regenerated)

## Why

From the game-of-life report (`private/game-of-life/cobc-bugs-found.txt`, P1): `read_file` of a file over
4096 bytes read it twice and copied it into its `Vec` one `Vec::push` a byte. Looking closer found a
correctness bug behind the cost. The primitive `file_read` reads the whole file at every call and copies
the first `cap` bytes, so `read_file` called it with 4096 bytes of room, learned the length, and called it
again with room for all of it. A pipe or a device cannot be read twice: `read_file("/dev/stdin")` with
10,000 bytes piped in returned `Ok` of an empty `String`, under `coby` and under `cobc`. A file another
program changed between the two reads gave the second read's bytes, with the first read's length taken
for granted. `[Read-File]` says the result is the file's contents; for these it was not.

## What changes

`read_file` is `read_bytes` (one `File::open`, `File::read_to_end` in 64 KiB pieces into the `Vec`'s
spare room) followed by `String::from_utf8`. It has no `unsafe` block of its own any more, so its row
leaves §0's trusted table. The errors are the same: `NotFound`, `Denied`, `Io` for a directory, `Utf8(e)`
with the offset of the first bad byte (checked under `coby` and `cobc` with GCC and Clang).
`file_read` stays, private to `std`, with no caller in it.

**Conformance:** `conf.read-file-pipe-once`
(`impl/conformance/21-standard-library-semantics/read_file_pipe_once_ok.cb`, unix): 4,200 bytes piped to
`read_file("/dev/stdin")` are all read. It printed `0 false` before.
