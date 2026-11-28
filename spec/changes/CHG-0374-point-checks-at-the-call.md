# CHG-0374 — A conflict at a call that only values decide is checked when the call is made

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-26)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0283, D-0282, D-0281
Affects: `spec/14` (1.31.0, `rule.control.frame-references`: point checks at the call), `spec/conformance.md`
(3.274.0); `impl/src/tier.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/interp/point_checks.rs`,
`impl/src/interp/calls.rs`, `impl/src/interp/access.rs`, `impl/src/value.rs`, `impl/src/lib.rs`;
`impl/cobc/src/lower/user_calls.rs`, `lower/calls.rs`, `lower.rs`; `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`; six conformance cases; the book's chapter 7; the guide's §08 and §14

## What changed

Where `rule.control.frame-references` finds that what a callee may do through one argument meets another
argument, or a live reference of the caller, only for some values (two computed indexes or ranges) or only on
some paths (a fact `?`), the run checks it when the call is made: after the arguments are bound and before the
body runs, each access the callee's summary declares through that argument is checked as if made at the call.
A clash faults at the call, `diag.aliasing-conflict`, whether or not the body would make the access. A path
with an empty range reaches nothing; `swap`'s two places are one or disjoint, and are not checked against each
other. The static pass records the checks; `coby` makes them after binding the parameters, `cobc` through
`cbrt`'s new `cb_point_check` before the call.

## What changed for programs

A program whose call passes two overlapping places that only values tell apart now faults at that call, with a
message naming the callee and the argument, instead of at the callee's first conflicting access. Output the
callee would have printed before that access is not printed. Programs whose places do not overlap are
unaffected, and so are conflicts through references kept in data structures.

## Conformance

`conf.state-and-own-field-input-dynamic` and `conf.overlapping-slice-arguments-dynamic` fault at the call;
`conf.vec-grow-fault-intact-dynamic` and `conf.fault-unwind-callee-tail-conflict-dynamic` pass the shared
reference in a struct (tier D), so their faults stay inside the callee; new: `conf.point-check-at-call-dynamic`,
`conf.point-check-at-call-distinct`, `conf.point-check-swap-same-place`, `conf.point-check-empty-view`.
