# CHG-0436 — Against checked Rust, round 4: text read in place, one-target `fn` calls; `cbc`'s own C faster

Status: implemented (2026-11-11), D-0296, D-0309, D-0077; agenda item 3 (decided programs equal or beat
checked Rust), Update 181
Affects: `impl/cbrt/include/cbrt.h` (and `impl/selfhost/back/header.cb`, generated from it); `impl/std/text.cb`
(and `impl/selfhost/front/prelude.cb`, generated from `std`); `impl/cobc/src/lower.rs`, `lower/equality.rs`,
`lower/exprs.rs`, `lower/closures.rs`, `lower/native_map.rs`, `lower/program.rs` and their twins in
`impl/selfhost/back` (`gen.cb`, `state.cb`, `native_map.cb`); `impl/selfhost/back/bodies.cb` (`cbc` only);
`impl/tools/validate_c.py`. No rule changes. No program's outcome changes.

## What was wrong

The owner, 2026-11-11: "the aim is to match or beat checked Rust everywhere". After Update 180, dijkstra still
ran at 1.25 / 1.19 times checked Rust's time (GCC / Clang), and its gap was not checks
(`stress/check_survival/LOG.md`). Split in two (`stress/vsrust_gaps/LOG.md`), it was +0.98 G instructions in
reading its input and +1.26 G (3.3 times Rust's) in its heap phase.

1. **`StringView::parse` of an integer was a call into the runtime** for every word, which built the number in
   a `u128`: 116 instructions a word.
2. **`String::from_utf8` validated a byte at a time** (`utf8_error_at`), with an overflow check per step:
   0.40 G of 1.36 G on a loop that only reads lines, and 0.55 G more under Clang than under GCC.
3. **`ascii_is_space` made three comparisons** for a byte above 32, and `space_run_end` compared its result
   with the run's kind on every byte.
4. **Every call through a `fn` value went through its thunk's dispatch**: a test of the box's kind, an indirect
   call through its code, and in the closure a stack check. Rust's `BinaryHeap` compiles its comparison in.
5. **`PriorityQueue`'s arrival order was a branch**: `before(i, j)` tested which of the two arrived first and
   then called `less` one way or the other, a branch no predictor learns (23 M mispredictions in dijkstra's heap
   phase, Rust's 2.3 M).
6. **A closure body that calls nothing kept its stack check**, where a function's (`calls_nothing`) has not.

And `cbc` took 1.28 times `cobc`'s time to write its own C (156 s against 122 s; `cobc` 403.6 G instructions,
`cbc` 394.6 G). `direct_dispatched`, asked at every call of a non-generic function, worked out the callee's
dual and direct parameters again each time: 9% of `cbc`'s cycles.

## What changes

1. `cbrt.h`'s `cb_parse_int` is an inline function for at most 64 bits and 19 digits, which gives
   `numtext::parse_int`'s answer (the value, `Empty`, `OutOfRange`, or the offset of the first byte that is not a
   digit); a wider type or a longer run of digits calls the runtime as before.
2. `utf8_error_at` passes over eight bytes at a time while all eight are ASCII.
3. `ascii_is_space` is one comparison for every byte above 32 and a mask for the rest; `space_run_end` has a
   loop for each kind of run.
4. **One-target thunks.** The lowering records every code a `fn` value of each type can hold, at the four
   places a box is made (`box_closure`'s static, stack and runtime boxes, and a fn item's handle), with the box's
   kind. `fn` types are nominal, with no conversion between two (D-0006), so a box made for one type is only called
   through that type's thunk. A thunk whose type has exactly one code calls it directly, through the same cast and
   with the `self` its kind gives; the null test and the location kept around the call stay. The C compiler can
   then inline the call: `PriorityQueue::new_by`'s comparison, `Vec::sort_by`'s, `Vec::from_fn`'s. Not in a
   library (`--lib`) and not in the bootstrap form.
5. `before(i, j)` is one call of `less` on the pair the arrival test chooses, its answer turned by that test:
   `e ? !less(j, i) : less(i, j)` without a branch on `e`. The same call with the same arguments.
6. A closure body that calls nothing, its prologue included, has no stack check (D-0077's argument for a leaf
   function).
7. `cbc` keeps `direct_dispatched`'s answer about each callee by name (the generator is made afresh for each of
   the two lowerings). The C is unchanged.

## Measured

`stress/vsrust`, time against Rust with overflow checks, GCC 12.3 / Clang 14.0, rustc 1.98.1; the median of
seven runs; instructions in user space.

| Program | Update 180 | This one | Instructions (GCC / Clang / Rust, G) |
|---|---|---|---|
| dijkstra | 1.25 / 1.19 | 1.17 / 1.11 | 5.30 / 5.18 / 4.31 (was 6.55 / 6.20) |
| gauss | 0.92 / 0.94 | 0.86 / 0.88 | 1.52 / 1.26 / 1.40 |
| huffman | 1.04 / 1.02 | 1.04 / 0.98 | 3.41 / 3.14 / 3.14 |
| lzss | 1.07 / 0.95 | 1.05 / 0.94 | 1.44 / 1.15 / 1.31 |
| sudoku | 1.05 / 1.08 | 1.05 / 1.09 | 16.47 / 14.80 / 13.73 |
| mini_interpreter | 0.90 / 0.82 | 0.91 / 0.82 | 11.88 / 10.29 / 8.16 |

Reading dijkstra's input alone (41.6 MB, 2.4 M lines, 7.2 M words): lines 1.36 / 1.91 G to 1.31 / 1.30 G (Rust
1.39), words 2.73 / 2.60 G to 2.33 / 2.07 G (Rust 2.21), with `parse` 3.54 / 3.46 G to 2.84 / 2.76 G (Rust 2.68).

The inline `parse` and the runtime's agree on 24,000,344 cases (every width and signedness; the limits, signs,
leading zeros, 20 digits and more, and three million random texts), under both C compilers.

The perf guide's `e06_lookup` (variant b, `HashSet::contains`) under GCC runs 6.52 M to 6.83 M instructions
(+4.9%), its time unchanged at 8 ms; Clang 6.70 M to 6.71 M. Its `HashSet` loop's C is the same: `main` inlines
`String::from_utf8` (for `arg(0)`), whose validation loop is now longer, and GCC allocates `main`'s registers
otherwise, a spill more in the loop (`callgrind`: all 0.32 M in `main`). The same C with `utf8_error_at` kept out
of line runs 6.46 M. Not worked around (a name-keyed `noinline`, as `Vec::grow`'s, would steer one allocation
of one compiler); the gate's baseline is this run's.

`cbc` writing its own C (`cbc --emit-c impl/selfhost/cbc/main.cb`): 156 s to 137 s, 394.6 G to 316.9 G
instructions, against `cobc`'s 122 s; the C the same as `cobc`'s.

## Not done

- **dijkstra, 1.17 / 1.11.** Its heap phase still takes 1.5 times Rust's cycles: `PriorityQueue` keeps each
  element's arrival number in a second `Vec`, so a step of the heap touches two cache lines; and the input phase
  is 1.05 times Rust's under GCC.
- **sudoku, 1.05 / 1.09**: the statement positions stored in its loops (`stress/check_survival/LOG.md`, the
  position-liveness option).
- **`cbc`'s own C, 1.12 times `cobc`'s time.** What is left is spread: allocation (malloc and free, 12% of its
  cycles), copies of declarations lent out while their lowering recurses, the text passes (9.5%).
