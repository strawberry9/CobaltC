# CHG-0354 — coby's constant folding and literal views written in CobaltC

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: CHG-0352, D-0036, D-0068, D-0134, D-0149
Affects: `impl/src/check.rs` (`DumpStage`, `run_dump`), `impl/src/main.rs`; `impl/selfhost/front/consts.cb`,
`impl/selfhost/front/views.cb` (new), `impl/selfhost/front/ast.cb` (`is_literal_expr`, `type_eq`),
`impl/selfhost/front/parser.cb`, `impl/selfhost/front/modres.cb`, `impl/selfhost/front/check.cb`,
`impl/selfhost/cbparse/main.cb`, `impl/selfhost/cobfmt/main.cb`, `impl/selfhost/compare_parser.sh`

## What changed

No rule changed. The second layer of the static pass written in CobaltC, the two passes between name
resolution and the type checker:

- `impl/selfhost/front/consts.cb` is `consts.rs`, function for function: `const auto`'s types (D-0068), the
  cycle check (`[Const-Cycle]`), the folding of integer, float, `bool` and `str` constants with `spec/06`'s
  checked arithmetic (the same diagnostics, at the constant), and every use rewritten to a literal of the
  constant's type, a borrowed constant (`&K`) excepted.
- `impl/selfhost/front/views.cb` is `views.rs`'s `process`: a text literal where a declared type asks for a
  `StringView` or a `String` (D-0134, D-0149, D-0155). The checker's own rewrites (`views.rs`'s
  `rewrite_literals_as_string`) come with the type checker.
- `is_literal_expr` moved from the CobaltC parser into `ast.cb`, beside a new `type_eq` (Rust's derived
  `Type == Type`), for the passes that follow.
- `coby --dump-folded` and `cbparse --dump-folded` print the program's own part once these two passes have
  run too, which is what the type checker reads; `compare_parser.sh`'s mode `folded` compares them (on by
  default), and all 1,648 tracked files give the same text from both.
- The CobaltC name resolution keeps, per module, its enums by variant name, and a count of the enums with
  each variant name, where `modres.rs` reads every item: a bare variant's lookup had been half of a resolved
  dump's time (2.7 s to 1.35 s). The results are the same.

## What changed for programs

Nothing.
