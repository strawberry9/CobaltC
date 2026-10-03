# CHG-0082 — Syntax errors are a diagnostic: `diag.syntax-error`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, fix requested by the owner)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: CHG-0081
Affects: `spec/22` (syntax errors), `spec/registry/diagnostics.md`

## Problem / motivation

A lexical or grammatical error was printed as
`error: parse error at file:3:9: expected Semi, found Ident("x")`: no
diagnostic id, no phase, and the lexer's names for tokens. Every other
rejection before running is a registered diagnostic marked `(static)`;
this one read as though the program had failed while running, and the
conformance table could not express it (`spec/conformance.md` rows name
diagnostics).

## Decision

A syntax error is `diag.syntax-error` (static), rendered as every
diagnostic is, at its file and line, with the message — what was
expected, what was found (as the program spells it), and the column —
after the location.

## What changed

- **`spec/22` 2.25.0:** the paragraph on syntax errors.
- **`spec/16` 1.13.1:** two mentions named.
- **`spec/registry/diagnostics.md` 1.31.0:** `diag.syntax-error`.
- **`spec/conformance.md` 3.65.0:** the cases below; the file cases that
  expected `parse-error` expect `diag.syntax-error (static)`.
- **`spec/02-schema.md` 1.0.55:** §5's "in use" ranges.
- **Implementations:** the front end renders a lexical or grammatical
  error as `diag.syntax-error`; tokens are shown as spelled (`Display`
  for `Tok`); the test harnesses read it as any diagnostic.

## Compatibility classification

Clarification: the same texts are rejected, before running, as before.

## Conformance changes

**Added:** `conf.syntax-stray-name`, `conf.syntax-missing-semicolon`,
`conf.syntax-unclosed-paren`, `conf.syntax-unexpected-token`, and rows
for `conf.local-fn-rejected` and `conf.local-fn-foreign-type-rejected`
(file cases since CHG-0079); and `conf.deref-non-reference` (`* 3`,
which the checker had let reach `coby`'s evaluator and `cobc`'s code
generator, found while writing these cases: now `diag.type-mismatch`,
static). **Changed:** the 24 file cases that
expected `parse-error`.

## Revisit conditions

None.
