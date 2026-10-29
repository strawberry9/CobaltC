# D-0275 — A reference to reference-free storage does not defer another binding's last-use end

Status: ACCEPTED (2026-10-21, decided by the assistant; the owner left the choice to it: "I let you decide on
D-0221 issue")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0111, D-0221 (`[Ref-Binding-Last-Use]`'s deferred end), D-0236 (forgetting)
Affects: `spec/14-control-flow.md` (`[Ref-Binding-Last-Use]`), `spec/OVERVIEW.md`; `impl/src/typecheck.rs`,
`impl/selfhost/front/typecheck.cb`, `impl/model/src/statics.rs`; `CHG-0365`

## Problem

`[Ref-Binding-Last-Use]` ends a reference-holding binding `x` after its last mention, but defers the end while
another binding that may hold a reference, and that shares a statement with `x`, is mentioned again: that
binding may hold `x`'s path. "May hold a reference" was read off the type alone, so a reference parameter
`ref<Vec<i32>, exclusive> log` mentioned in `x`'s declaring statement deferred `x` to its block's exit,
although nothing `log` reaches can hold a reference and the statement could not rebind `log`. Found by the
code generator's port (D-0274): a reference to a boxed type name, its end so deferred, was still live when
the `match` scrutinee holding the Box was moved into a temporary and destroyed, and the destruction faulted
`diag.destroy-while-aliased` (correctly, by the rule; `CHG-0364`).

## Options

1. Keep the rule. Sound; the programmer works around it by not mentioning such parameters in the
   statement, which nothing in the program text suggests.
2. A binding of type `ref<τ, m>`, `slice<τ, m>` or `StringView`, `τ` holding no reference and no `fn` value,
   counts through a statement only if that statement may rebind it: declares it, assigns it whole, borrows
   it whole exclusively, or has a closure that captures it. Such a binding reaches only reference-free
   storage, so the one way it can come to hold `x`'s path is to be rebound.
3. Track, per statement, which bindings receive a reference derived from `x` (a flow analysis). Most
   precise, but a second analysis beside the flow facts that §9's economy weighs against (D-0236's
   argument).

## Decision

Option 2. It is sound for the reason given, small to state and to implement (a syntactic test of one
statement), and covers the common case: reference parameters to plain data, slices of numbers, string
views. Aggregates, `fn` values, handles and shared-ownership types still count by their type alone, since
they can be given a reference without being rebound (a closure's own state, an `Arc`'s shared contents).
The flow-facts condition of the rule is unchanged.

## Consequences

- Programs whose reference bindings were held to their block's exit by such a binding now end them at the
  deferred point the other bindings allow: fewer `diag.destroy-while-aliased`/`move-while-aliased` faults,
  never more (an end is still never earlier than a last use).
- Cases `conf.ref-binding-end-not-deferred-by-plain-reference` and
  `conf.ref-binding-end-deferred-by-rebound-reference`; `conf.box-destroyed-while-contents-referenced`
  (CHG-0364) now defers through a `ref<Vec<StringView>, exclusive>`.
