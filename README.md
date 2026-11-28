![Project Logo](images/CobaltC_Circular_Tech_Logo_small.png)

This is the official home of the CobaltC programming language-design project.

CobaltC is a human-directed, AI-assisted, statically typed C-family systems programming language that emerged from an exploration of C's memory-safety flaws, informed by [The Wrong Memory](https://strawberry9.github.io/the-wrong-memory/cover.html), an open-source online book and collection of essays examining the problems and consequences of unsafe memory management.

CobaltC's design is guided by two central commitments: making safety-relevant facts explicit in the language, and grounding implementation behavior in stable, normative rules.

CobaltC's safety rules are checked in two places. The compiler proves what it can at compile time, eliminating the corresponding runtime checks. Conditions it cannot prove are checked during execution, by cbrt (the CobaltC runtime).

The language combines affine resource ownership, explicit shared and exclusive references, deterministic destruction, and a hybrid static/dynamic enforcement model. Its design prioritizes a small, composable language, predictable low-level control, and defined failure over maximal static verification.

CobaltC is specified, implemented and usable, but remains experimental.

This project began on 19 September 2026, based on the instructions provided (to the AI) by the project owner (me), as set out in [CobaltC_Master_Instructions.md](CobaltC_Master_Instructions.md).

Commit Update 216, 2026-11-28 08:21 UTC. Specification edition 2026.112802. The repository is kept as a single commit that each update replaces, so this line, rather than the history, says when that last happened; the edition, recorded in [spec/README.md](spec/README.md), changes only when the specification does.

