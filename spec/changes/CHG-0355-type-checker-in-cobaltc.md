# CHG-0355 — coby's type checker written in CobaltC

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: CHG-0352, CHG-0354, D-0052, D-0162
Affects: `impl/src/parser.rs` (`prelude_bodies`, `prelude_declarations`), `impl/src/check.rs`
(`--prelude-summary`, `--dump-checked`), `impl/src/lib.rs`, `impl/src/main.rs`;
`impl/selfhost/front/typecheck.cb`, `items.cb`, `each.cb`, `fmtcheck.cb`, `consteval.cb` (new),
`impl/selfhost/front/views.cb`, `check.cb`, `parser.cb`, `ast.cb`, `prelude.cb`;
`impl/selfhost/cbparse/main.cb`, `impl/selfhost/cobfmt/main.cb`, `impl/selfhost/compare_parser.sh`

## What changed

No rule changed. The third layer of the static pass written in CobaltC: the type checker, the last
part of the front end.

- `impl/selfhost/front/typecheck.cb` is `typecheck.rs`, function for function in file order: the
  worklist of functions and instantiations, `rule.control.flow-analysis` (validity, initialisation and
  derivation facts, joined at branches and iterated in loops), borrows and their conflicts, the escape and
  elision rules, calls (user functions, generics with their inference, intrinsics, variants, closures),
  `match` exhaustiveness and reachability, structs, slices, views, `simd`, atomics and bit fields, and the
  rewrites D-0162, D-0164, D-0218 and D-0225 ask for. Rust's facts keyed by a node's address are keyed
  by an expression number (`Expr.id`), given in a pass before checking.
- `items.cb` builds the item tables. `std`'s function bodies are not parsed with the program: the
  prelude summary carries each one's text, and a body is parsed, resolved and folded the first time the
  checker reaches it (`load_body`). `coby --prelude-summary` checks that each body parsed alone is the
  body parsed with all of `std`.
- `each.cb` is `each.rs` (`foreach`'s expansion), `fmtcheck.cb` the part of `fmt.rs` that checks a
  format against its arguments, and `consteval.cb` computes `static_assert`s, which `coby` computes with
  its evaluator: literals, constants, their fields and elements, `sizeof`, `alignof`, `min_value`,
  `max_value`, `widen`, `narrow` and `spec/06`'s checked arithmetic, and a failed assertion's message.
- `coby --dump-checked` and `cbparse --dump-checked` print `ok` when the whole static pass accepts a
  program, or its diagnostic; `compare_parser.sh`'s mode `checked` compares them, and all 1,649 tracked
  files give the same verdict from both. The compiled checker is slow: cbparse's own 40,000 lines take it
  175 s, `coby` 3 s.

## What changed for programs

Nothing. The port found four defects in the Rust tools, each its own record: CHG-0356 (a `cobc`
internal error), CHG-0357 (the checker accepting a large program partly unchecked), CHG-0358 (a false
`cobc` fault), CHG-0359 (`coby` losing a plain clone's type).
