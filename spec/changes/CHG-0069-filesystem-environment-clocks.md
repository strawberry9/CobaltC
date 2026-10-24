# CHG-0069 — Directories, the environment and the clocks

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0061
Affects: rule.stdlib.fs (new), rule.stdlib.env (new), rule.stdlib.prelude

## Problem / motivation

D-0061: no way to make, list, rename or remove files and directories,
read the environment, or measure time.

## Decision

D-0061: `make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`,
`path_kind` with `PathKind`; `env_var`, `current_dir`, `monotonic_ns`,
`unix_seconds`.

## What changed

- **`spec/21` 3.23.0:** §2e′ (new) `rule.stdlib.fs` (`[Make-Dir]`,
  `[Remove-File]`, `[Remove-Dir]`, `[Rename]`, `[List-Dir]`,
  `[Path-Kind]`) and `rule.stdlib.env` (`[Env-Var]`, `[Current-Dir]`,
  `[Clocks]`) with the `std` source; `PathKind` in §0's types; §0's
  table and scope paragraph.
- **`spec/conformance.md` 3.53.0:** the case below.
- **`spec/02-schema.md` 1.0.43:** §5's "in use" ranges.
- **Implementations:** `std` gains the functions and three primitives
  private to it (`fs_op`, `fs_query`, `clock_read`); `impl/src/fileio.rs`
  does the work for both tools (portable `std::fs`/`std::env`/
  `std::time`); `coby` realizes the primitives in `call_extern`, `cobc`
  compiles them to `cbrt`'s `cb_fs_op`, `cb_fs_query`, `cb_clock_read`.
  The guide (§21).

## Affected entities

`rule.stdlib.fs`, `rule.stdlib.env` (new); `rule.stdlib.prelude`.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.fs-env-clocks` (a file case with `$TMP`). A name that
is not UTF-8, a device's `Other` and permissions depend on the platform;
`impl/src/fileio.rs`'s unit test checks the primitives directly.

## Prior-art status

See D-0061.

## Revisit conditions

See D-0061.
