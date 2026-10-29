# CHG-0112 — `foreach` over an integer range

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0097
Affects: rule.control.foreach-range, spec/22 `foreach-expr`, the guide

## Problem / motivation

Counting through integers took a full counted `for`; D-0042 and D-0047
both left ranges in `foreach` open (D-0097).

## Decision

D-0097: half-open `lo .. hi`, written only in a `foreach`, with an
optional position name; two literal bounds are `usize`.

## What changed

- **`spec/14` 1.17.0:** `rule.control.foreach-range`, with
  `[Foreach-Range]` and `[Foreach-Range-Ill-Typed]`.
- **`spec/22` 2.32.0:** `foreach-expr` takes `expr '..' expr`.
- **Implementations:** the shared parser (`src/parser.rs`
  `foreach_range`) writes the loop; `src/each.rs` gains `$each_range`,
  which the checker, `coby` and `cobc` all expand. No back-end change.
- **Guide:** the `foreach` section gains ranges.

## Compatibility classification

Additive: the form did not parse before.

## Conformance changes

**Added:** `conf.foreach-range`, `conf.foreach-range-position`,
`conf.foreach-range-literal-takes-type`, `conf.foreach-range-empty`,
`conf.foreach-range-literals-index`,
`conf.foreach-range-negative-literal-typed`,
`conf.foreach-range-negative-literals-rejected`,
`conf.foreach-range-output`
(`impl/conformance/14-control-flow/foreach_range_ok.cb`),
`conf.foreach-range-mixed-rejected`, `conf.foreach-range-float-rejected`,
`conf.foreach-range-three-names-rejected`,
`conf.foreach-range-dollar-rejected`
(`impl/conformance/22-surface-syntax/foreach_range_dollar_rejected.cb`),
`conf.range-not-a-value` (`…/range_not_a_value_rejected.cb`).

## Revisit conditions

D-0097's.
