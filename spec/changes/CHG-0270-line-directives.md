# CHG-0270 — `#line` directives and `-g` in `cobc`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under the forward plan of 2026-10-12, WI-11)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0023
Affects: `impl/cobc/src/lower.rs`, `impl/cobc/src/cache.rs`, `impl/cobc/src/main.rs`,
`impl/cobc/tests/driver.rs`; `impl/conformance/20-trust-boundaries/raw_place_field_without_value_ok.cb`,
`spec/conformance.md`; the guide's "Reading the C that `cobc` generates"

## Why

A memory error in a compiled program's `unsafe` code, a sanitizer's report, a debugger's position, and the
C compiler's own messages all named lines of the generated C. The C was correct but they said nothing about
the program, except through the `cb_at(file, line)` calls a reader had to find by hand.

## What changed

- **Directives.** Each line of C made from one of the program's own statements is preceded by
  `#line N "FILE.cb"`. N is the statement's line: its `cb_at` gives it, or, when the location was dropped
  as unread (`drop_dead_locations`), the comment marker that pass now leaves in its place.

  Where a function's code ends, a directive returns to the C file's own numbering. Code made for `std` has
  no directives. The directives are added after every pass that reads a function's text line by line.
  The functions' size limits (`FROZEN_LINES`, `DUAL_LINES`) do not count the markers. So the C, apart
  from the directive lines, is the same with and without them. This was checked over every program in
  `impl/conformance`, `showcase`, `bench` and `impl/cobaltc_examples`: 748 programs, 275,246 directives,
  no difference.

  Cached `std` units have no directives, so the object cache is unaffected.
- **`--no-line-directives`** leaves them out.
- **`-g`** compiles with debug information (`cc -g`) and keeps it in the executable: on Linux the link
  leaves out `--strip-debug`, unless `-s` strips everything. The object cache keys `-g` objects apart.
  `-O0 -g` is the debugging combination. A sanitizer build reports a heap overflow in `unsafe` code at
  its `.cb` line and column, for example `over.cb:18:25`.
- **A raw place's struct is defined.** `rawptr<S>` names `struct S` by its forward declaration alone. A
  program that only ever reached `S` through `(*p).f`, and never made a value of `S`, compiled to C
  that GCC and Clang rejected ("incomplete type"). `coby` ran such a program. The raw place `*p` now has
  its struct defined. Found while writing this change's sanitizer test. The case
  `conf.raw-place-field-without-value` pins it.
- Tests (`impl/cobc/tests/driver.rs`):
  - `line_directives_name_the_program_lines`: under gcc and clang, every statement is named, the C's
    numbering resumes correctly, and the C is otherwise unchanged;
  - `sanitizer_reports_the_program_line`: AddressSanitizer names `over.cb:18`. It is skipped where clang
    has no sanitizer.

## Timing

`showcase/tier3/tinyos.cb` compiles in 3.1 s with or without the directives (three runs each). The
executable's code is the same, so nothing else is re-timed.

## What changed for programs

Nothing in their meaning. One program that `cobc` failed to compile now compiles.
