# CHG-0128 — A nested pattern looks through a reference payload

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0109)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0109
Affects: spec/16 `rule.agg.match`, `spec/conformance.md`

## What changed

- **Rules:** `[Match-Through-Ref]` added to `rule.agg.match`;
  `[Pattern-Type]` descends through a reference to an enum (and a
  literal through a reference to an integer or `bool`).
- **Rows:** `conf.match-through-ref-payload`,
  `conf.match-through-ref-literal`,
  `conf.match-through-ref-binder-borrows-rejected`,
  `conf.match-through-ref-non-exhaustive-rejected`.

## Compatibility classification

Additive.
