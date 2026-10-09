# CHG-0241 — Outcome sets by listing or by condition; six base judgment forms

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (3))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206
Affects: `spec/01` 1.2.0

## What changed

- §5: the braces of an `outcome` tag hold a documented set of admissible outcomes, listed member by member
  where finite and small, or given by a condition every member satisfies and a reader can decide for any
  proposed member; a listing is preferred where one exists. The two freedoms the tag bounds are named: which
  step is taken next (`[Thread-Step]`, `[Conc-Interleaving]`) and which value a step produces (`[Allocate]`,
  `[Os-Random]`, `[Repr-Enum]`); `impl-defined` is always the second.
- §2: six base judgment forms, §2.5a counted; it is primitive.

## What did not change

Every existing `outcome` tag already fit one of the two forms; none was rewritten. No rule changed.
