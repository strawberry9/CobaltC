# D-0272 — `copy_permissions`: a file's permissions given to another

Status: ACCEPTED (2026-10-19, delegated by the owner: "any decisions that need to be made to complete all
of this work, you are authorised to make")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9 (the std helper admission test)
Depends on: D-0061 (`std::fs`), D-0141 (the path functions over `path_op`), D-0181 (`File::flush`)
Affects: `spec/21` §2e′ (`[Copy-Permissions]`, the inventory rows, the listing); `spec/conformance.md`;
`impl/std/fs.cb`; `impl/src/fileio.rs` (shared by both tools); `impl/selfhost/cobfmt/files.cb`; `CHG-0348`

## Problem

The write-then-rename pattern replaces a file safely: write a temporary file beside it, make it durable
(`File::flush`), and `rename` it over the original. The new file is created with the default permissions,
so an executable script, a file readable only by its owner, or a read-only file loses its permissions on
the way. `std::fs` can see whether a file is read-only (`FileInfo::readonly`) but cannot set anything, so
a CobaltC program cannot keep them. The formatter written in CobaltC (`impl/selfhost/cobfmt`, CHG-0346)
replaces files this way and was the one place it differed from the Rust `cobfmt`, which copies them.

## Options

1. `copy_permissions(StringView from, StringView to) : Result<void, FileError>`: what is at `to` given the
   permissions of what is at `from`: on Unix the mode bits, on Windows the read-only flag, as the
   platform's library carries them. One intent, the one the pattern needs, with nothing to interpret.
2. `FileInfo` grows a `mode : u32` and a `set_mode(path, mode)` is added: general, but the number means
   nothing on Windows, and a program that only wants to keep a file's permissions must read, carry and
   write a number it never looks at.
3. `set_readonly(path, bool)`: portable, but it keeps only one bit of a Unix file's mode, so the
   executable script still loses its `x`.
4. Nothing: `copy_file` copies permissions already, but copying the new contents over the original is not
   atomic, which is the point of the pattern.

## Decision

Option 1. It passes the admission test: it names one very common intent (keep a file's permissions when
replacing it), and the alternatives are either not portable (2), not enough (3) or not safe (4). It is
written over the std-private `path_op` as operation 17, beside `copy_file` (9), and realized by the
platform's library (`std::fs::set_permissions` of `std::fs::metadata(from).permissions()`), so it is the
same in both tools. Links are followed, as `file_info` follows them. `Err(NotFound)` when nothing is at
either path; `Err(Denied)` when the system refuses.

## Compatibility impact

None: a new function. A program of its own named `copy_permissions` keeps its own, as for any std item.

## Results

`cobfmt-cb` gives a file it rewrites the original's permissions before the rename, as `cobfmt` does;
`impl/selfhost/compare.sh` checks the mode of a rewritten executable file.

## Revisit conditions

A program that needs to set permissions it did not copy (make a file executable) would bring option 2
back, as a separate decision.
