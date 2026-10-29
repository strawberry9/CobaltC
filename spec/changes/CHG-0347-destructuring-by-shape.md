# CHG-0347 — A destructuring is recognised by its shape

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0044, D-0104
Affects: `spec/conformance.md` (two rows); `impl/src/parser.rs`, `impl/src/typecheck.rs`

## What changed

No rule changed. spec/22's disambiguation (5) says that a brace body opening with a bare identifier
followed by `,` or `}` is a destructuring, and its grammar writes the statement `path '{' dfield … '}' '='
expr`. The parser took the statement as a destructuring only when its name was a struct or enum type the
text it was given declared, and only for a single name. A module file that takes apart a struct declared in
a sibling module file therefore did not parse on its own, which is how `cobfmt` parses a file before it
formats it and how `cobc --check-syntax` (CHG-0346) answers; the program as a whole was accepted. Found by
`cobfmt` written in CobaltC, whose layout module destructured its lexer's result. The parser now takes the
statement by its shape, with a qualified path too (`shapes::Pt { x, y } = p;`, which the grammar always
allowed). A name that is no struct is rejected by the type checker: `diag.unbound-name` when nothing of
that name is a struct or an enum, `diag.type-mismatch` for an enum (taken apart by `match`). Before, both
were syntax errors. New rows: `conf.module-file-destructure-sibling`, `conf.destructure-not-a-struct-rejected`.

## What changed for programs

A program that `cobfmt` refused to format now formats; a program destructuring by a qualified path now
compiles; `q { x } = q;` with `q` no struct is a name error instead of a syntax error.
