# CHG-0113 — A borrowing closure cannot escape its captures' scope

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29; implementation conformance fix, found by the round-6 stress programs)
Governed by: `CobaltC_Master_Instructions.md` §12
Depends on: rule.temporal.ref-escape, rule.fn.closure
Affects: `spec/10` (clarifying sentence), the shared checker

## Problem / motivation

`[Closure-Form-Borrow]` (`spec/15`) makes a closure without `move` a
struct whose fields are `&_{m_i} x_i`, and `[Ref-Escape-Rejected]`
rejects a borrow stored into a field of an aggregate whose value
escapes. So returning a borrowing closure from the function whose local
it captures, or storing it into a binding of an enclosing block, is
ill-formed (`diag.reference-escapes-scope`, static). The checker
recognised borrows only as written `&` expressions, so these programs
were accepted and failed only at run time — with different diagnostics
in the two implementations (`coby`: `diag.stale-binding` at the call;
`cobc`: `diag.destroy-while-aliased` at the return, for a `String`
capture). Found writing `stress/round6/wrap.cb`.

## What changed

- **`spec/10`:** one sentence under `rule.temporal.ref-escape` states
  what `[Closure-Form-Borrow]` already implied. No rule changes.
- **Implementations:** the shared checker (`src/typecheck.rs`
  `collect_stored_borrows`) treats each non-reference capture of a
  closure without `move` as a stored borrow of that binding; the
  message says to capture by value with `move`. One change serves
  `coby` and `cobc`.

## Compatibility classification

Conformance fix: the programs now rejected were ill-formed by the rules
already, and every one of them faulted when the closure was called.

## Conformance changes

**Added:** `conf.closure-borrow-escapes-to-caller`,
`conf.closure-borrow-escapes-block`, `conf.closure-move-escape-ok`
(`impl/conformance/10-temporal-validity/`).
