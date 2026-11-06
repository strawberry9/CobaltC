# CHG-0377 — In a decided program, values pass between functions with no object

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0286, D-0285, D-0281
Affects: `impl/cobc/src/lower/flights.rs` (new), `user_calls.rs`, `calls.rs`, `values.rs`, `intrinsics.rs`,
`fresh.rs`, `frames.rs`, `functions.rs`, `program.rs`, `analysis_strings.rs`, `matching.rs`, `stmts.rs`,
`exprs.rs`, `operators.rs`, `places.rs`, `lower.rs`; `impl/cobc/tests/driver.rs`; the guide's chapter P; no
change to the specification's rules

## What changed

In `cobc`'s decided lowering (D-0285) no value is in flight between functions. A by-value resource argument is
handed over as its value, the caller ending its object's record; every resource result leaves as a `__raw`
form's does; a callee binds such a parameter objectless where the objectless analysis allows (parameters are
candidates now) and otherwise gives it an object; a call's result goes with no object to an objectless binding,
an argument, a literal's part, a result or a `match`, and is given one elsewhere. The natives' fixed texts are
rewritten to match after each function is lowered (`ground_native`); a text the rewrite does not know keeps the
program on the first lowering. The objectless analysis also allows, in a decided program, borrows of a candidate
and of its fields, reads of its plain fields, and enums over objectless payloads.

## What changed for programs

Nothing a program can observe: a decided program ends as before, faster (extsort 431 → 288 ms, csvstat 73 → 72
ms, GCC, best of five).

## Conformance

No row changes. A driver test checks the decided C and its outcome under both C compilers.
