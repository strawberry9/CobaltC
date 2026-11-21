# The tools: what they do now

What each tool in `impl/` does, its limits, the platforms it runs on, and how it is built and tested. This
page describes the present state only; the dated record of how it came to be, one section for each piece of
work and each D-record, is `impl/CHANGELOG.md`. The language itself is `spec/`; a reader who wants the
model behind the rules first can start with `spec/OVERVIEW.md`.

## The workspace

`impl/` is built with a C compiler alone (D-0326): `impl/bootstrap/boot.c` builds the runtime (C) and the
compiler from the seed, the compiler's own C, and `impl/build.sh` builds the tools again with the compiler
installed (README, "Building"). Every tool is written in CobaltC; the runtime is C.

| Part | Directory | What it is |
|---|---|---|
| `cobc` | `front/`, `back/`, `cobc/` | The compiler: the front end (the parser, name resolution, the static pass), the lowering to C, and the driver, which compiles that C with a C compiler. It compiles itself |
| `cbrt` | `cbrt/` | The runtime every compiled program links (`libcbrt.a`), written in C (`cbrt/build.sh`): the checks the specification discharges at run time, over real addresses |
| `cobfmt` | `cobfmt/` | The formatter: changes layout only, never a token |
| `cbparse` | `cbparse/` | The front end standing alone: a program's parse, resolved and folded program, and the static pass's verdict, as text (`tools/factcheck.py` reads the parse) |
| `cobmodel` | `model/` | An executable model of the alias and thread rules, written from the specification alone, importing `std` only (D-0219) |
| `std` | `std/` | The standard library's CobaltC source, embedded in `cobc` |
| Bootstrap | `bootstrap/` | `boot.c`, the seed (`cobc-seed.c.gz`, `SEED`) and `mkseed.sh`, which writes it |
| Tests | `tests/` | `cbtest`, the test runner (`cbtest/`), the tools' cases (`cases/`), the rows' recorded runs (`rows/`), the tools' own program cases in the suite's format (`programs/`), the random-program tester (`differential/`), the foreign forms and the cold drafts |
| Conformance | `conformance/` | The file-based conformance suite, each case's recorded run (`NAME.expect`), and `run.py`, a standalone runner for any implementation |
| Fuzzing | `fuzz/` | The std parsers' fuzzer, its sanitizer build and the interoperability checks against OpenSSL |
| Tools | `tools/` | `speccheck.py`, `manifest.py`, `trusted_table.py`, `std_coverage.py`, `cobdoc.py`, `gate.py`, `certify_model.py`, `assurance.py`, `factcheck.py`, `validate_c.py`, `replay_runtime.py`, `fixpoint.sh` |
| Gate | `gate.sh` (`tools/gate.py`) | Every check, as units with declared inputs, on a snapshot; green verdicts kept by the hash of their inputs (D-0307). `battery.sh` is its release profile |

## `cobc`, the compiler

`cobc FILE.cb` reads the program and the module files it declares, checks it statically (`front/`), lowers it
to C (`back/`) and compiles that with `cc`, or the compiler `--cc` names, against `libcbrt.a`; `--run` runs it.
`--check` checks files or directories and runs nothing; `--check-syntax` parses one text; `--unittest` runs the
program's `unittest` blocks (`--unittest-filter TEXT` a subset, `--unittest-std` `std`'s own, which are the std
reference's examples); `--doc` shows `std`'s documentation; `--explain` (or `COBALTC_EXPLAIN=1`) adds each
diagnostic's registry entry; `--allow-deprecated` (or `COBALTC_ALLOW_DEPRECATED=1`, D-0303) accepts a name
marked `[[deprecated("…")]]`, with a warning; `--about` describes the tool. Exit status: 0 for `ok` (or `main`'s
`u8`); 1 for a diagnostic, static or dynamic; 2 for a usage error; 3 for `unsupported:`; 4 for an internal error,
a bug in the tool, printed as one line.

