# CHG-0109 — cobc runs a spawned thread whose result is a reference

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-29, the owner's decision)
Governed by: `CobaltC_Master_Instructions.md` §8, §9
Depends on: D-0094
Affects: the cobc implementation, conformance (no normative text)

## Problem / motivation

`[T-Spawn]` allows any result type and coby complied; cobc refused a
bare reference result while running the same reference inside a
struct (D-0094).

## Decision

D-0094: implement, don't restrict.

## What changed

- **Specification:** nothing — the behavior was already specified;
  this record exists because conformance rows move with a CHG.
- **Guide:** §19's `spawn`/`join` paragraph gains one sentence — a
  result may be or carry a reference, and the handle holds the borrow
  until the join.
- **cobc** (`impl/cobc/src/lower.rs`): the spawn trampoline stores a
  reference result as pointer-plus-slot in the thread block's first
  field, holding the token continuously past the arguments' slot
  forget (the result token can BE an argument's); `join` rekeys the
  slot, re-stamps the token into the joining statement's scope with a
  same-thread `cb_send_ref`/`cb_recv_ref` pair, and forgets the
  transient slot. No cbrt change: existing entry points only.

## Compatibility classification

Widening: previously `unsupported` (exit 3) programs now run,
identically to coby.

## Conformance changes

**Added:** `conf.spawn-ref-result`
(`impl/conformance/19-concurrency/spawn_ref_result_ok.cb`: shared and
exclusive results, a function returning its own reference parameter),
`conf.spawn-ref-result-source-written-rejected`
(`impl/conformance/19-concurrency/spawn_ref_result_source_written_rejected.cb`).

## Revisit conditions

None.
