# D-0041 — Hash Tables: `HashMap<K, V>` and `HashSet<K>`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17, §23
Depends on: D-0010, D-0024, D-0028, D-0032, rule.stdlib.vec, rule.stdlib.string
Affects: rule.stdlib.hashmap (new), rule.stdlib.prelude, rule.stdlib.vec

## Problem

After `Vec`, a map is the container programs reach for most. `std` had
none: the Tier 3 `hash_map` showcase spends 150 lines on one, keyed by
`str` only. A generic map needs to hash and compare a `K`, and CobaltC
has no bound system (D-0010) through which a body could ask a `K` to do
either.

## Constraints

- No new tokens and no bound system.
- `std` stays written in CobaltC wherever the language allows (spec/21's
  purpose: the library is evidence the language suffices).
- The same program prints the same thing under `coby` and `cobc`.

## Candidate mechanisms

Hashing: **(1) two std-only intrinsics, `key_hash` and `key_eq`, for a
fixed set of key types, checked at each call** (selected); (2) hash and
equality functions supplied by the caller and stored in the map;
(3) `String` keys only.

Removal: (a) constant-time, moving the last entry into the gap;
(b) order-preserving, linear time; **(c) both, as `remove` and
`remove_ordered`** (selected, the owner's choice). Also: `HashSet`
(selected), the name `HashMap` over `Map` (selected).

## Selected design

- Key types: every integer type, `bool`, `str`, `String`. Any other `K`
  in a call of a `HashMap`/`HashSet` function is `diag.type-mismatch`,
  checked where the call is (a type parameter at each instantiation),
  as `String::append`'s `T` is.
- `key_hash` is FNV-1a-64 over the key's bytes (an integer's two's
  complement, least significant byte first; a `bool`'s byte; text's
  UTF-8); `key_eq` compares the bytes. Both are std-only, like
  `append_native`.
- `HashMap` is CobaltC: keys and values in two `Vec`s in insertion
  order, and an open-addressing table (`Vec<usize>` of entry indices,
  linear probing, at most three-quarters full, doubling) with
  backward-shift deletion, so no tombstones.
- API: `new`, `len`, `insert` (the old value back), `entry` (insert if
  absent, then the value), `get`, `get_mut`, `contains`, `remove`,
  `remove_ordered`, `key_at`, `value_at`. `HashSet` wraps a
  `HashMap<K, bool>`: `new`, `len`, `insert`, `contains`, `remove`,
  `remove_ordered`, `at`.
- Iteration by index (`i < len`), in insertion order; `remove` moves the
  last entry into the removed one's place.

## Rejected alternatives

- **(2) caller-supplied functions:** two functions written and passed
  for every map, stored in it, and an indirect call per probe.
- **(3) `String` keys only:** integer keys are the other common case.
- **Tombstones for order-preserving removal:** they leave holes in the
  entry numbering, which index iteration (`key_at(i)`, `i < len`)
  cannot skip without an iterator protocol the language lacks.
- **Float keys:** NaN is not equal to itself, and `0.0 == -0.0` although
  their bytes differ.

## Semantic rationale

Only `[Key-Hash]` and `[Key-Eq]` are native; every other step is
ordinary CobaltC, so ownership (the map owns keys and values, and
destroys a replaced or unused one), borrowing (`get` returns a
reference into the map) and faults follow the existing rules with no
new case. `Vec::replace` and `Vec::remove_at` are private to `std`,
built as `Vec::pop` and `Vec::push` are.

## Usability

    HashMap<String, u32> counts = HashMap::new();
    *HashMap::entry(&mut counts, word, 0) += 1;
    for (usize i = 0; i < HashMap::len(&counts); i += 1)
    {
        printf("%s %d\n", HashMap::key_at(&counts, i), *HashMap::value_at(&counts, i));
    }

## Explainability

"A `HashMap` works for integer, `bool` and text keys; it iterates in
insertion order; `remove` is fast and moves the last entry into the
gap, `remove_ordered` keeps the order."

## Implementation-feasibility

`coby` reads a key's bytes from the value (a `String`'s from its
buffer); `cobc` emits `cb_hash_bytes`/`cb_bytes_eq` (inline in
`cbrt.h`) over the key's storage. The front end checks `K` in
`check_user_call`, beside `append`'s and `parse`'s checks.

## Compatibility impact

Extension. `HashMap` and `HashSet` are items of `std`; a program's own
of the same name takes precedence (D-0024), as the showcase's `map::Map`
never collided.

## Prior-art status

Python `dict` (insertion order); Rust `indexmap` (`swap_remove`,
`shift_remove`); Go `map` (built-in key types); FNV-1a (Fowler, Noll,
Vo).

## Invariant traceability

None new: the map is ordinary CobaltC over `Vec`, and the intrinsics
only read.

## Revisit conditions

- Composite keys (structs, enums) — would want per-field hashing.
- A randomly seeded hash (SipHash) against collision attacks: iteration
  order does not depend on the hash, so it would change no output.
- Iterators, which would let order-preserving removal use tombstones.
