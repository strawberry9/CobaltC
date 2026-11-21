# CHG-0451 — `--doc` finds by word, and shows the language's own functions

Status: implemented (2026-11-14), D-0296; Update 189
Affects: `impl/std/intrinsics.md` (new), `impl/src/doc.rs` (`intrinsic_items`, `entries`, `LANGUAGE`, `find`,
`lookup`, `run`), `impl/src/main.rs`, `impl/cobc/src/main.rs` (usage and help), `impl/tools/cobdoc.py`
(`intrinsic_names`, `intrinsics`, `--check`); their twins `impl/selfhost/cbc/doc.cb` (`find`, `lookup`, `run`,
`Near`), `impl/selfhost/cbc/driver.cb`, `impl/selfhost/front/prelude.cb` (`PRELUDE_DOCS`, regenerated);
`impl/tools/card.py`

## Why

Item 2 of `private/EASE-PLAN-2-2026-11-14.md`. Of the four programs written cold, eleven mistakes were library
names guessed wrong. `--doc` answered only an exact name, so `--doc read_to_string` could not lead to
`read_all`, and the language's own functions (`widen`, `spawn`, `join`, `slice_len`, `drop`, `clone`, about
seventy) were "nothing named … in `std`": they are intrinsics, defined by `spec/21` §0, with no item in `std`.

## What changes

- `impl/std/intrinsics.md` documents each intrinsic a program may call (71: `typecheck.rs`'s
  `INTRINSIC_NAMES` less those only `std` may call, the hidden `$` ones, the `std::` items `builtins.md`
  documents and `stdout_write`), in `builtins.md`'s form, from `spec/21` §0's rows; `cobdoc.py --check`
  fails when the two lists differ. `--doc widen` shows one, `--doc language` lists them, and `--doc` alone
  ends with a line naming them. They are not in `cobdoc.py`'s JSON (no items of `std`).
- `--doc --find WORD…`: the items whose name, signature or summary holds every word, case ignored, those whose
  name holds one first, then a count; exit status 1 when none does.
- A name that names nothing lists, after the nearest name, the items with a part of it in theirs (split at
  `_` and `::`, parts of three letters or more), ranked so that an earlier part weighs more (`read` before
  `string` for `read_to_string`), at most eight, then the `--find` that gives the rest.

## Checked

`doc_matches_cobdoc` (the intrinsics' entries against `cobdoc.intrinsics()`), `cli.rs`
(`doc_finds_by_word_and_shows_the_language_s_functions`), and `cbc --doc` against `cobc --doc` for every
record's name, no name, `language` and ten `--find` queries (`impl/selfhost/compare_doc.sh`, run by the gate's
`selfhost` unit).
