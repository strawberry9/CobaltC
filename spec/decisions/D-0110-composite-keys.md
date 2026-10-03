# D-0110 — Structs and enums of key types are key types

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 proposal P5, option A; D-0041's revisit condition)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0041 (hash tables), D-0062 (sorting)
Affects: spec/21 `rule.stdlib.hashmap`, `rule.stdlib.sort`

## Problem

A key type was an integer, `bool`, `str` or `String`. A grid keyed by
`(x, y)`, a cache keyed by `(name, version)` or a set of enum states
had to be packed into one integer or formatted into a `String` first.
D-0041 named composite keys as its revisit condition; the round-6
programs met it.

## Candidate mechanisms

- **A: a struct whose fields, and an enum whose payloads, are all key
  types (recursively) is a key type.** Its bytes are its parts' in
  order (an enum's variant index first); its order is field by field
  (variant order, then payload). Selected: no syntax, nothing to
  implement by hand, and `Vec::sort` gains the same types.
- **B: a user-supplied hash and equality** (`HashMap::new_by(hash,
  eq)`): more general, but two more closures per map and a new way for
  them to disagree.
- **C: keep the set closed** and document packing.

## Selected design

- **Key types:** an integer type, `bool`, `str`, `String`, and a struct
  (or enum), generic or not, whose every field (payload) type is a key
  type.
- **Bytes:** a struct's are its fields' in declaration order; an enum's
  are its variant index as a `u32`, then its payload's; text inside a
  struct or enum is its length as a `u64`, then its bytes (so the
  bytes decide the value).
- **Order** (`Vec::sort`, `binary_search`): field by field in
  declaration order, each in its own order; an enum by the order its
  variants are declared in, then by payload.
- `[Key-Not-Hashable]`'s message names the field or variant that is
  not a key type.

## Compatibility impact

Additive. Iteration is in insertion order and a hash is never
observable, so no existing output changes.

## Revisit conditions

A key that is a float, an array or a reference (none is proposed:
float equality is not an equivalence, and a reference's identity is
not its value).
