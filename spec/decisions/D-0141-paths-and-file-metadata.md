# D-0141 — Paths and file metadata in `std::sys`

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §4 and §10.2: "Path functions are native over Rust's `std::path` and return new `String`s; `path_normalize` is lexical")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0054 (`File`), D-0061 (directories, the environment), D-0134 (`std`'s naming conventions; text parameters as `StringView`), D-0136 (`std::sys`)
Affects: `spec/21` §0, §2e′ `rule.stdlib.fs` (4.7.0), `spec/conformance.md` (3.131.0), the conformance runners (the `platform:` header), the guide §21, `impl/std/sys.cb`, `impl/std/std.cb`, `impl/src/fileio.rs`, `impl/src/interp.rs`, `impl/cbrt`, `impl/cobc/src/lower.rs`

## Problem

A deployed program builds paths (a log directory and a file name in
it), takes them apart (a file's extension, its directory), and asks
about files (how big, how old, read-only?); it copies files, makes a
directory with its parents, removes a tree, and changes its working
directory. `std` had `make_dir`, `path_kind` and the rest of D-0061 but
none of these, so programs glued paths with `sprintf("%s/%s", …)` — wrong
on Windows, and wrong when the base already ends in `/` — and wrote
recursive removal by hand.

## Candidate mechanisms

1. **Path functions written in CobaltC over the text.** A path's grammar
   is the platform's: on Windows `C:\`, `C:foo`, `\\server\share\` and
   both separators. CobaltC code would have to be told the platform and
   carry each platform's rules; easy to get wrong and impossible to keep
   identical to what the system does.
2. **A `Path` type.** Every path in `std` is a `StringView` (D-0134); a
   second type would need conversions everywhere and buys nothing a
   program can see. Not adopted.
3. **Functions over `StringView`s returning new `String`s, realized by
   the platform's own path library (Rust's `std::path` and `std::fs`, in
   both tools).** Selected.

## Selected design

`spec/21` §2e′, `rule.stdlib.fs`: `path_join`, `path_parent`,
`path_file_name`, `path_stem`, `path_extension`, `path_is_absolute`,
`path_normalize`, `path_canonical`; `FileInfo`, `file_info`,
`copy_file`, `make_dir_all`, `remove_dir_all`, `set_current_dir`,
`temp_dir`, `home_dir`.

- **The path grammar is the platform's**, and the specification says so:
  the same text may have different parts on Unix and on Windows, as with
  the system's own functions. A case printing Unix results carries
  `// platform: unix` (below).
- **The part functions and `path_normalize` never touch the file
  system.** `path_normalize` is lexical: `.` removed, `x/..` resolved
  where `x` is a name, a leading `..` kept, `/..` is `/`, nothing left is
  `.`. It can change what a path names when a part is a symbolic link;
  `path_canonical` asks the system and follows links, but the path must
  exist.
- **Absent parts are `None`**: no parent for a root or the empty path, no
  file name for `..`, `/` or ``; a single relative name's parent is the
  empty path (the system's answer; joining it with a name gives the
  name). `.bashrc` has the stem `.bashrc` and no extension.
- **`FileInfo { kind, len, modified_unix, readonly }`**, following links
  as `path_kind` does; `kind` is a `PathKind`, which repeats a file's
  length; `len` is kept so a program need not match to read it.
  `modified_unix` is seconds since 1970, rounded down, the same unit as
  `unix_seconds`.
- `copy_file` returns the bytes copied; `make_dir_all` is `Ok` when the
  directory exists; `set_current_dir` changes the working directory of
  the whole program (every thread). `temp_dir` and `home_dir` replace
  bytes that are not UTF-8 with U+FFFD, so their signatures stay a plain
  `String` and `Option<String>`, as the plan gave them.
- **One new private primitive, `path_op`**, of the shape the process and
  networking primitives share (`op, h, a, an, b, bn, out, cap`): two byte
  strings in (`path_join` and `copy_file` need two), a number back, and
  up to `cap` bytes of an answer, asked again with room for all of it
  when longer (`byte_answer`, `fs_answer`'s protocol). The plan suggested
  new `fs_query`/`fs_op` codes; `fs_query` takes one string and `fs_op`
  answers no bytes, so `path_join` fitted neither, and one shape for all
  three new primitives lets `coby`, `cbrt` and `cobc` route them with one
  piece of code each. `fs_op` and `fs_query` are unchanged.
- **The `platform:` case header** (`impl/conformance/README.md`): `//
  platform: unix` runs a case only on Unix systems; every runner skips it
  elsewhere, saying so. Cases run on Linux today; the header records
  which cases could not run on Windows as written.

## Compatibility impact

Additive: new names in `std` (`FileInfo` and fifteen functions),
shadowed by a program's own items of those names.

## Revisit conditions

- A non-following `symlink_info`, `read_link`, and making links, if
  programs ask.
- Changing a file's permissions or times, if programs ask.
- `path_relative_to`, if programs measured to compute relative paths by
  hand.
