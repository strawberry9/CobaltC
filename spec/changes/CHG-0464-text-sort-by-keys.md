# CHG-0464 — `Vec::sort` of text sorts keys, not the texts (implementation only)

Status: implemented (2026-11-17); Update 196
Affects: `impl/cbrt/src/rt_init.c` (`cb_sort_text`);
`impl/conformance/21-standard-library-semantics/sort_many_texts_ok.cb`; `spec/conformance.md` 3.317.0;
`bench/RESULTS.md`, `bench/README.md`

## Why

`bench/RESULTS.md`, measured again for Update 196 (the first run on the runtime written in C), showed the
`strings` workload with seven times the branch mispredictions it had on the Rust runtime: 8.0 million, nine
tenths of them in the runtime's merge of texts, which compared two texts through their pointers and jumped on
the outcome, a jump the processor cannot guess. The owner asked whether it could be repaired, and left the
decision to the implementer (2026-11-17).

No rule changes: `[Sort]` (`spec/21`) fixes the order (`str` and `String` by their bytes, a prefix first) and
that equal elements stay as they were, and both hold as before.

## What changes

`cb_sort_text`, the runtime's `Vec::sort` of `String` or `str` (D-0297), sorts 32 texts or more by keys; fewer
go through the merge it had.

- **A key** is two words: eight of the text's bytes as one big-endian word, zero after the text's end, and a
  tag holding the count of bytes from there on (nine for nine or more) above the record's index. Two keys order
  as their texts do by the word and then the count; the index keeps keys alike in both in the order they came
  in, so the comparison is one 128-bit comparison with no jump.
- **The merge** reads the keys alone, in order, and picks the next one by arithmetic on the two addresses, not
  by a jump. (Written as `?:`, GCC turned the pick back into a jump.) The texts are read once, to make the keys.
- **Texts alike in the eight bytes that both go on** end next to one another: a tied run. A run of twelve or
  more gets keys from its next bytes and is sorted the same way, until no run is left (a list of the runs still
  to do, so the depth does not follow the texts' length); a shorter run is sorted by comparing its texts.
- **Bytes all the texts share** are skipped: the same pass that makes the keys finds how many of the eight
  every text shares, and keys are made again past the shared bytes when there are three or more.
- **The records move once**, at the end, each written to its place from a copy; records already in place at the
  front, all of them when the texts came in order, are left.

The sort's transient memory is 16 bytes a text for the keys, and beside them the larger of 8 bytes a text
(the merge's half) and a copy of the records (24 bytes a `String`, 16 a `str`); the merge it replaces took half
a copy of the records.

Tried and not kept: comparing tied texts one by one through their indices (twice the old sort's time on texts
in two groups with long prefixes); finding the shared prefix in a pass of its own before every keying (two
cache misses a text on input already in order); moving the records in place round the permutation's cycles
(each step waits for the one before: three times the copy's time).

## Checked

- **The order and the stability**, against a sort that shares nothing with it (`stress/rustfree/sorttext/
  difftest.c`: C's `qsort` by the bytes, ties by the original index), the records compared whole so that equal
  texts must stay in their order: 30 000 vectors of 0 to 60 000 texts, 23.9 million records, both record widths
  (`String`, `str`), texts of 0 to 174 bytes over seven alphabets (zero bytes, one letter, `0xff` among them),
  shared prefixes of 0 to 24 bytes, 1 to a million distinct texts, input in order, reversed and mixed. The same
  under AddressSanitizer and UBSan (6 000 vectors). Nine faults planted in the new code one at a time: the
  test reports eight; the ninth only changes which of the two methods sorts a short run.
- **`conf.sort-many-texts`**: ten vectors of texts and one of literals, each compared with an insertion sort by
  `<` written in the case; the same output before and after the change, in the default form and with every
  check on, under GCC and under Clang, and compiled for Windows under Wine. `conf.sort-search` and
  `conf.sort-order-is-lt` likewise.
- **Time**, half a million texts, the sort alone (`stress/rustfree/sorttext/timing.c`, best of five, quiet
  machine), before and after:

  | Texts | Before | After |
  |---|---|---|
  | the bench's words (`w` and a number under 5 000) | 206 ms | 120 ms |
  | 16 random letters, all different | 174 ms | 119 ms |
  | one 20-byte prefix, then a number | 315 ms | 136 ms |
  | two 24-byte prefixes, then a number | 310 ms | 197 ms |
  | 60 random letters | 206 ms | 118 ms |
  | the bench's words, reversed | 165 ms | 85 ms |
  | the bench's words, in order already | 28 ms | 37 ms |
  | vectors of 100, of 40, of 31 | 45, 33, 31 ms | 34, 29, 31 ms |

  Input already in order is the one shape that takes longer: the keys are made before anything is compared.
- **`bench/RESULTS.md`**, measured again on a quiet machine with this runtime: `strings` takes 0.293 s under
  GCC and 0.287 s under Clang where it took 0.372 s and 0.370 s, 0.59 and 0.58 of Rust's time (0.76 and 0.75
  before; 0.91 and 0.90 on the Rust runtime); 646 million instructions where there were 835 million; 1.76
  million branch mispredictions where there were 8.0 million. The other eight workloads are as they were.
- The runtime built by GCC, by Clang and by MinGW-w64 without a warning.
- **The gate's units on the commit with the sort** (`3bed100`, run `20261117_004114`): the gate profile's
  green but `model` (Lean 4.36.0, published during the run: `CHG-0465`); of the release profile's, `fixpoint`
  (stage 2 is stage 1, 101 048 122 bytes at level 10, 276 minutes), `all-checks` (1 423 of 1 427 under each C
  compiler, the four slow cases left to `slow-cases`), `slow-cases` (1 427), `runtime` (1 423 under each
  sanitizer) and `verify` green. The run was stopped in `wine` (`CHG-0466`), so `wine`, `programs` and
  `external` did not run on the sort; they were green on Update 196's first commit (run `20261116_144703`),
  and under Wine with this runtime `19-concurrency` passes (144 of 144) beside the case above.
