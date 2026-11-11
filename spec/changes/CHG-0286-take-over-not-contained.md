# CHG-0286 — A thread that takes over an exclusive path is not contained

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0237)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0237
Affects: `spec/18` (1.8.0), `spec/OVERVIEW.md`; `coby` (`interp/threads.rs`); `cbrt` (`affine_slow`);
`impl/model`; `spec/conformance.md`

## What changed

- `spec/18` §1: `contained(ℓ, Σ)` requires that ℓ took over no exclusive path; prose.
- `coby` and `cbrt`: a spawned thread that takes over an exclusive path joins the set of lent threads.
- Case: `conf.thread-taking-over-path-not-contained`.

## What changed for programs

A spawned thread that accesses through an exclusive reference it was not lent (one in a `Vec` moved to it, or
returned by a thread it joined) is from then on not cancelled, and its fault ends the program instead of being
handed to `join`/`try_join`.
