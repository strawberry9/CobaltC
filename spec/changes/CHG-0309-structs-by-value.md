# CHG-0309 — Structs by value across `extern`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0252, D-0250
Affects: `spec/20` §3 (1.18.0), `spec/16` (1.30.1), `spec/OVERVIEW.md`, `spec/registry/diagnostics.md`,
`spec/conformance.md`; `impl/src/typecheck.rs`, `impl/src/ffi.rs`; six conformance cases

## What changed

`FfiType ::= … | S where ffi-struct(S)`, with the exclusions and their reasons in `spec/20` §3.
`[Extern-Non-Ffi-Type]` is checked through struct fields, and its message names the field path. `coby`'s C
calls refuse a struct argument or result before the call.

## What changed for programs

A declaration that was `diag.extern-non-ffi-type` only because of a plain struct now compiles. Every other
program is unchanged.
