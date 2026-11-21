# CHG-0232 — The concurrency guarantee (`rule.conc.guarantee`)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0203)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0203
Affects: `spec/19` 1.10.0, `spec/03` 1.4.0, `spec/conformance.md` 3.197.0; `impl/src/interp.rs`

## What changed

- **`spec/19` §4 (new):** `rule.conc.guarantee`, with `[Conc-Interleaving]`, `[Conc-No-Race]`, `[Conc-Defined]` and
  `[Conc-Checks-Omitted]`.
- **`spec/19` §1:** `[Spawn]` binds the parameters itself, so a reference passed is held from the spawn until the
  thread finishes.
- **`spec/03`:** `inv.concurrency-validity` points to the new rule.
- **`coby`:** the reference arguments of `spawn` are held from the spawn (an object of the argument values, ended
  with the thread's body), not only once the new thread first runs.
- **Conformance:** `conf.conc-race-detected`, `conf.conc-disjoint-elements`.

## What did not change

No proposition and no other rule. `cbrt` already held a spawn's references from the spawn, in its argument block.
