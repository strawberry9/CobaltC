# CHG-0365 — `[Ref-Binding-Last-Use]`: a reference to reference-free storage defers an end only when rebound

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0275, D-0221, D-0111
Affects: `spec/14-control-flow.md` 1.29.0 (`[Ref-Binding-Last-Use]` and the prose after it); `spec/OVERVIEW.md`
1.3.0; `spec/conformance.md` (two rows, one case reworked); `impl/src/typecheck.rs` (`ref_end_blocked`,
`inert_ref`, `stmt_rebinds`; `consts::visit` visits a slice's operands); `impl/selfhost/front/typecheck.cb`
(the same); `impl/model/src/statics.rs`, `impl/model/src/ast.rs` (`end_blocked`, `stmt_rebinds`);
`impl/tests/conformance.rs` (two tests whose reborrow was never used again now use it after the write)

## What changed

D-0275. In `[Ref-Binding-Last-Use]`, a binding of type `ref<τ, m>`, `slice<τ, m>` or `StringView`, `τ` holding
no reference and no `fn` value, defers another binding's end through a statement only if that statement
declares it, assigns it whole, borrows it whole exclusively, or has a closure that captures it. Every other
binding that may hold a reference defers as before.

## What changed for programs

A reference binding whose end waited only for such a binding now ends where the others allow: a program that
faulted `diag.destroy-while-aliased` or `diag.move-while-aliased` because of the wait now runs. No program that
ran faults now: an end is still never earlier than a last use, and the flow-facts condition is unchanged. A
program that wrote through a reference while a reborrow of it, never used again, was held only by such a
binding (`auto r2 = &*r1; *r1 = 2;`) was rejected `diag.aliasing-conflict` and is now accepted.

## Found by

The code generator's port (D-0274), 2026-10-21; see `CHG-0364`.
