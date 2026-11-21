# CHG-0149 — Array lengths as constant expressions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0126)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0126
Affects: `spec/22` (2.42.0), `spec/17` (2.11.0), `spec/16` (1.20.0), the registry (1.42.0), `spec/conformance.md`, the guide §16, `impl/src/parser.rs`

## What changed

- **`spec/22`:** `length ::= const-expr`; **`spec/17`** §1a and **`spec/16`** `type.array` say so.
- **Parser:** `fold_const_tokens`; the constant prescan folds initializers to a fixpoint; `array_length` reads an expression ending at `>`, `>>`, `]`, `;` or `,` outside parentheses.
- **Rows:** `conf.length-constant-expression`, `conf.length-later-constant`, `conf.length-module-expression`, `conf.length-not-constant-rejected`, `conf.length-negative-rejected`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
