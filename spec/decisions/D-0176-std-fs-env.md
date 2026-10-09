# D-0176 — `std::sys` divided into `std::fs` and `std::env`; clocks to `std::time`

Status: ACCEPTED (2026-10-05, the owner: "accept option 1 as D-0176 and implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0136 (`std`'s submodules, re-exported by `std`), D-0031 (arguments), D-0054 (`File`), D-0061 (directories, environment, clocks), D-0134 (`FileError`), D-0140 (`std::time`), D-0141 (paths and file metadata)
Affects: `spec/21` §0, §2c, §2e, §2e′ (4.40.0), `spec/conformance.md` (3.168.0), `CobaltC_Master_Instructions.md` §9, the guide §21, `impl/std/` (`fs.cb`, `env.cb` new; `sys.cb` removed; `io.cb`, `time.cb`, `std.cb`), `src/prelude.rs`; `CHG-0204`

## Problem

`std::sys` had become the default home for anything system-shaped: the
program's arguments, environment variables and directories, the whole
filesystem API (directories, paths, metadata, copying), and three
clocks. Its items' siblings lived elsewhere: `unix_ms` in `std::time` but
`unix_seconds` in `std::sys`; `read_file`, `write_file` and `File` in
`std::io` but `copy_file`, `remove_file` and `file_info` in `std::sys`.
The grouping was an accident of the order the decisions were taken in
(D-0031, D-0061, D-0141), and the guide and spec taught it.

## Candidate mechanisms

1. **Divide by theme and retire `std::sys`:** `std::fs` (everything about
   files and paths, including `File`, the whole-file functions and
   `FileError` from `std::io`), `std::env` (arguments and environment),
   the clocks into `std::time`; `std::io` keeps the standard streams.
   Selected.
2. **Move only the clocks and rename `std::sys` to `std::fs`/`std::env`**,
   leaving files split between `std::fs` and `std::io`. Not adopted.
3. **No change.** Not adopted: the next addition would land in
   `std::sys` too.

## Selected design

| Submodule | Items |
|---|---|
| `std::io` | `printf`, `eprintf`, `stdout_write`, `stdin_read`, `read_line`, `read_all`, `read_all_bytes` |
| `std::fs` | `FileError`, `File` and its functions, `read_file`, `write_file`, `read_bytes`, `write_bytes`, `make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`, `PathKind`, `path_kind`, the `path_` functions, `FileInfo`, `file_info`, `copy_file`, `make_dir_all`, `remove_dir_all` |
| `std::env` | `arg_count`, `arg`, `arg_bytes`, `env_var`, `current_dir`, `set_current_dir`, `temp_dir`, `home_dir` |
| `std::time` | as before, and `monotonic_ns`, `unix_seconds`, `sleep_ms` |

No item's name, signature or meaning changes. `std` re-exports every
submodule (D-0136), so a program that writes `import std;` sees every
item by its short name as before; only `import std::io;` alone no longer
brings in the file items, and `std::sys` no longer exists. The
std-private helpers that `std::fs` and `std::env` share moved to `std`'s
root, as the primitives they wrap are.

**The rule, in §0 and in Master Instructions §9:** a `std` item goes in
the submodule whose existing items it works with; there is no catch-all
submodule.

## Compatibility impact

Additive for every program that imports `std` (all programs in the
repository do). A program that imported `std::sys`, or imported
`std::io` alone for its file items, must name `std::fs` or `std::env`.

## Revisit conditions

- A new subject of `std` that fits no existing submodule: a new
  submodule, not an existing one stretched.
