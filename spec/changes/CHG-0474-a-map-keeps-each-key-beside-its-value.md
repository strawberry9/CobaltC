# CHG-0474 — A map keeps each key beside its value

Status: implemented (2026-11-20); Update 203
Affects: `spec/21` 4.85.0 (`rule.stdlib.hashmap`: the listing of `HashMap`'s and `HashDrain`'s bodies; `Vec::replace`
and `Vec::remove_at` gone), `spec/14` 1.48.0 (`rule.control.foreach`: `[Foreach-Borrowed-Entry]`, §6's row for a
hidden binding); `impl/std/collections.cb` (`HashMap`, `HashEntry`, `HashDrain`); `impl/front/parser.cb`,
`impl/front/each.cb`, `impl/front/typecheck.cb` (`foreach` over a map); `impl/back/native_map.cb`,
`impl/back/native_misc.cb`, `impl/back/passes.cb`, `impl/back/strings4.cb`; `impl/cbrt/include/cbrt.h` (`cb_hash_bytes_drawn`),
`impl/back/header.cb` and `impl/front/prelude.cb` (generated); `impl/tools/validate_c.py`;
`spec/IMPLEMENTATION-NOTES.md` (the runtime contract's table); `CARD.md`;
`impl/conformance/14-control-flow/foreach_map_entry_ok.cb`, `spec/conformance.md` 3.319.0;
`htmlguide/generator/content_runtime.py` (appendix B); `impl/STATUS.md`; `bench/README.md`, `bench/RESULTS.md`;
D-0331

## Why

`CHG-0473` left three things of `HashMap` undone, and the owner asked for them as the next Update: removing a
key (two to five times Rust's time), the three places in memory a lookup that finds its key touches, and a
lookup in a map that fits the cache. D-0331 has what was decided and, for the third, what the measurements
said before anything was built.

## What changes

**The entries** (`impl/std/collections.cb`; `spec/21` lists the bodies). `HashMap<K, V>` has
`Vec<HashEntry<K, V>> entries` and `Vec<usize> slots`; `HashEntry` is a `std`-private struct of `key` and
`value`. The table is D-0330's, unchanged.

- `key_at`, `value_at`, `get`, `get_mut`, `entry`: a reference to one field of `entries[i]`.
- `insert` pushes one entry; a present key's value is replaced by `put_value` (new, `std`-private), which
  moves the entry out of its place and writes it back with the new value. `replace` on the entry's field was
  tried first: `replace<T>` of a function value is not decided, and two programs with maps of functions
  stopped being decided programs.
- `swap_remove` takes the entry with `Vec::swap_remove`, `remove` with `Vec::remove`; both destroy the key and
  return the value. `Vec::replace` and `Vec::remove_at`, which only the map called, are deleted.
- `clear` and a map's end destroy the entries in order, a key before its value.
- `HashDrain` (a consuming `foreach`): `take_key` takes the entry out whole and leaves its value at the
  front of the emptied place; `take_value` takes it from there. The holder's fields are unchanged.

**`foreach` over a map** (`impl/front/parser.cb`, `each.cb`, `typecheck.cb`; `spec/14`).

- A loop with two or three names over `&mut c`, or over a `c` written without `&`, has a hidden binding
  `$p = $each_slot(holder, $i)` at the head of each turn. For a map looped over through a reference it is the
  entry, borrowed once in the loop's mode (`Vec::index_exclusive(&mut (*$c).entries, $i)`, or
  `Vec::index_shared`), and the key is `&(*$p).key`, the value `&mut (*$p).value` (or `&`). For any other
  collection it is the position, which nothing reads.
- A loop over `&c` of a map has no `$p`: its key and value are `&Vec::index_shared(&(*$c).entries, $i).key`
  and `.value`.
- The type checker's facts for a map's binders name `entries.key` and `entries.value` where they named `keys`
  and `values`.
- The back end counts `$each_slot(d, …)` among the uses of a consuming loop's holder that keep it out of the
  run's records (`impl/back/strings4.cb`), as `$each_take` is; without it the holder of a two-name consuming
  loop had a record.

**The bodies the compiler writes** (`impl/back/native_map.cb`).

- `find` (for `String` and for integer or `bool` keys), and the probe inside `get`, `get_mut`, `contains`,
  `entry` and `insert`, read `entries[i].key`; an element's checked access is the entry's.
- `get` and `get_mut` return the value's reference as `std`'s text forms it: the entry's borrow
  (`cb_elem_borrow_here`), the field's from it (`cb_borrow`), sent as the result's one reference
  (`cb_send_tok_datum`); `entry` the same, sent as a reference (`cb_send_ref`). In a program lowered without
  paths these are nothing, as the calls they replace were.
- `insert` and `entry` write a new entry in one place and grow one vector.
- `swap_remove` and `remove` (new, `native_map_remove`): for integer, `bool` and `String` keys with a plain
  value, in a decided program. A table of more than 2^32 cells calls `std`'s body, emitted beside it.
- The hash: `insert` and `entry` call `cb_hash_start` first, and every body hashes with
  `cb_hash_bytes_drawn`, the seed as it stands. A lookup or a removal does not draw the seed (D-0331 §2 C).
- Gone from a lookup: the test of `slots.len` before `len − 1`, and the second bounds test of the found index.

## Measured

This machine (`bench/RESULTS.md` names it), quiet; GCC 12; nanoseconds an operation, best of three
(`stress/hashmap_bench/hm.cb`; `RESULTS.md` there has every row and Clang's column). "Before" is Update 202,
run again beside this build. The Rust columns are `CHG-0473`'s: Rust's `std` table with its own hash,
`indexmap` and `hashbrown` with `foldhash`.

| Operation, entries | Before | After | Rust | `indexmap` | `hashbrown` |
|---|---|---|---|---|---|
| `u64` `swap_remove`, 1,000 | 83.9 | 40.4 | 33.9 | 31.5 | 17.2 |
| `u64` `swap_remove`, 100,000 | 123.4 | 75.1 | 51.1 | 75.6 | 32.4 |
| `u64` `swap_remove`, 2,000,000 | 417.6 | 269.8 | 215.3 | 413.1 | 120.8 |
| text `swap_remove`, 1,000 | 410.9 | 124.7 | 147.5 | 80.2 | 83.6 |
| text `swap_remove`, 100,000 | 793.6 | 377.8 | 316.7 | 373.3 | 291.8 |
| text `swap_remove`, 2,000,000 | 1177.4 | 776.1 | 628.9 | 826.3 | 626.7 |
| `u64` found, 1,000 | 15.3 | 14.9 | 17.8 | 10.3 | 8.3 |
| `u64` found, 100,000 | 50.2 | 40.0 | 29.6 | 35.2 | 14.1 |
| `u64` found, 2,000,000 | 161.6 | 119.3 | 112.7 | 167.0 | 71.9 |
| `u64` absent, 1,000 | 20.0 | 18.8 | 18.4 | 7.1 | 6.1 |
| `u64` absent, 2,000,000 | 79.1 | 76.7 | 43.4 | 30.4 | 22.2 |
| `u64` `entry`, 100,000 | 72.6 | 65.4 | 36.5 | 54.4 | 19.3 |
| `u64` `entry`, 2,000,000 | 285.3 | 231.5 | 163.9 | 298.5 | 161.7 |
| `u64` insert, 1,000 | 61.8 | 54.4 | 81.8 | 49.8 | 42.1 |
| `u64` insert, 100,000 | 125.3 | 130.0 | 100.7 | 129.5 | 73.8 |
| `u64` insert, 2,000,000 | 174.1 | 164.6 | 202.6 | 372.2 | 159.2 |
| `u64` iteration, 1,000 | 1.1 | 0.7 | 3.3 | 0.4 | 3.5 |
| text found, 100,000 | 247.5 | 224.3 | 228.0 | 216.2 | 170.1 |
| text found, 2,000,000 | 461.7 | 431.3 | 449.2 | 438.1 | 352.0 |
| text insert, 2,000,000 | 340.0 | 348.1 | 582.4 | 419.3 | 515.7 |

- Under Clang the lookups gain more: a `u64` key found among a thousand in 12.6 ns (15.8 before), among a
  hundred thousand in 34.1 (46.3), among two million in 104.8 (168.4).
- A lookup alone (a thousand `u64` keys, each found a hundred thousand times): 62 instructions and 39 cycles a
  lookup under GCC where Update 202's took 77 and 47; under Clang 38 and 32.
- `bench/hashmap`: 1.09 s and 1,018 million instructions where Update 202 took 1.26 s and 1,172 million
  (Clang: 1.07 s and 885 million; 1.23 s and 1,042 million). Rust with overflow checks: 1.37 s and 2,342
  million. `bench/hashcount` executes 599 million instructions where it executed 605 million.
- `bench/RESULTS.md` is measured again from this build. In this run every program, Rust's too, took about a
  tenth longer than in Update 202's run (the same instructions; the machine's state), so the ratios are what
  compares, not the seconds.
- With every check on, `bench/hashcount` executes 8,528 million instructions where it executed 8,534 million;
  in the bootstrap form 129.3 thousand million where 126.6, `std`'s bodies there making an entry before they
  push it.

## What it costs

- **Memory**: an entry is padded where the value is smaller than the key's alignment. A `HashSet<u64>`'s
  entry, a map's to `bool`, takes 16 bytes where it took 9; a `HashSet<String>`'s 32 where 25; a
  `HashMap<u64, u32>`'s 16 where 12.
- **One kind of loop is no longer decided**: a loop over `&mut m` of a map that keeps a key's reference past
  its turn. It is accepted and runs as it did, lowered with its paths recorded (D-0331 §3).
- Inserting a hundred thousand `u64` keys takes 130 ns where it took 125, and two million texts 348 where 340.
- In the bootstrap form `bench/hashcount` executes 2% more instructions.

## Tried and not kept

D-0331 §4 has the figures, from the candidates written in C (`stress/hashmap_bench/proto.c`).

- **A table of groups**: seven tags and seven cells to a 64-byte group, compared a group at a time, an
  overflow count in place of tombstones. One jump in twenty mispredicted where the plain probe mispredicts one
  in two, and slower anyway on this processor: 13.5 ns to find a key among a thousand where the plain table
  with pairs takes 11.9, 121 among two million where it takes 78. Faster only for absent keys in the cache
  (10.4 against 17.5).
- **The cells in the order of their hash bits**: absent keys a quarter faster, insertion a tenth to a quarter
  slower.
- **Two or four cells looked at without a jump**: slower at every size.
- **The value's memory fetched early**, the two vectors kept: no gain.

## What is left

- **A lookup in a map that fits the cache** under GCC is where it was within a few percent (14.9 ns against
  `hashbrown`'s 8.3); what a table that keeps insertion order can do there is about `indexmap`'s 10.3, which
  Clang's 12.6 is near. D-0331 §4 says why the group table was not the way on this machine.
- **A lookup of an absent key** takes two to three times `hashbrown`'s time: the probe walks to an empty cell,
  and how far is what the processor cannot guess. The ordered cells of D-0331 §4 would shorten it at
  insertion's cost.
- **Removal** takes a fifth to a quarter longer than Rust's `std` table for text keys and for two million
  integer keys, half as long again for a hundred thousand, and twice `hashbrown`'s time for integer keys: the
  last entry's cell is found by hashing its key again.
- **A set's entries** carry a `bool` and its padding (D-0331 §3).
- **`swap_remove` and `remove` of a value that owns something**, and in the forms that check at run time, are
  `std`'s bodies.

## Checked

- `conf.foreach-map-entry` (new): loops over `&mut m`, over a map that is an exclusive reference, over `&m`,
  consuming loops left early, `clear`, with two-name loops over a `Vec` and a set beside them; the same output
  as Update 202 printed, in the default form, with every check on and in the bootstrap form, under GCC and
  under Clang.
- `conf.hashmap-many-keys`: the same output in the three forms under both C compilers; its `swap_remove` and
  `remove` are the compiler's bodies in the default form and `std`'s in the other two.
- `stress/hashmap_bench/hm.cb`: 30 phases of up to four million operations print the checksums Update 202
  printed, under both C compilers.
- The conformance cases of `spec/14` and `spec/21` under GCC and Clang.
- The compiler built by itself in two stages (the runtime's stamp changed) and the tools built by it; the
  prelude summary is current.
- `spec/21`'s listing brought to the source by a script; `speccheck.py`, `runtime_contract.py`,
  `trusted_table.py`, `card.py --check`, `store_env.py`, `sigcheck.py`, the guide's check of appendix B
  against `cbrt.h`.
- The gate's first run was red in three places, each put right: `conf.hashmap-fields-private` named the field
  `keys` (it names `entries`); `cost_rows` found a record for the holder of a two-name consuming loop
  (`$each_slot`, above); the sweep of decided programs lost two with maps of function values (`put_value`,
  above).
- The gate's run is named in the commit's message.
