# CHG-0475 — The bug reports of Update 200

Status: implemented (2026-11-20); Update 203
Affects: `spec/14` 1.49.0, `spec/15` 1.22.0, `spec/16` 1.36.0, `spec/19` 1.22.0, `spec/20` 1.21.0, `spec/21`
4.86.0, `spec/22` 2.66.0, `spec/registry/diagnostics.md` 1.71.0, `spec/conformance.md` 3.320.0,
`spec/IMPLEMENTATION-NOTES.md`; `impl/front` (`typecheck.cb`, `parser.cb`, `lexer.cb`, `each.cb`, `ast.cb`,
`consteval.cb`, `libc_names.cb` new), `impl/back` (`gen.cb`, `cnames.cb`, `literals.cb`, `state.cb`,
`tier.cb`, `native_map.cb`, `native_io.cb`, `std_values.cb` and `header.cb` generated), `impl/cobc`
(`main.cb`, `cc.cb`, `build_info.cb`), `impl/cobfmt`, `impl/cbrt`, `impl/std` (`collections.cb`,
`memory.cb`, `fs.cb`, `net.cb`, `http.cb`, …), `impl/front/prelude.cb` (generated); the cases under
`impl/conformance`, `impl/tests/cases`, `impl/tests/programs`, `impl/tests/rows`; `book/`, `htmlguide/`;
`CARD.md`; D-0332

## Why

An agent testing Update 200 (`e212300`) on another machine filed 45 reports, each with a reproduction
(`private/BUGS/`, outside the repository). The owner asked for every one to be fixed in the next push, under
one gate run. D-0332 has the choices that changed a rule; this record has every report.

## What changes

The number is the report's.

**The default form read freed memory** (the static pass decided a program whose conflicts only the run sees):

| # | Cause | Now |
|---|---|---|
| 004 | a closure literal inherited no facts (`inherited_facts_in`), so a returned `move` closure over a local's reference, view or slice reached nothing | it holds what its captures reach (`spec/14` §6); not decided, the default form faults as `--all-checks` does |
| 005 | a reference through a temporary guard counted as static data | kept past its statement, or past a bound guard's `drop` or block, it is `diag.destroy-while-aliased` (`spec/19`) |
| 008 | an assigned `Option` or `Result` of a reference recorded no borrow | it records what its new value borrows, as a declaration does |
| 009 | the call-site rule skipped `fn` parameters, so an inline closure's captures were not counted | they count, each in its mode; a certain overlap with another argument is `diag.aliasing-conflict` |
| 014 | `kept_from` (D-0290) did not count references the callee forms into parts of an argument | `part_leaves` counts them; a holder takes `?` facts, a store elsewhere is the run's |
| 031 | nothing kept an `Rc` in its thread | `[Thread-Confined]`, `diag.not-thread-safe` (D-0332 §2.3) |

**Accepted programs that `cobc` or the C compiler could not build:**

| # | Now |
|---|---|
| 001 | an `extern fn` entry is exported to C from a program with `main` too (`entry_tail`) |
| 010 | a closure whose result holds a `fn` value is lowered at the result type the checker recorded, and a closure it returns is boxed |
| 012 | a block, `if` or `match` whose value is a closure, with no type expected, is that closure's `fn` value |
| 015 | the closure's environment is the hidden `$self` (C `cb_clself`), so a parameter may be named `self` |
| 016 | a name beginning `_` and a capital, or `__`, takes a `cb` prefix in C |
| 017 | an entry named as C, its library or the runtime has a name is refused (D-0332 §2.6) |
| 020 | Clang is given `-fbracket-depth=4096`; the C still nests two braces for each `if` |
| 034 | a call of more than 64 arguments no longer overflows the argument masks' shift |
| 040 | a closure literal as a generic payload or field with no type expected is boxed as its `fn` value; a constructor's arguments take the type arguments its parameter fixes |

**Tools and the user's files:**

| # | Now |
|---|---|
| 024 | the generated C is written in `cobc`'s own temporary directory, and beside the executable only with `--keep-c` and only over a file `cobc` wrote; an output that is one of the program's own files (source, module, `extern "./x.c"`) is refused |
| 025 | an output `cobc` cannot create is reported before compiling, with the system's reason, status 2 |
| 026 | `cobfmt` rewrites a file in place, so links, owner and mode stay; a read-only file is left as it was, status 2 |
| 027 | `--certificate` is written once at the end, so a pipe or FIFO receives it |
| 035 | a mistake on the command line names the argument and the reason |
| 037 | during `--run`, SIGINT, SIGTERM and SIGHUP go to the program; `cobc` removes its directory and ends by the same signal (POSIX); Update 204: from the moment `cobc` makes a directory, while it compiles and under `--explore` (whose program gets the signal too) and a runner, a signal removes the directories before it ends `cobc` (POSIX); on Windows a console handler (Ctrl-C, Ctrl-Break, the console closed) waits for the programs `cobc` started, removes the directories and ends `cobc` with STATUS_CONTROL_C_EXIT, and `--run`'s program is in a job that ends it with `cobc` |

**Security:**

| # | Now |
|---|---|
| 030 | the HTTP server refuses conflicting `Content-Length` fields, one that is not digits, two `Transfer-Encoding` fields, a bad field name, a control byte in a field value or the target, and two `Host` fields (`BadMessage`); the client applies the length rules to responses |
| 023 | bidirectional control characters are refused in source and by `cobfmt` (D-0332 §2.5) |

