# CHG-0216 — Dual bodies: reference arguments checked once at entry

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-08; D-0188)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0188
Affects: `spec/08` (1.2.0) §4; `spec/19` (1.9.0) Purpose; `spec/conformance.md` (3.178.0); `impl/cobc/src/lower.rs`, `impl/cbrt/`

## What changed

- **`spec/08` §4:** "A path's standing changes only in its own thread":
  the consequence of the existing rules that lets an implementation
  check reference arguments once, at entry, instead of at each access,
  with its conditions (not lock-derived, pending arguments checked
  pairwise).
- **`spec/19` Purpose:** points to it.
- **`spec/conformance.md`:** `conf.state-and-input-arguments-disjoint`,
  `conf.state-and-own-field-input-rejected`,
  `conf.state-and-field-of-state-rejected`,
  `conf.overlapping-slice-arguments-rejected`; the header's stale
  version corrected.
- **`cobc`/`cbrt`:** dual bodies (`dual_eligible`, `lower_fn_dual`,
  `DUAL_LINES`), `cb_paths_disjoint`, `cb_peek_datum_tok` with its
  in-flight assertion.

## What did not change

No rule, diagnostic or observable behavior. A single-threaded program
faults where and how it did and prints what it did; a threaded one gives
a result `[Thread-Step]` admits, as before. `coby` is unchanged.
