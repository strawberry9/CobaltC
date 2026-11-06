# CHG-0339 — `coby`: a fault leaving a statement ends the statement's temporaries, a call's pending arguments among them

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, the overnight run of 2026-10-18, owner's instruction: a tool defect found by the fuzzer's fault-unwinding mode)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: `spec/18` `[Fault-Unwind]`, `spec/14` `rule.control.stmt` (a statement's temporaries), D-0007 (argument order), D-0150 (`[Terminate-Exit]`), D-0261
Affects: `impl/src/interp/exec.rs` (`end_temps_on_exit`), `impl/src/interp/eval.rs` (`arg_now`); `spec/conformance.md` (3.249.0); `spec/02` §5 (1.0.119)

## What changed

- **The defect.** `take(v, R { .id = 5 }, boom(1))`, with `boom` faulting (or calling `exit`): the
  arguments are evaluated left to right (D-0007), so `R { .id = 5 }` exists, a temporary of the
  statement, when the third argument's call raises the fault. `[Fault-Unwind]` runs every destructor
  D-0008 would run on ordinary exit, and `cobc` printed `[drop 5]` between the callee's locals and the
  caller's; `coby` did not. Two causes: a temporary built for an argument (a struct literal, a call's
  result) was not registered with the statement, only a copy made from a place was; and the statement's
  exit on a fault left its temporaries alone ("a fault keeps its own unwinding"), which the frames' unwind
  then never reached, since a temporary is owned by no frame. Found by the fuzzer's fault-unwinding mode
  (seed 14, program 152: `exit` in a nested call; the fault form reduced beside it).
- **The repair.** `arg_now` registers a temporary argument with the statement (`bind` adopts it when the
  call binds the parameter, so an ordinary call is unchanged); `end_temps_on_exit` ends the statement's
  temporaries on a fault too, last first, keeping the first fault as the outcome (a destructor's fault
  during the unwind is dropped, as the frames' unwind drops it).
- **Verification.** `conf.fault-ends-pending-argument` and `conf.exit-ends-pending-argument`
  (`impl/conformance/18-error-failure-semantics/`): `[drop 901][drop 5][drop 12]` under both tools, the
  first with `diag.index-out-of-bounds`, the second with exit status 3. The trace comparison and the
  interpreter suites unchanged.

## What changed for programs

A program whose fault or `exit` was raised while a call's arguments were being evaluated runs, under
`coby`, the destructors of the arguments already built, as it did under `cobc` and as the rule says. No
other program changes.
