# CHG-0074 — Field and element access through a guard

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0064
Affects: rule.agg.field, rule.conc.lock

## Problem / motivation

D-0064: `(*g).f` everywhere, and the tools disagreeing on `g.f`.

## Decision

D-0064: `g.f` and `g[i]` through a guard, as through a reference.

## What changed

- **`spec/16` 1.12.0:** `[Guard-Auto-Deref]` and its prose.
- **`spec/19` 1.8.0:** a note at `[Guard-Deref]`.
- **`spec/21` 3.27.0:** `Channel`'s source writes `g.count` for
  `(*g).count`.
- **`spec/conformance.md` 3.56.0:** the cases below.
- **`spec/02-schema.md` 1.0.47:** §5's "in use" ranges.
- **Implementations:** the checker (`check_field`, `Index`,
  `static_place_type`); `cobc` (`guard_through` for a guard-typed base of
  a field or index); `std`'s `Channel`. The guide (§19).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.guard-auto-deref`, `conf.guard-auto-deref-stale`.

## Prior-art status

See D-0064.

## Revisit conditions

None.
