# D-0218 — What a shared reference reaches is read-only, at every depth

Status: ACCEPTED (2026-10-12; the owner chose option B of the proposal, "Go with B and implement it")
Kind: Decision (`spec/08-alias-validity.md`, `spec/09-identity-origin-extent.md`, `spec/12-type-system.md`)
Governed by: `CobaltC_Master_Instructions.md` §6 (aliasing), §17 (Function Interfaces: "enough semantic information
to preserve interprocedural guarantees without undocumented caller/callee agreements")
Depends on: D-0004, D-0018, D-0046, D-0089, D-0116, D-0213, `spec/AUDIT-2.md` B-15
Affects: `spec/08` 1.5.0, `spec/09` 1.6.0, `spec/12` 1.23.0, `spec/15` 1.13.0, `spec/16` 1.23.0,
`spec/registry/diagnostics.md` 1.52.0, `spec/examples.md` 3.18.0, `spec/IMPLEMENTATION-NOTES.md`,
`spec/conformance.md` 3.212.0, `spec/02-schema.md` 1.0.76 (CHG-0257); twelve new cases, two revised

## Problem

`spec/08` §2 said "a `ref<τ, shared>` value can never be a source of mutation — this is the interface guarantee
Master Instructions §17 requires of a shared-reference parameter." The rules did not deliver it.
`[Borrow-Exceeds-Source]` stops an exclusive borrow *from* a shared path, but a reference *stored* in the object
kept its own mode. Found during D-0213 (`stress/review2_oct11/m01.cb`). Every one of these was accepted by
`coby` and by `cobc` with GCC and Clang (`stress/m01/`):

| Probe | Shape |
|---|---|
| `m01.cb`  | `fn w(ref<S, shared> s) { *s.inner = *s.inner + 1; }` |
| `m01b.cb` | `fn leak(ref<S, shared> s) : ref<i32, exclusive> { s.inner }`, then `*r = 5` |
| `m01c.cb` | `fn bump(ref<Vec<ref<i32, exclusive>>, shared> v) { *v[0] = *v[0] + 1; }` |
| `m01d.cb` | `match (e) { A(p) : **p = 7, … }` with `e : ref<E, shared>` |
| `m01e.cb` | `fn dup(ref<S, shared> s) : S { *s }`, then a write through the copy |
| `m01f.cb` | a closure `[x]() { x = x + 1; }` stored in a `fn` field, called as `(h.f)()` through `ref<H, shared>` |
| `e2.cb`   | `match (*e) { A(p) : *p = 7, … }`: a payload bound by value through a shared reference |
| `e4.cb`   | `clone(s)` of an `S` holding an exclusive reference, then a write through the clone |
| `e7.cb`   | `match (s.r) { A(n) : *n = 3, … }`, `s.r : ref<E, exclusive>` read through a shared `s` |

The proposal (`private/D-0218-shared-path-reaches-exclusive-proposal.md`) found the first five. Implementing it
turned up the rest: `fn` values, a match by value, `clone`, and a match on a reference read through a shared one.

## Options

A. Keep the rules and correct the text: a shallow guarantee, as `const` is in C++. B. Transitive: a reference
read through a shared place is shared, and a value holding an exclusive reference is not copied out through
one. C. B's first rule only. D. Forbid a shared borrow of any type holding an exclusive reference. The proposal
recommended B.

## Decision

B, completed for the shapes found while implementing it (`spec/08` `rule.alias.read-through-shared`):

1. `[Read-Ref-Through-Shared]`: an exclusive reference or slice read as a value from a place reached through a
   shared path is a shared borrow of the stored path, typed `ref<τ, shared>` (`[T-Read-Through-Shared]`). So a
   write through it is `[Write-Not-Exclusive]`, an exclusive borrow from it `[Borrow-Exceeds-Source]`, and where
   an exclusive reference is expected the read is `diag.borrow-exceeds-source`. A match on such a reference binds
   shared references.
2. `[Read-Exclusive-Through-Shared]`: a value holding an exclusive reference (a field, an array element or a
   payload, at any depth, not behind a reference) is not copied out through a shared path:
   `diag.borrow-exceeds-source`, static. The same holds for a payload bound by value
   (`[Match-Exclusive-Through-Shared]`), and `clone-holds` excludes such types, because `clone` reads through a
   shared reference.
3. `[Read-Fn-Through-Shared]`: a `fn` type does not show its closure's captures. A value holding `fn` values,
   reached through a shared path and read or called, is checked where that happens. A closure with an exclusive
   capture (a borrow capture its body writes, or a moved exclusive reference) is `diag.borrow-exceeds-source`,
   dynamic.

Reads stay legal: `*s.inner + 1`, `auto r = s.inner;` (shared), passing `s.inner` where a shared reference is
expected, returning it as one. Owned copies stay legal (`S t = s;`). Mutation through a shared handle keeps one
route, `mutex<_>`.

## Results

- One static pass serves both tools (`cobc` uses `coby`'s front end), and `src/typecheck.rs` holds rules 1 and 2:
  `crosses_shared` follows `*r` through a reference read from a shared place; `check_expr` handles a field, an
  element or `*r` read as a value (`read_through_shared`); `check_match` handles the scrutinee and its binders;
  `clone_problem` handles `clone`.
- Rule 1's borrow is formed by a front-end rewrite (`Rewrite::SharedRef`/`SharedSlice`, `src/views.rs`). The
  read becomes `&*e` (`&e[0..$]` for a slice) and the program is checked again, the D-0162 mechanism. Neither
  tool has a code path of its own for it, and a program without the shape is not checked twice. A `*e` operand is
  not rewritten: the tools access through the stored path, with the same outcome (`spec/IMPLEMENTATION-NOTES.md`
  §4).
- Rule 3: the static pass records the places (`Items::shared_fn_reads`). `coby` checks them in `eval` for a
  field, an element and `*r` (`fn_through_shared`). `cobc` emits `cb_fn_shared`/`cb_fn_shared_value` (new in
  `cbrt`, scanning a closure box's reference slots for an exclusive path) after such a read and before an
  in-place call.
- A `coby` defect found on the way: `*f()` for an `f` returning a reborrow of an exclusive path faulted
  `diag.aliasing-conflict`. The temporary reference's ancestry lived in a weak table entry that ended with the
  value, before the read was checked. `eval_deref_result` now carries the ancestry with the place
  (`conf.reborrow-through-shared-param-returned`). `cobc` was right.
- A consequence that changes an earlier case: a closure that writes what it captured, passed as
  `ref<fn(…) : R, shared>` and called through it, now faults. `closure_binding_borrowed_as_fn_reference_ok.cb`
  (CHG-0100, a tree walker's visitor) did that; it now takes the writing visitor as
  `ref<fn(…) : R, exclusive>`, the reference that may write, and `conf.shared-closure-binding-exclusive-capture`
  pins the shared form. `cobc` caught it on the first suite run. `coby` did not: it checked closures held as `fn`
  values and missed a closure binding borrowed as one, which it now checks by the place's type. No program outside
  the conformance suite takes a `ref<fn(…)>`.
- Cases: `conf.shared-reaches-exclusive-field-write`, `-ref-returned`, `conf.shared-vec-of-exclusive-refs-write`,
  `conf.shared-match-exclusive-payload-write`, `conf.shared-copy-holding-exclusive`,
  `conf.shared-match-by-value-binds-exclusive`, `conf.shared-closure-exclusive-capture`,
  `conf.shared-closure-binding-exclusive-capture`,
  `conf.shared-copy-of-exclusive-then-owner-write`, `conf.shared-reaches-exclusive-reads`,
  `conf.reborrow-through-shared-param-returned`, `conf.clone-of-exclusive-ref`.
  `conf.exclusive-ref-of-another-thread` now reads through the reference it used to write through: the read
  borrows from `main`'s path, and `[Thread-Affine]` faults it as before.
- Corpus: no struct in `impl/std`, `showcase`, `bench`, `stress/round6`/`round8`/`round9` holds an exclusive
  reference.
- Cost: `cobc` emits nothing new for a program without the shape. The C it generates for the nine `bench`
  programs and the twelve `stress/perfguide` examples is identical to Update 101's, apart from the two new
  declarations in the embedded runtime header, so their timings stand. `coby` reads one atomic flag per field,
  element and `*r` it evaluates.
- `impl/tools/suites.sh` (run `stress/suites/runs/d0218-2`): every target passed under both tools, GCC and
  Clang, and `--all-checks`; `conf.exclusive-ref-of-another-thread` explored to 40 schedules with one outcome. The
  first run (`d0218-1`) failed only on `closure_binding_borrowed_as_fn_reference_ok.cb`, above.
