# CHG-0421 — `std`'s containers allocate once for what they know they will hold

Status: implemented (2026-11-10), step 1 of the allocation plan (the agent's proposal of 2026-11-10 on the owner's speed test; the owner: "implement the proposal in full")
Affects: `impl/std/collections.cb` (`HashMap::new`, `HashMap::clone`, `Vec::clone`, `Vec::filled`), `spec/21` 4.81.1
(the `HashMap::new` listing; a non-normative change-log line), `impl/selfhost/front/prelude.cb` (regenerated). No rule
changes. No program's outcome, output or fault changes.

## What changes

1. **`HashMap::new`** reserves its eight slots before it fills them: one allocation instead of two and a copy. The
   eight slots stay eight, so an empty map still allocates and `alloc_failure` can occur where it could (D-0091's
   `clear` returns to the same size).
2. **`HashMap::clone`** copies the keys, the values and the slot table as they are: the same entries at the same
   indices, so the slots are valid in the copy. Before, it inserted entry by entry into a new map, hashing each key
   again and growing the three vectors by doubling. D-0116's contract ("a map or set of copies of the keys and
   values, in the same insertion order") holds by construction. `HashSet::clone` is `HashMap::clone`.
3. **`Vec::clone`** and **`Vec::filled`** reserve their length first, as `Vec::from_slice` already did.

## Why

`cbc`'s profile on the Lisp interpreter (Update 175): `HashSet::clone`, rebuilding by insert, was 7.0% of its
cycles, almost all of it the probe snapshot's (`CHG-0424`); `Vec::grow` 11.6%; `cbc` made 2.6 times `cobc`'s
reallocations. Every CobaltC program that clones a map, a set or a vector gains, not only `cbc`.

## Measured

A prototype of this change alone, against the Update 175 `cbc` on the same machine: `complex_lisp.cb` −6.1%,
`large_200.cb` −3.2%; reallocations 1.52 M → 0.71 M (Lisp) and 2.46 M → 1.32 M (`large_200`); bytes requested
999 → 841 MB (Lisp). The C `cbc` and `cobc` write is unchanged except inside the four functions' own bodies. With the other three records,
`cbc` on the Lisp program makes 8.59 M allocations (12.87 M before) and 0.70 M reallocations (1.52 M), requesting
633 MB (999 MB); `cobc` makes 11.73 M.
