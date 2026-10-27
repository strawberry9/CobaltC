# The implementations: what they do now

What each tool in `impl/` does, its limits, the platforms it runs on, and how it is built and tested. This
page describes the present state only; the dated record of how it came to be, one section for each piece of
work and each D-record, is `impl/CHANGELOG.md`. The language itself is `spec/`; a reader who wants the
model behind the rules first can start with `spec/OVERVIEW.md`.

## The workspace

`impl/` is one Cargo workspace. It uses only the Rust standard library and builds with Rust 1.77.2 and
later; `Cargo.lock` stays at version 3 so that older Cargo reads it.

| Part | Directory | What it is |
|---|---|---|
| `coby` | `src/` | The reference interpreter: the front end, then a tree-walking evaluator over the specification's abstract state |
| `cobc` | `cobc/` | The native compiler: `coby`'s front end, then a lowering to C compiled by a C compiler |
| `cbrt` | `cbrt/` | The runtime every compiled program links (`libcbrt.a`): the checks the specification discharges at run time, over real addresses |
| `cobfmt` | `cobfmt/` | The formatter: changes layout only, never a token |
| `selfhost` | `selfhost/` | Parts of the toolchain written in CobaltC, built by `cobc`: coby's whole front end, the type checker included (`front/`, `cbparse`), `cobfmt` (`cobfmt-cb`) and `cobc` itself (`back/` and `cbc/`: `cbc`, the same command line and the same C, which compiles itself to a fixed point), each compared byte for byte with the Rust original |
| `cobmodel` | `model/` | An executable model of the alias and thread rules, written from the specification alone (D-0219) |
| `std` | `std/` | The standard library's CobaltC source, embedded in both tools |
| Conformance | `conformance/` | The file-based conformance suite, and `run.py`, a standalone runner for any implementation |
| Fuzzing | `fuzz/` | The std parsers' fuzzer, its sanitizer build and the interoperability checks against OpenSSL |
| Tools | `tools/` | `speccheck.py`, `manifest.py`, `trusted_table.py`, `std_coverage.py`, `cobdoc.py`, `suites.sh` |
| Battery | `battery.sh` | Every check run before a push, in staged order |

## `coby`, the interpreter

`coby FILE.cb [ARG…]` reads the program and the module files it declares, checks it statically, and only
then runs `main`. The static pass (`src/typecheck.rs`) is shared with `cobc`, so the two accept the same
programs and give the same static diagnostics, byte for byte.

- **Execution.** The evaluator is `src/interp.rs`, which holds its types, and the modules under
  `src/interp/`, one for each concern. Every value lives in the abstract state of `spec/04`: objects,
  cells, paths and their tokens. Each access is checked as the rules say: aliasing (`clash`), temporal
  validity, initialization, resource authority, bounds and arithmetic. A fault is the specification's diagnostic, with its location.
  `std` is real CobaltC source (`std/*.cb`), run like the program's own code; the items both tools build
  in (formatting, the float functions, a few hot text and vector operations) are native.
- **Threads** are operating-system threads (`std::thread`). `--schedule-seed N` makes one schedule
  repeatable, and `--explore N` runs the program N times under different schedules and groups the
  outcomes.
- **Modes.** `--check` checks files or directories and runs nothing; `--check-syntax` parses one text
  and does nothing else; `--dump-ast` prints a program's parse in a canonical text, and `--dump-resolved` and
  `--dump-folded` the same once its names are resolved and its constants folded, and `--dump-checked` prints `ok` once the whole static pass accepts it (each for comparing the front end written in CobaltC). `--unittest`
  runs the program's
  `unittest` blocks; `--unittest-filter TEXT` runs a subset; `--unittest-std` runs `std`'s own tests,
  which are the std reference's examples. `--explain` (or `COBALTC_EXPLAIN=1`) adds each diagnostic's
  registry entry to the short form. `--about` describes the tool.
