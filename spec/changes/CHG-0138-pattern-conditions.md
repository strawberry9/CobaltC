# CHG-0138 — `if (pattern = e)` and `while (pattern = e)`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0115)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0115
Affects: `spec/22` §2 (2.39.0), `spec/14` §3–§4 (1.19.0), `spec/conformance.md` (3.111.0), the guide §14 and §22, `impl/src/parser.rs`

## What changed

- **`spec/22`:** `cond ::= expr | pattern '=' expr` in `if-expr`,
  `while` and `block-like`; disambiguation (7).
- **`spec/14`:** `[If-Pattern]`, `[While-Pattern]`, each `≡` the `match`
  it stands for, with the notes on binders, consumption and the loop's
  exit on a `Result`.
- **Parser (shared):** `parse_pattern` factored out of `match`;
  `try_condition_pattern` reads a pattern and a single `=` or restores
  its position; `if` and `while` build the `match` (and `while (true)`)
  of the rules. No checker, interpreter, compiler or runtime change.
- **Rows:** `conf.if-pattern-some`, `conf.if-pattern-else-value`,
  `conf.if-pattern-nested`, `conf.if-pattern-literal`,
  `conf.if-pattern-binder-scope-rejected`,
  `conf.if-assignment-still-rejected`, `conf.while-pattern-pop-moves`,
  `conf.while-pattern-break-continue`,
  `conf.if-pattern-qualified-variant`.
- **Guide:** §14's `if` and `while` sections and the §22 reference.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a `pattern = e` condition was a syntax-level assignment to a
non-place before, rejected.
