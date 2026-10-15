# CHG-0257 — What a shared reference reaches is read-only, at every depth

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-12; D-0218)
Governed by: `CobaltC_Master_Instructions.md` §6, §17
Depends on: D-0218
Affects: `spec/08` 1.5.0, `spec/09` 1.6.0, `spec/12` 1.23.0, `spec/15` 1.13.0, `spec/16` 1.23.0,
`spec/registry/diagnostics.md` 1.52.0, `spec/examples.md` 3.18.0, `spec/IMPLEMENTATION-NOTES.md`,
`spec/conformance.md` 3.212.0, `spec/02-schema.md` 1.0.76; `impl/src/typecheck.rs`, `impl/src/views.rs`,
`impl/src/interp.rs`, `impl/src/lib.rs`, `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`; twelve new cases in `impl/conformance/08-alias-validity/`, `12-type-system/`,
`15-function-semantics/` and `16-aggregates/`; `19-concurrency/exclusive_ref_of_another_thread_ok.cb` and
`15-function-semantics/closure_binding_borrowed_as_fn_reference_ok.cb` revised; `impl/tests/conformance.rs`
(covers line)

## What changed

- `spec/08`: `rule.alias.read-through-shared` — `[Read-Ref-Through-Shared]`, `[Read-Exclusive-Through-Shared]`,
  `[Read-Fn-Through-Shared]`; `rule.alias.borrow`'s interface sentence holds at every depth; `[Thread-Affine]`'s
  prose notes the read and the static write rejection.
- `spec/09`: `[Ref-Deref-Place]`: `*e` of a reference read through a shared path is a shared place.
- `spec/12`: `[T-Read-Through-Shared]`; `clone-holds` excludes exclusive references and types holding one.
- `spec/15` §6: a `fn` value read or called through a shared reference (`[Read-Fn-Through-Shared]`).
- `spec/16`: `[Match-Exclusive-Through-Shared]`.
- `spec/registry/diagnostics.md`: `diag.borrow-exceeds-source` (now also dynamic) and `diag.write-through-shared`
  name the new rules.
- `spec/examples.md`: `ex.shared-is-read-only` shows the write through a held exclusive reference.
- `spec/IMPLEMENTATION-NOTES.md` §4: how the tools carry out the reads.
- Tools: the static pass (shared by `coby` and `cobc`) applies rules 1 and 2. Rule 1's borrow is a front-end
  rewrite to `&*e`. Rule 3 is checked by `coby`'s evaluator and by `cbrt`'s `cb_fn_shared`/`cb_fn_shared_value`,
  which `cobc` emits. Separately, a `coby` defect is fixed: a temporary reference's ancestry was lost before
  `*f()` was checked.

## What did not change

Every program that only reads through a shared reference, and every program that owns what it writes, behaves as
before. The accepted set shrinks by programs that wrote, or could write, through an exclusive reference reached
through a shared one, including a closure that writes its captures called through `ref<fn(…) : R, shared>`
(take it as `ref<fn(…) : R, exclusive>`); the corpus outside the conformance suite has none.
