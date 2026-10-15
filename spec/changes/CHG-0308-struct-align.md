# CHG-0308 — `align(N)` on a struct

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0251
Affects: `spec/22` (2.51.0), `spec/16` (1.30.0), `spec/conformance.md`; `impl/src/ast.rs`, `impl/src/parser.rs`,
`impl/src/interp/layout.rs`, `impl/cobc/src/lower/layout.rs`; three conformance cases

## What changed

`struct-align` in the grammar; `[Layout-Struct]`'s alignment takes the maximum with `N`. `coby` and `cobc`
compute it; `cobc` emits `__attribute__((aligned(N)))`.

## What changed for programs

`struct S align(N)` is new. `align` stays an identifier everywhere else, so no program changes meaning.
