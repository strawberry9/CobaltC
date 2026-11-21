# CHG-0459 — The tests leave Cargo, the interpreter is retired, the toolchain has no Rust

Status: implemented (2026-11-16), D-0326 phases D to G; Update 194
Affects: `impl/src/` (removed: `coby`, the Rust front end), `impl/cobc/` (the Rust compiler removed; the driver
written in CobaltC moved here from `impl/selfhost/cbc/`), `impl/front/`, `impl/back/`, `impl/cobfmt/`,
`impl/cbparse/` (moved from `impl/selfhost/`), `impl/model/` (the model in CobaltC in place of the Rust one),
`impl/tests/` (the Rust test targets removed; `cbtest`, its cases, `PORT-MAP.md`, the rows' recorded runs, the
cases from `tests/conformance.rs`, the random-program tester), `impl/conformance/*/NAME.expect` (new),
`impl/Cargo.toml`, `impl/Cargo.lock`, `impl/cbrt/src/link_info.rs` (removed), `impl/build.sh`,
`impl/bootstrap/`, `impl/back/stack_checks.cb`, `impl/back/gen.cb`, `impl/tools/gate.py`,
`impl/tools/battery_stages.sh`, `impl/tools/factcheck.py`, `impl/tools/assurance.py`,
`impl/tools/trace_compare.py` (removed), `impl/front/consteval.cb`, `impl/front/ast.cb`, `impl/cobc/main.cb`, `impl/cobc/host.c`,
`impl/cbrt/src/rt_threads.c`, `spec/conformance.md`,
`htmlguide/generator/`, `book/`, `showcase/`, the documents

## Why

D-0326's last phases, in one update as the owner amended it: the tests run without Cargo, every case's run is
recorded while the interpreter can still agree to it, the interpreter and every Rust tool are removed, and the
tools take the names everything uses (M2, M3, M4: a change to the language or the lowering is written once, and
nothing in the repository is Rust).

## What changes

- **The tests** are files run by `cbtest`, a runner written in CobaltC (`impl/tests/cbtest/`): the conformance
  suite and the specification's rows as before, under GCC and Clang, held to each case's header and to its
  recorded run; the cases the 674 Rust `#[test]` functions made (`impl/tests/PORT-MAP.md` names each one's new
  home: a case of `impl/tests/cases/`, a check of `impl/tests/checks/`, one of `cbtest`'s commands, or, for the
  567 of `tests/conformance.rs`, a file of `impl/tests/conformance_rs/`); the random-program tester
  (`impl/tests/differential/`, the Rust generator ported, the same programs from the same seed).
- **Every case's run is recorded** (`NAME.expect` beside it: the exit status and both streams), when the
  interpreter, GCC and Clang agreed on it and on the header; an inline row's in `impl/tests/rows/`. A case added
  from now on is recorded from GCC and Clang agreeing (`cbtest record` without `--coby`), its text reviewed
  against the rule.
