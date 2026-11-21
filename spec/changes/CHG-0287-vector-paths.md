# CHG-0287 — Paths over a vector's elements: `range`, `index`, `meet`; `[Index-Vec]` a rule of its own

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0238)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0238
Affects: `spec/04` (1.8.0), `spec/16` (1.28.0), `spec/08` (H4), `spec/OVERVIEW.md`; `spec/conformance.md`

## What changed

- `spec/04`: `state.access-paths` says what a path over a vector's elements is; `overlap` uses the new `meet`
  where a storage step is present; `steps` names `range(i, j)` and `index(k)`.
- `spec/16`: `[Slice-Form]` gives the slice's path record; `[Slice-Index]` says which object an element access
  is of; `[Index-Vec]`, `[Index-Vec-Out-Of-Bounds]`, `[Index-Vec-Conflict]` replace the `≡`; the prose.
- `spec/08` §4: H4 by the `range` step; "overlap is by target, or by steps".
- Case: `conf.empty-slice-meets-vec-fields`.

## What changed for programs

Nothing: a clarification of what both implementations did.
