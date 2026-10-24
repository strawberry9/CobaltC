# CHG-0272 — A standalone conformance runner

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; WI-12 of the forward plan)
Governed by: `CobaltC_Master_Instructions.md` §20
Affects: `impl/conformance/run.py` (new), `impl/conformance/README.md`, `NAMING-POLICY.md`

## What changed

`impl/conformance/run.py` runs the conformance cases against any implementation. It uses Python 3's
standard library alone: no Rust, `cargo` or build of this repository.

- `--impl "COMMAND {file}"` gives the command.
- Each case's header is read as the Rust runners read it (`kind`, `expect`, `expect-stdout-hex`,
  `stdin-hex`, `args` with `$TMP`, `cobc-only`, `platform`, `slow`). Fixtures are skipped.
- Each case is run in its own directory and killed with its process group after `--timeout` seconds.
- The outcome is the exit status, and for a diagnostic the id and phase from stderr's first line,
  `error[diag.ID]` or `fault[diag.ID]` (D-0228). Output is compared where `expect-stdout-hex` fixes it.
- `--no-c-code` makes the `cobc-only` cases ones that must be refused.
- Also `--slow`, `--allow-unsupported`, `--only`, `--jobs` and `--list`.
- The exit status is 1 when a case fails.

`NAMING-POLICY.md` names it as how a conformance claim is checked, and `impl/conformance/README.md`
describes it.

## Checked

- Against `coby --no-c-code`: 896 cases, 895 passing with the 21 slow ones skipped. The one failure was
  `thread_is_finished_ok.cb`, which still read the old form of a failure's text (CHG-0271) and is fixed.
- Against `cobc --run`: see `impl/STATUS.md`.

Both give the pass set that `cargo test` gives.
