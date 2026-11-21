# CHG-0457 — Nothing generated depends on a Rust tool; the formatter is the CobaltC one; one tool for programs

Status: implemented (2026-11-15), D-0326 phase A; Update 192
Affects: `impl/selfhost/cbc/summary.cb` (new), `impl/selfhost/front/parser.cb`, `impl/selfhost/back/refs4.cb`,
`impl/selfhost/back/gen.cb`, `impl/selfhost/back/gen_std_values.sh`, `impl/selfhost/cbc/driver.cb`,
`impl/selfhost/cbc/main.cb`, `impl/selfhost/cobfmt/main.cb`, `impl/selfhost/cobfmt/tests.py` (new),
`impl/build.sh` (new), `impl/cobfmt/` (removed), `impl/selfhost/compare.sh` (removed), `impl/Cargo.toml`,
`impl/cobc/src/main.rs`, `impl/cobc/tests/driver.rs`, `impl/tools/gate.py`, `impl/tools/battery_stages.sh`,
`impl/tools/cobdoc.py`, `impl/tools/card.py`, `CARD.md`, `README.md`, `impl/STATUS.md`, the book, the guide

## Why

D-0326's first phase: before any Rust part can go, nothing the CobaltC toolchain is built from may need a Rust
tool to be written, and the smallest Rust part with a CobaltC twin, the formatter, goes first. The owner's
question of 2026-11-15 ("why both [coby and cobc] need to exist, and which to use and when") is answered for
newcomers at once: they are shown one tool.

## What changes

- **`cbc --prelude-summary STD_DIR`** writes `impl/selfhost/front/prelude.cb` from `std`'s files, as
  `coby --prelude-summary` writes it from the `std` compiled into coby: the assembly of the prelude, its
  skeleton, its declarations, the counters its parse leaves, each body with the counters where it begins, the
  intrinsics' names, the run-time diagnostics, where each line came from, and `std`'s documentation records,
  with each check the Rust makes (the skeleton gives the prelude's pre-scan tables; the declarations parse to
  the prelude's items; each body parses alone to the prelude's parse of it). The parser written in CobaltC
  gained what that needs (`prescan_tables`, `parse_whole`, which records where each item begins with the
  counters there, `parse_alone`, `parse_item_after`). The tables the Rust keeps in `typecheck.rs`
  (`INTRINSIC_NAMES`, `FLOAT_FNS`, `FLOAT2_FNS`) are kept in `summary.cb` as well, in step with the Rust while
  it exists (D-0296); the two summaries are compared by the gate.
- **`cbc` under `COBALTC_STD_FN_VALUES`** prints the names `std`'s own bodies use as values, as `cobc` does
  (`std_value_names`, every `std` body parsed for it); `gen_std_values.sh` uses `cbc` by default.
- **`impl/build.sh`** builds the toolchain written in CobaltC with the compiler already installed (the `cbc`
  of the previous build, kept as `impl/target/prev/cbc`, else the Rust `cobc`) and installs the formatter as
  `impl/target/release/cobfmt`, the name every script, test and document already uses.
- **The Rust formatter is retired.** `impl/cobfmt/` leaves the workspace and the tree; the CobaltC formatter,
  which reproduced it byte for byte on every tracked file five ways (`selfhost/compare.sh`, removed with it),
  takes its name, and its messages say `cobfmt`. Its 17 unit tests and 18 command-line tests are carried over,
  case for case, to `impl/selfhost/cobfmt/tests.py` (Python 3's standard library only), which the gate's new
  `formatter` unit runs.
- **The records read the CobaltC sources**: `cobdoc.py` its natives and intrinsics (`front/modres.cb`,
  `front/typecheck.cb`, `cbc/summary.cb`; the lists are the ones the Rust gave), `card.py` the tools' text
  (`cbc/driver.cb`).
- **One tool for programs.** `cobc --about` and `--help` (both compilers, still byte for byte) present
  `cobc` as the one command a program needs and name no other; `CARD.md` drops `coby`'s usage; the README, the
  book and the guide show `cobc` for checking, running, testing, exploring and documentation; `impl/STATUS.md`
  describes `coby` as the project's internal checker, to be retired (D-0326 phase E).

## Checked

- `cbc --prelude-summary impl/std` = `coby --prelude-summary` = the committed `prelude.cb`, byte for byte.
- `cobdoc.py`'s natives (26) and intrinsics (71) from the CobaltC sources equal those from the Rust.
- `impl/selfhost/cobfmt/tests.py`: 35 of 35 on the Rust formatter before its removal.
- The gate on the phase's commit (run `20261115_043905`), green in every unit: the build with `prelude.cb` current
  as `cbc` and `coby` write it and the value names current as `cbc` writes them, the formatter's 35 tests on the
  CobaltC formatter, the records, the portable check, the test targets, both corpora under both compilers, `std`'s
  151 tests, the guide's 426 examples, the book's 79, the 91 showcases, the model, the self-hosted toolchain
  (923 `--doc` queries the same) and the lowering's 18 instruction counts within 3% of the baseline.
