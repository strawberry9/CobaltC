# CHG-0445 — A diagnostic names the CobaltC form of another language's habit

Status: implemented (2026-11-14), D-0296; Update 188
Affects: `impl/src/hints.rs` (new), `impl/src/parser.rs` (`foreign_hint`, `err_at`, `Site`),
`impl/src/typecheck.rs` (`method_hint`, `function_meant`, `name_meant`, the unbound-name and field messages, a
last expression written with `;`), `impl/src/modres.rs` (`type_meant`); their twins
`impl/selfhost/front/hints.cb` (new), `parser.cb`, `typecheck.cb`, `modres.cb`; `impl/tests/foreign_forms/`

## Why

Item 2 of the plan `private/EASE-PLAN-2026-11-14.md`. A message's wording is not normative (the registry), and
most of a newcomer's errors are habits from another language: `let`, `v.len()`, `2usize`, `x as u64`,
`println!`, `i32 a[10];`. The experiment in `impl/tests/foreign_forms/` wrote one program per habit (100) and recorded
the message each got (`before.txt`): about 70 named only what the parser expected.

## What changes

- **Syntax.** The five generic messages (`expected X, found Y`, `expected an expression`, `a type`, `a name`,
  `a declaration`) end with the CobaltC form when the tokens around the error are another language's
  (`foreign_hint`, one table in each parser): macros (`println!`, `vec!`, `assert_eq!`), `as`, a literal's
  suffix, `++`, `->`, `|x|` closures, `&T`, `mut`, `self`, `let`/`:=`, `loop`, `do … while`, `switch`, `elif`,
  `and`/`or`/`not`, `?:`, `for x in`, `for (auto x : v)`, `if let`, `match x {`, `=>`, `new`, `struct P p`,
  `int a[10]`, `unsigned int`, `(T)x`, `sizeof(T)`, `i32::MAX`, `const N =`, and at file scope `impl`, `pub`,
  `use`, `class`, `namespace`, `template`, `typedef`, `void`, `func`/`def`.
- **Names.** A plain name that names nothing gets another language's meaning (`NULL`, `malloc`, `len`,
  `println`, `assert_eq`, `format`) or the single nearest name within two edits (one for four characters or
  fewer) among the locals in scope and the functions it could name; `T::name` gets the function of `T` another
  language's name means (`length` → `len`, `push_back` → `push`, `put` → `insert`, `from` → `copy`), or the
  nearest, or `--doc T` (CHG-0446); a type name gets C's or another language's meaning (`int`, `double`,
  `string`, `var`) or the nearest type.
- **Method calls.** `x.f(…)` on a type with no function `f` names the function meant; `v.len()` reaching `Vec`'s
  private field says how to call `Vec::len`; a method call on a built-in type says how functions are called.
- **A tail with `;`.** `fn f() : i32 { 1; }` adds that a block's value is its last expression, without `;`.
- The test `doc_answers_an_item_a_type_and_a_misspelling` holds every name the table offers for `T::name` to a
  function of `std`; `impl/tests/foreign_forms/run.sh --compare expected.txt` holds the 100 messages (the gate's
  `records` unit).
