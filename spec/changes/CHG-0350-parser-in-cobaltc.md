# CHG-0350 — coby's parser written in CobaltC

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0021, CHG-0346, CHG-0347
Affects: `impl/src/astdump.rs` (new), `impl/src/parser.rs` (`prelude_state`, `prelude_skeleton`),
`impl/src/check.rs` (`run_dump`, `run_prelude_summary`), `impl/src/main.rs`; `impl/selfhost/front/` (new),
`impl/selfhost/cbparse/` (new), `impl/selfhost/cobfmt/`, `impl/selfhost/compare_parser.sh`,
`impl/selfhost/mutate.py`, `impl/selfhost/build.sh`

## What changed

No rule changed. The second self-hosting pilot (the first is CHG-0346's formatter):

- `impl/selfhost/front/` is coby's front end up to the parse, written in CobaltC: the lexer (both of coby's
  forms: with trivia for the formatter, with decoded values and adjacent strings joined for the parser),
  the abstract syntax, the parser, function for function with coby's (the same grammar, rewrites, hoisting,
  pre-scans and messages), the loader of file-backed modules, and the parse check `coby::check::syntax`
  answers. About 7,000 lines. A CobaltC variant carries one payload, so each Rust variant with several
  parts carries a boxed struct of them.
- What a program's parse takes from the prelude (all of `std`, 28,000 lines) is generated rather than
  parsed each time: `coby --prelude-summary` writes `front/prelude.cb`, the prelude's skeleton (the
  tokens the parser's pre-scans read: braces, `module NAME {`, `const` declarations, `struct`/`enum`/
  `bitstruct` heads), which the generator checks gives the full prelude's tables, and the counters the
  prelude's hidden names leave (`__compound`n, `__pattern`n, `$c`n, `N$`k, `__unittest_`n). The parser
  pre-scans the skeleton in front of the program's tokens and continues the counters.
- `coby --dump-ast [--origin PATH] (FILE | -)` prints the program's own part of the parse in a canonical
  text (`astdump.rs`: one S-expression per node, every expression's `line:col`, strings in hex, floats as
  bits), or the error the parse check gives; `cbparse --dump-ast` prints the same from the CobaltC parser,
  and `cbparse --check-syntax` answers as `cobc --check-syntax` does.
- `compare_parser.sh` compares the two on every tracked `.cb` file (the dump, and the parse check against
  `cobc --check-syntax`) and on seven mutated copies of each (`mutate.py`: a line deleted, a character
  deleted, `(`, `}` or `;` inserted, at positions from the file's hash), where nearly every copy is an
  error, so the messages and their positions are compared; and that `prelude.cb` is current.
- `cobfmt-cb` decides whether a text parses with this parser, in the process, and no longer runs
  `cobc --check-syntax` (which stays, for other tools).

## What changed for programs

Nothing.
