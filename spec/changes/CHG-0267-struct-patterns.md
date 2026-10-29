# CHG-0267 — Struct patterns; destructuring checks its value's type statically

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; D-0224)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0224
Affects: `spec/22` (2.47.0), `spec/16` (1.27.0), `spec/conformance.md`; `impl/src/parser.rs`, `impl/src/typecheck.rs`

## What changed

- `spec/22`: `pattern` gains the struct pattern; §5's restriction 2 says what it is and is not.
- `spec/16` §4: struct patterns (the sentence saying literal and struct patterns are not provided was out of
  date for literals since D-0057).
- Parser: a struct pattern binds a hidden name; the arm, or the `if`/`while` body, begins with its
  destructuring.
- Checker: `[T-Let-Destructure]`'s premise that the value is the struct named is checked statically (a
  reference, or another type, is `diag.type-mismatch`, with a hint for a struct pattern); an unknown field is
  named in the message.
- Cases: `conf.struct-pattern`, `conf.struct-pattern-missing-field`, `conf.struct-pattern-unknown-field`,
  `conf.struct-pattern-through-ref`, `conf.struct-pattern-consumes`, `conf.destructure-reference`.

## What changed for programs

An extension: struct patterns. And a program that destructured a reference, which `coby` faulted on when the
statement ran and `cobc` refused with an internal error, is now rejected statically.
