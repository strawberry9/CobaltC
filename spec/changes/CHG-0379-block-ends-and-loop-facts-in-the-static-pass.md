# CHG-0379 — The static pass checks a block's end, and carries a loop body's facts

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0287, D-0282, D-0284
Affects: `spec/14-control-flow.md` 1.33.0 (`rule.control.frame-references`), `spec/conformance.md` 3.276.0;
`impl/src/typecheck.rs`; `impl/conformance/21-standard-library-semantics/http_error_closes_connection_ok.cb`

## What changed

Three repairs to `rule.control.frame-references` as implemented, found when D-0287 made splitter loops tier S.
Each let a program be decided while it read a `String` that had been destroyed (`cobc`'s decided build printed
what was left in freed storage; `coby` and the tracked build faulted):

1. **A block's end** (`check_block_exit`, at the end of a nested block and of a `match` arm). A frame-held
   reference declared outside the block whose fact `T` is rooted at a binding the block declares, and which is
   live after the block, is refuted: `diag.destroy-while-aliased` (static), "`X` is destroyed at the end of its
   block while `R` still refers to it, and `R` is used later". A fact `?` there leaves the program to the run.
   The escape check saw only a borrow written in place, not a copy of a reference (`keep = v;`).
2. **A loop's body.** The facts that copying a reference makes were made only on the pass over a loop's body that
   reports, the last, so they never reached the loop's head or exit: a reference kept from one iteration to the
   next, or past the loop, was taken to hold nothing. `tier_inherit` now makes the facts on every pass and
   reports on the last.
3. **A literal argument.** A call whose first reference argument is static data (`StringView::trim("  a ")`)
   gave a result of unknown origin, which kept the program's tracking; the result is now static data.

`conf.http-error-closes-connection`'s case sets its server's read timeout to ten minutes: under an unoptimised
`coby` its client takes longer than the default 30 s to write the request, so the case timed out in the debug
`cargo test` run, before this change as after it.

## What changed for programs

A program that reads a view of a block's `String` after the block is now rejected before it runs; `coby` faulted
on each at run time. Some programs that were decided now keep their tracking, and fault where `coby` faults.
No correct program changes outcome.

## Conformance

`conf.block-end-view-outlives-owner-rejected` (static), `conf.loop-kept-view-conflict-dynamic` (dynamic).
