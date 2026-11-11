# CHG-0338 — `cbrt`: the fault entry points allocate nothing a contained fault's jump would lose

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, the overnight run of 2026-10-18, owner's instruction: a tool defect found by the fuzzer's fault-unwinding mode)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0204 (`[Fault-Contain]`), D-0065 (assertion messages), D-0261 (the fuzzer and its sanitizer oracle)
Affects: `impl/cbrt/src/lib.rs` (`cb_fault`, `cb_fault_msg`, `fault`, `contained_fault`); `spec/conformance.md` (3.248.0); `spec/02` §5 (1.0.119)

## What changed

- **The defect.** `cb_fault` copied the diagnostic's name out of the C string into a Rust `String`, and
  `cb_fault_msg` built the message and the joined text the same way, before calling `fault`. A fault in a
  spawned thread that `[Fault-Contain]` contains leaves those frames by a jump to the thread's trampoline
  (`cb_thread_jmp`), which drops nothing, so every such allocation was lost: 24 bytes for an index fault,
  19 for an overflow, and for an assertion its message and the joined text besides. A program that keeps
  going after contained failures leaked a little memory at each, and the fuzzer's LeakSanitizer oracle
  reported every contained fault (seed 14: 30 of 300 programs). The main thread's fault keeps its frames
  to the process's end, so nothing was reported there.
- **The repair.** `cb_fault` borrows the name from the C string (a literal in the generated C) with
  `CStr::to_str`; `cb_fault_msg` builds the one joined text and hands it on owned (`fault_owned`);
  `fault_impl` takes the text as a `Cow<str>`, and `contained_fault` moves an owned text into the
  failure's record (`Failure.raw`) instead of copying it. The terminating path is unchanged.
- **Verification.** `conf.contained-fault-message-kept` (`impl/conformance/19-concurrency/
  contained_fault_message_kept_ok.cb`): an assertion with a formatted message fails in a thread holding
  a lock; `try_join` gives the failure, whose diagnostic name and report (the message in it) read back,
  and a second thread finds the mutex poisoned. The fuzzer's seed-14 programs under the ASan wrapper
  report no leak from these entry points any more (`stress/verify/fuzz/campaign_s14b.log`).

## What changed for programs

Nothing observable: the same faults, reports and failure records. A program containing many failures no
longer grows by a few dozen bytes per failure.
