# CHG-0467 — After the move off Rust: what the Rust tools and the interpreter left behind (implementation only)

Status: implemented (2026-11-17); Update 197
Affects: `impl/tools/gate.py`; the comments of `impl/front`, `impl/back`, `impl/cobc`, `impl/cobfmt`,
`impl/cbparse`, `impl/model`, `impl/std`, `impl/cbrt`, `impl/tests` and of the cases of `impl/conformance`;
`impl/cobc/driver.cb`, `driverbase.cb`, `host.cb`, `host.c`, `main.cb`, `summary.cb`, `impl/back/gen.cb`
(messages, `--explain --run`); `impl/tests/cbtest`, `impl/tests/differential`, `impl/model/src/compare.cb`
(`--coby`); `impl/bootstrap/cobc-seed.c.gz` (renamed), `boot.c`, `mkseed.sh`, `SEED`; `impl/tests/programs`
(renamed from `conformance_rs`), `impl/tests/cases`; `impl/tests/PORT-MAP.md`, `portmap_check.py` (removed);
`CARD.md`; the guide (`content_intro.py`); the book; `README.md`; `impl/STATUS.md`; `spec/ASSURANCE.md`;
`showcase/README.md`; `spec/21` §0 (the trusted table's line numbers); `spec/IMPLEMENTATION-NOTES.md` (the
runtime contract's table); D-0328; D-0326 (a note)

## Why

The move off Rust (D-0326, Updates 192 to 196) left the tree speaking of things that are no longer there: a
compiler in Rust, an interpreter (`coby`), a second name for the compiler (`cbc`), the Rust files each part
was ported from. On 2026-11-17, the day Update 196 was pushed, the owner went through them one by one.

- **The gate's `wine` unit** ran the whole suite, compiled for Windows from Linux, under Wine, and was added
  with D-0326 while the runtime's Windows side was being written in C. GitHub's Windows runner runs the suite
  on Windows itself at every push; the one Windows defect since (CHG-0460) showed there and never under Wine;
  two cases cannot run under Wine, and running them took the machine's memory (CHG-0466). "Drop the wine unit
  in the next update, keep portable."
- **The ref `rust-reference`**, removed from the published repository (D-0328), was still named by six texts
  as where the Rust tools are.
- **A backtrace the tool does not have.** `cobc`'s usage text and the guide said an internal error comes with
  "Rust's backtrace when RUST_BACKTRACE is set". Asked whether CobaltC needs a `COBALTC_BACKTRACE`, the owner
  decided against one: a debugger gives the calls already, and the guide did not say how.
- **Comments that name a Rust file** (`typecheck.rs`, `lib.rs` lines 1-1400, "as the Rust runtime did"), the
  interpreter or `cbc`: about two thousand lines of them in the tools' sources, the runtime and the cases.
  They were the port's working notes; with the Rust tree unpublished they point at nothing a reader can open.
- **The dormant `--coby` options** of `cbtest`, `difftest` and `cobmodel compare`, which D-0326 kept for a
  reference run "while there is one". There is none, and no tree to build one from.
- **The seed's name**, `cbc-seed.c.gz`.
- **The tests' provenance**: `impl/tests/PORT-MAP.md`, which gave each of the Rust tools' 674 test functions
  its new home, and its check; the first line of the command cases, which named the Rust test each came
  from; the directory `impl/tests/conformance_rs`, each of whose 683 cases named its Rust test in two header
  lines. The map could no longer be held to anything but its own count.
- **The book**, audited against the tools as they are.

The audit of the book found a defect of the port, not of the book: `cobc --explain --run` showed a fault in
its short form. `--explain` sets `COBALTC_EXPLAIN` for the program `--run` starts, as the usage text says; the
compiler written in CobaltC keeps the variables its options set in a table of its own, gives them to every
command it runs, and on Unix starts the program of `--run` another way (its arguments byte for byte), which
did not take them.

No rule changes.

## What changes

- **The gate.** The release profile has no `wine` unit; the gate profile's `portable` unit is as it was (the
  runtime built by MinGW-w64 and by Clang without a warning, and a program compiled for Windows run under
  Wine). `impl/conformance/run.py --skip` (CHG-0466) stays; the memory cap went with the unit. The texts under
  `impl/tests` (`*.md`) are no longer inputs of the units that take the toolchain.
