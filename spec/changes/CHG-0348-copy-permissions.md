# CHG-0348 — `copy_permissions` in `std::fs`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0272
Affects: `spec/21` 4.72.0 (§2e′ `[Copy-Permissions]`, the inventory rows, the listing); `spec/conformance.md`
(one row); `impl/std/fs.cb`; `impl/src/fileio.rs`; `impl/selfhost/cobfmt/files.cb`, `impl/selfhost/compare.sh`

## What changed

D-0272: `copy_permissions(StringView from, StringView to) : Result<void, FileError>` gives what is at `to`
the permissions of what is at `from` (the mode bits on Unix, the read-only flag on Windows), operation 17
of the std-private `path_op`, realized in `fileio.rs`, which both tools share. A documented `unittest` and
the row `conf.fs-copy-permissions`. `cobfmt-cb` calls it before renaming its temporary file over the
original, as the Rust `cobfmt` copies the permissions; `compare.sh` checks that a file of mode 750 keeps it.

## What changed for programs

A new function of `std`.
