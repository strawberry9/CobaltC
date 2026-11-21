# D-0106 — Enum codes

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 friction 26)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0006 (no implicit conversion), D-0057 (literal patterns)
Affects: spec/22 `enum-decl`, `variant`; spec/16 `rule.agg.enum-construct`

## Problem

A CobaltC enum is not an integer, and nothing took an enum to or from
one. Binary formats, protocol opcodes, instruction sets and C constants
all need that mapping, so programs wrote two hand-kept `match` functions
per enum, which drift apart as variants are added. `enum E { A = 1 }` was
a syntax error with no way to say it.

## Candidate mechanisms

1. **`enum Op : u8 { A = 1, B, … }` with generated `Op::code` and
   `Op::from_code`.** Selected. The `: type` form is the one the planned
   `bitstruct Name : u32` uses; the functions are associated functions,
   as the rest of CobaltC's operations are (`Vec::len`, `String::eq`), so
   no intrinsic name (a generic `code(e)` would clash with every variable
   called `code`).
2. **Intrinsics `code(e)` and `from_code<E>(n)`.** The form first
   proposed; the associated functions are the same operations without
   reserving two common words.
3. **Implicit integer values (C's).** Against D-0006.

## Selected design

- `(':' int-type)?` after the enum's name; with it, a variant may be
  `V = code` (a signed literal allowed for a signed type); a variant
  without one is the previous code plus one, the first 0.
- The enum has no type parameters and no payloads; codes are distinct
  and fit the type; it is declared at module level (a local enum is
  renamed when hoisted, D-0069).
- `Name::code(Name v) : τ` and `Name::from_code(τ c) : Option<Name>`
  are generated as ordinary functions of the enum's module, exported
  with it: plain CobaltC, the same in every implementation.

## Compatibility impact

Additive. A program that already declares `fn Op::code` for an enum
with codes has a duplicate item (`diag.duplicate-item`).

## Revisit conditions

With `bitstruct`, if both are adopted, the `: type` form is shared.
