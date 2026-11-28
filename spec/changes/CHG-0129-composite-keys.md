# CHG-0129 — Structs and enums of key types are key types

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0110)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0110
Affects: spec/21 `rule.stdlib.hashmap`, `rule.stdlib.sort`; `spec/conformance.md`

## What changed

- **Rules:** a key type includes a struct or enum of key types;
  `[Key-Bytes]` gives its bytes; `[Sort]` orders it field by field.
- **Rows:** `conf.hashmap-struct-key`, `conf.hashmap-enum-key`,
  `conf.sort-struct-key`, `conf.hashmap-key-float-field-rejected`;
  `conf.hashmap-struct-key-rejected` now uses a struct with an `f32` field.

## Compatibility classification

Additive.
