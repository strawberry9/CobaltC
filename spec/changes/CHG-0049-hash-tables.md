# CHG-0049 — Hash tables: `HashMap<K, V>`, `HashSet<K>`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0041
Affects: rule.stdlib.hashmap (new), rule.stdlib.prelude, the cases listed below

## Problem / motivation

D-0041: `std` had no map.

## What changed

- **`spec/21` 3.12.0:** §2g `rule.stdlib.hashmap` (`[Key-Bytes]`,
  `[Key-Hash]`, `[Key-Eq]`, `[Key-Not-Hashable]`, and the CobaltC
  bodies); the scope paragraph and §0's table list the new types.
- **`spec/registry/diagnostics.md` 1.18.0:** `diag.type-mismatch` names
  `[Key-Not-Hashable]`.
- **`spec/conformance.md` 3.35.0:** the cases below.
- **`spec/02-schema.md` 1.0.25:** §5's "in use" ranges.
- **Implementations:**
  - `std` (src/prelude.rs): `HashMap`, `HashSet`, and the private
    `Vec::replace`, `Vec::remove_at`;
  - typecheck: `key_hash`, `key_eq` (std-only); `K` checked at each
    call of a `HashMap`/`HashSet` function;
  - `coby`: the two intrinsics over a key's bytes;
  - `cobc`/cbrt: `cb_hash_bytes`, `cb_bytes_eq`.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.hashmap-basic`, `conf.hashmap-insert-replaces`,
`conf.hashmap-entry-counts`, `conf.hashmap-remove-swaps`,
`conf.hashmap-remove-ordered`, `conf.hashset-basic`,
`conf.hashmap-float-key-rejected`, `conf.hashmap-struct-key-rejected`,
`conf.hashmap-generic-key-checked`, `conf.hashmap-fields-private`,
`conf.key-hash-std-only`, `conf.hashmap-output` (file case
`21-standard-library-semantics/hashmap_ok.cb`, output pinned).

## Prior-art status

See D-0041.

## Revisit conditions

See D-0041.