**The checker and the specification:**

| # | Now |
|---|---|
| 003 | in a generic body a struct or variant pattern through a reference binds references to its fields |
| 006 | a `String` through a temporary guard can be viewed and sliced; the message no longer says a `String` is not sliceable |
| 007 | `simd_load` and `simd_store` take `&a` of a whole array or `Vec` (`[Coerce-Slice]`) |
| 011 | `match (e) { }` is a syntax error (D-0332 §2.8) |
| 013 | a `const` of the intrinsics is evaluated when checked (D-0332 §2.9) |
| 018 | not a defect (D-0332 §3); `[Sort-Not-Key]`'s text and the doc comments are corrected |
| 036 | a `requires` failure in a function `std` calls back is reported at the program's call of `std` |

**Time and memory of checks and lowering:**

| # | Before | After |
|---|---|---|
| 042 | a type recursing at a larger argument: `--check` never ended | refused in 0.1 s (D-0332 §2.10) |
| 043 | a doubling type argument: never ended | refused in 0.1 s |
| 019 | 16 nested `for` loops 11.3 s, 20 over 120 s | 0.09 s each (a loop's fixed points kept per entry flow) |
| 021 | 500 references live at once 32 s | 0.8 s (per-name indexes for the early-end plan) |
| 044 | 100,000 nested parentheses 39 s | about linear (`whole_slice_lint` checks the wanted type first) |
| 033 | `--emit-c` of 40,000 array elements 13.4 s | 1.4 s |
| 045 | removing 40,000 keys from the end back 11.1 s; `--check` of 32,000 locals or match arms over 20 s | 4 ms; about 1 s (D-0332 §2.11) |
| 022 | not a defect (D-0332 §3) | documented |

**Diagnostics:**

| # | Now |
|---|---|
| 002 | a type parameter is shown by name (`W<T>`) |
| 029 | a message can carry its own repair line, printed in place of the registry's; the reported situations have fitting ones; calling a non-function, a closure's arity and `%5%` have their own messages |
| 035 | (above) |
| 038 | `..=` (D-0332 §2.7) |
| 039 | turbofish, `char* p`, `T x = { … }`, `auto&`, `try`/`catch`/`throw`, a leading `::` and `===` have hints |
| 041 | new error variants (D-0332 §2.12) |
| 028 | `[Print-Failure]` (D-0332 §2.4) |
| 032 | a frame larger than the stack's guard is probed (Clang `-fstack-clash-protection`, GCC's default), and a SIGSEGV on the guard, caught on a signal stack of its own, is `diag.stack-exhausted`, contained by `try_join` in a thread (POSIX); Update 204: on Windows (x86-64) a vectored handler turns EXCEPTION_STACK_OVERFLOW (MinGW's `___chkstk_ms` probes reaching the guard), or an access violation below the floor in the stack's reservation, on a watched thread into the same fault, each thread asks for a 64 KiB stack guarantee, and a contained overflow makes the guard page again (as `_resetstkoflw`), so a second overflow is caught too; a library entry's `_try` call puts the floor back after one |

**Before the gate** (`impl/tools/pregate.py`, `impl/tools/docs_coverage.py`, `impl/tools/gate.py`): a profile's
run first checks, in a few minutes, what its units would find an hour in: that every generated file is what
its generator writes (the registry inside `cobc`, which no unit checked; the prelude; the runtime's tables;
`spec/21`'s trusted table; `CARD.md`; the guide's HTML; the seed), the records, and that the documentation
keeps up: every diagnostic has a row in the guide's table (it lacked 28, now written from the registry), and
every rule and diagnostic new since the last push is cited in the guide or the book, or listed in
`impl/tools/docs_exempt.txt` with the reason it needs none. `--no-pregate` skips it. The gate's build unit
checks the registry inside `cobc` too.

## What was left

Update 203 left these open; Update 204 (`CHG-0476`) closed each, and its "What is left" says what limits remain.

- 001 (a C thread the program did not start), 013 (`sizeof` and the intrinsics in an array length), 014 (a callee storing
  into a part of an argument given exclusively, and into a part of what its referent keeps), 020 (the generated C's
  nesting), 021 and 044 (checks that were still quadratic), 022 (the derived `clone` of a deep chain, and the drop of
  one in the bootstrap form), 029 (the oversized literal's id and the repair lines), 031 (an `Rc` hidden in a `fn`
  value), 032 and 037 (on Windows; a thread's signal stack; a signal while compiling, under `--explore` and under a
  runner; `COBALTC_CPU_LIMIT` on Windows), 045 (removal in insertion order).

## Checked

- Each report's reproduction behaves as its "how to verify" asks, under GCC and Clang.
- 49 new conformance cases (`spec/conformance.md` 3.320.0) and new cases under `impl/tests/cases`; the
  positive ones print the same in the default form, with every check on and in the bootstrap form.
- The static rejections' recorded messages re-recorded where the text changed.
- The decided programs of `bench` and the decided list stay decided.
- The gate's first run was red in three units, each put right before the run the commit names: a case of
  `spec/10` that the run had refused is now refused before it (`conf.option-reference-escape-checked-at-run`,
  issue 008's fix); the new error variants had no case (`conf.error-variants-text`); a test of a private
  function of `std` was marked as a published example.
- The gate's run is named in the commit's message.
