# D-0321 — A rejected program's report lists each independent error

Status: ACCEPTED (2026-11-14, the owner: "proceed use your recommendations", on the plan
`private/EASE-PLAN-2026-11-14.md`, item 1 and its decisions Q1 and Q2); implemented by `CHG-0444`
Kind: Decision (`spec/02-schema.md`); the diagnostic registry's reporting order
Governed by: `CobaltC_Master_Instructions.md` §19
Depends on: CHG-0130 (the first error in the source is the one reported), D-0228 (how a diagnostic is shown),
D-0296 (both toolchains in step)
Affects: `spec/registry/diagnostics.md` (Purpose); `spec/conformance.md` (3.311.0); `impl/src/` (`lib.rs`,
`parser.rs`, `modres.rs`, `typecheck.rs`, `check.rs`, `certificate.rs`, `main.rs`), `impl/cobc/src/main.rs`;
`impl/selfhost/front/` (`parser.cb`, `modres.cb`, `typecheck.cb`, `check.cb`), `impl/selfhost/cbc/`
(`diags.cb`, `frontend.cb`, `driver.cb`, `main.cb`)

## 1. Problem

A rejected program got one diagnostic. A program with four errors in three functions took four runs to put
right, though the type checker had found the errors of every function in its first run and kept only the
earliest (CHG-0130). For a person that is slow; for a model writing CobaltC, each run is a round trip.

## 2. Options

1. **Every error the checker finds.** A body's later errors may follow from its first (a binding whose
   declaration failed, a type that could not be named), and an error in one phase can produce errors in the
   next. Rejected: a report must not name an error that is not there.
2. **One error per item, in one phase; a later phase only when the earlier found nothing.** An item (a
   function, a declaration) stops at its first error, and nothing is checked on top of a failed phase, so no
   reported error follows from another. **Chosen.**
3. **Types only.** The checker already has the errors; syntax and name errors stay one per run. Kept as the
   fallback if syntax recovery produced a false error (it did not, §4).

## 3. Decision

The checks run in phases: the text's syntax, then names, then each declaration, then the bodies' types and the
static pass, then the static assertions; a later phase runs only when the earlier found nothing. A phase reports
at most one error for each item, its earliest, in source order; the first is the one reported before (CHG-0130).
A syntax error ends its item: the parser goes on at the next item, found by bracket depth from the item's start,
unless the text's brackets do not balance or the place found cannot begin an item, when the rest is not read.
The report shows each diagnostic as D-0228 shows one, a blank line between, then `N errors` (`N errors; the
first 10 are shown` past ten). The exit status, and what a conformance case requires (the first diagnostic's id
and phase), do not change.

## 4. Consequences

- The review (`stress/ease188/LOG.md`): of 534 rejected conformance cases, 2 now report two errors, both true;
  of 4,074 rejected programs under `stress/` (a* and b*), 14 report several, all true; of 1,320 programs
  with one syntax error put into a showcase, none reports a second error. One of them showed a coby/cobc bug
  older than this decision (CHG-0444): a program's `struct n` made `n` a type in `std`'s own text.
- `--certificate` and the other readers of the first error are unchanged.
