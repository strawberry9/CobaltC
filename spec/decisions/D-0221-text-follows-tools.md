# D-0221 — Closure calls, deferred last-use ends and `Vec` element places: the text follows the implementations

Status: ACCEPTED (2026-10-12; the owner chose D-0219's options 7(i), 8(i) and 10(ii): "7i, 8i, 9i, 10ii")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §21 (change management)
Depends on: D-0219 (the executable model and its findings), D-0111, D-0133 (`[Ref-Binding-Last-Use]`), D-0070
(`[Index-Vec]`'s element check), D-0218 (`[Read-Fn-Through-Shared]`)
Affects: `spec/15` 1.14.0 (`[Closure-Call]`), `spec/14` 1.26.0 (`[Ref-Binding-Last-Use]`, §6), `spec/16` 1.25.0
(`[Index-Vec]`), `spec/conformance.md`, the guide (§15, the data-parallelism section), `impl/model` (CHG-0260)

## Problem

The executable model (D-0219) found three places where the text rejects programs both implementations accept,
and where the implementations' reading looked sound:

- **Finding 7.** `[Closure-Call]` borrowed the closure exclusively, so a closure could not be called while a
  shared borrow of it lived, nor through `ref<fn(…), shared>`. Both tools borrow it shared; D-0218's
  `[Read-Fn-Through-Shared]` and an existing case assume they do.
- **Finding 8.** `[Ref-Binding-Last-Use]` ended a binding at its last mention, or — when it shared a statement
  with another reference-holding binding mentioned later — at its block's exit. Both tools defer the end
  until that other binding is no longer mentioned, and then judge from the flow facts whether anything is
  still rooted in it.
- **Finding 10.** The flow analysis had places `[i]` for a literal index into a `Vec`, so two borrows of `v[0]`
  were refuted statically. Both tools record an element borrow as a borrow of the whole container and leave
  element-against-element conflicts to the run, where `[Index-Vec]`'s check is exact.

## Options

For each: (i) the text follows the implementations; (ii) the implementations follow the text. D-0219 gives the
trade-offs; for 10 the options are reversed in number.

## Decision

The text follows the implementations for all three (7(i), 8(i), 10(ii)):

- `[Closure-Call]` borrows the closure **shared**. A body's writes go through its exclusive captures, whose paths
  are checked as any reference's; a closure holding an exclusive capture still cannot be called through a shared
  reference (`[Read-Fn-Through-Shared]`), and a call while an exclusive borrow of the closure lives still clashes.
- `[Ref-Binding-Last-Use]` states the **deferred end**: a binding waits from its last mention until no binding
  that may hold a reference, sharing a statement with it since its declaration, is mentioned again; it is then
  ended unless the flow facts root another binding's reference in it, in which case it lives to its block's
  exit. Bindings due at one statement end one at a time in declaration order.
- In `rule.control.flow-analysis`, a **`Vec`'s elements are not places**: `&_m v[k]` forms `deriv(r, v, m)`;
  an element access is not compared with the facts; `[Index-Vec]`'s dynamic check decides it. Literal-index
  places stay for arrays.

## Rationale

Each is a clarification in the sense of `spec/STABILITY.md` §2: it rejects nothing either implementation
accepts and changes no outcome of an accepted program. Each implementation reading is sound by the argument
D-0219 gives: a shared self-borrow does not let a write past a live reference, because writes go through
capture paths; a deferred end is later than a last use, never earlier; and the dynamic element check is exact
where a static one would have to treat a computed index as every index. Programs have been written against
these readings since D-0111 (last use) and D-0070 (element checks).

## Results

- Text: `spec/15` 1.14.0, `spec/14` 1.26.0, `spec/16` 1.25.0; the derivations of `conf.closure-borrow-capture`
  and `conf.e2e-closure-move-rc` now borrow shared.
- Cases: `conf.closure-called-while-shared-borrow-lives`, `conf.ref-holding-binding-end-deferred`,
  `conf.vec-element-borrows-compared-at-run`; each passed under both implementations before the text changed.
- The model: its deferred last-use reading is now the text's; `--literal-last-use` is gone.
