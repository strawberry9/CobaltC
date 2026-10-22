# CHG-0341 — `cobc`: a contained fault's unwind ends the objectless locals of the frames it folds

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19, with D-0269)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0269, D-0200, D-0204 `[Fault-Contain]`, `spec/18` `[Fault-Unwind]`
Affects: `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; `impl/cobc/src/lower/{frames,functions,closures,passes}.rs`; `impl/cobc/tests/driver.rs`; `spec/conformance.md` (3.251.0); `spec/IMPLEMENTATION-NOTES.md` (the runtime contract); `spec/02` §5 (1.0.121)

## What changed

- **The defect.** An objectless local (D-0200) is freed by a C statement at its block's end; a contained
  fault's unwind (`[Fault-Contain]`) leaves the C frames by a jump, so the statement never runs and the
  buffer leaks, once per contained failure. `coby` frees it. Found by the fuzzer's fault-unwinding mode
  under LeakSanitizer and by round 10's pool (the overnight run of 2026-10-18).
- **The repair** (D-0269). A function that binds objectless locals declares a table of them (`struct
  cb_note`: address, moved flag, type, live), registered with the runtime at entry and withdrawn at every
  exit while threads run (`cb_threaded`, set by the first `spawn`); the binding sets an entry live and the
  local's drop clears it; the contained unwind destroys the live, unmoved entries of the thread's tables
  after folding its frames, by `cb_drop_local`'s path. Runtime: `cb_threaded`, `cb_notes_push`,
  `cb_notes_pop`, `end_noted_locals` in `contained_fault`. Compiler: `Fx::objectless_bound` writes the
  entry and the clearing, `Fx::notes_finish` declares the table and places the pops at the root frame's
  exits; the table's names are free of scope effects (`passes.rs`).
- **Verification.** `conf.contained-fault-frees-objectless-locals` (`impl/conformance/19-concurrency/`):
  six pooled jobs, three faulting, the destructor order and the totals printed alike by both tools; under
  Clang's AddressSanitizer the old compiler leaked 120 bytes in six allocations, the new one nothing
  (`cobc/tests/driver.rs`, `contained_fault_frees_objectless_locals`, Linux with clang only). The
  fuzzer's probe and round 10's `pool.cb` likewise clean.

## What changed for programs

A program whose spawned thread's fault is contained no longer leaks, under `cobc`, the container locals
of the frames the fault unwound. No other program changes; the generated C of a function that binds
objectless locals gains a table and a guarded registration.