- **Exit status.** 0 for `ok` (or `main`'s `u8`); 1 for a diagnostic, static or dynamic; 2 for a usage
  error; 3 for `unsupported:`, a program the interpreter cannot run (C code it cannot call on this
  platform); 4 for an internal error, a bug in the tool, printed as one line.
- **Limits.** It is slow: a tree-walking evaluator that keeps the full state, far slower than `cobc`'s
  output, so it is for checking a program's meaning, not for running it at scale. `extern fn` calls into C follow the x86-64 System V
  convention, so on Windows (and any other ABI) `coby` refuses them with status 3.

## `cobc`, the compiler

`cobc FILE.cb` checks the program as `coby` does, lowers it to C (`cobc/src/lower.rs` and the modules
under `cobc/src/lower/`) and compiles that with `cc`, or the compiler `--cc` names, against `libcbrt.a`.

- **Output.** `-o OUT` keeps the executable, `--run` builds in a temporary directory and runs it, and
  `--keep-c` keeps the C. `-O LEVEL` (default `-O2`), `-march=CPU`, `-g` (debug information, with the
  program's lines in `#line` directives; `--no-line-directives` leaves them out), `-s` (strip) and `-v`
  pass through to the C compiler.
- **Checks.** A compiled program makes the same run-time checks as the interpreter. The lowering leaves
  out the checks it can prove redundant (`spec/08` §4 `[Standing-Local]`, `spec/14` §6), and it keeps
  every check whose hypotheses it cannot show. A decided program (`spec/14` §6, D-0284: no conflict left
  to the run) is built with no record of references at all; `-v` says "lowered pathless" or names the
  first conflict that kept the record, and `COBALTC_PATHLESS=0` keeps it. Such a program is lowered a
  second time knowing it is decided (D-0285): a value holding references is then plain data, with no
  object, and statements that would record nothing open no scope; `-v` says "lowered pathless and
  decided", and `COBALTC_DECIDED=0` keeps the first lowering. There values pass between functions with no
  object (D-0286): an argument or a result is handed over as its value, and the receiver binds it objectless
  where it can. `--all-checks` keeps every
  check, for differential testing. `--report-tracking` says which functions kept which tracking.
- **Threads.** `--explore N` and `--schedule-seed S`, as for `coby`, with the runtime choosing the
  schedule at each synchronization point.
- **One C file.** A program's C, the `std` it reaches included, is one file, compiled every time (D-0277:
  the object cache that compiled pure-`std` functions once and reused their objects is gone); a large
  program's C is compiled in parts, at most eight, all at once, and linked with GCC's LTO or Clang's ThinLTO.
- **Diagnostics and status** as for `coby`: the same static diagnostics. A compiled program's fault is
  the same diagnostic at the same line, with exit status 1.
- **Limits.**
  - The targets are x86_64 Linux, with GCC or Clang, and x86_64 Windows, with MinGW-w64's GCC
    (`--target x86_64-pc-windows-gnu`, D-0255; a Windows runtime built with Rust 1.77.2), from Linux or
    on Windows. Checked under Wine; the check on real Windows is the owner's (README, "Compiling for
    Windows"). macOS is not a target (D-0234). MSVC's `cl.exe` would need more than a port, since the
    generated C uses statement expressions and `__builtin_*_overflow`.
  - A program whose type arguments the lowering cannot solve is `unsupported:` (status 3) where `coby`
    can still run it. Such cases are rare and each one is a known gap.

## `cobfmt`, the formatter

`cobfmt PATH…` rewrites `.cb` files, or each `.cb` file under a directory, in the house style: Allman
braces for functions, aligned `match` arms and bitstruct fields, lines broken past `--width` (default 120,
`--no-wrap` to keep them). It also has `--check` (lists the files it would change; exit 1 if any), `--diff`,
`--stdout`, `--backup`, and `-` for standard input to standard output. It changes layout only, and it keeps
comments, `///` documentation included, where they are.

`cobfmt-cb` (`selfhost/cobfmt/`, CHG-0346) is the same formatter written in CobaltC, a module for each of
`cobfmt`'s, built with `selfhost/build.sh`, deciding whether a text parses with coby's parser written in
CobaltC (`selfhost/front/`, CHG-0350). `selfhost/compare.sh` runs both formatters on every tracked `.cb`
file five ways and on the command line's errors, and compares their output, messages and exit statuses
byte for byte; `selfhost/compare_parser.sh` compares the two front ends on every tracked file (the parse,
the program resolved and folded, and the static pass's verdict) and on mutated copies of each. The type checker in
CobaltC (`front/typecheck.cb` with `items.cb`, `each.cb`, `fmtcheck.cb` and `consteval.cb`, CHG-0355) parses a
`std` function's body when it first reaches it, from the text `front/prelude.cb` carries. Name resolution in CobaltC (`front/derive.cb`,
`front/modres.cb`, CHG-0352) reads `std`'s declarations from `front/prelude.cb`; the compiled front end is
about 20 times slower than coby's on large inputs (the runtime's object tracking, spread over every
operation).

