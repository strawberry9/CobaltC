# CHG-0275 — `lower.rs` in modules, internal errors, and intrinsic shapes checked

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under the forward plan of 2026-10-12, WI-15)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0206 (`--check` is acceptance), D-0228 (the diagnostic form)
Affects: `impl/cobc/src/lower.rs` and the new `impl/cobc/src/lower/`, `impl/cobc/src/main.rs`,
`impl/cobc/tests/driver.rs`; `impl/src/internal.rs`, `impl/src/lib.rs`, `impl/src/main.rs`,
`impl/src/typecheck.rs`; five cases in `impl/conformance`, `spec/conformance.md`; the guide's tour

## Why

`impl/cobc/src/lower.rs` had grown to 20,284 lines in one file, with six functions over 400 lines (the
longest, `lower_intrinsic`, 1,053). A panic in either tool showed as Rust shows one, a `thread … panicked
at` message, and exited with status 101. Reading the compiler's 1,541 `unwrap`, `expect` and `panic` sites
for this change found calls that a program can write and both tools then crashed on.

## What changed

- **`lower.rs` in modules.** The generator's types, `Gen` and `Fx`, and the shared constants stay in
  `lower.rs`; their methods and the free functions moved, by concern, into 32 modules under
  `impl/cobc/src/lower/`: the program and its layouts, one function's bodies, the natives of each std
  type, the per-function analyses, and, for `Fx`, frames, statements, places, expressions, closures,
  calls, `match` and the intrinsics; the syntax walks and the text passes over the C. The move was made
  by a script that checked that no line was lost or duplicated.

  The six long functions were then split, so that none is over 400 lines:
  - `lower_intrinsic` dispatches to five, by kind of intrinsic;
  - `lower_user_call`'s direct paths for an argument went to `lower_user_arg_direct`;
  - `lower_expr_inner`'s assignment, path, `while` and place arms became methods;
  - `scan_vec_uses`'s scanner is the module `lower/vec_scan.rs`;
  - `lower_match`'s binding of an arm's binder is `bind_match_arm`;
  - `native_vec_index` hands the other std types to `native_std_body`.

  The generated C was compared before and after over 2,583 programs: every conformance case, the
  examples, the showcases, `bench`, the stress rounds, and samples of the model's and the differential
  generator's programs. The C, the standard error and the exit status are byte for byte the same for all
  of them. `cobc`'s own time is unchanged (`showcase/tier3/tinyos.cb`: 0.42–0.46 s before, 0.43–0.45 s
  after, three runs each).
- **Internal errors.** A panic anywhere in `coby` or `cobc` is now one line on standard error,

  ```
  internal error in cobc: <what> (<the tool's source file>:<line>); please report it with the program
  ```

  with exit status 4. With `RUST_BACKTRACE` set, Rust's backtrace follows. Status 3 stays `unsupported`.
  `COBALTC_TEST_INTERNAL_ERROR` makes either tool panic on purpose, for the driver test
  `internal_error_is_one_line`.
- **Intrinsic shapes.** The static pass now checks the number of type arguments and of arguments of
  each intrinsic of a fixed shape, before evaluation or lowering relies on it. A call of another shape
  is `diag.type-mismatch` (static), and its message gives the written form:
  `` `sizeof` is written `sizeof<T>()` ``. Before, `sizeof()`, `widen<i64>()`, `to_int(5.0)`,
  `str_len()`, `drop()`, `allocate()`, `release(p)`, `spawn()`, `join_status()` and their kin were
  accepted, and then either tool panicked or a run faulted; so were `Mutex::new()` and
  `Result::map_err()`. Where the operand's type is known, the static pass also checks what the thread
  and raw intrinsics take: `spawn` a function, `join` a handle, `cancel` and `join_status` a handle's
  reference, `lock` a mutex's reference, `rawptr_of` a reference. Before, `spawn(7)`, `join(7)`,
  `lock(1)` and `rawptr_of(1)` were accepted: `coby` faulted when it ran them and `cobc` refused them as
  unsupported. The spec's rules give these calls no type, so the tools now follow the text, and no
  specification text changes.

  A sweep called every intrinsic a program can name with no arguments, four arguments, a type argument
  and two, and the right count of arguments of the wrong types, inside and outside `unsafe`. No call
  reaches an internal error in either tool now (646 probes, `stress/plan_oct12/wi15/probes`). Cases:
  `conf.intrinsic-without-type-argument-rejected`, `conf.conversion-without-operand-rejected`,
  `conf.raw-intrinsic-wrong-arity-rejected`, `conf.spawn-of-non-function-rejected`,
  `conf.join-of-non-handle-rejected`.

## The audit

The 1,541 sites in `impl/cobc/src/lower*`:

| Count | Kind | Disposition |
|---:|---|---|
| 1,377 | `write!`/`writeln!` into a `String`, `.unwrap()` | cannot fail |
| 40 | the top of a scope or frame stack the caller pushed | invariant of the lowering |
| 14 | a `Mutex` lock | fails only after another panic |
| 60 | `expect(…)` | invariants of the front end (a declaration, a field or an enum that type checking found; a signature the lowering wrote) or of the lowering; the five on an intrinsic's type argument were reachable, and the shape check above closes them |
| 33 | other `unwrap()` | each after the test that guards it |
| 13 | `unreachable!` | each after a `matches!` of the same shape |
| 4 | `panic!` | three on the cleanup stack's order; one a debugging switch (`COBC_PANIC`) |

None of the 2,583 programs above, and none of the sweep's, reaches any of them.

## What changed for programs

A program calling an intrinsic in a shape it does not have is now rejected statically, where it used to
crash a tool. Nothing else changes.
