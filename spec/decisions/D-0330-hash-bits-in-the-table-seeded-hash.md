# D-0330 — A hash table's cells keep hash bits, and the hash is seeded

Status: ACCEPTED (2026-11-20). Decided by the owner: asked whether `HashMap` is the fastest it can be, the
implementer measured it against Rust's tables and put four options (A: the hash kept in the table; B: a hash that
takes eight bytes at a time; C: a seed for each run; D: leave it); the owner chose A, B and C together, as one
Update; implemented by `CHG-0473`
Kind: Decision (`spec/02-schema.md`); `std`'s normative bodies and one std-only intrinsic's value, which no
program observes
Depends on: D-0041 (hash tables written in CobaltC), D-0110 (composite keys), D-0134 (`remove`, `swap_remove`),
D-0312 (lookups take the key's probe), D-0142 (the system's randomness), D-0326 (the runtime in C)
Affects: `spec/21` `rule.stdlib.hashmap` (`[Key-Hash]`, the bodies of `HashMap`); `impl/std/collections.cb`;
`impl/cbrt/include/cbrt.h`, `impl/cbrt/src/osrand.c`; `impl/back/native_map.cb`, `impl/back/gen.cb`,
`impl/back/passes.cb`, `impl/back/header.cb`; the guide's appendix B. No program's meaning changes.

## 1. Problem

`HashMap` and `HashSet` keep their entries in insertion order (`keys`, `values`) beside a table of entry
indices (`slots`), with linear probing. The table held nothing but indices, and the hash was FNV-1a over the
key's bytes, one byte at a time, from a fixed start. Measured on maps of a thousand to two million entries
against Rust's `std` table, `hashbrown` and `indexmap` (the one built like CobaltC's), with the same keys
and the same answers (`stress/hashmap_bench`):

- **Every taken cell on a lookup's way cost a key.** With no part of the hash in the cell, the lookup read the
  entry's key to compare it: for a `String` key the record and then its bytes, each somewhere else in memory.
  A lookup of an absent key in a two-million-entry map took 119 ns for an integer key and 257 ns for an
  18-byte text, where `indexmap` with the same hash function took 52 and 90.
- **Growing hashed every key again**, and compared it with every key in its way, though keys already in a map
  are all different. Closing the gap a removal leaves hashed every key behind the gap. Inserting a hundred
  thousand texts took 404 ns each (`indexmap`: 115).
- **The hash took a multiplication for each byte**, one waiting for the last: eight in a row for a `u64` key.
  A lookup that hit, in a map of a thousand integers, took 18.7 ns; with the table as it was and only the hash
  replaced, 13.8.
- **The hash was the same in every run**, so a party who chooses the keys (a peer's header names, the names in
  a DNS answer) could choose keys that all land together and make each insertion walk them all.

The order a program sees is the order of insertion and never depends on a hash (`spec/21`: "`key_hash`'s value
is not observable"), so all of this can change without changing any program.

## 2. Decision

**A. The table's cells keep hash bits.**

1. A cell of `slots` is empty (`max_value<usize>()`, as before) or holds two things: the entry's index in its
   low bits and the upper bits of the entry's hash above them. With `mask` the number of cells less one (the
   number is a power of two), the cell is `(h & ~mask) | index`. The table is at most three-quarters full, so
   an index is below `mask` and no cell with an entry equals the empty one.
2. An entry's first cell is named by the uppermost bits of its hash (`h >> (64 − b)` in a table of 2^b cells),
   where it was the lowest. Those bits are among the ones the cell holds, and so are the ones that name the
   entry's first cell in a table twice the size, for any table of up to 2^32 cells: a cell says where its
   entry belongs, now and after the table grows.
3. A lookup compares a key only in a cell whose hash bits are those of the hash it is looking for: 64 − b bits,
   b of them shared by every entry that belongs in the same first cell. In a table of a million cells a key
   that is not the one sought is read in about one such cell in sixteen million.
4. Growing (`reslot`) makes the new table from the old table's cells, taken in order: no key is read or
   hashed, and since a cell's entry goes to one of two neighbouring cells of the new table, the new table is
   written from one end to the other. Closing a gap (`unslot`) moves cells back by the bits they hold.
   `swap_remove` hashes the one key it moves and finds its cell by that hash and its index, comparing no key.
5. A table of more than 2^32 cells (32 GB of cells) has too few bits in a cell for this; there `reslot` and
   `unslot` hash the keys, as they did.
6. The map has the three fields it had and takes no more memory. (Keeping every entry's whole hash in a
   fourth vector was built and measured first: a word more an entry, and each `swap_remove` paid one more
   place in memory to move the hash, which made removal from a large map a quarter slower than before.)

