# CHG-0282 — The built-in items documented; the ordered-view idiom in the guide; speccheck names fixtures

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; the owner: "give the `sorted_order` idiom a paragraph in the existing guide;
document the built-in natives; make `speccheck.py` name fixtures in its summary")
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0227, D-0229, D-0234, CHG-0276
Affects: `impl/std/builtins.md` (new); `impl/tools/cobdoc.py`; `impl/tools/speccheck.py`;
`htmlguide/generator/std_reference.py`, `std_inventory.py`, `content_c.py`; `impl/STATUS.md`; `spec/02` §5 (1.0.88)

## What changed

- **The built-in items are documented.** Twenty-six exported items of `std` have no CobaltC body: both tools
  build them in (`printf`, `sprintf`, `eprintf`, `String::appendf`, `File::printf`, `TcpStream::printf`,
  `assert`, `exit`, `Result::map_err`, and the seventeen float functions of `std::math`). CHG-0276 documented
  every item with a body and left these as the one gap. Their documentation is now `impl/std/builtins.md`, by
  hand, one entry per item (module, signature, text), in the words of `spec/21` §0's table, §2f and §3d and
  `spec/18` §1a. `impl/tools/cobdoc.py` reads it beside the `///` documentation: its JSON lists them with kind
  `built in fn`, and `--check` (battery stage 0) fails on a built-in item the tools know (`impl/src/modres.rs`'s
  `native(…)` registrations and `typecheck.rs`'s float lists) that the file does not document, and on an entry
  that names no such item. The std reference (section R) shows them as *built in*, and §21's inventory links
  every exported name, not only those with a body.
- **The ordered-view idiom.** D-0227 declined an ordered map and set and named the idiom; the plan of
  2026-10-12 sent it to a cookbook that D-0234 declined. The guide's `HashMap` section now has a paragraph and
  a checked example: copy the keys into a `Vec`, `Vec::sort` it, and look each one up again (or walk the sorted
  keys beside the map); `Vec::binary_search` on the sorted keys for membership.
- **`speccheck.py` names fixtures.** Its summary said "N case files named by no row" and counted fixtures
  (`// CONFORMANCE-FIXTURE` on the first line, files another case loads) with the rest, while
  `std_coverage.py` reported zero. It now says "N fixture files, M other case files named by no row", and an
  *other* such file is a warning (it fails `std_coverage.py` in stage 1).
- `impl/STATUS.md`'s Known gaps no longer lists the undocumented built-ins. `spec/02` §5's range: CHG-0282.

No rule, no `std` item and no conformance row changed.

## What changed for programs

Nothing.
