# CHG-0125 — Enum codes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0106
Affects: spec/22 `enum-decl`, `variant`; spec/16 `rule.agg.enum-construct`

## What changed

- **spec/22, spec/16:** `enum Name : τ`, codes, `[Enum-Code]`,
  `[Enum-From-Code]`, `[Enum-Code-Invalid]`.
- **Implementations:** the shared parser reads the code type and codes
  and generates the two functions as CobaltC, hoisted beside the enum.
- **Rows:** `conf.enum-codes`, `conf.enum-from-code`,
  `conf.enum-code-duplicate-rejected`, `conf.enum-code-without-type-rejected`.

## Compatibility classification

Additive.
