# CHG-0333 — `--warn-shadow`: a note for each item whose name hides an imported one; two alike-printing types named apart

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, the owner: "do A plus D" of the options offered)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0024 (`import std;`), D-0136 (re-exports), `spec/17` `[Resolve-Unqualified]`
Affects: `impl/src/modres.rs`, `impl/src/lib.rs`, `impl/src/main.rs`, `impl/cobc/src/main.rs` (the flag);
`impl/src/ast.rs`, `impl/src/typecheck.rs` (the message); `spec/conformance.md` (3.243.0); `spec/02` §5 (1.0.114);
`htmlguide/generator/content_intro.py`

## What changed

- **`--warn-shadow`, on both tools.** `[Resolve-Unqualified]`'s first clause makes a module's own item the
  one a bare name denotes, before anything an `import` brings in: a program's `struct Vec`, `struct Option`
  or `fn min` beside `import std;` hides `std`'s, silently and by design (`spec/17` §1). With
  `--warn-shadow` (before the file; `cobc` takes it among its options and after `--check` too) each tool
  writes, on standard error, one note per such item — its kind, its name, the module declaring it, the
  qualified name it hides, and its file and line (`note: struct \`Vec\` of this program hides
  \`std::Vec\`; the program's own is used`) — and then does what it would have done. The exit status is
  unchanged; the notes are not diagnostics and have no registry entry. The check is the resolver's: after
  every `import` target is resolved, each module's own items (not its re-export aliases, not its
  submodules, and not an associated function of its own type, which the type's note covers) are compared
  with the exported items of the modules it imports and with the targets of its `import p::name`s. Off by
  default: nothing a program prints changes, and the conformance runners pass no flag.
- **Two types that print alike are named apart.** A message that shows two types — "`w` is declared
  `X`, but its initializer is `Y`", "`f` returns `X`, but its body gives `Y`" — printed `Vec<i64>` twice
  when one was the program's `Vec` and the other `std::Vec<i64>` (the `std::` prefix is dropped for
  readability). When the two differ but display alike, both are now shown with their modules
  (`Type::shown_apart`): `Vec<i64>` against `std::Vec<i64>`. Case `conf.shadowed-std-type-named`.

## What changed for programs

Nothing, without the flag: no program's acceptance, output or exit status changes; one message's spelling
of a type does, only when it would otherwise have shown the same spelling for two types. With the flag, a
note per hidden name on standard error.
