# D-0331 — A map keeps each key beside its value

Status: ACCEPTED (2026-11-20). Decided by the owner: `CHG-0473` ended with three things left of `HashMap`
(removing a key; a lookup in a map that fits the cache; the three places in memory a lookup that finds its key
touches), and the owner asked for the three as the next Update. What was measured changed the second: §2 C and
§4 say how. Implemented by `CHG-0474`
Kind: Decision (`spec/02-schema.md`); `std`'s normative bodies, the expansion of `foreach` over a map, and
bodies the compiler writes in place of `std`'s, none of which a program observes
Depends on: D-0330 (the table's cells), D-0041 (hash tables written in CobaltC), D-0042 and D-0044 (`foreach`),
D-0134 (`remove`, `swap_remove`), D-0274 (bodies written by the compiler), D-0315 (decided programs)
Affects: `spec/21` `rule.stdlib.hashmap` (the bodies of `HashMap` and `HashDrain`), `spec/14`
`rule.control.foreach` (`[Foreach-Borrowed-Entry]`, §6's row for a hidden binding); `impl/std/collections.cb`;
`impl/front/parser.cb`, `impl/front/each.cb`, `impl/front/typecheck.cb`; `impl/back/native_map.cb`,
`impl/back/native_misc.cb`; `impl/cbrt/include/cbrt.h` (`cb_hash_bytes_drawn`). No program's meaning changes.

## 1. Problem

After D-0330 a `HashMap` was a table of cells beside two vectors, `keys` and `values`. `CHG-0473` named what
that left, measured against Rust's tables on the same machine:

- **Removing a key** took two to five times Rust's time at every size, for integer keys as for text. The table
  was not the cost. `swap_remove` and `remove` were `std`'s bodies, which take a key and a value out of two
  vectors and put two others in their places; each of those moves is, in the generated C, a record made and
  ended for the thing moved. `insert`, `entry` and the lookups did not pay this, because for integer, `bool`
  and `String` keys the compiler writes those bodies itself.
- **A lookup that finds its key in a large map** read three places far apart: the cell, `keys[i]`, `values[i]`.
- **A lookup in a map that fits the cache** took about twice `hashbrown`'s time. `CHG-0473` put this down to
  the jump on whether the key's first cell is the right one, and named the remedy: a table that looks at a
  group of cells at once.

## 2. Decision

**A. An entry is a key and its value side by side.** `HashMap<K, V>` holds `Vec<HashEntry<K, V>> entries`,
`HashEntry` a `std`-private struct of `key` and `value`, where it held `Vec<K> keys` and `Vec<V> values`. The
table and its cells are D-0330's. A lookup that finds its key has the value in the same place.

1. `key_at`, `value_at`, `get`, `get_mut` and `entry` give a reference to one field of an entry.
2. `insert` replaces a present key's value by moving the entry out of its place and writing it back with
   the new value (`put_value`; `replace` on the field would leave a map of function values undecided);
   `swap_remove` takes its entry with `Vec::swap_remove` and `remove` with `Vec::remove`. `Vec::replace` and `Vec::remove_at`, `std`-private
   helpers only the map called, are gone.
3. **`foreach` over a map.** A loop over `&mut m` gives a shared reference to each key and an exclusive one to
   its value. While those were elements of two vectors they were two borrows of two fields. As two fields of
   one entry they come from one borrow: the loop borrows the entry once, exclusively, into a hidden binding
   (`$p`), and the key and the value are references to its two fields (`spec/14` `[Foreach-Borrowed-Entry]`).
   The same for a map that is itself an exclusive reference. A loop over `&m` needs no such binding: two
   shared borrows of an entry may stand together.
4. A consuming `foreach` takes an entry whole when it takes the key; the value waits at the front of the
   entry's emptied place until the loop takes it, a moment later. The holder (`HashDrain`) has the fields it
   had.
5. A map's entries are destroyed entry by entry, each key before its value, where `clear` destroyed the values
   and then the keys and a map's own end the keys and then the values. A key type has no destructor of its own
   (D-0271), so the only order a program can see is the values', and that is the entries' order as before.

**B. `swap_remove` and `remove` written by the compiler**, for integer, `bool` and `String` keys and a plain
value, in a decided program: the probe; the entry taken out (the last entry moved into its place and its cell
found by its hash and its index, or the later entries moved down and their cells' indices lowered); the gap in
the table closed by the cells' own hash bits; the removed key's buffer freed; the value returned. Nothing is
recorded for the key or the value on the way. A table of more than 2^32 cells takes `std`'s body, as do the
other forms of a program and every other key or value type.

**C. A lookup in a map that fits the cache: the group table is not taken; the lookup loses what it did not
need.** Before any of it was built in the compiler, the candidates were written in C with the compiler's hash
and the benchmark's keys and timed on the machine the project measures on (§4 has the figures):

1. the table as it is, written plainly in C, finds a key among a thousand in 11.7 ns where the compiler's body
   took 15.2: a quarter of the time was not the table's;
2. a table of groups (seven tags and seven cells to a 64-byte group, an overflow count in place of
   tombstones) found keys more slowly at every size, and absent keys faster in the cache and no faster out of
   it;
3. the same cells probed in order of their hash bits, so that a lookup of an absent key stops early, found
   absent keys a quarter faster and inserted a tenth to a quarter slower.

So the table stays, and the compiler's lookup is made the plain one:

- A lookup or a removal does not test whether the run's seed is drawn. Until the first hash that places an
  entry, no map in the program has an entry, and an empty table gives the same answer wherever a probe
  starts; `insert` and `entry` draw the seed before they hash (`cb_hash_start`), and every body then hashes
  with the seed as it stands (`cb_hash_bytes_drawn`). The call that draws the seed is no longer in a
  lookup's text, and the C compiler no longer saves registers around it.
- The found entry's index is checked against the vector's length once, in the probe, and not again for the
  value, which is now the same element.
- `len − 1` for the mask is not tested for overflow: the table has eight cells or more.

**D. In the forms that check at run time**, a body the compiler writes forms a reference to a value as
`std`'s text does: the entry's borrow, then the field's from it. An element's access is the entry's. The
reference is to a field inside a vector's element, so a borrow of the value's bytes alone, as the bodies made
of an element of `values`, would be an object the vector does not know of.

## 3. Consequences

- No program changes meaning. Iteration order, `key_at`, `value_at`, what `remove` and `swap_remove` leave,
  and every fault are as they were.
- **Memory.** An entry takes the size of the struct: where the value is smaller than the key's alignment the
  pair is padded. A `HashMap<u64, u32>` entry is 16 bytes where the two vectors took 12; a `HashSet<u64>`,
  which is a map to `bool`, 16 where it took 9; a `HashSet<String>` 32 where it took 25. A map whose key and
  value have the same alignment takes what it took. A set that kept no value would take the key alone (§4).
- A loop over the keys alone of a map with large values steps over the values.
- `foreach (k, v in &mut m)` holds the entry exclusively for the turn. No program is newly refused and none
  behaves otherwise, but one kind is no longer decided: a loop over `&mut m` that keeps a key's reference past
  its turn (it pushes `k` into a vector that outlives the loop). The kept reference reaches `entries`, which
  the next turn borrows exclusively; while the keys and the values were two vectors the two never met. Such a
  program is lowered with its paths recorded (`cobc -v` says where) and runs as it did, more slowly. A loop
  over `&m` that keeps its keys is decided as before.
- A loop of two names over `&mut c`, or over a `c` written without `&`, has one more hidden binding, the
  position for any collection but a map; the C compiler removes it.
- The runtime's interface gains `cb_hash_bytes_drawn`.
- In the checked forms the compiler's `get`, `get_mut` and `entry` make two paths (the entry's, the field's)
  where they made one.

## 4. Alternatives considered

Timed in C (`stress/hashmap_bench/proto.c`: GCC 12 `-O2`, the Core i7 920 `bench/RESULTS.md` names, quiet;
`u64` keys, nanoseconds an operation; "now" is D-0330's table with the two vectors, "pairs" this decision):

| | now | pairs | groups | ordered |
|---|---|---|---|---|
| found, 1,000 | 11.7 | 11.9 | 13.5 | 12.6 |
| found, 100,000 | 29.6 | 24.1 | 37.2 | 28.5 |
| found, 2,000,000 | 101.8 | 77.6 | 121.4 | 78.5 |
| absent, 1,000 | 17.2 | 17.5 | 10.4 | 12.6 |
| absent, 100,000 | 19.8 | 20.6 | 14.5 | 15.2 |
| absent, 2,000,000 | 56.4 | 57.2 | 55.9 | 42.4 |
| insert, 1,000 | 39.5 | 38.7 | 40.0 | 49.0 |
| insert, 2,000,000 | 121.9 | 125.7 | 160.7 | 140.8 |
| `swap_remove`, 1,000 | 27.6 | 28.3 | 29.0 | 25.4 |
| `swap_remove`, 2,000,000 | 202.9 | 188.7 | 213.3 | 169.9 |

- **A table of groups** ("groups": the entries as pairs; a group is a word of seven one-byte tags and a count
  of the entries that overflowed past it, then seven cells; the tags compared all at once in one 64-bit word;
  removal lowers the counts, so there are no tombstones). It does what `CHG-0473` asked of it: a lookup
  mispredicts one jump in twenty where the present table mispredicts one in two. It is slower all the same.
  On this processor a lookup's time follows the instructions it executes more than the jumps it mispredicts,
  and the group lookup, as GCC and Clang compile it, executes about eighty where the plain probe executes
  under fifty; out of the cache the longer lookup also leaves the processor room to overlap fewer of them.
  `hashbrown` is fast for another reason: its group holds the keys and values themselves, so there is no
  index to follow, and with it no insertion order to keep. `indexmap`, which is built as this table is with
  groups over it, finds a key among a thousand in 10.3 ns, among a hundred thousand in 35 and among two
  million in 167 on this machine: the plain table with pairs is level with it in the cache and faster out of
  it. A processor with a wider window may order these otherwise; the project measures on this one.
- **The cells in the order of their hash bits** ("ordered": no wrapping, a few spare cells at the end; a
  lookup stops at the first cell whose bits are greater). Absent keys a quarter faster, removal a little
  faster, insertion a tenth to a quarter slower, since a new cell moves the cells after it. Most misses in a
  program are the ones `insert` makes. Not taken; it could be, for a program that mostly asks for absent keys.
- **Looking at two or four cells at once without a jump.** Slower than the plain loop at every size (16 to
  22 ns in the cache): more work for each lookup than the mispredicted jump costs.
- **Fetching the value's memory early** while the key is compared, with the two vectors kept. No gain: 139 ns
  where the plain lookup took 141 and pairs 102, in the same run.
- **Keys and values in two views of one allocation**, each still a field of the map, so that `foreach` and
  the checked forms change nothing. It makes the map a container of raw cells with its own destructor, as
  `Vec` is, and takes from it everything the compiler knows about a struct of two `Vec`s.
- **A set with no value** (`HashSet<K>` as a map to a type of no size), which would give back the padding a
  set's entries now have. The bodies the compiler writes decline a value of no size; left for a change of its
  own.
- **A body for `swap_remove` and `remove` in every form and for every value type.** The gain is in decided
  programs, where the body's records are what it costs; with a value that owns something the body must run
  what `std`'s runs. Not done.
- **Leaving `foreach` over `&mut m` as two calls.** They would be a shared and an exclusive borrow of the same
  vector at once: `diag.borrow-conflict`, at compile time or at run time.

## 5. Verification

See `CHG-0474`.
