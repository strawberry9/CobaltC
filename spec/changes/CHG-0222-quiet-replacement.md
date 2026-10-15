# CHG-0222 — Quiet types and `[Write-Quiet-Replace]`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0194)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0194
Affects: `spec/05` 1.5.0, `spec/11`, `spec/examples.md`, `spec/registry/diagnostics.md` 1.49.0, `spec/conformance.md` 3.187.0; `impl/`

## What changed

- **`spec/05`:** `quiet(τ)`; `[Write-Resource-Overwrite-Rejected]` requires `¬quiet(type(a,Σ))`; new
  `[Write-Quiet-Replace]`. **`spec/11`:** `[Assign-Reestablish]`'s note. **`spec/examples.md`:**
  `ex.no-silent-resource-overwrite` uses a `File`. **Registry:** `diag.overwrite-of-live-resource`'s requirement.
- **`coby`/type checker:** `quiet_ty`; `write_evaluated`; the pending value destroyed on an overwrite fault.
- **`cbrt`:** `cb_write_quiet`. **`cobc`:** quiet places written through it.
- **Conformance:** `conf.quiet-replace`, `conf.overwrite-fault-destroys-pending`, `conf.quiet-not-channel`; four
  overwrite cases moved to a type declared `resource`.
- **Guide:** §07's overwrite note and example, §18's example, the diagnostics table.

## What did not change

`overwrite`, `replace` and `drop`. Every program that compiled before compiles and runs as before, except that
writes which faulted over a quiet value now replace it.
