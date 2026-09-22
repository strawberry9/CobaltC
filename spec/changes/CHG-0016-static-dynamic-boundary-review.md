# CHG-0016 — Static/Dynamic Boundary Usability Review

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §21
Depends on: rule.control.flow-analysis, rule.temporal.elision
Affects: rule.control.flow-analysis, rule.temporal.elision

## Problem / motivation

Two documented behaviors that will surprise users, assessed for
whether a sharper flow-analysis rule is warranted (which
would be a design change, since it would alter which programs are
rejected) or whether the trade is intentional and should simply be
stated where a reader will find it.

## What was assessed

1. Any `&mut x` passed to a call whose return type is not itself a
   reference marks `x` `escaped` for the rest of the function
   (`conf.sequential-exclusive-borrows-ok`'s derivation shows the
   consequence: two sequential `Vec::push` calls plus a read are all
   accepted, but only the first can ever be *proven* safe — every
   later access is `discharge: dynamic`).
2. `conf.vec-ref-then-push-rejected` is rejected *statically*, ahead of
   ever reaching the *dynamic* `diag.stale-binding` a differently
   shaped access to the same hazard gets
   (`conf.e2e-vec-realloc-stale-ref`, `CHG-0012`).

## Verdict

Both are intended trades, not gaps. (1):
`rule.control.flow-analysis`'s own stated `Scope` is intraprocedural
and syntactic, with no call-graph or value-range reasoning beyond
literals; distinguishing
"this call could not have stored a reference to `x` anywhere" would
need exactly the interprocedural alias-summary reasoning that scope
excludes, for a benefit that is provability only (no program is
rejected either way — `escaped(x) = T` only prevents *proving* safety,
never proves unsafety, so it can never cause a rejection by itself).
(2): rejecting the same-function shape early is strictly safer for the
programmer than deferring to a dynamic fault, and is free — `deriv`
already tracks exactly this shape for the elision case, so no
additional mechanism is needed to catch it; narrowing the rejection to
only the reallocating instances would need value-range reasoning the
analysis deliberately does not have, for no soundness gain (the
dynamic path already covers every instance the static one misses).
Master Instructions §9 (conceptual economy) favors leaving both as
they are.

## Rule changes

None. `rule.control.flow-analysis` and `rule.temporal.elision`'s CFN
are unchanged; each gains a confirming prose paragraph stating the
trade explicitly, in `spec/10` §3 and `spec/14` §6 respectively.

## Affected invariants

None restated.

## Dependency impact

None.

## Compatibility classification

Non-normative (documentation only).

## Migration implications

None.

## Example changes

None.

## Conformance changes

None (both existing cases, `conf.sequential-exclusive-borrows-ok` and
`conf.vec-ref-then-push-rejected`, already state the outcome this
record explains).

## Future implementation implications

None beyond what the existing rules already require.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

None. If a future pass finds a *soundness* consequence of either
trade (not merely a provability one), that would warrant reopening
this as a design decision; usability alone does not.

## Hygiene

- `spec/14-control-flow.md` 1.3.0 → 1.3.1: §6 gains the `escaped(x)`
  confirming paragraph.
- `spec/10-temporal-validity.md` 1.0.1 → 1.0.2: §3 gains the elision/
  `deriv` confirming paragraph.
