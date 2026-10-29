# D-0246 — The model covers closures, slices and handles in containers

Status: ACCEPTED (2026-10-15; asked for by the owner: "fix the model with what it's missing")
Kind: Decision (`spec/02-schema.md`); evidence (D-0219), no normative change by itself
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0219, D-0241, D-0238, D-0239
Affects: `impl/model` (all of `src/`, README, `sample/`) (CHG-0298)

## Problem

The model left out the three parts of the alias-and-thread corner the last reviews worked in: closures
(`[Closure-Call]`, capture paths, writes-self), slices (`[Slice-Form]`, `[Slice-Index]`, D-0238's `range` and
`index` steps and `meet`), and a handle stored in a container or a struct (`[Handle-Destructor]` run by
`Vec::drop` and `[Destroy-Composite]`).

## Decision

Add them, from the text: closure literals with and without `move`, capture modes by the body's syntax,
`[Closure-Call]` with `self` shared or exclusive, borrow captures used as stored, move captures as the
closure's fields, `[Read-Fn-Through-Shared]`, closures passed by value and by shared reference, and spawned
(their captures lent); slices of a `Vec` and of a slice, indexed, passed to functions and threads, with path
steps `range` and `index` compared by `meet`, and `v[k]` by D-0238's element borrow; `Vec<handle<void>>`
and `struct Job { handle<void> h; }`, their destruction waiting for each thread in order and raising a failed
one's fault. The alphabet gains all of these.

## Results

What the extension found:

- **coby** (CHG-0293): a handle stored in a `Vec`, a struct, an `Option` or an array was destroyed without
  waiting for its thread, so the thread outlived its handle; a guard so stored never unlocked. cobc was right.
- **The text**: D-0242 (assignments and `Vec::push(&mut v, v[0])` faulted by the letter), D-0243 (a reference
  captured by a closure was proven clash-free while borrowed), D-0244 (a moved handle's facts kept), D-0245
  (a self-writing closure call: text static, tools dynamic); and CHG-0299, where the tools' static pass dropped
  a closure's facts when it was copied, accepting (then faulting on) a program the table rejects.

Later rounds the same day added loops (`while`, the flow analysis's fixed point), arrays with literal
indices, closures that own a resource (`[Fn-Value-Read]`'s move, `[Fn-Value-Empty-Call]`), and guards,
slices and closures together across threads. Loops and arrays found nothing; the rest found CHG-0301 (coby
copied a `move` capture unchecked), CHG-0302 (the tools faulted late on an owning closure passed by value),
D-0247 (a slice binding passed to `spawn`) and D-0248 (a data race through a slice, in the text and coby).

Counts are in CHG-0298.
