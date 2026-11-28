# CHG-0458 — The runtime in C; the seed and the first build on a new machine

Status: implemented (2026-11-15), D-0326 phases B and C; Update 193
Affects: `impl/cbrt/` (the runtime in C: `src/*.c`, `src/*.h`, `FLAGS`, `SOURCES`, `build.sh`, `tools/`,
`tests/cbmc/`, `tests/sanitize.sh`; `src/lib.rs`, `src/replay.rs`, `build.rs`, `stamp.rs`, `Cargo.toml`,
`tests/miri_smoke.rs` removed), `impl/cbrt/include/cbrt.h` (a comment), `impl/bootstrap/` (new: `boot.c`,
`mkseed.sh`, `cbc-seed.c.gz`, `SEED`), `impl/build.sh`, `impl/Cargo.toml`, `impl/cobc/Cargo.toml`,
`impl/cobc/build.rs`, `impl/cobc/src/target.rs`, `impl/cobc/src/main.rs`, `impl/cobc/src/lower/frames.rs`,
`impl/cobc/src/lower/stack_checks.rs`, `impl/cobc/tests/driver.rs`, `impl/cobc/tests/oracle/mod.rs`,
`impl/src/loader.rs`, `impl/selfhost/front/loader.cb`, `impl/selfhost/cbc/target.cb`,
`impl/selfhost/cbc/driver.cb`, `impl/selfhost/back/gen.cb`, `impl/selfhost/back/stack_checks.cb`,
`impl/selfhost/back/header.cb`, `impl/tools/gate.py`, `impl/tools/battery_stages.sh`, `impl/tools/assurance.py`,
`.github/workflows/bootstrap.yml` (new), `README.md`, `impl/STATUS.md`, `spec/ASSURANCE.md`, D-0326 (amended)

## Why

D-0326's phases B and C, in one update as the owner amended it: the runtime every compiled program links is
ported from Rust to C, and CobaltC is built on a machine that has a C compiler and nothing else (M1).

## What changes

- **The runtime is C** (`impl/cbrt/src/`, GNU C11; about 34,000 lines with its tables), ported function for
  function from the Rust runtime and its eight shared modules. `cbrt.h` is unchanged but for a comment: the
  same 170 functions and their types (`tools/sigcheck.py`), so both compilers write the same C as before. The
  library is one relocatable object whose only global symbols are the Rust runtime's exported ones
  (`tools/exports.txt`). Its own float algorithms (shortest digits, fixed and exponent forms, parsing) replace
  Rust's; the platform layer is POSIX or Windows (calls Windows 7 has). The table of diagnostics is generated
  from the registry (`tools/gen_diag_table.py`) and checked current by the gate.
- **Built by `impl/cbrt/build.sh`** (flags in `FLAGS`), and by `cobc`'s build script beside `cobc`. **The
  stamp** is a hash of the files `SOURCES` lists (`tools/stamp.c`, 64-bit FNV-1a, carriage returns skipped),
  computed the same way by `cobc/build.rs` and `boot.c`.
- **`COBALTC_CBRT_DIR`** names the runtime's directory at run time, for both compilers (an instrumented
  runtime, another build).
- **The seed** (`impl/bootstrap/cbc-seed.c.gz`): `cbc`'s C of its own source without `#line` directives,
  gzip'd (`mkseed.sh`); `SEED` records its length and CRC-32. **`boot.c`** builds the runtime, unpacks and
  checks the seed with its own inflate, compiles the compiler, proves it (the new compiler writes its own C
  again, which must be the seed), builds the formatter and runs a first program: `gcc impl/bootstrap/boot.c
  -o boot && ./boot`, or `boot.exe` from a Windows command prompt. The gate's build unit checks the seed is
  current.
- **`impl/build.sh`** builds the runtime when its sources changed (the old one kept in `impl/target/prev`) and,
  when the runtime changed since the previous `cbc` was built, builds in two stages: the previous `cbc` with
  its own runtime builds a `cbc` that knows the new stamp, which builds everything again.
- **A module file's name is joined with `/` on every host** (both front ends), so the C, a diagnostic and the
  fault table name it the same way on Windows as on Linux, and `boot.exe`'s proof holds there.
- **`cb_lock_bare` passes the turn under a schedule**, as `cb_lock_pathless` does: with a guard held through
  `auto`, `cobc --explore` ran two threads' bumps one after the other and found one outcome of the two
  (`explore_reaches_the_lost_update_through_auto_guards`).
