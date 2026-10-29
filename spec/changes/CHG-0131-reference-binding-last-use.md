# CHG-0131 — A local reference binding ends after its last use

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0111)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0111
Affects: spec/14 `rule.control.block`, spec/10 §1, `spec/conformance.md`, `spec/examples.md`

## What changed

- **Rules:** `[Ref-Binding-Last-Use]` in `rule.control.block`; spec/10's
  list of what ends a binding's object names it.
- **Rows:** `conf.ref-binding-ends-at-last-use`,
  `conf.ref-binding-used-later-rejected`,
  `conf.ref-binding-used-in-loop-rejected`.

## Compatibility classification

Additive: only programs that were rejected change outcome.
