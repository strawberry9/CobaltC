# CHG-0173 — `read_all`, `read_all_bytes`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0145)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0145
Affects: `spec/21` (4.11.0) §0, §2b; `spec/conformance.md` (3.135.0); the guide §21; `impl/std/io.cb`

## What changed

- **`spec/21` §2b `rule.stdlib.read`:** `[Read-All]` and the two functions' source.
- **`spec/21` §0:** a row in the input/output table; the `std::io` submodule row.
- **`std` (`impl/std/io.cb`):** `read_all_bytes` and `read_all`, written in CobaltC over `stdin_read`; both tools run them as written.
- **Rows:** `conf.read-all`, `conf.read-all-bytes`, `conf.read-all-not-utf8`.

## Compatibility classification

Additive: two new names in `std`, shadowed by a program's own.
