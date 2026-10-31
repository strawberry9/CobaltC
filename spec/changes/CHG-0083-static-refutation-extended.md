# CHG-0083 — Static refutation through references, pattern binders, and thread handles

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §17, §19, §21
Depends on: D-0071
Affects: rule.control.flow-analysis

## Problem / motivation

D-0071: four shapes of certain aliasing conflict were reported only when
they ran. One of them, a write to a place a still-running thread
borrows, had an outcome that depended on thread timing.

## Decision

D-0071: `deriv` facts through referent roots `*q`, match-by-reference
binders, and spawn handles until `join`.

## What changed

- **`spec/14` 1.11.0:** `rule.control.flow-analysis`'s facts, transfer
  functions, and discharge table.
- **`spec/conformance.md` 3.66.0:** the cases below.
- **`spec/02-schema.md` 1.0.56:** §5's "in use" ranges.
- **Implementations:** the shared checker (`impl/src/typecheck.rs`):
  `deref_place_of`, `borrowed_place`, `spawn_facts_of`,
  `match_referent`, `join` clearing a handle's facts, and
  `flow_consume` dropping what a moved binding held. The runtime and
  code generation are unchanged. The test harnesses drop their racy
  special case for `conf.cross-thread-write-conflict`.

## Compatibility classification

Tightening, within existing outcomes. Three rows move from (dynamic) to
(static) at the same line. `conf.cross-thread-write-conflict` moves from
`unspecified { ok, diag.aliasing-conflict }` to
`diag.aliasing-conflict` (static).

## Conformance changes

**Changed:** `conf.parent-use-while-child-live-rejected`,
`conf.guard-interior-borrow-conflict`,
`conf.match-by-ref-replace-while-bound-rejected`,
`conf.cross-thread-write-conflict`.

**Added:** `conf.reborrow-field-through-param-rejected`,
`conf.reborrow-disjoint-field-ok`, `conf.reborrow-reassigned-ok`,
`conf.match-by-ref-after-arm-ok`, `conf.spawn-borrow-after-join-ok`,
`conf.spawn-exclusive-read-rejected`, `conf.spawn-join-in-branch-dynamic`.

## Revisit conditions

See D-0071.
