# D-0138 — `HashSet::union`, `intersection` and `difference`

Status: ACCEPTED (2026-10-03, the owner: "add union, intersection and difference to HashSet")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0041 (hash tables), D-0116 (the `clone` bound), D-0134 (`std`'s naming conventions)
Affects: `spec/21` §0 and §2g `rule.stdlib.hashmap` (4.4.0), `spec/conformance.md` (3.128.0), the guide §21, `impl/std/collections.cb`

## Problem

`HashSet<K>` had membership, insertion and removal but none of the
operations that make a set a set. A program wanting the keys two sets
share, or those one has and the other lacks, wrote the loop each time:
walk one set, ask the other, insert into a third. Each of the three
names one very common intent, and the name says what the loop has to be
read to discover (§9's admission test).

## Candidate mechanisms

1. **Leave the loops to programs.** Four lines each; the intent is
   hidden in a condition.
2. **New sets: `HashSet::union(&a, &b)`, `intersection`, `difference`,
   each returning a `HashSet<K>`.** Selected.
3. **In place: `HashSet::extend(&mut a, &b)`, `retain_in`, `remove_all`.**
   They avoid copying the kept keys, but change their argument, need a
   name for each that is not the set operation's, and the common use
   (compare two sets, keep both) then needs a `clone` first. A later
   decision if programs ask.
4. **Operators (`a | b`, `a & b`, `a - b`).** No user-defined operator
   overloading exists in CobaltC, and adding it for one type is a
   language change. Not adopted.

## Selected design

`spec/21` §2g, `[Set-Ops]`: each function takes both sets by shared
reference, leaves them unchanged, and returns a new set whose keys are
copies made with `clone` — so `K` must be `clone` (D-0116), as for
`HashSet::clone`; every key type is (integers, `bool`, `str`, `String`,
and structs and enums of them by derivation). Order is defined, as every
set's is (insertion order): the first set's keys in its order, and for
`union` then the second set's keys the first lacks, in the second's
order. Symmetric difference, subset and disjointness tests are left out
until programs ask; each is one of these plus `len`.

## Compatibility impact

Additive: three new associated functions of `HashSet`; a program cannot
declare `HashSet::` functions of its own (`[Assoc-Fn-Foreign-Type]`).

## Revisit conditions

- In-place forms, if programs measured to copy keys needlessly.
- `is_subset`, `is_disjoint`, `symmetric_difference`, if programs ask.