- **`cb_threaded` is read with a relaxed atomic load** in the lowered entry of a frame with noted locals
  (D-0269), in both compilers: `cb_spawn` stores it while other threads may run that code, which
  ThreadSanitizer reports as a race once the runtime is instrumented with the program.
- **The gate**: the build unit checks the diagnostics table, the interface and the seed; `portable` builds the
  runtime with MinGW-w64 and Clang without a warning; the debug unit runs the CBMC harnesses in place of
  `cargo test -p cbrt`. The battery runs CBMC and ASan with UBSan over the conformance suite.
- **A fresh clone on every push**: `.github/workflows/bootstrap.yml` (Linux with GCC and Clang, Windows with
  MinGW-w64 from a command prompt, no Rust on `PATH`), each `boot` then the conformance suite; this stands for
  the owner's Windows 7 checkpoints (D-0326 §5).
- **The documents**: the README's building and Windows sections (`boot` first, the Cargo steps under "being
  retired"), `impl/STATUS.md`, the assurance case's runtime rows.

## Checked

The C runtime against the Rust one (the same generated C linked against each, exit status, standard output and
error the same bytes):

- The conformance suite under GCC, under Clang, and with every check (`COBALTC_ALL_CHECKS`): 853 compiled cases
  the same each way (542 have no C: static diagnostics). The examples (14), the showcases (90), the book (58).
- `COBALTC_SCHEDULE_SEED` 1, 2, 3 and 17 on the concurrency cases: 122 the same.
- `COBALTC_TRACE_EVENTS`: the suite the same; on the model's sample one program differs, whose read of `y`
  races the thread it is lent to, and the model lists both outcomes for it.
- `COBALTC_RT_STATS`: the counts the same on every case without threads or timing; on the 34 with them, the
  counts of entries while threads ran and of a timed wait's polls depend on timing (the Rust runtime's vary
  run to run too).
- The float text against Rust's own (`fmt.rs`, `numtext.rs`): every `f32` bit pattern, 3 × 10^8 random
  `f64` and 1.3 M edge cases, rendering, integers, text and 2 × 10^7 parses: no difference.
- CBMC: the six Kani harnesses' properties on the C, at Kani's bounds, with unwinding assertions: proved (the
  harness models what CBMC 5.12 lacks: the bit scans, an internal error, `realloc`).
- ASan with UBSan, the runtime and every program instrumented together: the conformance suite, 1395 of 1395,
  no report. ThreadSanitizer likewise after the `cb_threaded` fix.
- Speed, the bench and the performance guide's programs under both C compilers, the median of five runs on a
  quiet machine: 0.98 (GCC) and 0.96 (Clang) of the Rust runtime's instructions on average, none more than 3%
  above it in count or time but one small program (`e05_take_front` b, 4.5% more instructions, less time).
  Two were slower at first and are fixed in the runtime: the sort of text moved records with a call per move
  and compared every pair through `memcmp` (`bench/strings`, 1.5 times the instructions, now 0.83 of the time);
  standard output's lock was two calls into the C library's mutex per `printf` (a `printf` a line, 10% slower;
  now one atomic exchange uncontended, as Rust's `Mutex`).
- Windows: the runtime by MinGW-w64, the suite under Wine: 1383 of 1385, the two 300-thread cases Wine
  cannot run, as before.
- Fresh machines, a clone and `boot` with a C compiler alone: Ubuntu 22.04 (GCC 11), Ubuntu 24.04 (GCC 13),
  Debian 12 (GCC 12), Fedora 41 (GCC 14.3) and Ubuntu 24.04 with Clang 18 only, about ten minutes each; the
  conformance suite passes in full on Ubuntu 24.04 and Fedora 41, and on Clang 18 but for
  `conf.zero-sized-addresses` (two zero-sized locals at one address under Clang 15 and later: a lowering
  gap older than this change, `impl/STATUS.md` Known gaps, to be fixed in the next update).
- The gate on this commit (run `20261115_142913`): green in every unit but `lowering`, red on one row of the
  performance guide only: `e05_take_front` b under GCC, 3,498,135 to 3,655,869 instructions (+4.5%; a
  3.5-million-instruction program, its time within noise; the bench's 18 counts within 3%, no decided program
  lost). Traced with callgrind: each of its 10,000 `Queue::pop_front`s freed an empty vector through the C
  library (`atvec_free`), and the slot scan's page visit was not inlined; with both fixed the row is +2.2%
  (Clang 0.93 of the Rust runtime's), which the next update carries. Accepted here and the lowering's
  baselines saved from this run (the earlier ones kept as `*.before-update193`); `lowering` and `records`
  run again green.
