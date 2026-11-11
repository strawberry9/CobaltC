# CHG-0446 — `--doc NAME`: the standard library's documentation from the tools

Status: implemented (2026-11-14), D-0296; Update 188
Affects: `impl/src/doc.rs` (new), `impl/src/prelude.rs` (`std_root`, `std_file`), `impl/src/main.rs`,
`impl/cobc/src/main.rs`, `impl/src/check.rs` (`run_prelude_summary`: `PRELUDE_DOCS`);
`impl/selfhost/cbc/doc.cb` (new), `impl/selfhost/cbc/driver.cb`, `impl/selfhost/cbc/main.cb`,
`impl/selfhost/front/prelude.cb` (regenerated); `impl/tests/doc_matches_cobdoc.rs` (new)

## Why

Item 3 of the plan. `impl/tools/cobdoc.py` reads `std`'s `///` documentation (D-0229) for the guide, as 513 KB of
JSON; looking one function up meant reading `impl/std`.

## What changes

`coby --doc`, `cobc --doc` and `cbc --doc` take an item (`Vec::push`, `std::collections::Vec::push`, `printf`),
a type (its entry, then each of its functions as signature and summary), a module (`std::text` or `text`: its
items), or nothing (the modules and how many items each exports). Nothing found: exit status 1 and the nearest
name. `doc.rs` reads `std`'s files, compiled into the tools, as `cobdoc.py` does (the test
`doc_matches_cobdoc` compares every item's module, kind, signature, documentation, requirements and examples);
each item becomes a record laid out once. cbc has no `std` source, so `coby --prelude-summary` writes the
records into `prelude.cb` (`PRELUDE_DOCS`, U+001F between fields, U+001E after each record), which
`compare_parser.sh` keeps current, and `doc.cb` is the twin of `doc::lookup`. The three tools printed the same
for every one of the 836 names of `std`'s items and modules.
