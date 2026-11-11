# CHG-0183 — Text literals in arms and branches

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0155)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0155
Affects: `spec/21` (4.20.0) §2a; `spec/conformance.md` (3.145.0); `impl/src/views.rs`

## What changed

- **`spec/21` §2a:** `[Str-Literal-String]`'s note: either rule reaches a literal given by a block, an `if` branch or a `match` arm in the position.
- **Both tools:** `impl/src/views.rs`'s `wrap` follows those tails.
- **Rows:** `conf.str-literal-in-arms`.

## Compatibility classification

Additive.
