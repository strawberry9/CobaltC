# CHG-0210 — `std::json`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0182)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0182
Affects: `spec/21` (4.44.0) §0, §2r; `spec/conformance.md` (3.174.0); the guide; `impl/std/json.cb`, `impl/std/std.cb`, `src/prelude.rs`

## What changed

- **`spec/21` §2r `rule.stdlib.json`:** `[Json-Parse]`, `[Json-Text]`, `[Json-Pretty]`, `[Json-Get]`, `[Json-Set]`, `[Json-Error]`; the listing.
- **`spec/21` §0:** `std::json` in the module table; `Json`, `JsonMember` and the functions in the items table.
- **Rows:** `conf.json-parse`, `conf.json-text`, `conf.json-tree`, `conf.json-errors`.

## Compatibility classification

Additive.
