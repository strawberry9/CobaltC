# D-0297 — Formats written in place, and text sorted by the runtime

Status: ACCEPTED (2026-10-30; the owner, on the bench's findings: "do all three as Update 152 after the push")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0038, D-0062, D-0100, D-0199, D-0296
Affects: `impl/cobc/src/lower/intrinsics.rs`, `impl/cobc/src/lower/native_vec.rs`, `impl/cobc/src/lower/passes.rs`,
`impl/cobc/src/main.rs`; `impl/selfhost/back/gen.cb`, `impl/selfhost/back/native_vec.cb`,
`impl/selfhost/back/passes.cb`, `impl/selfhost/cbc/driver.cb`; `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`;
`spec/conformance.md`; `bench/`; `CHG-0392`

## Problem

The bench rerun of 2026-10-30 (Update 151) left two programs behind their Rust twins, and the profiles named the
causes:

- **Formatting.** `hashcount` builds 1,000,000 keys with `sprintf("w%v", n)`; `cb_format` was 41% of its cycles,
  the whole of its gap to Rust. Every `$fmt` (the text of `printf`, `sprintf`, `String::appendf`) went through the
  runtime's general formatter: three thread-local cells reached per call, the format's bytes compared against a
  cache of parsed formats, every argument copied into the runtime's own list, the text rendered into a thread
  buffer — and, for `sprintf`, copied again into the `String`. `printf("text\n")` with no argument took the same
  road.
- **Sorting text.** `strings` sorts 100,000 `String`s of 5,000 distinct values; the native `Vec::sort` for text
  (D-0062's prelude form made native) was a bottom-up merge of fixed widths, one `memcmp` per comparison, and took
  about 70 M cycles where Rust's `sort` took 39 M: the library's stable sort takes the runs and the repeated keys
  that a merge of fixed widths does not.

## Decision

1. **A literal format of the common specifiers is written in place by the compiler.** When `$fmt`'s format is a
   string literal and each of its specifiers is `%v`, `%d`, `%i` or `%u` of an integer of at most 64 bits, `%x` or
   `%X` of one, any of these with a width and the `-` and `0` flags, or `%v` or `%s` of text (a `str`, a `bool`, a
   `StringView`, a `String` passed by reference) with no width, and `%%`, `cobc` and `cbc` write the text with
   straight-line C: the format's own bytes copied from its literal, each number written by an inline renderer of
   the runtime's header (`cb_fmt_u64`, `cb_fmt_i64`, `cb_fmt_hex`, padded by `cb_fmt_pad`), each text's bytes
   copied — into an array of the frame when the length has a bound of at most 256 bytes, else into the thread's
   space for exactly that length (`cb_fmt_space`, apart from `cb_format`'s buffer, which an argument may still
   point into). A format with no specifier is its literal. Every other format — floats, enums, 128-bit integers,
   `+`, ` `, `#`, a precision, a `*`, a width on text — goes to `cb_format` as before. The bytes are the
   formatter's (`impl/src/fmt.rs`): text is well-formed UTF-8 by its type (`spec/21` `type.str`), so its bytes are its
   text, and a width counts characters only where the text is a number's, which is ASCII. Each argument is
   evaluated and checked exactly as before; only the rendering moved.

2. **`cb_format` keeps one thread-local state** (its buffer, its cache, its arguments) instead of three, and looks
   the cache up most recent first.

3. **`Vec::sort` of `String` and `str` holding no reference sorts in the runtime** (`cb_sort_text`, the library's
   stable sort by bytes), after the elements' objects are ended and their cells released as before. A stable
   sort's result is determined, so the order is the merge's.

4. **`cobc --run` and `cbc --run` remove their temporary directory** when the build stops short of a run for a
   reason other than the C compiler's failure (a C function no library has, a library the linker cannot find, a
   foreign program with no `COBALTC_RUNNER`). The C compiler's failure keeps the C file, as the message says; 198
   such directories had collected in `/tmp` on this machine.

Both code generators make the change, byte for byte (D-0296: both toolchains in the same Update).

## Why these, and not the alternatives

- **Formatting in the generated C rather than a faster runtime formatter**: the runtime cannot know a format is a
  literal (one built at run time may reuse an address with other text, so its cache compares bytes), nor the
  arguments' types; the compiler knows both, and the common formats are short. A runtime path writing straight
  into the `String` would still parse, look up and copy. The subset is the one the formatter's own fast path
  (`fast` in `fmt.rs`) and the corpus use; widening it to floats would put the shortest-round-trip algorithm into
  every program's C.
- **The library's sort rather than a better merge in C**: it is the algorithm Rust's `sort` is, already linked into
  the runtime, and tuned for runs and repeated keys; a C copy would be a second implementation to keep right.

## Consequences

- `hashcount` and `strings` (bench/): see `CHG-0392` and `bench/RESULTS.md`.
- A program's C holds the in-place forms; `cb_format` is called only for the other specifiers.
- `conf.format-in-place` covers the in-place forms against the formatter's text.
