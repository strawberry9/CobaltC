# CHG-0373 — Conflicts of frame-held references are compile-time errors

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-25)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0282, D-0281
Affects: `spec/14` (1.30.0, `rule.control.frame-references`), `spec/05` (1.8.2), `spec/07` (1.3.1), `spec/08`
(1.8.0), `spec/conformance.md` (3.273.0); `impl/src/typecheck.rs`, `impl/src/tier.rs`, `impl/src/interp.rs`,
`impl/src/lib.rs`; 42 conformance cases; the book's chapter 7

## What changed

`rule.control.frame-references` (D-0282) refutes, before the run, the conflicts among frame-held references
that the program text decides: through calls (the call-site rule, using each callee's access summary), through
references inherited from other references, fields, calls and branches, with liveness by last use; and three
forms that are always errors (an element place found before a value that borrows its container exclusively, a
value moved out through a reference, a view of a temporary kept past its statement). Both `coby` and `cobc`
apply it; it lives in their shared static pass.

## What changed for programs

A program with such a conflict no longer runs until the fault: it is rejected, with a diagnostic that names the
callee and what it does through which argument. Programs without one are accepted as before. Conflicts that
depend on values (computed indexes or ranges) and conflicts of references kept in data are still found at run
time. The self-hosted toolchain, frozen at Update 135 (D-0278), still finds these conflicts at run time.

## Conformance

38 rows move from dynamic to static (`spec/conformance.md` 3.273.0); four new rows keep the run-time forms:
`conf.vec-grow-fault-intact-dynamic`, `conf.fault-unwind-callee-tail-conflict-dynamic`,
`conf.state-and-own-field-input-dynamic`, `conf.overlapping-slice-arguments-dynamic`.
