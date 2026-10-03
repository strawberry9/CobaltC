# D-0105 — A module's exported constant as an array length

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 friction 14)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0074 (constant array lengths), D-0021 (file-backed modules)
Affects: spec/22 `length`, spec/17 §1a

## Problem

D-0074 let an array length be the bare name of a constant declared with
an integer literal, so it could be known before types are. A library
could not export a size its clients use for arrays: `array<f64, geom::DIM>`
was rejected, and a client's own `const usize DIM = geom::DIM;` was not
literal-initialized.

## Candidate mechanisms

1. **A qualified name of a literal-initialized constant.** Selected.
   Modules are one token stream (a file-backed module is spliced in,
   `spec/17` §5), so the parser's pre-scan sees every module's constants.
2. **Any constant expression** (D-0074's candidate 2) — constant
   evaluation before types; still open.

## Selected design

`length ::= int-literal | identifier ('::' identifier)*`. A qualified
name is resolved from the current module outward, as `[Resolve-Qualified]`
resolves `q1` (the current module, then each enclosing one, then the
root), to a constant declared with an integer literal; it must be
visible there — exported, or used inside the module that declares it.

## Compatibility impact

Additive.

## Revisit conditions

Constant expressions as lengths (D-0074, candidate 2).
