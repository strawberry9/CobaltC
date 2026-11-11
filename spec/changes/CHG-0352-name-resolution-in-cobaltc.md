# CHG-0352 — coby's name resolution written in CobaltC

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: CHG-0350, D-0116, D-0118, D-0136
Affects: `impl/src/parser.rs` (`prelude_declarations`), `impl/src/check.rs` (`run_dump`,
`run_prelude_summary`, `describe_diagnostic`), `impl/src/lib.rs` (`resolve_front`), `impl/src/modres.rs`
(the order of three loops), `impl/src/main.rs`; `impl/selfhost/front/derive.cb`,
`impl/selfhost/front/modres.cb` (new), `impl/selfhost/front/check.cb`, `impl/selfhost/front/parser.cb`,
`impl/selfhost/front/prelude.cb`, `impl/selfhost/cbparse/main.cb`, `impl/selfhost/cobfmt/main.cb`,
`impl/selfhost/compare_parser.sh`

## What changed

No rule changed. The first layer of the static pass written in CobaltC, the step after the parse (the
owner's plan of 2026-10-20: the static pass in layers, each compared with `coby`; then a code generator for
the subset the compiler itself uses; then the fixed point; `coby` and `cbrt` stay in Rust, and the Rust
tools stay the reference until then):

- `impl/selfhost/front/derive.cb` and `modres.cb` are `derive.rs` and `modres.rs` (with `lib.rs`'s
  `[Item-Duplicate]` check), function for function: the derived `T::clone`, `Name::bits` and
  `Name::from_bits` (D-0116, D-0118); the module tree, the re-exports (D-0136) and the imports validated
  once; every reference in a body, a field, a payload or a signature resolved by `[Resolve-Unqualified]`
  and `[Resolve-Qualified]` and rewritten to its qualified key, with the rewrites this pass makes (a
  `const`'s use a call, `printf` and its kin through `$fmt`, `assert`, `checked_neg`, a pattern's
  constant or variant). `--warn-shadow`'s notes are not made.
- Resolution needs all of `std`'s items; parsing all of `std` takes the CobaltC parser 10.5 s (`coby`
  0.54 s). `coby --prelude-summary` now also writes `std`'s declarations into `front/prelude.cb`
  (`prelude_declarations`: the prelude's text with every function's body emptied and every `unittest` left
  out, each line where it was, checked to give the prelude's items), which parse in 1.4 s; and the
  intrinsics' names and `std::math`'s floating-point functions, which `modres` reads.
- `coby --dump-resolved [--origin PATH] (FILE | -)` prints the program's own part once `[Item-Duplicate]`,
  the derived functions and name resolution have run (`lib::resolve_front`, now the one place the
  pipeline runs them), in `--dump-ast`'s text, or the error, `PLACE: ID: MESSAGE`; an unloadable module
  file is the error. `cbparse --dump-resolved` prints the same from the CobaltC front end, and
  `compare_parser.sh`'s new mode `resolved` compares the two on every tracked file.
- Three loops in `modres.rs` that read a hash map now read it in sorted order, so which of two errors is
  reported, and the order of the names in an ambiguity's message, are the same for any hash function:
  the imports' validation (by module), a module's items brought in by `export import` (by name), and the
  enums an ambiguous variant could belong to.

## What changed for programs

Nothing, except that the order of names in a "variant of more than one enum in scope" message is now
sorted.

## Found on the way

The compiled CobaltC front end is about 20 times slower than Rust's on large inputs (`std` parsed: 10.5 s
against 0.54 s; a resolved dump of a small program: 2.6 s against 0.37 s, most of it the declarations'
parse). The profile is flat: two thirds of the time is the runtime's object tracking (`check_access`,
`mint_in`, `retire_token`, `cb_stmt_pop`, …) spread over every operation of a program that builds trees;
no one cause like `CHG-0349`. Whether a compiler written in CobaltC is usable depends on it; left for the
owner as a separate line of work.
