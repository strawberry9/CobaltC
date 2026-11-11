# CHG-0144 — `read_volatile` and `write_volatile`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0121)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0121
Affects: `spec/20` (1.11.0), `spec/21` §0, the registry (1.42.0), `spec/conformance.md`, the guide §20, both tools

## What changed

- **`spec/20`:** `[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]`.
- **Checker:** two trusted intrinsics (inside `unsafe`), typed for a `rawptr<T>` with `T` not a resource.
- **`coby`:** the access; **`cobc`:** an access through a `volatile` lvalue.
- **Rows:** `conf.volatile-read-write`, `conf.volatile-outside-unsafe-rejected`, `conf.volatile-resource-rejected`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning.
