# CHG-0433 — Decided programs against checked Rust, round 3: loops unrolled, numbers parsed once, `std` static

Status: implemented (2026-11-11), D-0296, D-0277, D-0286, D-0315; agenda item 3 (decided programs equal or beat
checked Rust), Update 179
Affects: `impl/cobc/src/cc.rs` and `impl/selfhost/cbc/cc.cb` (the C compiler's flags);
`impl/cobc/src/lower/native_text.rs`, `native_misc.rs`, `native_vec.rs`, `passes.rs`, `program.rs`,
`stack_checks.rs` and their twins in `impl/selfhost/back`; `impl/cbrt/include/cbrt.h`; `impl/src/numtext.rs`,
`impl/src/fileio.rs` (shared by `coby` and the runtime). No rule changes. No program's outcome changes.

## What was wrong

After Update 178 three of `stress/vsrust`'s six programs still ran well behind Rust with overflow checks: sudoku
1.86 times its time under GCC, dijkstra 1.55, gauss 1.31. None made runtime objects of its own. Profiles gave
seven causes, none in the programs' own code.

1. **GCC at `-O2` does not unroll a loop of a few iterations known before it runs**; LLVM does for Rust at `-O`.
   Sudoku's `foreach (c in 0..9)` loops computed `c / 3` by a multiply on each pass. Clang 14's `-O2` threshold for
   that unrolling (150) is below what the same loop needs (between 200 and 250).
2. **`StringView::parse<T>` read its text twice**: `parse_check`, then `parse_value`, each a call of the runtime
   that scans from the start; for a float, Rust's whole conversion twice. `parse_int` built every number in a
   `u128` with a checked multiply and add per digit, and `parse_float` validated as UTF-8 the text its own
   grammar check had just shown to be ASCII.
3. **A native body's entry test for an object over its slice (`cb_live_in`) was a call into the runtime** on
   every use: 7% of dijkstra's instructions, in `split_whitespace`.
4. **`cb_fn_pure`, a native body's test of its function argument, hashed the function's address** on every
   `PriorityQueue::push` and `pop`.
5. **Two native bodies made objects in a decided program.** `Vec::pop`'s result was made, bound, taken, moved
   and forgotten: `elide_record_chains` removes such a chain, but required each of its names to appear nowhere
   else in the text it was given, and that text holds the native's `_once` twin, which uses the same names. And
   `Vec::append` bound its argument so that the frame's end would free it.
6. **`read_line`'s runtime half found the newline a byte at a time.**
7. **Pure `std` units kept external linkage**, for the object cache D-0277 removed in Update 134. GCC then left
   a call per `Vec::push`, Clang a call per `HashMap::get` and `seek` (27% of lzss under Clang).

## What changes

1. `cobc` and `cbc` give GCC `-fpeel-loops` (which `-O3` turns on) and Clang `-mllvm -unroll-threshold=300`
   (its `-O3` value). Not all of `-O3`: under Clang it made the Lisp interpreter 30% slower.
2. `StringView::parse<T>` has a native body for every number type and `bool` (`native_parse`): one call that
   gives the code and writes the value. `parse_int` converts in the pass that finds the digits, in a `u64` for up
   to 19 of them; a longer run takes the checked `u128` path. `parse_float` does not validate again.
3. `cb_live_in` and `cb_live_in2` sit behind the page buckets, inline, as `cb_cells_busy` sits before
   `cb_cells_held` (CHG-0427): a live object over the cells has an entry on the bucket of every page they share.
4. In a pathless program `cb_fn_pure` is 1: every reference is token 0 in the native body and in `std`'s alike,
   as CHG-0431 argued for `cb_paths_disjoint`.
5. `elide_record_chains` counts a chain's names within the function the chain is in. `Vec::append` in a decided
   program ends its argument as a decided body ends a plain `Vec` parameter (`cb_drop_plain_buf`), with no frame.
6. The newline is found by the C library's `memchr`.
7. `localize` makes pure `std` units `static` too. `inline_small` (CHG-0430) still marks only functions outside
   pure `std` units: every small `std` function forced inline under GCC made Huffman's `compress` 12% slower,
   where GCC's own choice for a `static` function made it faster.

## Measured

`stress/vsrust`, time against Rust with overflow checks, GCC 12.3 / Clang 14.0, rustc 1.98.1; the minimum of
nine interleaved runs; instructions in user space.

| Program | Update 178 | This one | Instructions (GCC / Clang / Rust, G) |
|---|---|---|---|
| sudoku | 1.86 / 1.37 | 1.07 / 1.09 | 16.47 / 14.80 / 13.73 |
| mini_interpreter | 0.90 / 0.82 | 0.90 / 0.81 | 11.88 / 10.29 / 8.16 |
| lzss | 1.13 / 1.15 | 1.07 / 0.96 | 1.45 / 1.22 / 1.31 |
| huffman | 1.19 / 1.23 | 1.04 / 1.02 | 3.45 / 3.34 / 3.14 |
| gauss | 1.31 / 1.27 | 0.90 / 0.92 | 1.54 / 1.35 / 1.40 |
| dijkstra | 1.55 / 1.59 | 1.21 / 1.20 | 6.55 / 6.20 / 4.31 |

Every `parse` result and error is the same in `coby` and in `cobc` under both C compilers (50 texts: each
integer width's limits, a 31-digit text with leading zeros, floats, `inf`, `NaN`, `bool`). `Vec::append` in a
decided program: the same output as `coby`, and no leak or error under valgrind, both C compilers.

## Costs, accepted

The gate's perf-guide unit measured five rows more than 3% above their baselines; each was traced to its change
by compiling the same generated C with that change taken back out.

- **The inline entry test (item 3)** changes what GCC inlines around it: `e10_struct_param` a +7.7% instructions
  and +7% time (123 to 132 ms); `e03_string_param` b, c, d +2 instructions per call (4.28 M to 6.28 M for
  1,000,000 calls; the time within noise). The test itself is not on those loops; GCC's inlining of the `std`
  function that holds it changed. Taking it out costs dijkstra 10% of its time (2,661 to 2,952 ms), so it stays.
- **Pure `std` units `static` (item 7)**: `e02_reserve` b under Clang +6.7% instructions, the time unchanged
  (1,035 and 1,036 ms).

The perf-guide baselines were saved from this run, the old ones kept as `*.before-update179`.

## Not done

- **dijkstra, 1.2.** Each comparison of `PriorityQueue` is a call through a `fn` value: a thunk, an indirect
  call, and the caller's position saved and restored around it. Rust's `BinaryHeap` has its comparison compiled
  in. `PriorityQueue` also orders equal items by arrival, which is its specified behaviour. What a call through a
  `fn` value should cost is a design question of its own.
- **`read_line`** still validates UTF-8 a byte at a time, in CobaltC, and allocates its line.
- **sudoku, 1.07 to 1.09.** `count_ones` of a `u16` is a 64-bit count under GCC without POPCNT (a 32-bit one
  measured 5% fewer instructions), and `solve` copies `Used` twice on its way from `used`.