**B. The hash takes eight bytes at a time.** `[Key-Hash]` is no longer FNV-1a. The state is 64 bits. Each step
exclusive-ors eight of the key's bytes into it, multiplies by an odd 64-bit constant to a 128-bit product, and
exclusive-ors the product's two halves. Whole words are taken first, then the key's last eight bytes (which may
overlap the last whole word); a key of fewer than eight bytes is one word made of its bytes. The length goes
into the state before the bytes. A `u64` key is one multiplication. A struct or enum key takes its parts in
turn (`[Key-Bytes]`), a text part's length before its bytes, as before.

**C. The state starts from a seed drawn once for each run** of a program: eight bytes of the system's
randomness (`[Os-Random]`'s source), drawn at the first hash the program makes, so a program that uses no hash
table draws nothing and links nothing for it. Where the system has no randomness the seed comes from the clock
and an address: a table still works, and the program does not fault for want of a seed it never asked for.
`COBALTC_HASH_SEED=N` in the environment fixes the seed (for measuring: the instructions a run executes then
repeat exactly).

The specification now says of `key_hash` what a program may rely on, and no more: equal keys have equal hashes
within one run. `spec/21` gives the function the implementation uses, as the definition of this implementation,
not as a requirement on another.

## 3. Consequences

- No program changes meaning: nothing reads a hash or a cell. Iteration, `key_at`, `value_at`, what `remove`
  and `swap_remove` leave, and every fault are as they were.
- A map's memory is what it was. While a table grows, the old and the new table exist together (before, the
  table grew in place).
- The instructions a program with a hash table executes vary a little from run to run with the seed (where its
  keys land), unless `COBALTC_HASH_SEED` is set. `bench/run.sh` sets it for its counts, the gate's `lowering`
  unit for its own, and `impl/tools/replay_runtime.py` for the traces it compares.
- A program's first hash makes one system call for the seed.
- **What the seed does and does not give.** Keys chosen without knowing the seed land as random keys do; a
  list of colliding keys computed beforehand does not collide. The function is not a cryptographic one: it is
  not claimed that a party who can measure a server's lookups closely enough could never learn the seed. It is
  the protection the fast hashes of other languages' tables give, and it costs nothing a fixed start did not.
- The runtime's interface: `cb_fnv_more` is replaced by `cb_hash_more`; `cb_hash_start`, `cb_hash_fold`,
  `cb_hash_seed_cell` and `cb_hash_seed_make` are new; `cb_hash_bytes` keeps its name and takes the seed.
- The hash's bytes are read in the machine's byte order, so a hash differs between machines of different byte
  order, as it now differs between runs.

## 4. Alternatives considered

- **Leave it (D).** Correct and simple; costs the numbers of §1 and leaves a fixed hash before the network.
- **The hash alone (B without A).** Measured: integer keys 20 to 35% faster, text keys unchanged, since their
  cost was the keys read on the way.
- **Every entry's hash in a vector beside the keys** (as `indexmap` keeps it). Measured, see §2 A.6: once a
  cell names its entry's first cell there is nothing left for the vector to do but cost memory and a miss.
- **A table in the manner of `hashbrown`** (control bytes scanned sixteen at a time, the entries in the table
  itself). The fastest lookups measured: a third to a half of the time of this design on integer keys. It
  gives up the dense insertion-ordered `keys` and `values`: the order a program sees would have to be kept
  some other way, and iteration, which is over twice as fast as Rust's `std` table, would slow to its speed.
  Control bytes over the index table (what `indexmap` has) would keep the order; they need the table's probing
  rewritten around groups of cells, in `std`'s CobaltC and in the native bodies both. Not taken now; what it
  would gain is the lookup's jump on whether the first cell is the right one, which the processor cannot guess.
- **Keys and values in one vector of pairs.** One fewer place in memory for a lookup that hits. It changes what
  `key_at` and `value_at` borrow and how `foreach` over a map and `HashDrain` take entries apart: a larger
  change. It can follow.
- **SipHash-1-3, as Rust's `std` uses.** A cryptographic guarantee against chosen keys, at about twice the time
  of a lookup in a small map (`std-sip` against `std-fold` in the measurements). The seed gives the protection
  that matters at no cost; a program that needs the guarantee can key its map by a keyed digest
  (`std::crypto`).
- **The seed drawn when the program starts.** Simpler than a test at each hash, but every program would then
  make the call and link the code, table or no table (D-0329 made an empty program 54 KB).
- **Fault when the system has no randomness**, as `os_random_bytes` does. A program that asked for random bytes
  must not get a clock; a program that only made a map asked for nothing.

## 5. Verification

See `CHG-0473`.
