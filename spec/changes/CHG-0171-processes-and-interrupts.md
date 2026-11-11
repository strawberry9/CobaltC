# CHG-0171 — `std::process`: `Command`, `Child`, interrupts

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0143)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0143
Affects: `spec/21` (4.9.0) §0, §2k (new); `spec/conformance.md` (3.133.0); the guide §21; `impl/std/process.cb` (new), `impl/std/std.cb`, `impl/src/procio.rs` (new), `impl/src/prelude.rs`

## What changed

- **`spec/21` §2k (new):** `rule.stdlib.process` with `[Command]`, `[Output]`, `[Status]`, `[Spawn-Child]`, `[Child-Streams]`, `[Wait]`, `[Kill]`, `[Interrupt-Flag]` and the CobaltC source.
- **`spec/21` §0:** the submodule table and listing gain `std::process`; a `std::process` table.
- **`std` (`impl/std/process.cb`, new):** `Command`, `Output`, `Child`, `watch_interrupts`, `interrupt_requested`, written in CobaltC over the std-private `proc_op`; failures are `FileError`s.
- **Both tools:** `proc_op` in `impl/src/procio.rs` (child table, read-ahead for `read_output_line`, the interrupt flag and its handlers); routed as D-0141's `path_op` is (`cb_proc_op` in `cbrt`).
- **Rows:** `conf.process-output`, `conf.process-input`, `conf.process-status`, `conf.process-signal-status`, `conf.process-not-found`, `conf.process-dir-env`, `conf.process-spawn-lines`, `conf.process-kill`, `conf.process-try-wait`, `conf.interrupt-flag`.

## Compatibility classification

Additive: a new submodule `std::process` and its names, shadowed by a program's own.
