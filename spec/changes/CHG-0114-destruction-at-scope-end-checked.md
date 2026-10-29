# CHG-0114 — Destruction at a scope's end is checked for solitary

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29; implementation conformance fix, found by the round-6 stress programs)
Governed by: `CobaltC_Master_Instructions.md` §12
Depends on: rule.resauth.destroy, spec/14 [Block-Exit], [Stmt-Exit]
Affects: `coby` only; conformance

## Problem / motivation

`[Block-Exit]` and `[Stmt-Exit]` destroy a resource through `destroy`,
whose precondition is `solitary`; `[Destroy-Not-Solitary]` faults
`diag.destroy-while-aliased` when another live path reaches it. `coby`
checked this only for an explicit `drop`: a temporary `String` destroyed
at its statement's end while a `Vec<StringView>` held views of it ran to
completion, or faulted later at a use as `diag.stale-binding`. `cobc`
already faulted at the destruction. Found writing
`stress/round6/functional.cb`.

## What changed

- **No normative text.**
- **Implementations:** `coby` (`src/interp.rs` `destroy_at_scope_end`)
  checks `solitary` for each resource ended by a block's or a
  statement's exit; and `spawn` of a closure temporary now gives the
  temporary to the thread (it gave a copy and never ended the original,
  whose captured references then blocked that check).

## Compatibility classification

Conformance fix: the programs affected fault as the rules always said.

## Conformance changes

**Added:** `conf.destroy-temporary-while-viewed`,
`conf.destroy-local-while-viewed-outside`
(`impl/conformance/07-resource-authority/`),
`conf.spawn-closure-capturing-reference`
(`impl/conformance/19-concurrency/`).
