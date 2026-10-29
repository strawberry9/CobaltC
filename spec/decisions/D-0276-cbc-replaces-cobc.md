# D-0276 — `cbc`, the compiler written in CobaltC, replaces `cobc`

Status: ACCEPTED (2026-10-23, decided by the assistant under the owner's delegation of 2026-10-22: "proceed
with the plan as recommended ... full authorisation is provided to make all necessary decisions needed to
complete the plan in full")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0274 (the bootstrap form), `CHG-0368` (its levels), the self-hosting plan's definition of done
(decisions D1–D10 of 2026-10-22)
Affects: `impl/selfhost/` (`cbc/`, `back/`, `front/`); `impl/selfhost/compare_c.sh`, `fixpoint.sh`,
`build.sh`; `impl/cobc/tests/` (`COBC=`); `impl/src/check.rs`, `impl/src/parser.rs` (`--prelude-summary`);
`impl/STATUS.md`, `README.md`, the guide's introduction

## Problem

Steps 1–3 of self-hosting gave a front end and a code generator written in CobaltC that reproduce `coby`'s
front end and `cobc --bootstrap-form`'s C, and compile themselves to a fixed point. `cobc`'s optimisations,
its driver (options, the C compiler, linking, targets, the object cache, `--unittest`, `--run`), and the
default form it writes were still Rust only. Step 4 was to port them, and to decide when the compiler
written in CobaltC can stand in for the Rust one.

## Options

1. Declare completion at the bootstrap form's fixed point (Update 130): the self-built compiler exists, but
   it writes C that is many times slower and has no driver; `cobc` stays the only compiler a user can use.
2. Port the optimisations and the driver, and require, before completion, byte-identical C in the default
   form on every tracked program and in every option that changes the C, the fixed point in the default
   form, and `cobc`'s own test suites passing with `cbc` as the compiler under test, under gcc and clang.
3. As 2, and also retire `cobc` from the tree.

## Decision

Option 2. `cbc` is `cobc` written in CobaltC: `cbc [options] FILE.cb` takes every option `cobc` takes and
writes the same C, byte for byte, which it compiles and links as `cobc` does. It replaces `cobc` as the
compiler the project is built and verified with. `cobc` stays in the tree as the Rust reference and the
oracle `compare_c.sh` compares against (retiring it is a later decision of its own); `coby` stays the
reference interpreter and `cbrt` stays Rust.

What completion rests on, as measured on 2026-10-23:

- The C: `compare_c.sh` at level 0 (the default form) on every tracked `.cb` file `cobc` accepts (1,199
  programs: conformance, examples, showcase, book, the selfhost sources): all the same. The same with
  `--all-checks`, at each of the ten levels of D-0274's bootstrap form (`COBALTC_BOOTSTRAP_UPTO=1..10`),
  and through both drivers with `-g` and `--no-line-directives` (`FLAGS=`) on a sample of the corpus;
  `--lib` gives the same C and header on the library tests' library, and the same objects in the archive
  but for their names (object-cache keys, which name the compiler).
- The fixed point in the default form (`fixpoint.sh`): `cbc` built by `cobc` writes its own C (110 MB),
  which is `cobc`'s; gcc builds a second `cbc` from that C, which writes the same C again.
- `cobc`'s tests with `COBC=cbc` (`cargo test -p cobc`): the compiled conformance suite, the
  specification's rows, the driver, the object cache, library mode, differential, tracking, output order,
  input, extern code and the runtime's bookkeeping, under gcc and clang.

What it does not rest on: speed. `cbc` compiles about 15 to 25 times more slowly than `cobc` (its own source:
about 25 minutes against 75 seconds), against the plan's working bound of ten; the programs it builds are the
same programs, as fast. The cost is the runtime's object tracking on compiler-shaped code, spread over every
operation (the profiles in `stress/selfhost4/perf/`), which is work on compiled CobaltC in general, not on
`cbc`: the next phase.

## Consequences

- A change to `cobc`'s lowering is now made in two places until `cobc` is retired: `compare_c.sh` and the
  fixed point say when the two agree.
- `--prelude-summary` carries what `cbc`'s driver needs of `std` beyond the front end's: where each prelude
  line came from (a diagnostic inside `std` is placed `<std>/FILE:LINE`) and `std`'s own tests
  (`--unittest-std`).
- `cbc --run` starts a program on Unix as `cobc` does (ended with `cbc`, `COBALTC_CPU_LIMIT`), its
  arguments passed on byte for byte through `cbc`'s own C shim (`cbc/host.c`), since `std`'s commands take
  text.
