# CHG-0476 — The reports Update 203 left open

Status: implemented (2026-11-21); Update 204
Affects: `spec/14` 1.50.0 (§6, frame-held aggregates: the part-store summary), `spec/16` 1.37.0 (§4a, a chain's
derived `clone`), `spec/17` 2.21.0 (an array length), `spec/19` 1.23.0 (`[Thread-Confined]`, the run-time
completion), `spec/21` 4.87.0 (`rule.stdlib.rc`, `rule.stdlib.hashmap`: `remove`, `key_at`, `value_at`, the
destructor, the trusted table), `spec/registry/diagnostics.md` 1.72.0 (`diag.not-thread-safe` phase *both*,
`diag.write-through-shared`), `spec/conformance.md` 3.321.0 (15 rows), `spec/IMPLEMENTATION-NOTES.md`;
`impl/front` (`typecheck.cb`, `parser.cb`, `each.cb`, `derive.cb`, `diags.cb`, `lexer.cb`), `impl/back` (`passes.cb`,
`gen.cb`, `native_map.cb`, `header.cb` generated), `impl/cbrt` (`rt_objs.c`, `rt_threads.c`, `plat_posix.c`,
`plat_win.c`, `rt_init.c`, `cbrt.h`, `exports.txt`), `impl/cobc` (`host.c`, `host.cb`, `driver.cb`,
`build_info.cb` generated), `impl/std` (`memory.cb`, `collections.cb`), `impl/front/prelude.cb` and
`impl/back/std_values.cb` (generated), `impl/tools`; the cases under `impl/conformance`, `impl/tests`; the guide,
the book, `CARD.md`, `README.md`, `impl/STATUS.md`, `impl/CHANGELOG.md`; D-0333

## Why

The owner asked that Update 204 close every report of `private/BUGS` that Update 203 (`CHG-0475`) left open or
answered in part, so that no later push is needed. D-0333 has what was decided; this record has each report and
what it cost.

## What changes

| # | Update 203 left | Now |
|---|---|---|
| 031 | an `Rc` captured by a closure that reaches `spawn` as a plain `fn` value raced on the count | each `RcBox` has an `owner` word (the first field, read by a plain load); `Rc::clone`, `get`, `downgrade`, `drop` and `Weak::upgrade`, `clone`, `drop` fault with `diag.not-thread-safe` in another thread (`impl/std/memory.cb`, `cb_thread_id`/`cb_thread_here` in the runtime) |
| 014 | a callee storing into a part of an argument given exclusively was judged by the argument's type | a summary of each body's stores (`impl/front/typecheck.cb`, `spec/14` §6) says when the parts count; the shape `&(**q)[0]` is judged by the parts too |
| 045 | `remove` in insertion order took time in proportion to the later entries | the place is left empty and closed up in one pass; 20,000 keys removed in order 2,621 ms → 6 ms, in random order 2,369 → 12, 40,000 text keys the report's 53 s → 11 ms |
| 022 | the derived `clone` of a deep chain recursed; in the bootstrap form so did the drop of a borrowed chain | past 1,024 links a loop (`derive.cb`); `tree_visit` hands a Box in tail position to the drain loop (`rt_objs.c`); a 10-million-link chain clones and drops in every form |
| 021 | check time quadratic in statements and waiting references | a binding whose end is blocked waits for its blocker's last mention; each root's holders are indexed once a flow has more than 24 of them; 4,000 references check in about 2 s (2,000 took 10 s) |
| 044 | nested `Some(…)` and arrays quadratic | the parser matches brackets once; the checker no longer copies the growing type at each level; 100,000 levels in 6 s and 4 s |
| 020 | the C nested two braces for each `if`; Clang needed a flag past 128 | `merge_sole_scopes` (`passes.cb`) removes the braces of a bare block that names nothing; 200 nested `if`s compile with plain Clang |
| 001 | a foreign C thread calling an entry was not registered | `cb_entries_on()` after `cb_init` and `cb_entry_enter()` in each entry's stub register and release it, as `--lib` does |
| 013 | an array length refused `sizeof` and the intrinsics | the parser's length folder takes them (and `sizeof` of a plain struct of the file, checked against the real layout by a `static_assert`); a failure is reported where the length is read |
| 029 | an oversized literal was a syntax error; many repair lines were generic | `diag.literal-out-of-range`; fitting repair lines for the commonest diagnostics (229 recorded outputs re-recorded) |
| 032 | the signal stack of an exited thread leaked; Windows had no containment | the stack is freed when the thread ends (5,000 threads); on Windows a vectored handler contains a stack overflow (`plat_win.c`) |
| 037 | Windows and several Unix paths left the directory | a console handler and a job object on Windows, `COBALTC_CPU_LIMIT` for `--run`'s program there; on Unix the directories are watched from their making, the runner and the C compiler are sent the signal |

