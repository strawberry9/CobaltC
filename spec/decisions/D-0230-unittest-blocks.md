# D-0230 — `unittest` blocks, after D

Status: ACCEPTED (2026-10-14; the owner chose D's `unittest` in place of Q10's convention, and decided U1, U3,
U4, the flags and the filter; U2 and U5–U11 as recommended; draft in `private/unittest-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0204 (contained faults, `try_join`), D-0206 (`--check` is acceptance), D-0229 (`///`)
Affects: `spec/22` §1, §3 (2.50.0), `spec/17` §6 (2.15.0), `spec/18` §1b (1.7.0),
`spec/registry/diagnostics.md` (1.54.0), `spec/conformance.md` (3.220.0) (CHG-0274)

## Decision

```
unittest "parse_port: bounds"
{
    assert(parse_port(StringView::of("65536")) == None);
}
```

- **U1, the keyword.** `unittest` is reserved: one new keyword. No file in the corpus used it.
- **U2, placement.** A test is an item at module scope: the top level of a file, inside `module m { … }`,
  or in a file-backed module. One inside a function is a syntax error, and so is `export` on one.
- **U3, the name.**
  - A test may have a name, a text literal. Adjacent literals join, as everywhere.
  - The name is not empty and holds no control character.
  - Two tests of one module have different names; the same name in two modules is allowed.
  - Breaking any of these is `diag.unittest-name`.
  - The name is data only: it is in no namespace, and the report shows it beside the test's file and line.
- **U4, checking.**
  - A test's body is checked as the body of a `void` function of no parameters, every time the program is
    checked (`coby`, `cobc`, `--check`). A static error in a test makes the program ill-formed, so D-0206's
    `--check` is unchanged.
  - Tests are never compiled into the program or run without `--unittest`.
  - `std`'s tests are checked only under `--unittest-std` or `COBALTC_CHECK_ALL=1`, as `std`'s other
    unreached items are, so the startup cost of Update 83 is unchanged.
- **U5, privacy.** A test is part of its module and sees its non-exported items.
- **U6, order.** The tests run in the order of the program's text, a file-backed module's tests where the
  module is declared. With the usual layout, modules declared before the program's own items, that tests
  a program's imports first.
- **U7, isolation.** Each test runs as `try_join(spawn(test))`: a thread of its own whose failure is
  contained (`spec/19` [Fault-Contain]). A failed `assert`, any other fault, or a cancellation is reported,
  that test's scopes unwind, and the next test runs. Tests run one at a time.
- **U8, the body.** A `void` block: `return;` is allowed, and `return e;` with a value is rejected. A
  test sees the arguments the test run was given (`coby --unittest FILE.cb ARG…`), usually none; it may spawn,
  lock, and use files and the network as `main` may. `?` is not available in a test (no `Result` return).
- **U9, documented tests.** A `///` comment on a `unittest` right after an item makes that test the
  item's example in `impl/tools/cobdoc.py`'s output, and so in the std reference (WI-18).
- **U10, test-only code.** No conditional compilation. A helper only tests use is an ordinary non-exported
  function, and `cobc` emits only what the program reaches.
- **U11.** There is no separate test-program convention or `cobtest.py`. The battery compares the reports
  that `coby --unittest` and `cobc --unittest` give, under GCC and Clang.

### Running tests

- `coby --unittest FILE.cb`, `cobc --unittest --run FILE.cb`, or `cobc --unittest -o tests FILE.cb` to build
  a test program. `--test` is not used.
- `--unittest-filter TEXT` runs only the tests whose name contains TEXT. The match is case-sensitive, and
  an unnamed test never matches. The summary adds `, N filtered out`. A filter that matches nothing is
  refused before anything runs, with status 1.
- `--unittest-std` runs `std`'s own tests.
- A file of tests needs no `main`. The program's `main`, if it has one, is checked but not run.

```
test port.cb:19 parse_port: bounds ... ok
test port.cb:26 ... FAILED
fault[diag.assert-failed]: x is not a port
  --> port.cb:28:5
…
3 tests: 2 passed, 1 failed
```

The report goes to standard output, and the status is 1 when a test failed.

### How it is built

- Each test is the hidden, exported function `__unittest_N` of its module. Names that begin `__unittest_`
  are reserved.
- Under `--unittest`, the front end (`impl/src/unittest.rs`) writes the runner as CobaltC: a `main` that
  prints each test's line and calls `try_join(spawn(…))`, printing `ThreadFailure::text` on a failure. It
  then parses the program with that runner and renames the program's own `main`.
- Because the runner is CobaltC, the interpreter and the compiled program report a run byte for byte alike.
  No runtime code was added.

## Alternatives considered

- **A naming convention for test programs plus a runner script** (Q10's recommendation). Declined by the
  owner in favour of tests beside the code they test.
- **`test` as the keyword.** Declined: it is a common name, so reserving it would break programs.
- **Only tokenising a test's body unless tests are run** (D's rule). Declined: tests would rot, and the
  validity of a program would depend on a flag.
- **Stop at the first failure; run tests in parallel.** Not now: `--unittest-jobs N` can come later.

## Left for later

`--unittest-jobs N`; a body returning `Result` so that `?` works in a test; benchmarks.
