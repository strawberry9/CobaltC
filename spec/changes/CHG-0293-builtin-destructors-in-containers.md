# CHG-0293 — coby: built-in destructors of a handle or guard held in a container

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.conc.join` `[Handle-Destructor]`, `rule.conc.mutex` `[Guard-Drop]`, `spec/07` `[Destroy-Composite]`
Affects: `impl/src/interp/destroy.rs`; `impl/conformance/19-concurrency` (four cases); `spec/conformance.md`

## What changed

- `run_destructor` reads a handle's or guard's value where it lives (`object_value`): a reclaimed object's
  (a `Vec` element's) value is in its cells, and its record held a placeholder, so the handle's destructor
  waited for nothing.
- `destroy_field_value` runs the built-in destructor of a handle or guard held in a field, payload or array
  element (`builtin_destructor`); it ran only a struct's own `drop`.

## What changed for programs

In coby, a thread whose handle was held in a `Vec`, a struct, an `Option` or an array now ends before that
holder does, as the text and cobc always had it; before, the thread outlived its handle, and an access to
what it was lent faulted (`conf.handle-in-vec-waits`, `conf.handle-in-struct-waits`,
`conf.handle-in-option-waits`). A guard held in a struct or a `Vec` now unlocks its mutex
(`conf.guard-in-struct-released`). Found by the model's extension (D-0246).
