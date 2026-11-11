# CHG-0161 — `HashSet::clear`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0134)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0134
Affects: `spec/21` §0, §2g; `spec/conformance.md`; the guide; `impl/src/prelude.rs`

## What changed

- **`HashSet::clear<K>(ref<HashSet<K>, exclusive> s)`:** every key removed, as `HashMap::clear` removes every entry.
- **Rows:** `conf.hashset-clear`.

## Compatibility classification

Additive: a new name in `std`, shadowed by a program's own.
