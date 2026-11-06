# CHG-0134 — coby: a closure body keeps the static pass's records; a slice clashes with its `Vec`'s fields

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; two interpreter conformance bugs found while exercising D-0112)
Governed by: `CobaltC_Master_Instructions.md` §13, §19
Depends on: D-0108, D-0070, D-0107, rule.fn.closure, rule.agg.slice
Affects: the interpreter (`impl/src/interp.rs`, `impl/src/value.rs`, `impl/src/ast.rs`), `spec/conformance.md`

## Problem / motivation

Two programs behaved differently under `coby` and `cobc`; in both the
compiled tool was right and the interpreter wrong.

1. **`==` on a `String` captured by borrow, inside a closure**
   (`[key](ref<String, shared> t) : bool { *t == key }`) faulted
   `diag.read-of-resource` in `coby` and ran in `cobc`. D-0108 makes the
   comparison read both operands in place; the static pass records that
   per expression, keyed by the expression's address, and the
   interpreter kept a *copy* of every closure's body when the closure
   was formed, so inside a closure none of those records (text
   comparisons, match hints, temporaries ending at `?`, nested closures'
   result types) were found.
2. **A slice of a `Vec` passed beside `&mut` of the same `Vec`**
   (`grow(&mut a, &a[0..2])`, then `Vec::push` inside) ran in `coby` and
   faulted `diag.aliasing-conflict` in `cobc`. Both borrows are pending
   until bound (D-0107); at the first write through the exclusive one,
   `Vec::push` writes `len` and may move the buffer the slice views —
   spec/16 `[Slice-Form]`: "an access that reaches the whole source —
   `&mut v`, `Vec::push`, a read of `v` — conflicts with any slice of
   it". The interpreter's overlap test held a slice's range and a field
   of the same object apart, so the write to `len` never clashed. (With
   the slice in a binding the static pass refutes the program first,
   which hid the gap.)

## What changed

- **Interpreter:** a closure expression's body is shared
  (`Arc<Block>`), and the interpreter keeps that pointer, so the static
  pass's per-expression records apply inside closure bodies. A range
  step and a field step of one object overlap (`value::overlap`).
- **Rows:** `conf.closure-compares-captured-string` (positive),
  `conf.slice-argument-held-across-push-rejected` (dynamic).

## Compatibility classification

Conformance fixes: the interpreter now agrees with the compiler and the
text. Programs that ran only in `coby` because of (2) were invalid; none
in the repository was.
