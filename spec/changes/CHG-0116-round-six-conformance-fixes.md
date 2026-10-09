# CHG-0116 — The implementations enforce rules the specification already states (round-6 findings)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; conformance corrections found by the round-6 stress programs, `stress/round6/NOTES.md` findings 3–34)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: spec/06, spec/12, spec/13, spec/15, spec/17, spec/21
Affects: `spec/conformance.md` (rows only); no rule text

## Problem / motivation

Real-world programs written against both implementations found places
where the shared checker, `coby` or `cobc` did not do what an existing
rule says: typing premises never checked (`[T-Arith]`, `[T-Logic]`,
`[T-Neg]`, `[T-Not]`, `[T-Cmp]`, `[T-Index]`, `[T-Shift]`, `[T-Return]`,
`[T-If]`, `[T-Struct]`, `[T-Enum]`, `[T-Array]`), a private enum's variant
reachable as `m::V` (`rule.module.visibility`), `[Narrow-Overflow]`'s
"static where constant" and `[Const-Checked-Failure]` not discharged, a
tail call that one C compiler turned into a jump (`[Call-Stack-Exhausted]`),
and interpreter or lowering defects (`map_err` results, calls of captured
`fn` values, captured `Vec` indexing, module-qualified variants).

## What changed

- **Rules:** none. Each fix makes an implementation follow the text.
- **Rows** (each pins one corrected behaviour): `conf.closure-calls-borrowed-capture`,
  `conf.closure-borrowed-capture-operations`, `conf.qualified-variant-clash`,
  `conf.map-err-resource-error`, `conf.return-bare-in-arm`,
  `conf.float-rem-rejected`, `conf.float-bitand-rejected`,
  `conf.bool-arith-rejected`, `conf.compare-resource-results-rejected`,
  `conf.logic-int-rejected`, `conf.neg-bool-rejected`,
  `conf.bitnot-bool-rejected`, `conf.order-bool-rejected`,
  `conf.index-string-rejected`, `conf.shift-float-rejected`,
  `conf.shift-amount-not-u32-rejected`, `conf.return-value-in-void-rejected`,
  `conf.return-bare-in-value-fn-rejected`, `conf.unit-equality`,
  `conf.struct-field-type-rejected`, `conf.struct-field-twice-rejected`,
  `conf.variant-payload-type-rejected`, `conf.array-element-types-rejected`,
  `conf.auto-untyped-variant-rejected`, `conf.assign-fn-item-rejected`,
  `conf.if-without-else-value-rejected`,
  `conf.private-enum-variant-qualified-rejected`,
  `conf.foreach-over-map-err-propagate`,
  `conf.generic-struct-lit-field-shape-rejected`,
  `conf.tail-recursion-stack-exhausted`,
  `conf.narrow-constant-static`, `conf.narrow-in-const-static`,
  `conf.to-int-constant-static`.
- **Implementations** (no rule): `cobc` passes `-fno-optimize-sibling-calls`;
  the runtime's aliasing check no longer allocates per ranged path or
  re-tests closed ancestor lists (a large speed-up with threads); `coby`'s
  aliasing check visits each reference once; about a dozen diagnostics
  now name the fix for a C, C++ or Rust habit.

## Compatibility classification

Corrective. A program the new static checks reject violated a stated
rule and either faulted at run time or behaved differently in the two
implementations. Measured before each tightening: across every program
in the repository (about 650), the only programs affected were two of
the round-6 stress programs and one conformance case, all adjusted.
