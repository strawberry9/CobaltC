# CHG-0090 — Ordering text, and shortening a `String`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §9, §19, §21
Depends on: D-0076
Affects: `spec/21` §0

## Problem / motivation

D-0076: no text order in the library, no message on `<` for text, and
no way to shorten a `String`.

## Decision

D-0076.

## What changed

- **`spec/21` 3.32.0:** `StringView::less`, `String::less`,
  `String::truncate`.
- **`spec/conformance.md` 3.73.0:** the cases below.
- **`spec/02-schema.md` 1.0.63:** §5's "in use" ranges.
- **Implementations:**
  - **`std`:** the three functions.
  - **The checker:** the message for `<`, `<=`, `>` and `>=` on a
    `str` or a `StringView`, and for other operators on `str`.
- **The guide:** the `String` table.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.string-less-sort`, `conf.string-truncate`,
`conf.string-truncate-inside-character`.

## Revisit conditions

See D-0076.