- **The interpreter is retired**: `coby`, its evaluator and the Rust front end (`impl/src/`) are removed, and
  nothing runs it (the gate, the battery, the guide's, the book's and the showcases' checkers, the fuzzer, the
  model's comparison, the record scripts).
- **The Rust compiler and the Rust model are removed**; the model is the port in CobaltC (D-0326 phase F:
  byte-identical to the Rust one on every deterministic check), importing `std` only.
- **Names** (D-0326 Q5): the compiler written in CobaltC is `cobc` (`impl/target/release/cobc`), the formatter
  `cobfmt`, the model `cobmodel`; the sources are `impl/front`, `impl/back`, `impl/cobc`, `impl/cobfmt`,
  `impl/cbparse` (kept: `impl/tools/factcheck.py` reads its `--dump-ast`) and `impl/model`. `impl/build.sh`
  builds all of them, the runtime first when it changed.
- **What replaces the two-implementation checks** (D-0326 §3), in the gate: `bootstrap` (a clone built from the
  seed by GCC and by Clang, each compiler writing the seed again), `differential` (the compiler against itself
  on random programs: the default form, every check, the full record, GCC and Clang), the recorded runs
  (`corpus`), the model, the Python readings and the replays.
- **Two zero-sized locals have two addresses under Clang 15 and later** (F193-1): a local of a zero-sized type
  is declared beside a byte of its own (`union { struct T v; uint8_t cb_anchor; }`), its value's size still 0;
  `conf.zero-sized-addresses` now passes under Clang 18. The GitHub Clang job runs on the latest runner again.
- **`static_assert` computes every constant expression of `spec/17` §1a** (F194-2): the compiler's evaluator
  (`impl/front/consteval.cb`) had the checked arithmetic, `sizeof`, `alignof`, the integers' limits, `widen` and
  `narrow`; the shifts, a minus applied to a literal as one value (`-2147483648: i32`), `narrow_wrapping`,
  `reinterpret`, `to_float`, `to_int`, float arithmetic in the operands' type, the floats' limits, `wrapping_*`,
  `saturating_*` and text comparison were the interpreter's alone, which six of the guide's examples used and no
  conformance case did. A `narrow` that does not fit in one is `diag.narrowing-overflow`, as the registry names
  it. Three cases: `conf.static-assert-constant-forms`, `conf.static-assert-shift-amount`,
  `conf.static-assert-conversion-fails` (`spec/conformance.md` 3.314.0).
- **`cobc --emit-c` of a library** says it is one, as the build does (it failed an assertion of the compiler's
  own; `impl/tools/validate_c.py` reads the message).
- **The compiler reads deeply nested expressions as the Rust one did** (F194-3): 100 000 nested parentheses and a
  sum of 100 000 terms compile again. The Rust compiler ran its work on a thread with a 1 GiB stack and remembered
  each node's answer to "is this a literal expression"; the compiler written in CobaltC ran on a program's 256 MiB
  and walked the subtree at every level (a sum of 30 000 terms took a minute, 40 000 exhausted the stack, as did
  10 000 parentheses). Now a node holds the answer from when it was made (`Expr::lit`, `impl/front/ast.cb`;
  a node whose kind is replaced in place takes it again), and the compiler asks the runtime for a 4 GiB stack
  before `main` starts (`cbrt_request_stack`, `impl/cbrt/src/rt_threads.c`, called by `impl/cobc/host.c`;
  reserved, so only what a program's nesting uses is committed; not part of `cbrt.h`, which generated C alone
  uses). `impl/tests/checks/deep_expressions.py` holds it in the gate.
- **The documents**: the README, `impl/STATUS.md`, `CARD.md`, the guide, the book, the showcases', the suite's,
  the model's and the examples' READMEs, `spec/ASSURANCE.md`, `spec/IMPLEMENTATION-NOTES.md`, `NAMING-POLICY.md`;
  seven chapters' asides that named `coby` (editorial patch versions). The last tree with the Rust tools is the
  tag `rust-reference`.

## Checked

- **The recording** (before the interpreter was removed): the interpreter, GCC and Clang agreed on every case's
  run: the suite's 1 417 cases (the slow ones included), the 545 inline rows that run, the 683 cases from
  `tests/conformance.rs`. Two stack-exhaustion cases agreed on the diagnostic and differed in the location the
  interpreter gave, which contradicted the registry ("the call"); `cbtest` compares that diagnostic's first line.
- **The gate**, run `20261115_215656` on the tree before the edition's number was written in, all fifteen units
  green: `build` (the seed current, 75 996 015 bytes of C), `format`, `formatter` (35 of 35), `records`,
  `portable` (the runtime by MinGW-w64 and by Clang without a warning; a Windows program under Wine), `sweep-check`,
  `tests` (143 of 143 cases), `corpus` (the suite 1 416 passed and 4 skipped, the rows 2 115 passed and 8 skipped,
  the cases from `tests/conformance.rs` 683 of 683, each under GCC and under Clang, against the header and the
  recorded run), `std-tests` (151 of 151), `guide` (426 examples), `book` (79), `showcase` (91), `model` (315
  programs compared, none with an outcome outside the model's; 239 certified and 76 rejected by both; 403 bodies
  read by `factcheck.py`, all agreeing; `validate_c.py` 412 programs, no finding, the library told apart; the
  replays), `bootstrap` (a clone built from the seed by GCC and by Clang, each writing the seed again byte for
  byte), `differential` (100 random programs, the compiler against itself), `lowering` (the decided sweep: 6 928
  decided, none decided before and not now; the perf guide's 72 rows and the bench's 18 counts within 3% of the
  baseline).
- **The gate before it**, run `20261115_190153`, was red in four units, each for a fault above: `guide` (F194-2),
  `model` (`--emit-c` of a library), `lowering` (F194-2's six programs and F194-3's seven), `portable` (the
  gate's own command).
- **F194-3**: the C and the diagnostics the compiler writes are the same before and after `Expr::lit`, byte for
  byte, on 3 105 programs (the suite, the cases from `tests/conformance.rs`, the showcases, the book, the
  examples, the bench, the guide's examples, the model, the formatter, `cbtest`, the random-program tester);
  100 000 nested parentheses compile in 5 s (2.9 GB of stack at the peak), a sum of 100 000 terms in 9 s.
- **GitHub's runners** (run 43766508918, this tree): `boot` on Ubuntu with GCC and with Clang 18 (F193-1: the
  suite passes there), and on Windows Server with MinGW-w64's GCC from a command prompt, the seed written again
  byte for byte and the suite 1 406 of 1 406 (D-0326's checkpoint W2, on a runner in place of the Windows 7
  machine, as the owner decided).
- **F194-1** (a nested `foreach`'s synthesized ids): `impl/tests/cases/cobc/lowering_nested_foreach_alias.case`.
- The release profile was not run (it runs on the owner's word).
