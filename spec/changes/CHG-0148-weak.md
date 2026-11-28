# CHG-0148 — `Weak<T>`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0125)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0125
Affects: `spec/21` §0 and §3 (3.49.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §3:** the `Rc` source with a `weak` count and an `Option<T>` value; `Weak`, `Rc::downgrade`, `Weak::upgrade`, `Weak::clone`, `Weak::drop`.
- **`std`:** the same, written in CobaltC.
- **Rows:** `conf.weak-upgrade-while-alive`, `conf.weak-upgrade-after-end`, `conf.weak-keeps-box`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
