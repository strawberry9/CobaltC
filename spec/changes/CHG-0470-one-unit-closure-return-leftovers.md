# CHG-0470 — The compiler built as one C file; a closure typed by its `return`; leftovers (implementation only)

Status: implemented (2026-11-18); Update 198
Affects: `impl/cobc/driver.cb`, `driverbase.cb`; `impl/build.sh`; `impl/back/gen.cb`, `state.cb`;
`impl/tests/cases/cobc/driver_one_unit.case` (new), `closure_result_from_return.case` (new);
`impl/tests/programs` (three cases renamed, their runs recorded again), `impl/tests/cases/cobc` (one renamed);
`impl/tools/battery_stages.sh`; `CARD.md`

## Why

Four things the owner put into the update after Update 197 (2026-11-17 and 18).

1. **The compiler built by itself ran slower than the one built from the seed, and was larger** (the owner's
   finding). Measured on `tinyos.cb`, ten runs each, the same C from all: built as `impl/build.sh` built it
   (its C in eight parts, GCC's link-time optimisation, `-fpeel-loops`) it executed 2.9% more instructions,
   took 6% more cycles, and had 9% more code than built as `impl/bootstrap/boot.c` builds it (one file,
   `-O2`). Link-time optimisation's inlining gives the instructions (2.7% with one file and LTO alone); the
   eight-way split gives the size and the rest of the time; `-fpeel-loops` nothing.
2. **F30** (the checker's open items, `stress/checker_gaps/LOG.md`): a closure with no written result type
   whose body ends in a `return` (`[]() { return 5; }`) was accepted by the static pass and then stopped the
   compiler ("a format argument of an unformattable type" where its result was printed): the back end learns
   a closure's result type from the body's value, which is `never` when the body ends in a `return`.
3. **Four test files** still named for the interpreter and for Rust's panics.
4. **`impl/tools/battery_stages.sh`** still wrote the installed `rustc`'s version into each summary, counted
   cargo's `test result` lines (always 0), and looked for `panicked`.

No rule changes.

## What changes

- `cobc --one-unit`: the program's C compiled as one file with no link-time optimisation, whatever its size.
  `impl/build.sh` builds the compiler with it (where the compiler doing the build has the option: one made
  before this update builds in parts, and the compiler that makes builds the next as one file; the two-stage
  build a changed runtime takes does both). The seed and `boot.c` are as they were; every other program's
  default is as it was. The compiler's own C compile takes about eight minutes where it took four.
- A closure's result type, where none is written, is the type of the first value a `return` in its body
  gives when the body's own value is `never` (`Gen.closure_returns`, set in `lower_return` while the body is
  lowered to learn the type; a closure inside it keeps its own).
- F34 and F35 are closed as they stand. F34 (a view declared in a block and named as the block's value,
  stored where it outlives the block): refused at run time (`diag.destroy-while-aliased`), not statically,
  because the view's origin is no longer known when the store is checked, after the block's scope has ended;
  a static answer needs the escape check made at the block's end, a change to the static pass that this
  update does not make. No wrong program is accepted. F35 (`m.p = &z; H r = h;` with `m` an exclusive
  borrow of `h`): refused statically by design: a store through `m` can put a reborrow of `m` into `h`.
- The cases: `e2e_i128_arithmetic_at_the_bounds` (and `_2`), `conf_spawn_closure_arg_count_mismatch_is_diagnosed`
  (programs; their recorded runs differ from the old ones in the file's name alone),
  `driver_runtime_internal_error_one_line` (command case).
- `battery_stages.sh`: no `rustc` line, no cargo counts (a stage's line is its failure lines), no `panicked`.
