# CHG-0312 — `simd<τ, N>`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0254
Affects: `spec/16` (1.31.0), `spec/21` (4.66.0), `spec/22` (2.53.0), `spec/registry/diagnostics.md` (1.57.0),
`spec/conformance.md`; both tools; conformance cases

## What changed

The type `simd<τ, N>`, its layout, lanes, operators and intrinsics; `simd` is a reserved type name.

## What changed for programs

A program using `simd` as a name no longer parses (`diag.syntax-error`, with a message saying it is reserved);
none in the repository did. Everything else is new.