- **The Rust ref.** `README.md`, `impl/STATUS.md` and `spec/ASSURANCE.md` (two rows of kind `port`, which now
  point at `CHG-0459`) say that the comparisons with the Rust tools are recorded and the tools no longer
  published. D-0326 has a note; D-0328 is the decision.
- **`cobc`.** The usage text's line for exit status 4 says an internal error is one line, and no more; the
  internal-error test point no longer reads `RUST_BACKTRACE`. The eleven messages that began `cbc:` begin
  `cobc:`. `--explain --run` explains a fault: the program `--run` starts is given the variables the options
  set (`env_flags`, `run_raw`), on Unix as on Windows. The C functions of the compiler's host file are
  `cobc_host_*`.
- **The comments.** In the tools' sources, the runtime, the library, the model and the cases, a comment says
  what the code does and why, with no pointer to a Rust file, the Rust tools, the interpreter or `cbc`; one
  that was only such a pointer is gone. A comparison with Rust the language or its library that says what a
  function does (`{:?}` of a text, `f64::round`) stays. The runtime's header changed in its comments, so the
  compiler's copy of it (`impl/back/header.cb`) and the seed did too.
- **`--coby`** is gone from `cbtest` (the reference of a case is its recorded `.expect`), from `difftest`
  (`--self` is its one way; `--reference coby` and `--all-checks` went) and from `cobmodel compare`.
- **The seed** is `impl/bootstrap/cobc-seed.c.gz`.
- **The tests.** `PORT-MAP.md` and `portmap_check.py` are removed. A command case begins with what it checks.
  `impl/tests/conformance_rs` is `impl/tests/programs`; its cases have lost their `from:` and `helper:` lines,
  two lines each, and their recorded runs are recorded again, two lines up.
- **The guide.** The tour's paragraph on internal errors no longer names `RUST_BACKTRACE`. "Debug it" says how
  to see the calls that led to a fault: `break exit` in `gdb` before `run`, then `bt`.
- **The book**: `slice_eq` is the library's, not chapter 10's; `handle` is not a name `std` exports; the
  showcases named in chapter 16 are ones that exist; `Atomic::notify_one` by its name.
- **Other texts**: `README.md` no longer names `impl/tools/trace_compare.py`, a tool that left with the
  interpreter; `showcase/README.md` no longer speaks of two implementations in the present.

Left as it is: the history in `impl/CHANGELOG.md`, `spec/changes` and `spec/decisions`, and in
`showcase/README.md` and `impl/model/README.md` where they tell what was once found.

## Checked

- **The comments changed and nothing else**: of the 252 files of the tools, the runtime, the library, the model
  and the suite that the sweep changed, 239 are, with their comments taken out, the text of the commit before;
  the other 13 are the ones whose code changes are listed above (two of them generated: the compiler's copy of
  the runtime's header, and the prelude's summary, which follows the library's lines). The cases of
  `impl/conformance` keep their line counts, so their recorded runs stand; a command case changed in its
  leading `#` lines alone.
- **The recorded runs of `impl/tests/programs`**, recorded again by GCC and Clang agreeing, 683 of them: 379
  are the bytes they were, and 304 the same but for line numbers two smaller.
- **`--explain --run`**: a new case (`driver_explain_run.case`): the fault's whole entry, the same as under
  `COBALTC_EXPLAIN=1`, and the short form without either.
- **The debugger's answer**, on a fault three calls deep built with `cobc -g -O 0`: `bt` at `exit` lists the
  three functions with their `.cb` lines and the faulting call's arguments.
- **The book**: every chapter read against `CARD.md` and the tools; its 79 examples give their recorded output.
- `conf.discard-underscore-destroys-now` was held covered by the map alone; its case names the row now
  (`impl/conformance/13-expression-semantics/discard_destroys_now.cb`), and `conf_coverage.py` finds all 2 130.
- The tables written from the sources are written again: `spec/21` §0's trusted table (six line numbers of
  `io.cb`) and the runtime contract in `spec/IMPLEMENTATION-NOTES.md` (one comment).
- `impl/tools/speccheck.py`, `assurance.py`, `card.py --check`, `impl/bootstrap/mkseed.sh --check`, the
  generators of `impl/tests/gen` (`--check`), and the gate profile on the final commit.
