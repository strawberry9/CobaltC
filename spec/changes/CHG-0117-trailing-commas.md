# CHG-0117 — Trailing commas in calls and parameter lists

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0098
Affects: spec/22 §2 (`postfix` call), §3 (`fn-decl`, `extern-decl`, `closure`, `fn` type)

## What changed

- **spec/22:** `','?` before `)` in call arguments, parameter lists and
  `fn(…)` types.
- **Implementations:** the shared parser accepts it.
- **Rows:** `conf.trailing-comma-call`, `conf.trailing-comma-params`,
  `conf.trailing-comma-empty-call-rejected`.

## Compatibility classification

Additive.
