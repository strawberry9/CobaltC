# CHG-0406 — The gate (D-0307)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Implements: D-0307
Affects: `spec/README.md` ("Evidence tiers"); `impl/gate.sh`, `impl/tools/gate.py` (new);
`impl/battery.sh`, `impl/tools/suites.sh` (now the gate's profiles); `impl/tools/battery_stages.sh` (the former
battery, from which the release profile runs the stress and external stages); `impl/tools/manifest.py`;
`impl/tests/common/mod.rs` (`store`), `impl/cobc/tests/oracle/mod.rs`, the four corpus runners;
`htmlguide/generator/gen.py` and `book/check.sh` (per-example results); `impl/STATUS.md`, `README.md`

## What changed

1. `impl/gate.sh [gate|release|all|UNIT…] [--dirty] [--list]`. Units of the gate profile, in order: `build`
   (release build, the tests built, the self-hosted toolchain, the prelude summary current), `format`, `records`
   (speccheck, proofcover, assurance, runtime contract, cobdoc, std coverage), `portable` (Windows, Rust 1.77.2),
   `sweep-check`, `tests` (every test target but the four corpus runners, with the perf-flake rule), `corpus-coby`
   (three shards each), `corpus-cobc` (rows, then the file suite, GCC and Clang), `std-tests`, `guide`, `book`,
   `showcase`, `model` (the sample against the model, licensed, certified, the certificates read again, the C
   validated, both replays, `lake build`), `selfhost` (`compare_c.sh` on every tracked file with a kept result per
   file, the formatters and parsers on the showcases and the surface-syntax cases), `lowering` (the undecided
   sweep over tracked programs and the local stress programs, the perf guide, the bench, each against the last
   green run's numbers, a rise above 3% red). The release profile adds `fixpoint`, `selfhost-whole`,
   `all-checks`, `slow-cases`, `debug`, `verify`, `programs`, `external` (advisory).
2. Kept verdicts under `<home>/store/units/<unit>/<hash>.json`; per-case results under `<home>/store/cases`
   (`COBALTC_RESULT_STORE`, `COBALTC_TOOLCHAIN_KEY`; without them a runner runs every case, as before); runs
   under `<home>/runs/<stamp>` with `progress`, a log per unit, `REPORT.md` and `SUMMARY`; the snapshot in
   `<home>/snap`. `<home>` is `stress/gate` where `stress/` exists, else `~/.cobaltc-gate`.
3. Both compiled runners pass a case's path canonical, so their runs of a file are one run; the rows' runner
   writes an inline row's program to the same scratch directory in every run while the store is on (a diagnostic
   prints the path).
4. `spec/README.md`'s evidence tiers are defined by the gate (D-0307 item 5); `manifest.py` derives the tier from
   the kept verdicts for this tree's inputs and lists every unit's verdict in `spec/MANIFEST.md`.

5. Found by the first run of the `lowering` unit: the hand-kept list the sweep started from
   (`stress/d0281/objectless/decided.list`, late October) named eleven programs as decided that `cobc` no longer
   decides; all eleven were already undecided under Update 158's `cobc`, with the files unchanged (four conformance
   cases of `15-function-semantics` and `20-trust-boundaries`, `showcase/tier4/vec_from_scratch.cb`, six local
   stress programs). They are not a change of Updates 159-162; which earlier Update lost them is left for a record
   of its own. The sweep's baseline is the gate's last green run from here on, never a hand-kept list, and its
   result per program is kept under the hash of the lowering's inputs and the program.

## Verification

The gate profile run on this Update's commit, its verdicts named in `spec/MANIFEST.md`.
