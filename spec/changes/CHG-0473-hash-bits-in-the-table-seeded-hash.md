# CHG-0473 — A hash table's cells keep hash bits, and the hash is seeded

Status: implemented (2026-11-20); Update 202
Affects: `spec/21` 4.84.0 (`rule.stdlib.hashmap`: `[Key-Hash]`, the listing of `HashMap`'s bodies);
`impl/std/collections.cb` (`HashMap`); `impl/cbrt/include/cbrt.h` (`cb_hash_bytes`, `cb_hash_more` in place of
`cb_fnv_more`, `cb_hash_start`, `cb_hash_fold`, `cb_hash_seed_cell`, `cb_hash_seed_make`),
`impl/cbrt/src/osrand.c`, `impl/cbrt/tools/exports.txt`; `impl/back/native_map.cb`, `impl/back/gen.cb`,
`impl/back/passes.cb`, `impl/back/header.cb` and `impl/front/prelude.cb` (generated);
`impl/tools/validate_c.py`, `impl/tools/gate.py`, `impl/tools/replay_runtime.py`,
`impl/tests/checks/store_env.py`; `spec/IMPLEMENTATION-NOTES.md` (the runtime contract's table);
`impl/conformance/21-standard-library-semantics/hashmap_many_keys_ok.cb`, `spec/conformance.md` 3.318.0;
`htmlguide/generator/content_runtime.py` (appendix B); `impl/STATUS.md`; `bench/hashmap.cb`,
`bench/hashmap.rs`, `bench/run.sh`, `bench/README.md`, `bench/RESULTS.md`; D-0330

## Why

The owner asked whether CobaltC's `HashMap` is the fastest it can be. It was measured, alone, against Rust's
`std` table, `hashbrown` and `indexmap` (`stress/hashmap_bench`: `u64` and 18-byte `String` keys; a thousand,
a hundred thousand and two million entries; the same keys and the same checksums in every build), and it was
not: D-0330 §1 has what was found, and the owner's choice of what to do about it. `bench/hashcount`, the one
workload with a map, had not shown it: its map holds 5,000 keys and its time is the making of the keys.

## What changes

**The table** (`impl/std/collections.cb`; `spec/21` lists the bodies). `HashMap` has the three fields it had.

- A cell of `slots` is `max_value<usize>()` for empty, as before, or `(h & ~mask) | index`: the entry's index
  below, the upper bits of its hash above.
- An entry's first cell is `h >> slot_shift(n)`, its hash's uppermost bits, where it was `h & mask`.
  `cell_names_home(n)` says whether a cell's hash bits include those (any table of up to 2^32 cells).
- `find` and `seek` take the hash as an argument (their callers need it to write the cell) and call `key_eq`
  only in a cell whose upper bits are the hash's. `entry_in` (new) gives the entry index a slot names.
- `reslot` makes a new vector of cells and fills it from the old one's cells in order; the old vector is then
  dropped. No key is read or hashed, and none is compared (before: every key hashed, and compared with every
  key in its way).
- `unslot` takes each entry's home from its cell (before: from the key, hashed again).
- `swap_remove` hashes the moved entry's key and finds its cell by `(h & ~mask) | last`, with no key compared;
  `remove_slot` lowers the indices under the mask.
- In a table of more than 2^32 cells, `reslot` and `unslot` hash the keys, as they did.

**The hash** (`impl/cbrt/include/cbrt.h`, inline in the generated C). `cb_hash_bytes(p, n)`: the state is the
seed exclusive-ored with a multiple of `n`; `cb_hash_more` takes eight bytes at a time into it, each step a
128-bit product folded (`cb_hash_fold`), then the last eight bytes, or one word made of fewer than eight.
`spec/21` `[Key-Hash]` has it exactly. A struct or enum key's function (`cb_kh_…`, written by the compiler)
starts from `cb_hash_start()` and takes its parts through `cb_hash_more`, where it took them through
`cb_fnv_more`.

**The seed** (`impl/cbrt/src/osrand.c`). `cb_hash_start` loads `cb_hash_seed_cell`; while that is zero it
calls `cb_hash_seed_make`, which takes `COBALTC_HASH_SEED` if set, else eight bytes from the system
(`os_random`: `getrandom` or `/dev/urandom`; `BCryptGenRandom`), else the clock and an address, and stores
the seed with a compare-and-exchange, so threads that race agree. A program with no hash table never calls
it and does not link it.

**The native bodies** (`impl/back/native_map.cb`): `find` for `String` and scalar keys, and the probe inside
`get`, `get_mut`, `contains`, `entry` and `insert`, follow the bodies: the first cell from the upper bits,
the key compared only under a matching cell, the cell written with its hash bits.

**Where a count or a trace must repeat**, the seed is fixed: `bench/run.sh` for its instruction counts (its
times run with a drawn seed), the gate's `lowering` unit, and `impl/tools/replay_runtime.py`, which compares
the traces of two builds. `COBALTC_HASH_SEED` changes no program's result, and the test store's key does not
take it (`store_env.py`).

**`bench/hashmap`** (new): a `HashMap<u64, u64>` of a million entries filled, looked up, updated, half emptied
and summed, and a `HashMap<String, u64>` of 200,000 filled and looked up, against the same program in Rust.

## Measured

This machine (`bench/RESULTS.md` names it), quiet; GCC 12 unless said; nanoseconds an operation, best of three
(`stress/hashmap_bench`; `RESULTS.md` there has every row and Clang's column, which is within a tenth of
GCC's). "Rust" is its `std` table with its own hash (SipHash); `indexmap` and `hashbrown` with `foldhash`,
the fast hash of that kind.

| Operation, entries | Before | After | Rust | `indexmap` | `hashbrown` |
|---|---|---|---|---|---|
| `u64` insert, 2,000,000 | 296 | 174 | 203 | 372 | 159 |
| `u64` insert, 100,000 | 158 | 124 | 101 | 130 | 74 |
| `u64` found, 2,000,000 | 177 | 150 | 113 | 167 | 72 |
| `u64` found, 1,000 | 18.7 | 15.2 | 17.8 | 10.3 | 8.3 |
| `u64` absent, 2,000,000 | 119 | 79 | 43 | 30 | 22 |
| `u64` absent, 100,000 | 38.5 | 24.8 | 25.3 | 15.6 | 13.1 |
| `u64` `entry`, 2,000,000 | 264 | 270 | 164 | 299 | 162 |
| `u64` `swap_remove`, 2,000,000 | 437 | 405 | 215 | 413 | 121 |
| `u64` iteration, 2,000,000 | 2.5 | 2.4 | 5.7 | 2.9 | 5.6 |
| text insert, 2,000,000 | 728 | 334 | 582 | 419 | 516 |
| text insert, 100,000 | 404 | 207 | 165 | 98 | 115 |
| text found, 2,000,000 | 563 | 450 | 449 | 438 | 352 |
| text found, 1,000 | 52.1 | 36.1 | 40.6 | 31.2 | 28.5 |
| text absent, 2,000,000 | 257 | 100 | 96 | 77 | 69 |
| text absent, 100,000 | 121 | 42 | 53 | 46 | 38 |
| text `swap_remove`, 2,000,000 | 1400 | 1121 | 629 | 826 | 627 |

- `bench/hashmap`: 1.74 s and 1,435 million instructions before; 1.26 s and 1,172 million after (Clang: 1.70 s
  and 1,330 million; 1.23 s and 1,042 million). Rust with overflow checks: 1.34 s and 2,342 million.
- The compiler, which keeps its tables in `HashMap`s, writing the C of `showcase/tier3/tinyos.cb`: 1,319
  million instructions where it took 1,338 million; of `impl/cobfmt/main.cb`, 44.4 thousand million where
  45.2. Its C does not depend on the seed (the same file from three runs with three seeds).
- Without `COBALTC_HASH_SEED`, three runs of `bench/hashmap` executed 1,171.59, 1,171.62 and 1,171.66 million
  instructions.

## Tried and not kept

- **Every entry's hash kept in a fourth vector**, the first cell the hash's low bits. Built whole and
  measured: lookups and insertions as in the table above or a little slower, a word more an entry, and
  `swap_remove` on two million `u64` keys 547 ns where it had been 437: moving the entry's hash is one more
  place in memory. With the first cell named by the hash's upper bits the cells hold all that growing and
  gap-closing need, and the vector went.

## What is left

- **Removing a text key** is still twice Rust's time, at every size. The table is not what it costs: in a
  decided program half the time of `swap_remove` on `String` keys is the runtime's records for the key and
  value the body moves out of the vectors (`cb_new`, `cb_move_to`, `cb_raw_move_in` and what they call), which
  `insert`, `entry` and the lookups do not pay, since the compiler writes those natively. A native
  `swap_remove` and `remove` would be the same kind of body.
- **A lookup in a map that fits the cache** takes about twice `hashbrown`'s time: after the hash comes a jump
  on whether the first cell is the one, which the processor cannot guess. Tables that look at a group of
  cells at once have no such jump; D-0330 §4 says what that would take.
- **A lookup that finds its key in a large map** touches three places: the cell, the key, the value. Keys and
  values in one vector would make it two (D-0330 §4).

## Checked

- `conf.hashmap-many-keys` (new): a `HashMap` and a list of pairs through the same 20,000 random steps for
  `u64`, `String` and struct keys, compared entry by entry 200 times; the same output in the default form,
  with every check on (`--all-checks`) and in the bootstrap form, under GCC and under Clang, and with several
  seeds.
- `stress/hashmap_bench`: 30 phases of up to four million operations each print the same checksums before and
  after, under both C compilers, and the same as five Rust tables.
- The compiler built by itself in two stages (the runtime's stamp changed) and the tools built by it; the
  prelude summary is current.
- `spec/21`'s listing was brought to the source by a script (`stress/hashmap_bench/sync_spec_listing.py`);
  `speccheck.py`, `runtime_contract.py --check`, `sigcheck.py` (the two new exports), `trusted_table.py`,
  `card.py --check`, `store_env.py`, the guide's check of appendix B against `cbrt.h`.
- The gate's run is named in the commit's message.
