# CHG-0050 — `foreach`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0042
Affects: rule.control.foreach (new), rule.stdlib.format, rule.stdlib.hashmap, spec/22 §1–§2, the cases listed below

## Problem / motivation

D-0042: no loop over a collection's elements.

## What changed

- **`spec/22` 2.14.0:** keyword `foreach`; `foreach-expr`.
- **`spec/14` 1.6.0:** `rule.control.foreach`.
- **`spec/21` 3.13.0:** the consuming loop's `std`-private holders in
  §2g; §2f formats a reference to a printable value as the value.
- **`spec/registry/diagnostics.md` 1.19.0:** `diag.type-mismatch` names
  `[Foreach-Not-Iterable]`.
- **`spec/conformance.md` 3.36.0:** the cases below.
- **`spec/02-schema.md` 1.0.26:** §5's "in use" ranges.
- **Implementations:**
  - lexer: `foreach`; parser: the rewrite to a counted `for` over hidden
    bindings and the `$each_*` intrinsics;
  - `src/each.rs`: each intrinsic's expansion for a collection type,
    used by the typechecker (with `std`'s field visibility), `coby` and
    `cobc`;
  - `std`: `Vec::take_raw`, `Drain`, `HashDrain`, `SetDrain`;
  - `printf`'s `$fmt` in all three: a reference to a number, `bool` or
    `str` is formatted as the value.

## Compatibility classification

Breaking only for a program naming something `foreach`.

## Conformance changes

**Added:** `conf.foreach-borrowed`, `conf.foreach-mut`,
`conf.foreach-consumed`, `conf.foreach-index`, `conf.foreach-array`,
`conf.foreach-hashmap`, `conf.foreach-hashset`,
`conf.foreach-ref-binding`, `conf.foreach-in-is-a-name`,
`conf.foreach-change-collection-rejected`,
`conf.foreach-consumed-then-used-rejected`,
`conf.foreach-not-iterable`, `conf.foreach-hashmap-one-name-rejected`,
`conf.foreach-hashset-mut-rejected`, `conf.foreach-output` (file case
`14-control-flow/foreach_ok.cb`, output pinned), `conf.printf-ref-value`.

## Prior-art status

See D-0042.

## Revisit conditions

See D-0042.
