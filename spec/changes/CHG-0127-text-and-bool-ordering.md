# CHG-0127 — `bool`, `str` and `String` are ordered, and a `String` is compared in place

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0108)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0108
Affects: spec/06, spec/12, spec/21, spec/examples.md, `spec/conformance.md`

## What changed

- **Rules:** `rule.arith.cmp` gains `[Cmp-Text]` and `[Cmp-Bool]`;
  `[T-Cmp]`'s ordering premise is "τ ordered" (a number, `bool`, `str`,
  `String`, `StringView`), and a `String` operand is exempt from "not
  resource" because `[Cmp-Text]` borrows it; `rule.type.bound`'s table
  lists `bool`, `str` and `String` under `eq` and `ordered`;
  `type.str` no longer says orderings are ill-typed.
- **Rows:** `conf.string-compare-operators` (was
  `conf.string-eq-operator-rejected`), `conf.order-bool` (was
  `conf.order-bool-rejected`), `conf.str-ordering` (was
  `conf.str-ordering-rejected`), `conf.view-ordering`,
  `conf.string-compare-in-place`, `conf.string-compare-while-exclusive-rejected`,
  `conf.bound-ordered-text`; `conf.bound-unsatisfied-rejected` now uses
  `Option<i32>` (`bool` satisfies `ordered`).

## Compatibility classification

Additive.