The language specification is available [here](https://github.com/strawberry9/CobaltC/tree/main/spec)

NOTE: The language specification is intentionally designed by AI for AI consumption rather than human consumption. For human consumption, a plain prose overview of the specification is available in [spec/OVERVIEW.md](spec/OVERVIEW.md).

In most cases, the best place to begin learning is the language guide and reference, which is available [here](https://strawberry9.github.io/the-wrong-memory/Appendix_06.html); its source is primarily from `htmlguide/index.html` in this repository, regenerated from `htmlguide/generator/` (every example in it is checked against both the interpreter and the compiler).

The language guide is dynamically updated as soon as new language clarifications and/or capabilities arrive.

The CobaltC Programming Language Book is also a useful introduction to the language, and is available [here](https://strawberry9.github.io/CobaltC)

Another good way to begin learning CobaltC is by viewing the many showcase examples (see further below - `showcase/README.md` describes them all).

For writing CobaltC with the details at hand, `CARD.md` puts the syntax, the differences from C and Rust, the tools, the diagnostics and every item of the standard library on one page; it is generated from those sources (`impl/tools/card.py`), so it is always current.

## What is in the repository

- `spec/` — the language specification: numbered normative artifacts, decision records (`spec/decisions/`), change records (`spec/changes/`), the diagnostic and feature registries, the conformance table, and the examples.
- `impl/` — the toolchain:
  - `cobc`, the compiler, and the one command a program needs: it checks a program, builds it into a native executable through C compiled by the system `cc`, runs it, runs its unit tests and shows the standard library's documentation. It is written in CobaltC (`impl/front/` the front end, `impl/back/` the code generator, `impl/cobc/` the driver) and compiles itself; `impl/bootstrap/` holds the seed it is first built from (D-0326).
  - `cbrt`, the runtime every compiled program links, written in C: it keeps the checks the specification discharges at run time — access paths, aliasing, temporal validity, resource destruction — over real addresses (`impl/cbrt/`).
  - `cobfmt`, the code formatter, written in CobaltC (`impl/cobfmt/`): it rewrites `.cb` files into the house style of the guide, the showcases and the examples, changing only layout, never a token.
  - `cobmodel`, an executable model of the alias and temporal rules written from the specification alone, sharing no code with the compiler (`impl/model/`), against which the compiler's runs are compared.
  - `impl/tests/` — the test runner `cbtest` (written in CobaltC), its cases, and the random-program tester.
- `impl/conformance/` — the file-based conformance suite; `impl/cobaltc_examples/` — small example programs; `htmlguide/` — the human-readable guide and its generator.
- `showcase/` — complete example programs chosen to show what is particular to CobaltC; `showcase/README.md` describes them all.
- `book/` — *The CobaltC Programming Language*, a book for learning the language from the beginning, in Markdown (`book/src/`), published at https://strawberry9.github.io/CobaltC/ (rendered by mdBook through `.github/workflows/book.yml` on each push); every program in it is kept under `book/examples/` and checked against its recorded output, under GCC and Clang, by `book/check.sh`, which the gate runs.
- `bench/` — the compiler measured against Rust on two compute-heavy workloads, with a script that reproduces the numbers; the results are in `bench/RESULTS.md`, and `bench/README.md` explains them.
- `impl/model/` — `cobmodel`, an executable model of the alias and thread rules written from the specification alone; `impl/fuzz/` — the standard library parsers' fuzzer and the interoperability checks; `impl/conformance/run.py` — a standalone runner that checks any implementation against the conformance suite.
- `spec/OVERVIEW.md` — the semantics in prose, citing the rules paragraph by paragraph; `spec/STABILITY.md` — what an edition promises about the ones after it; `spec/ASSURANCE.md` — the assurance case: what is proved, what is checked and what is tested about the rules, the compiler and the runtime, and what a user can check of their own program.
- `impl/STATUS.md` says what the tools do now, its limits and platforms, and how to build and test it; `impl/CHANGELOG.md` is the dated record of the work, one section for each piece of it; `impl/COBC-PLAN.md` is the compiler's original plan and milestone record.

## Building

CobaltC is built with a C compiler alone (D-0326): GCC 10 or later, or Clang 14 or later, for x86-64; on Windows, MinGW-w64's GCC. Nothing else is needed: no Rust, no Python, no shell.

Linux:

```
sudo apt install gcc                     # or clang; any distribution's package
git clone https://github.com/strawberry9/CobaltC && cd CobaltC
gcc impl/bootstrap/boot.c -o boot && ./boot
```

Windows (7 or later), in a command prompt, with a MinGW-w64 GCC on `PATH` (w64devkit or WinLibs; the `msvcrt` build for Windows 7):

```
git clone https://github.com/strawberry9/CobaltC
cd CobaltC
gcc impl\bootstrap\boot.c -o boot.exe
boot.exe
```

`boot` builds the runtime (`impl/cbrt`, C), unpacks the seed (`impl/bootstrap/cobc-seed.c.gz`: the C that the compiler's own source in the same commit compiles to), compiles it into `impl/target/release/cobc`, and proves the result: the new compiler writes the C of its own source again, which must be the seed byte for byte (`--no-verify` skips the proof). Then it builds the formatter and the model with that compiler, compiles and runs a first program, and prints the line that puts the tools on `PATH`. It takes a few minutes, most of it the one large compile, and about 3 GB of memory at the peak. `impl/bootstrap/mkseed.sh` writes the seed from the source; the gate checks on every update that the committed seed is the C of the committed source. `impl/bootstrap/README.md` gives each of these steps as commands you can run yourself, and what `impl/bootstrap/mkseed.sh` does to write the seed again.

Later builds on the same machine do not use the seed: `impl/build.sh` builds the runtime when its sources changed and the tools with the compiler already installed, into `impl/target/release/` (`cobc`, `cobfmt`, `cobmodel`, `cbparse`) and `impl/target/tests/` (`cbtest`, `difftest`). When the runtime has changed it builds in two stages, since a compiler refuses a runtime built from other sources than its own (the runtime's stamp).

Every push is built this way on machines that have never seen the project: GitHub's Linux runners (GCC and Clang) and a Windows runner (MinGW-w64, from a command prompt), each a fresh clone with no Rust on `PATH`, then the conformance suite (`.github/workflows/bootstrap.yml`).

## Testing

```
impl/build.sh
impl/target/tests/cbtest conformance --cobc impl/target/release/cobc        # the file suite, GCC and Clang
impl/target/tests/cbtest rows --cobc impl/target/release/cobc               # every row of spec/conformance.md
impl/target/tests/cbtest cases --root impl/tests/cases --cobc impl/target/release/cobc   # the tools' own cases
impl/gate.sh                                                                # everything, as before a push
```

`cbtest` (`impl/tests/cbtest/`, written in CobaltC) compiles every conformance case with `cobc --run` under GCC and under Clang and holds it to its header (outcome, phase, the diagnostic) and to its recorded run, `NAME.expect` beside it: the exit status, standard output and standard error, written when the retired interpreter, GCC and Clang agreed on them (D-0326 phase E). An inline row of the specification's table is held to `impl/tests/rows/ID.expect` likewise. C that one compiler accepts but the other reads differently shows up as a failure, as a bare `-9223372036854775808` once did; both compilers must be installed (Ubuntu: `sudo apt install gcc clang`), or `--cc` names those to use. `cbtest cases` runs the tools' own cases (`impl/tests/cases/`): commands, their inputs and what their output must and must not hold; `cbtest conformance --root impl/tests/programs` runs the tools' own program cases, in the suite's format.

**These runs take a while.** To test everything the way the project does before a push, run `impl/gate.sh`: it runs every check as a unit on a snapshot of the commit, so you can keep working while it runs, keeps each green verdict under the hash of the unit's inputs and never runs a unit again on the same inputs, and the corpus runner keeps a result per case, so after a small change only what the change can reach runs again (`impl/gate.sh --list` shows the units; `impl/gate.sh release` adds the slow ones).

The static pass checks the standard library's items as a program reaches them (through calls and function values, and on demand when a tool needs one it did not reach), not all of `std` on every start; `COBALTC_CHECK_ALL=1` checks every item, which is how the test suite verifies `std` itself.

The compiler is also tested against itself on random programs (`impl/tests/differential/`): random, well-typed programs exercising the checks the compiler proves unnecessary or makes cheaper (vector pushes and element accesses, element references held across reallocation, reference parameters, moves, resources ending in nested blocks, arithmetic that may fault), each built in the default form, with every check (`--all-checks`) and with the full record (`COBALTC_RECORD=full`), under GCC and Clang, must behave alike:

```
mkdir -p /tmp/d && TMPDIR=/tmp/d impl/target/tests/difftest --self --cobc impl/target/release/cobc --n 2000 --seed 7
```

A program they disagree on is kept, and the report names the form and the C compiler.

The compiler has a verify mode (D-0261): with `COBALTC_VERIFY=1` in the environment, a compiled program's runtime re-derives, at every statement's end, that its path and object tables agree with each other (every valid path's object lists it, every listed token is a path of that object, every ancestors list is closed). A violation is a defect of the tool, never of the program, and the runtime stops naming it. `COBALTC_VERIFY=N` checks every Nth time instead, and the runtime checks no more often than its state's size allows (once per 64 live objects' worth of events), so the mode costs a bounded factor on any program. The battery runs both conformance suites under it.

`proofs/` is a Lean 4 development (D-0265; `cd proofs && lake build`, with the toolchain `elan` installs) that states the alias and temporal fragment's abstract machine and proves, by cases on its steps, that every reachable state keeps the lemmas `spec/23` argues in prose; it is not needed to build or test the tools. The runtime also carries bounded proofs for its smallest tables: `impl/cbrt/tests/cbmc/run.sh` (CBMC, `apt install cbmc`) proves six harnesses over the runtime's own C (`impl/cbrt/tests/cbmc/harness.c`), which a normal build does not compile.

`cobc --certificate FILE prog.cb` (D-0299) writes what the static pass decided before the run: each function body the compiler keeps no record of references for (`decided`), each conflict it left to the run, the checks made when a call is made, and the program's verdict. For a program in the fragment the model covers, `cobmodel certify FILE prog.cb` runs every interleaving with those bodies kept unrecorded and reports any alias or temporal fault the rules would find there (`model.decided-but-false`); the battery does this for every sample program. `spec/ASSURANCE.md` sets out what each kind of evidence the project keeps establishes, and what remains trusted.

With `COBALTC_TRACE_EVENTS=1` a program also writes one `EV` line on standard error per event the compiler's elisions must not disturb: each user destructor run, the program's fault, an `exit`, the end (D-0264).

The formatter's tests (`python3 -I impl/cobfmt/tests.py`) check each layout rule, the command line (backups, `--check`, standard input, `--width`, `\r\n` files), and that the showcases, the examples, the benchmarks and the conformance cases are already formatted.

## Running a program

`cobc` takes the program's entry file, which may declare modules whose bodies are other files (`module m "./m.cb";`, `spec/17` §5). Whatever follows the file is passed to the program as its arguments: `cobc --run prog.cb a b` gives it `a` and `b`, which it reads with `arg_count()` and `arg(i)` (`spec/21` §2c). Argument 0 is `a`, not the program's name.

The standard library (`Vec`, `String`, `HashMap`, `HashSet`, `Rc`, `Option`, `Result`, `printf`, …) is the module `std`; a program uses it by beginning with `import std;` (`spec/21` §0).

```
./target/release/cobc --run cobaltc_examples/12_collatz.cb    # compile to a temporary directory and run
./target/release/cobc -o collatz cobaltc_examples/12_collatz.cb   # keep the executable (--keep-c keeps the C too)
./target/release/cobc --check cobaltc_examples/                # check statically; build and run nothing
./target/release/cobc --unittest --run cobaltc_examples/12_collatz.cb   # run the file's unittest blocks
./target/release/cobc --doc Vec::push                          # a standard library item's documentation
./target/release/cobc --about                                  # what cobc is
```

`cobc` passes `-O2` to `cc` by default; `-O LEVEL` (`0`–`3`, `s`, `g`) replaces it, and `-march=CPU` (e.g. `-march=native`) targets a specific processor, at the cost of the executable possibly not running on an older one. `--cc COMPILER` uses another C compiler, such as `clang`, in place of `cc`; which is faster depends on the program (`bench/RESULTS.md`). `cobc --help` describes every option.

`cobc --check PATH...` runs the static pass alone (the loader, the parser, name resolution and every static check) on each file named and each `.cb` file in each directory named, and runs or builds nothing: a program that waits for input, serves the network or writes files can be checked without doing so, and the C compiler is not run. A rejected program prints exactly the diagnostics a run or a build would print: every error that stands on its own, each statement of a function's body its own (D-0323; a last line says when statements that depend on a failed one were not checked); an accepted one prints nothing. `--doc NAME` shows an item of the standard library or one of the language's own functions, and `--doc --find WORD…` finds them by a word of their name, signature or summary. `cobc --check` also reports a missing file named by `extern "./…";`. In a directory, a file another program there loads as a module (`module m "./m.cb";`) is checked as part of that program, not on its own. The exit status is 0 when every program is accepted, 1 when one is rejected, 2 for a path that does not exist, a file that cannot be read, or a missing `extern` file; every file is checked whatever an earlier one gave.

`cobc --check-syntax [--origin PATH] (FILE | -)` runs the parser alone, on one file or on standard input (`-`), its module files found relative to `PATH` (the file itself by default): exit 0 when it parses, 1 when it does not (the error on standard error), 2 when it cannot be read. A program the static pass would reject but the parser builds parses. This is the test `cobfmt` makes before formatting a file.

A program that runs to completion exits with status 0, or with `main`'s value if it is declared `fn main() : u8` (`spec/15` §7); its destructors have all run by then. If the system refused some of its standard output (a full disk, a closed pipe), a program that would exit with 0 exits with 1, and says so on standard error unless the reader closed the pipe (`spec/21` `[Print-Failure]`). A program the specification rejects prints the diagnostic, the phase it was caught in (`static` or `dynamic`), the rule it violated and the `file:line`, then exits with status 1. A program rejected before it runs gets every error that stands on its own, one for each function or declaration, then a count (D-0321). The program `cobc --run` starts never outlives `cobc` (it is killed if `cobc` ends: on Linux by the system, on Windows by a job `cobc` joins for the run). An interrupt does not leave `cobc`'s temporary directories behind, from the moment it makes them, whether it comes while `cobc` compiles, during `--run`, `--explore` or a run under `COBALTC_RUNNER`: on Unix `SIGINT`, `SIGTERM` or `SIGHUP` to `cobc` goes to the program (or the runner) and to the C compiler `cobc` has started, and `cobc` removes the directories before it ends by the same signal; on Windows Ctrl-C, Ctrl-Break or closing the console reaches the program too, and `cobc` waits for it to end, removes them and ends with the status Ctrl-C gives, and `COBALTC_CPU_LIMIT=<seconds>` caps its CPU time; the test runners set it. `cobc` exits with status 3 and `unsupported: …` for a construct it cannot compile. A program reads and writes files, whole (`read_file`, `write_file`, `read_bytes`, `write_bytes`) or through an open `File`, make, list, rename and remove files and directories, read environment variables and the clocks, and read standard input (`read_line`, or `std`'s `read`). `extern fn` declarations call real C functions from libc and libm. A program can also name its own C code or an installed library with `extern "./mine.c";` or `extern "z";`, which `cobc` compiles and links with it, and that C may call the program's `extern fn` functions with a body by name. `cobc` writes the generated C in a directory of its own, and no output of `cobc` (the executable, the C `--keep-c` keeps, a library's header) ever replaces one of the program's own files: its source, its modules, or a C file it names; an output it cannot write is reported, with the system's reason, before anything is compiled.

## Building a library C calls

An `extern fn` with a body is a function C calls by that name (`spec/20` §3, D-0257); a file of such functions with no `main` is a library, and `cobc --lib` builds it. Its name is its C symbol, so a name C or its library already has (`write`, `time`, a keyword, one beginning with `_`) is refused:

```sh
./target/release/cobc --lib geo.cb          # writes libgeo.a, geo.h, and copies libcbrt.a beside them
cc main.c -L. -lgeo -lcbrt -lpthread -ldl -lm   # the header's first line gives the link line for the target
```

`geo.h` declares each struct that crosses by value as C does (its layout asserted), the library's own calls (`cobalt_init`, `cobalt_shutdown`, `cobalt_last_fault`) and every entry point twice: `name(…)`, whose fault ends the process as a program's would, and `name_try(…, &result)`, which contains a fault and returns 1 with the result untouched. Every check stays on inside the library; C buffers and strings come in as values (`Vec::from_raw`, `String::from_c`). `--target x86_64-pc-windows-gnu` builds the library for Windows.

## Compiling for Windows

`cobc` compiles for two targets (D-0255): `x86_64-unknown-linux-gnu`, the default on Linux, and `x86_64-pc-windows-gnu`, the default when `cobc` itself runs on Windows. Both are x86-64 with the same layouts, so a program means the same on each. Windows programs are compiled by MinGW-w64's GCC, which accepts the GNU C that `cobc` writes; Microsoft's `cl.exe` does not.

The runtime is C, built for Windows by MinGW-w64's GCC; it calls only what Windows 7 has (`_WIN32_WINNT` 0x0601).

**From Linux.** Install MinGW-w64 (`apt install mingw-w64`) and build the Windows runtime once:

```
impl/cbrt/build.sh --cc x86_64-w64-mingw32-gcc --out impl/target/x86_64-pc-windows-gnu/release
impl/target/release/cobc --target x86_64-pc-windows-gnu prog.cb      # writes prog.exe
```

`cobc` finds that runtime in `impl/target/x86_64-pc-windows-gnu/release` and uses `x86_64-w64-mingw32-gcc` unless `--cc` names another compiler. `--run` and `--explore` need a way to run a Windows program here: `COBALTC_RUNNER=wine64 impl/target/release/cobc --target x86_64-pc-windows-gnu --run prog.cb` (with Wine installed: `apt install wine64`). The conformance runner does the same with `--platform windows`:

```
COBALTC_RUNNER=wine64 python3 impl/conformance/run.py --platform windows \
    --impl "$PWD/impl/target/release/cobc --target x86_64-pc-windows-gnu --run {file}"
```

**On Windows.** Build with `boot.exe` as above ([Building](#building)); then `impl\target\release\cobc.exe --run prog.cb`. `cobc.exe` uses `gcc` by default.

On Windows, conformance cases marked `// platform: linux` or `unix` are skipped, saying so.

**Checked so far.** On every push a GitHub Windows runner builds CobaltC from a fresh clone with `boot.exe` and MinGW-w64's GCC and runs the conformance suite (`.github/workflows/bootstrap.yml`); here a program compiled for Windows from Linux is run under Wine at every update (the gate's `portable` unit), and the whole suite was, while the runtime was being written in C (Updates 193 to 196). A runner is Windows Server, not Windows 7; the runtime keeps to the calls Windows 7 has. Under Wine two cases cannot run, `19-concurrency/heaps_meet_ok.cb` and `many_threads_owning_frames_ok.cb`, whose 300 threads' reserved stacks Wine makes resident (19 GB).


## Formatting a program

`cobfmt` rewrites CobaltC files into the house style: four-space indentation, Allman braces, the spacing of the guide and the showcases, match arms and `bitstruct` fields with their `:` aligned, and trailing comments kept in their column. It changes only layout. Before it writes anything it checks that the result is the same program, token for token and comment for comment, and that it still parses; if that check ever failed, the file would be left as it was. It rewrites a file in place, so a symbolic or hard link stays one and the file keeps its owner and permissions; a read-only file is reported and left alone. Line breaks stay where the author put them (a one-line `if (n == 0) { return; }` stays one line), except that a function's body always opens on a line of its own, even a one-line body, and two statements on one line of a block are put on two lines. A line longer than 120 characters that holds a list of arguments, parameters or array elements is broken one item per line, with a trailing comma; `--width N` sets another width, and `--no-wrap` leaves every line as long as it is. A file that does not parse is reported and left alone.

```
./target/release/cobfmt prog.cb                 # format in place
./target/release/cobfmt --backup src/           # every .cb under src/, keeping NAME.cb.format.bak
./target/release/cobfmt --check src/            # list what would change; exit 1 if anything would
./target/release/cobfmt --diff prog.cb          # show the changes as a diff, write nothing
./target/release/cobfmt - < prog.cb             # standard input to standard output
```

A directory is searched for `.cb` files (hidden directories and `target/` are skipped). `cobfmt --help` lists every option and the exit statuses: 0 done, 1 when `--check` or `--diff` found changes, 2 for a usage error or a file that cannot be read, cannot be written or does not parse, 3 for an internal error. It compiles nothing, so it runs on Windows as on Linux, and it keeps a file's `\r\n` line endings.

### Building and testing cobfmt

`cobfmt` is written in CobaltC (`impl/cobfmt/`), with the parser written in CobaltC deciding whether a text parses (D-0326 retired the Rust formatter it was written to reproduce). `impl/build.sh` builds it and installs it as `impl/target/release/cobfmt`; its tests are one Python file, Python 3's standard library only:

```
impl/build.sh                                   # impl/target/release/cobfmt
python3 -I impl/cobfmt/tests.py        # its tests
```

- **Layout tests**: each layout rule — Allman braces, indentation, spacing, aligned `match` arms and `bitstruct` fields, trailing-comment columns, blank lines, one-line blocks, struct literals, continuation lines — on small inputs given to `cobfmt -`, including that formatted text formats to itself.
- **Command-line tests**: the options and exit statuses, formatting in place, `--backup` naming (`NAME.cb.format.bak`, then `.1`, `.2`, …) and never overwriting an existing backup, `--check` and `--diff` writing nothing, standard input with `-`, a file that does not parse being left alone, `--width`, `\r\n` files, and that the showcases, the examples, the benchmarks and the conformance cases are already formatted (so a change to the formatter that would reformat the corpus fails here first).

### The toolchain, written in CobaltC

The whole toolchain is written in CobaltC and compiles itself (D-0326); the runtime is C, which is what CobaltC compiles to.

- `front/`: the front end: the lexer, the abstract syntax, the parser, the loader of file-backed modules, the parse check, the derived functions and name resolution (`derive.cb`, `modres.cb`: the module tree, imports and re-exports, visibility, every reference rewritten to its qualified name), the folding of constants and the views of text literals (`consts.cb`, `views.cb`), and the type checker (`typecheck.cb`, with `items.cb`, `each.cb`, `fmtcheck.cb` and `consteval.cb`). What these take from `std` is generated (`front/prelude.cb`, by `cobc --prelude-summary impl/std`: the tokens the parser's pre-scans read, `std`'s declarations with every function's body empty, and each body's text) instead of parsing 28,000 lines of `std` on every run; a `std` function's body is parsed when the checker first reaches it. The gate checks it is current.
- `back/` and `cobc/`: the code generator and the driver: the command line, the C compiler invocation and linking, and the C in every form (the default, optimised one, `--all-checks`, `-g`, `--lib`, and D-0274's bootstrap form); `cobc --emit-c [-o OUT] FILE` prints the C alone.
- `cir/`: CobaltC IR (CIR, D-0342), the two forms between the checked program and the C: the typer, which gives every expression's type without lowering it, and the walk that says from the types alone whether a program is decided (core CIR; `cobc --emit-cir=core FILE` prints it); the statements the lowering writes, as nodes with a kind and parts, the passes over them and the table of the runtime's entries (protocol CIR; `cobc --emit-cir=proto FILE` prints it and `cobc --from-cir FILE.cir` prints the C of one).
- `cobfmt/`: the formatter, deciding whether a text parses with the parser above.
- `cbparse/`: the front end standing alone: `cbparse --dump-ast FILE` prints the program's own part of the parse as text, `--dump-resolved` once its names are resolved, `--dump-folded` once its constants are folded too, and `--dump-checked` the static pass's verdict; `impl/tools/factcheck.py` reads the first.

The compiler reproduces itself: built from the seed, it writes the seed again, which `boot` checks on every new machine and the gate's `bootstrap` unit on every update, with GCC and with Clang. `impl/tools/fixpoint.sh` checks the same in D-0274's bootstrap form, every check on in its second stage. Until D-0326 the toolchain also existed in Rust, and each part written in CobaltC was compared with its Rust original byte for byte on every tracked file (the last comparison is recorded in `CHG-0459`); the Rust tools are no longer published (D-0328).

`cobc --check-syntax` remains for tools written in other languages: the parser alone, on a file or standard input.

## Compiler status

All three planned stages of the compiler are complete. Every conformance row, file case, and example — concurrency included (`spawn`, `join`, `mutex`) — compiles to its stated outcome, phase, and recorded output, as does every runnable example in the guide. The suites grow with the language, so the test runs, not this file, report the counts. Compiled threads are real OS threads that run in parallel; every access to memory two threads can reach is checked by the runtime, under its lock, before it happens. The compiled program performs every check the specification discharges dynamically, which is what makes it conforming; it skips only checks that provably cannot fail. Compute-bound threads scale across cores. The plan, its measurements and its decisions are in `impl/COBC-PLAN.md`; `impl/STATUS.md` says what the tools do now, and `impl/CHANGELOG.md` how it came to.

License

Copyright © 2026 strawberry9.

This repository contains software, the CobaltC language specification, and other creative content, which are licensed separately.

Source Code

Unless otherwise stated, all source code in this repository is licensed under the BSD 3-Clause License. This includes the reference implementations in `impl/` and the standard library's source.

See LICENSE for the full license text.

Language Specification

The CobaltC language specification — the contents of the `spec/` directory — is licensed under the Creative Commons Attribution-NoDerivatives 4.0 International License (CC BY-ND 4.0).

This means anyone may copy and distribute the specification unchanged, including commercially (for example, inside a product's documentation), provided that they give appropriate attribution. No one may distribute a modified version of it: the specification remains under its author's control.

See LICENSE-CC-BY-ND for the complete license text.

Full license:
https://creativecommons.org/licenses/by-nd/4.0/

Implementing CobaltC

Anyone may implement the CobaltC language — a compiler, interpreter, runtime, library or tool — for any purpose, commercial or not, without permission or fee. See LICENSE-IMPLEMENTATION for the grant, which also covers using the specification's syntax, names and diagnostic identifiers in an implementation.

An implementation may call itself a CobaltC compiler only if it conforms to the specification, as set out in NAMING-POLICY.md.

Other Documentation and Creative Content

Unless otherwise stated, the other documentation, written content (including the language guide in `htmlguide/`), images, graphics, and other original creative content in this repository are licensed under the Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License (CC BY-NC-ND 4.0).

This means you may share that content for non-commercial purposes, provided that you give appropriate attribution and comply with the license terms. You may not distribute modified versions of it.

See LICENSE-CC-BY-NC-ND for the complete license text.

Full license:
https://creativecommons.org/licenses/by-nc-nd/4.0/

Third-Party Content

Third-party materials included in this repository may be subject to their own licenses and terms. Such materials are not necessarily covered by either license above. Where applicable, their respective licenses and attribution notices are provided alongside the relevant materials.
