# CHG-0274 — `unittest` blocks

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under D-0230)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0230
Affects:
- spec: `spec/22` (2.50.0), `spec/17` §6 (2.15.0), `spec/18` §1b (1.7.0), `spec/registry/diagnostics.md`
  (1.54.0), `spec/conformance.md` (3.220.0);
- implementation: `impl/src/{lexer,parser,ast,lib,main,unittest,interp}.rs`, `impl/cobc/src/main.rs`,
  `impl/cobfmt/src/format.rs`, `impl/tools/cobdoc.py`;
- examples: `impl/cobaltc_examples/14_unittest.cb`;
- tests: `impl/tests/{unittest_run,examples_run}.rs`, `impl/cobc/tests/driver.rs`;
- the guide: "Testing a program".

## What changed

- **The language.**
  - The keyword `unittest`.
  - `unittest-decl ::= 'unittest' str-literal? block`, an item.
  - `rule.module.unittest` and `[Unittest-Name]` (`diag.unittest-name`).
  - `rule.test.run`.
- **Both tools.**
  - `--unittest`, `--unittest-filter TEXT` and `--unittest-std`.
  - A test is the hidden, exported function `__unittest_N` of its module (the prefix is reserved).
  - Under `--unittest`, the runner `impl/src/unittest.rs` writes, in CobaltC, becomes the program's `main`.
  - `cobc --unittest` builds or runs the test program.
- **`cobfmt`.** A test's body is formatted as a function's (Allman).
- **`cobdoc.py`.** A `///` test right after an item is that item's example.
- **Conformance.** Nine cases in `impl/conformance/17-modules/` (`conf.unittest-*`):
  - not run by the program, and seeing private items;
  - checked: an error in a test rejects the program;
  - a `void` body;
  - duplicate, empty and control-character names;
  - the same name in two modules;
  - inside a function;
  - `export`.
- **Tests.**
  - `tests/unittest_run.rs`: the report, failures that do not stop the run, the filter and its refusal, a
    file without `main`, file-backed modules' order, `--unittest-std`.
  - `cobc/tests/driver.rs`: the compiled test program reports byte for byte as `coby` does, under GCC and
    Clang, with `-o` and with `--run`.
  - `examples_run.rs` runs the tests of an example that has any.

## What changed for programs

A program that uses `unittest` as a name no longer parses; none in the corpus did. Otherwise nothing:
tests are checked with the program and run only by a test run.
