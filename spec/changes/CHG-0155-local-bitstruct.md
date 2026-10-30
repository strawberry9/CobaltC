# CHG-0155 — A `bitstruct` declared in a function

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-02; D-0132)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0132
Affects: `spec/16` (1.22.0), `spec/17` (2.13.0), `spec/22` (2.44.0), `spec/conformance.md`, the guide §16, `impl/src/parser.rs`, `impl/src/derive.rs`

## What changed

- **`spec/22` §2:** `statement` gains `bitstruct-decl`, a local type.
- **`spec/17` §1c `rule.module.local-type`:** a `bitstruct` is a local type, as a `struct` or an `enum` is.
- **`spec/16` §2a:** a `bitstruct` is declared at item level or in a block; a local one has `Name::bits` and `Name::from_bits` derived as at item level.
- **Implementations:** the parser accepts the declaration as a statement and hoists it under a hidden name, as it does a local `struct`; the derived functions are made for it.
- **Rows:** `conf.local-bitstruct` (a file case).

## Compatibility classification

Additive: a declaration the grammar rejected is accepted.