## Measured

- `Rc`: 3,000,000 clone, get and drop: 4,998 million instructions → 5,067 (GCC +1.4%), 4,986 → 5,070 (Clang +1.7%).
- The compiler checking its own source: 52.00 thousand million instructions → 52.69 (+1.3%).
- `HashMap` (best of three, ns per operation, Update 203 → now): a loop over a map with no empty place 0.74 → 1.47 (GCC,
  1,000 keys), 1.24 → 1.54 (100,000), 2.23 → 2.58 (2,000,000), Clang 0.56 → 1.47, 0.93 → 1.55, 2.19 → 2.55 (about
  +0.3 to +0.9 ns an entry: the loop reads the count of empty places each turn); `get`, `insert` and `entry` within
  noise; `swap_remove` +7% to +11% at 100,000 and more keys; removal in order of 20,000 keys 2.6 s → 6 ms, in random
  order 2.4 s → 10 ms, the report's 40,000 text keys 53 s → 15 ms. The bench `hashmap` (`bench/RESULTS.md`): the
  compiled program's instructions 1,017.6 → 1,091.9 million under GCC (+7.3%), 885.4 → 959.9 under Clang (+8.4%); the
  baseline was moved by the owner's standing decision to take this cost for the removal.
- The decided programs of `bench`, `showcase` and the HTTP and compression cases stay decided (the sample of 134 and
  the gate's sweep).

## What it costs

- An `Rc` box is a word larger, and an `Rc` operation costs a load and a compare.
- A program with entry points keeps the runtime's locking on.
- The compiler's check of its own source takes 1.3% more instructions.
- A `HashMap` has a destructor and a hidden counter of empty places; a loop over a map with none costs a comparison
  more each turn (0.3 to 0.9 ns an entry); hoisting the test out of the loop is a later optimisation.

## What is left

Update 205 closed 022's trees, 013's layouts, 037's silent exit (the child says why) and the Windows tests (`impl/tests/windows/cleanup.py`,
a step of the bootstrap workflow, run on GitHub's Windows runner: the processor limit, a console break, a killed `cobc`).

- 022: closed after this Update: the derived `clone` of a tree (a struct with two or more links, or owning something on
  both sides of its link, and an enum whose variant with fields links to it) recurses for the first 1,024 levels and
  goes on with a loop over a stack of its own (`conf.clone-deep-tree`, a tree two million levels deep). A type that
  links to itself only through another type (`Node(Box<Pair>)`) still recurses. Found on the way, not fixed: in the
  bootstrap form the destruction of a deep tree that was walked or cloned recurses, and in the default form that of a
  deep chain or tree whose values own a `String`.
- 013: closed after the push: an array length may use `sizeof` and `alignof` of any type the program lays out (a
  generic's instance, `align(N)`, a reference, a container, an enum, a type of another module), computed once the
  program's items are known (`check`'s `parse_program_text`); what remains is a type parameter (one length serves every
  instance) and a length that depends on the size of a type holding it (it does not settle); both are rejected with a
  message saying so (`conf.const-array-length-layout`, `conf.const-array-length-type-param-rejected`,
  `conf.const-array-length-self-size-rejected`).
- 001: the runtime's locking stays on in a program with entry points for the whole run; `--lib` still has the race
  of a second thread arriving while the first runs unlocked.

## Checked

- Each report's reproduction, under GCC and Clang, in the default form, with every check on and in the bootstrap
  form where the report concerns the run.
- 15 new conformance cases (`spec/conformance.md` 3.321.0) and the cases of `impl/tests/cases`; the corpus of 1,598
  files gives the same `--check` output as Update 203's compiler.
- The pregate checks (`impl/tools/pregate.py`) and the gate's run, named in the commit's message.
