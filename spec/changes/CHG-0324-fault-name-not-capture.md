# CHG-0324 — A `fault` call's diagnostic name is no closure capture

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0017, D-0083
Affects: `spec/15` §6 (1.18.0); `spec/conformance.md` (one row); `impl/src/typecheck.rs`

## What changed

`[Closure-Capture-List-Rejected]` compares the written capture list with the body's free variables. The
checker's scan counted the first argument of `fault(d, …)` — a diagnostic's name, not an expression
(D-0083) — as a free variable, so a closure that faults on purpose was `diag.capture-list-mismatch` whatever
its list said. `spec/15` §6 now says the name is no variable, and the scan skips it. Row
`conf.closure-fault-name-not-capture`.

## What changed for programs

Closures that call `fault` are accepted with the capture list that names their variables.

## Found by

The unit test of `Once` (D-0260): an initialiser that faults inside a closure (2026-10-17).