`cbc` (`selfhost/cbc/`, with `selfhost/back/`) is `cobc` written in CobaltC, over that front end
(D-0276): the same command line (`cbc [options] FILE.cb`, every option `cobc` takes, the
stale-runtime check, `--lib`, `--unittest`, `--run`, `--target`), the same C compiler invocation and linking
against `cbrt`, and the same C, byte for byte, as `cobc` writes in every form (the default, `--all-checks`,
`-g`, `--no-line-directives`, `--lib`, and each level of D-0274's bootstrap form, `COBALTC_BOOTSTRAP_UPTO=N`).
`cbc --emit-c [--c-name NAME] FILE` prints the C alone. `selfhost/build.sh` builds it with `cobc`;
`selfhost/compare_c.sh` compares the two compilers' C on any set of files (`LEVEL=`, `FLAGS=`);
`selfhost/fixpoint.sh` checks the fixed point: `cbc` built by `cobc` writes its own C, which is `cobc`'s; the
C compiler builds a second `cbc` from that C, which writes the same C again (about 45 minutes at the default
level). `cobc`'s tests take `COBC=PATH` and pass with `cbc` as the compiler under test. `cbc` compiles about
15 to 45 times slower than `cobc` (its own C: about 18 minutes against 26 seconds; the runtime's
object tracking, as in the front end, chiefly of `StringView` copies); the programs it builds
are the same programs. `cobc` stays in the tree as the Rust reference and the oracle. Since D-0278 the Rust
toolchain leads: `cbc`, `cbparse` and `cobfmt-cb` accept the language of edition 2026.102501 and are ported
forward at milestones, so a program using anything newer is compiled by `cobc` alone until the next port. Edition 2026.102503 (D-0282) makes the conflicts of frame-held references compile-time errors; until the port,
`cbc` accepts those programs and finds the conflicts at run time, so the conformance rows D-0282 made static are
`cobc`'s alone. D-0283 checks a conflict at a call that only values decide when the call is made; `cbc` still finds it at
the callee's first conflicting access. D-0284 builds decided programs with no record of references; `cbc`
still keeps it.

## `std`

The standard library is the module `std` and its submodules (`spec/21` §0 lists them; the map is frozen,
D-0223). Every exported item has `///` documentation (D-0229), and the guide's section R is generated from
it. `std` links no third-party C library: TLS, HTTP, the PostgreSQL client and the cryptography are
written in CobaltC.

The static pass checks `std`'s items as the program reaches them; `COBALTC_CHECK_ALL=1` checks every item.
`spec/21` §0 labels the security-relevant modules (cryptography, X.509, TLS, HTTP, PostgreSQL) as tested
against their standards' vectors and against OpenSSL, and fuzzed, but not independently audited.

## Platforms

| Platform | `coby` | `cobc` | `cobfmt` |
|---|---|---|---|
| Ubuntu 22.04, x86_64 (GCC 11, Clang 14) | built and tested, every suite | built and tested, every suite, both C compilers | built and tested |
| Windows 7 and later, x86_64 (Rust 1.77.2) | built; its test suite passes; no C calls | type-checks only | built and tested |
| macOS, other Linux | expected to work; not tried | not tried | expected to work |

## Building

```
cd impl
cargo build --release --workspace      # coby, cobc, cobfmt, cobmodel and libcbrt.a in target/release
```

`cobc` links against the `libcbrt.a` beside its own executable, which only a build of the `cbrt` package
writes: `cargo build -p cobc` alone, or `cargo test`, leaves an older one in place. After any change, build
with `--workspace` before running anything. A change to `std/` needs all three of `coby`, `cbrt` and `cobc`
rebuilt, since `std` is embedded. A new source file the runtime shares goes into `cbrt/build.rs`'s stamp
list, so that `cobc` can tell a stale runtime.

## Testing

| What | Command | Covers |
|---|---|---|
| Everything, as before a push | `impl/battery.sh` (`SCOPE=full MAXLOAD=3` on a small machine) | The staged battery below, with a report |
| The suites | `impl/tools/suites.sh` | Both tools' conformance suites and the model sample; writes the `TESTED` record |
| One implementation | `python3 impl/conformance/run.py --impl "CMD {file}"` | Any implementation against the file suite |
| The corpus checks | `python3 impl/tools/speccheck.py`, `std_coverage.py`, `cobdoc.py --check`, `trusted_table.py --check` | The specification's cross-references, std's cases, std's documentation, the trusted table |
| The guide | `cd htmlguide/generator && python3 gen.py --check` | Every example under `coby` and under `cobc` with GCC and Clang |
| std's tests | `coby --unittest-std F.cb`, `cobc --unittest-std --cc CC --run F.cb` | The std reference's examples |

The battery's stages, each gating the next:

0. Builds, the Windows and Rust 1.77.2 checks, formatting, `cobdoc --check`, the `--check` sweep, unit
   and command-line tests.
1. The hand-written tests, the model sample (compared with both tools, and checked as licensed: no access the
   flow table proves is one `clash` refutes, D-0241), `speccheck`, std coverage, std's tests under both
   tools, the showcases and the guide.
2. The interpreter's conformance suites.
3. The compiler's conformance suites, GCC and Clang side by side.
4. The hand-written tests in the debug profile.
5. The local stress programs, sanitizer runs, the fuzzer under ASan and UBSan, and the interoperability
   checks.
6. Public endpoints. This stage is advisory.

Every compiled test runs under both GCC and Clang (`COBC_TEST_CCS` and `COBC_CCS` choose others). Each
run sets a CPU limit (`ulimit -t`, `COBALTC_CPU_LIMIT`, `timeout -k`). Do not change `impl/`, `impl/std` or
`spec/conformance.md` while a suite is running: the compiled suites rebuild the interpreter they compare
against.

## Known gaps

- None open. The items both tools build in (`printf`, `sqrt`, …), which have no CobaltC source, are
  documented in `impl/std/builtins.md`, checked by `cobdoc.py --check` against the tools' native lists.