- **Output.** `-o OUT` keeps the executable, `--run` builds in a temporary directory and runs it, and
  `--keep-c` keeps the C. The C is written in a directory of `cobc`'s own; `--keep-c` puts it beside the
  executable over a file there only if `cobc` wrote it, and no output replaces one of the program's own
  files (its source, modules, `extern "./x.c"` files). An output that cannot be written is reported before
  compiling (status 2). During `--run`, SIGINT, SIGTERM and SIGHUP go to the program, and `cobc` removes its
  directories and ends by the same signal; so too while `cobc` compiles, under `--explore` and under a runner
  (POSIX). On Windows a console handler waits for the programs `cobc` started (they get Ctrl-C, Ctrl-Break or
  the console's closing too), removes the directories and ends `cobc` with STATUS_CONTROL_C_EXIT, and the
  program `--run` starts is in a job that ends it with `cobc`. A command-line mistake names the argument and the reason. `-O LEVEL` (default `-O2`), `-march=CPU`, `-g` (debug information, with the
  program's lines in `#line` directives; `--no-line-directives` leaves them out), `-s` (strip) and `-v`
  pass through to the C compiler.
- **Checks.** A compiled program makes every run-time check the specification discharges dynamically. The lowering leaves
  out the checks it can prove redundant (`spec/08` §4 `[Standing-Local]`, `spec/14` §6), and it keeps
  every check whose hypotheses it cannot show. A decided program (`spec/14` §6, D-0284: no conflict left
  to the run) is built with no record of references at all; `-v` says "lowered pathless" or names the
  first conflict that kept the record, and `COBALTC_PATHLESS=0` keeps it. Such a program is lowered a
  second time knowing it is decided (D-0285): a value holding references is then plain data, with no
  object, and statements that would record nothing open no scope; `-v` says "lowered pathless and
  decided", and `COBALTC_DECIDED=0` keeps the first lowering. There values pass between functions with no
  object (D-0286): an argument or a result is handed over as its value, and the receiver binds it objectless
  where it can. A consuming `foreach` over a text splitter's call (`StringView::lines(v)`, `String::split(&s,
  ",")`, …) walks the text, holding a view of it, and makes no `Vec` (D-0287), so it leaves a
  program decided. A local `Vec<StringView>`, `Vec<ref<T>>` or struct of references that stays in its frame
  is checked before the run as a reference is (D-0290), and leaves it decided too; so do a handle wrapped in
  an `Option`, a reference `call(…)?` gives, and a slice lent whole to a thread (D-0291), which lets the
  self-hosted compiler and the showcase `parallel_chunks` be decided. In a program that keeps paths, a
  function the static pass decided forms *light* references (D-0292): recorded nowhere, checked once at their
  formation against the owner's recorded references, and recorded where they are stored into data or arrive in
  a function the static pass left a conflict in; `-v` says "lowered with holders", `COBALTC_RECORD=full` keeps
  the full record (the differential oracle) and `COBALTC_RECORD=check` aborts where a light reference reaches
  what should have recorded it; `COBALTC_REPLAY_TRACE=FILE` (D-0300) writes the run's steps in the proven
  machine's action format (`proofs/Replay.lean`; `impl/tools/replay_runtime.py` builds, runs and replays). The token's kind is tested inline (D-0293): a light reference, and an owner's
  binding whose object has no recorded reference on it, make no runtime call for a check, a read or (in a
  decided function) a borrow. In a decided program the lowering places no object where no check could read it
  (D-0294): recursive types are objectless, an objectless local may be assigned, matched, sliced and moved like
  any other, results built in place leave with no object, every reference parameter is unstored, and a record
  nothing reads is removed from the C; the self-hosted compiler runs in half the instructions. In a program
  that keeps paths, a decided function's element references are light too when the element's object persists,
  and a reference's flight leaves the statement's scope pending (D-0295); phase 3 of D-0281 is closed, its
  tier-D promises restated in `spec/14` §6. In a decided program the compiler, not the runtime, ends the
  values a body owns (D-0315): a value whose end frees memory alone passes between bindings, arguments and fields
  with no object and is dropped by generated C (drop glue `cb_dg_*`), a statement's temporaries end where it ends,
  a `fn` parameter the body only calls and a value whose type has a destructor of its own (a local, a pattern's
  binder, a statement's temporary, a thread's handle, a lock's guard) are noted in the function's table, read
  only by a fault's unwind, closures and threads' values take no object, and a statement's location is stored
  before a call only where the callee may read it; `spec/14` §6 states the cost. The C of every decided showcase,
  benchmark and speed-test program keeps no frame, scope or object (CHG-0420). `--all-checks` keeps every
  check, for differential testing. `--report-tracking` says which functions kept which tracking, and for a
  decided program lists, by line, the frames, statement scopes and objects its C still writes.
  `--certificate FILE` (D-0299) writes the decision itself: for each body the static pass checked, `decided`
  or every conflict it left to the run, the point checks, and the program's verdict (`decided`, or why paths
  were kept); `cobmodel certify FILE PROG.cb` checks a fragment program's certificate against every
  interleaving (`impl/model/README.md`), and `spec/ASSURANCE.md` says what each kind of evidence establishes.
- **Threads** are operating-system threads. An `Rc` stays in its thread: `spawn` refuses an `Rc` or `Weak`,
  or a value holding or reaching one (`diag.not-thread-safe`, `spec/19` `[Thread-Confined]`). `--explore N` runs the program N times under different
  schedules and groups the outcomes; `--schedule-seed S` repeats one, the runtime choosing the schedule at
  each synchronization point.
- **One C file.** A program's C, the `std` it reaches included, is one file, compiled every time (D-0277:
  the object cache that compiled pure-`std` functions once and reused their objects is gone); a large
  program's C is compiled in parts, at most eight, all at once, and linked with GCC's LTO or Clang's ThinLTO;
  `--one-unit` compiles it as one file with no link-time optimisation, which is how the compiler itself is
  built (`CHG-0470`).
- **Memory.** A block of up to 1,024 bytes that `allocate` returns is the runtime's own (D-0329): one of 64
  size classes, with no header, since `deallocate` is given the size again; while the program has one thread
  the generated C takes and returns such a block in line. A larger or over-aligned block is the C library's.
  Under a sanitizer, of the runtime or of the program, every block is `malloc`'s. A block from `allocate`
  must not be given to C's `free`, nor a `malloc` block to `deallocate`.
- **What a program links.** The linker keeps what the program refers to (`--gc-sections`), and the runtime
  reaches what only some programs need through pointers filled where the need arises: the locks and heap
  holds when a thread is first started, the ends of objects, of objects over raw cells and of stored
  references when the first of each is made (`CHG-0468`). An empty program is about 54 KB; one that makes
  no object, such as the `strings` benchmark, about 118 KB.
- **Language additions in Update 203.** `foreach (x in lo ..= hi)` counts to `hi` included, without
  overflow (`spec/14` `[Foreach-Range-Inclusive]`). A reference through a lock guard ends with the guard
  (`spec/19`). `FileError` gained `IsDirectory`, `NotDirectory`, `NoSpace` and `TooManyOpen`, and `NetError`
  `NoDescriptors` and `MessageTooLong`, so an exhaustive `match` on either without `_` needs the new arms.
- **Hash tables.** `HashMap` and `HashSet` are `std`'s CobaltC (`spec/21` `rule.stdlib.hashmap`): the
  entries, a key beside its value, in one vector in insertion order (D-0331), and a table of their indices
  whose cells keep hash bits (D-0330). For integer, `bool` and `String` keys the compiler writes the lookups,
  `insert` and `entry` itself, and in a decided program `remove` and `swap_remove` for a plain value.
  `remove` keeps the order without moving the later entries: the removed entry's place is left empty, the
  empty places counted in a Fenwick tree, and closed up together when they outnumber the entries, when the
  table grows, and before the map is cleared, drained or destroyed (bug 045). A
  `HashSet<K>` is a map to `bool`, so one of its entries takes the key's size rounded up for the one byte
  (16 bytes for a `u64` key, 32 for a `String`). The runtime
  gives the hash of a key's bytes: eight bytes at a time, from a seed drawn from the system's randomness at
  the first hash of each run (D-0330), so a program with no table draws nothing. `COBALTC_HASH_SEED=N` fixes
  the seed, for measuring: without it the instructions a program with a table executes differ a little from
  run to run, and its output does not.
- **The checked forms.** With every check on (`--all-checks`) and in the bootstrap form, a program's time is
  the runtime's: each frame, statement scope, object, path and stored reference is a record made and ended.
  While the program has one thread and writes no trace, the entries that run at every statement do their
  common case in line (`CHG-0471`, `CHG-0472`); the compiler in the
  bootstrap form compiles `tinyos.cb` in 93 s where the default form's takes under a second, and
  `CHG-0472` has what each form costs on four workloads.
- **Diagnostics.** A fault is the specification's diagnostic, with its location, exit status 1. A program
  whose standard output the system refused, and that would exit with 0, exits with 1 (`[Print-Failure]`).
  A frame too large for what is left of the stack faults as `diag.stack-exhausted` (Clang probes large
  frames, `-fstack-clash-protection`; GCC does by default, MinGW's GCC with `___chkstk_ms`), contained to its
  thread: on POSIX the SIGSEGV is caught on a signal stack, on Windows EXCEPTION_STACK_OVERFLOW by a vectored
  handler, which makes the guard page again after a contained overflow. Source text with
  bidirectional control characters is refused (`spec/22` `[Bidi-Controls]`), by `cobc` and `cobfmt`.
- **Limits.**
  - The targets are x86_64 Linux, with GCC or Clang, and x86_64 Windows, with MinGW-w64's GCC
    (`--target x86_64-pc-windows-gnu`, D-0255; the runtime built by MinGW-w64's GCC), from Linux or
    on Windows. Checked under Wine and on GitHub's Windows runners (README, "Compiling for Windows"). macOS is not a target (D-0234). MSVC's `cl.exe` would need more than a port, since the
    generated C uses statement expressions and `__builtin_*_overflow`.
  - A program whose type arguments the lowering cannot solve is `unsupported:` (status 3). Such cases are
    rare and each one is a known gap.

## `cobfmt`, the formatter

`cobfmt PATH…` rewrites `.cb` files, or each `.cb` file under a directory, in the house style: Allman
braces for functions, aligned `match` arms and bitstruct fields, the author's line breaks kept (no line is
broken unless `--width N` asks for it; `--no-wrap` spells out that default). It also has `--check` (lists the files it would change; exit 1 if any), `--diff`,
`--stdout`, `--backup`, and `-` for standard input to standard output. It changes layout only, and it keeps
comments, `///` documentation included, where they are. It rewrites a file in place, keeping links, owner and
permissions, and leaves a read-only file as it was (status 2). An attribute that begins a line (`[[deprecated("…")]]`,
D-0303) stands on a line of its own above what it marks.

The formatter is written in CobaltC (`cobfmt/`, CHG-0346), built by `build.sh` as `target/release/cobfmt`;
it decides whether a text parses with the compiler's own parser (`front/`). Its tests, carried over from the Rust
formatter it reproduced until D-0326 retired that, are `cobfmt/tests.py`.

## The compiler compiling itself

`cobc` is a decided program (D-0284), so it runs pathless and objectless. Built from the seed, it writes the
seed again: `bootstrap/boot.c` checks that on every new machine, and the gate's `bootstrap` unit on every update,
once built by GCC and once by Clang; `tools/fixpoint.sh` checks it in each level of D-0274's bootstrap form
(`COBALTC_BOOTSTRAP_UPTO=N`). The seed is written by `bootstrap/mkseed.sh` and the gate's build unit checks it is
the C of the committed source. Until D-0326 the toolchain also existed in Rust, and each part written in CobaltC
was compared with its Rust original byte for byte on every tracked file (the parse and its mutants, the C in every
form, the reports, `--doc`); the last comparison is recorded in `CHG-0459`, and the Rust tools are no longer
published (D-0328).

## `std`

The standard library is the module `std` and its submodules (`spec/21` §0 lists them; the map is frozen,
D-0223). Every exported item has `///` documentation (D-0229), and the guide's section R is generated from
it. `std` links no third-party C library: TLS, HTTP, the PostgreSQL client and the cryptography are
written in CobaltC.

The static pass checks `std`'s items as the program reaches them; `COBALTC_CHECK_ALL=1` checks every item.
A retired name keeps its declaration: a deprecated one is marked `[[deprecated("…")]]`, and a removed one is a
tombstone at the end of its module's file (`STABILITY.md` §3a), as `String::from_str` (D-0305) and the fourteen
`String::` text wrappers (`as_view`, `find`, … `split_whitespace`; D-0306) are, and the six `_str` lookups and the ten
other text twins D-0308 retired (`String::len`, `chars`, … `StringView::sub`): a literal or a `str` is the `String`
reference where one is declared, `&s` the view, `==` and `&s[lo..hi]` the comparison and the part.
`spec/21` §0 labels the security-relevant modules (cryptography, X.509, TLS, HTTP, PostgreSQL) as tested
against their standards' vectors and against OpenSSL, and fuzzed, but not independently audited.

## Platforms

| Platform | `cobc` and the tools |
|---|---|
| Ubuntu 22.04, x86_64 (GCC 12, Clang 14) | built and tested, every suite, both C compilers |
| Ubuntu 22.04 and 24.04, Debian 12, Fedora 41 (GCC 11 to 14, Clang 18) | built by `boot` from a fresh clone in a clean container, the conformance suite run there (D-0326) |
| Windows 7 and later, x86_64 (MinGW-w64) | built by `boot.exe` on GitHub's Windows runner, the conformance suite run there; a program compiled for Windows from Linux run under Wine here (`portable`) |
| macOS, other architectures | not targets (D-0234) |

## Building

```
gcc impl/bootstrap/boot.c -o boot && ./boot   # a new machine: the runtime, cobc from the seed, cobfmt, cobmodel
impl/build.sh                                 # later: the runtime if changed, then every tool, with the cobc installed
impl/bootstrap/mkseed.sh                      # the seed, after a change to the compiler's source or std
```

`build.sh` builds with the `cobc` an earlier build left (copied first to `target/prev/cobc`, which stays as the
one to go back to), and compiles the compiler's own C as one file with no link-time optimisation
(`cobc --one-unit`), as `boot.c` compiles the seed: in parts with link-time optimisation it ran 6% slower and
was 9% larger. What the compiler takes from `std` is generated and tracked: `front/prelude.cb`, which
`cobc --prelude-summary std > front/prelude.cb` writes, and `back/std_values.cb` (`back/gen_std_values.sh`);
after a change to `std/`, write both, then build again, since `cobc` carries the old ones until it is rebuilt,
then write the seed.

`cobc` links against the `libcbrt.a` beside its own executable (or in `COBALTC_CBRT_DIR`), which `build.sh` and
`boot.c` write. A compiler refuses a runtime built from other sources than its own: the stamp, a hash of the files
`cbrt/SOURCES` lists (`cbrt/tools/stamp.c`, computed the same way by `bootstrap/boot.c`); a new source file of the
runtime goes into that list. After a runtime change `build.sh` builds `cobc` in two stages (the previous `cobc`
with the runtime it was built with, kept in `target/prev`). The `cobc` `boot` makes from the seed has no stamp and
accepts any runtime.

## Testing

| What | Command | Covers |
|---|---|---|
| What a commit and a push need | `impl/gate.sh` (`--dirty` for the working tree) | The gate profile's units below, on a snapshot; a unit whose inputs have a green verdict is not run again |
| Everything | `impl/gate.sh release` (on the owner's word) | The gate and the release profile's slow units |
| One implementation | `python3 impl/conformance/run.py --impl "CMD {file}"` | Any implementation against the file suite |
| The corpus checks | `python3 impl/tools/speccheck.py`, `std_coverage.py`, `cobdoc.py --check`, `trusted_table.py --check`, `assurance.py --check` | The specification's cross-references, std's cases, std's documentation, the trusted table, the assurance case's evidence |
| The certificates | `python3 impl/tools/certify_model.py impl/model/sample --check` | The compiler's certificate for each sample program, checked against every interleaving by the model (D-0299) |
| The certificates read again | `python3 impl/tools/factcheck.py impl/model/sample showcase --check` | A second reading of `spec/14` §6, sharing no code with the static pass, agrees with every body the certificate calls decided, for the bodies inside its core (D-0300) |
| The decided C | `python3 impl/tools/validate_c.py impl/model/sample showcase --check` | The C a decided program is lowered to forms, records and reads no access path, and every body left to the run records its references (D-0300) |
| The runtime's runs | `python3 impl/tools/replay_runtime.py impl/model/sample --check` | Each sample program's run, built with every check, is a run of the proven machine up to where it leaves the fragment, its faults the machine's refusals (D-0300) |
| The suites | `impl/target/tests/cbtest conformance` / `rows` / `cases --root impl/tests/cases` (`--cobc PATH`) | The file suite, the specification's rows and the tools' own cases, each under GCC and Clang, against its header and its recorded run |
| The guide | `cd htmlguide/generator && python3 gen.py --check` | Every example under `cobc` with GCC and Clang |
| std's tests | `cobc --unittest-std --cc CC --run F.cb` | The std reference's examples |

The gate's units (D-0307; `impl/gate.sh --list` shows whether each has a green verdict for this tree):

- *gate* profile: `build` (`build.sh`; the prelude summary, the value names, the runtime's table and interface and
  the seed current), `format`, `formatter`, `records` (speccheck, proofcover, assurance, the runtime contract,
  cobdoc, std coverage, the card, the foreign forms, the cold drafts, the conformance coverage),
  `portable` (the runtime by MinGW-w64 and Clang, a program run under Wine), `sweep-check`, `tests` (cbtest's
  cases), `corpus` (the suite, the rows and the tools' own program cases, GCC and Clang, against the
  recorded runs), `std-tests`, `guide`, `book`, `showcase`, `model` (the sample, the certificates, the replays,
  `lake build`), `bootstrap` (a clone built from the seed by GCC and by Clang, each compiler writing the seed
  again), `differential` (the compiler against itself on random programs), `lowering` (the undecided sweep, the
  perf guide, the bench, against the last green numbers).
- *release* profile: `fixpoint` (the bootstrap form), `all-checks`, `slow-cases`, `runtime` (CBMC, ASan with UBSan,
  ThreadSanitizer), `verify`, `programs` (stress programs, sanitizers, crypto sweep,
  local TLS, the fuzzer, interop), `proofs-kernels`, `external` (advisory).

The interpreter `coby` was, until D-0326, a second reading of the run-time rules: the suites required every
compiled program's output and diagnostics to be its, and the random programs compared the two. Each case's
output, on which the interpreter and both C compilers agreed, is recorded beside the case (`NAME.expect`) and
holds the compiler to it; `spec/ASSURANCE.md` says what stands in the place of that second reading.

## Known gaps

- From the bug reports (`CHG-0475`, `CHG-0476`, "What is left"): the derived `clone` of a type
  that links to itself only through another type (`Node(Box<Pair>)`) recurses to the depth of its height; the
  destruction of a deep tree that was walked or cloned recurses in the bootstrap form (a node's second borrowed `Box`
  finds the runtime's one tail slot taken), and in the default form that of a deep chain or tree whose values own a
  `String`; an array length cannot use `sizeof` of a type parameter (one length serves every instance) or depend on
  the size of a type holding it; a program with entry points keeps the runtime's locking on, and `--lib` keeps the
  race of a second thread arriving while the first runs unlocked.
- The items the compiler builds in (`printf`, `sqrt`, …), which have no CobaltC source, are
  documented in `impl/std/builtins.md`, checked by `cobdoc.py --check` against the compiler's native lists.
