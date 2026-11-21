# CHG-0280 — `interp.rs` in modules

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; at the owner's request, WI-15 item 3)
Governed by: `CobaltC_Master_Instructions.md` §20
Depends on: CHG-0275; D-0219 (the model, whose permitted outcomes the evidence cites)
Affects: `impl/src/interp.rs` and the new `impl/src/interp/`; `spec/IMPLEMENTATION-NOTES.md` §5 (where three
choices are made); `spec/02-schema.md` (1.0.86)

## Why

`impl/src/interp.rs`, `coby`'s evaluator, was 9,479 lines in one file, and its intrinsics were one function
of 1,198 lines. CHG-0275 split `cobc`'s lowering in the same way and left this as the optional third item
of WI-15.

## What changed

- **`interp.rs` in modules.** The evaluator's types (`Interp`, `Items`, `Object`, `Frame`, the thread
  state and the lock), its constructor and the free functions stay in `interp.rs`, now 1,416 lines.
  `Interp`'s methods moved, by concern, into 24 modules under `impl/src/interp/`: threads, frames,
  objects, keys, access checks, destruction, statements, expressions, literals, operators, borrows,
  references, indexing, assignment, `match`, closures, calls, the intrinsics, memory, `extern` calls and
  layouts. The move was made by a script (`stress/plan_oct12/wi15/interp/split_interp.py`) that checked
  that no line was lost or duplicated; a moved method is `pub(super)`, which is the same reach as a
  private method of `interp.rs`.
- **The intrinsics in four.** `try_intrinsic` keeps the value and numeric intrinsics; its last arm hands
  any other name to `intrinsic_raw` (raw pointers, raw strings, allocation, `fault`), whose last arm hands
  it to `intrinsic_threads` (`lock`, `spawn`, `cancel`, `join`), whose last arm hands it to
  `intrinsic_runtime` (exit, printing, std's native helpers, assertions, formatting). The arms are tried
  in the order they were. `spawn`'s arm is the method `intrinsic_spawn`. No function is now over 400
  lines; the longest is `call_extern`, 373.

## Evidence

- `coby`'s release suites pass: the 560 conformance rows, the file suite, the command-line, `unittest`
  and examples tests, and `std`'s 124 tests; so do `cobc`'s driver and differential tests, which compare
  against `coby`. The workspace checks for Windows and under Rust 1.77.2 without a warning.
- Every program of the corpus outside the conformance suite and `bench` (1,566: the examples, the
  showcases, the stress rounds and the samples of the model's and the generator's programs) was run
  under the old and the new `coby`. 36 ran past 60 s under one or both; two of them only under one,
  because three programs shared the machine, and timed alone they finish under both. Of the rest, 1,525 give the same
  output, standard error and exit status. The other five are four thread programs, each with two
  outcomes that the model permits and that the old `coby` gives in turn too, and
  `round6/bugs/stackprobe/mutual.cb`, whose stack runs out one call deeper, so that the fault is reported
  in `odd` rather than `even`: 162,731 nested calls against 162,730.
- The time is unchanged, three runs each, alternating: `stress/m01/…/records.cb` 50.55–51.28 s before,
  50.39–51.35 s after; `box_list_drop_loop.cb` 52.23–53.83 s and 52.31–54.09 s; `crypto_kdf_ok.cb`
  25.48–25.71 s and 25.89–26.81 s; `showcase/tier3/tinyos.cb` 1.79–1.83 s and 1.78–1.84 s.

## What changed for programs

Nothing.
