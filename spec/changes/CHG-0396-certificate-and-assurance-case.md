# CHG-0396 — The certificate of the static pass's decisions, the model's check of it, and the assurance case (D-0299 step 1)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0299, D-0241, D-0270, D-0284, D-0292, D-0296
Affects: `spec/ASSURANCE.md` (new, non-normative, 1.0.0), `spec/02-schema.md` §5 (1.0.140), `spec/README.md`;
`impl/src/certificate.rs` (new), `impl/src/lib.rs`, `impl/src/typecheck.rs` (`tier_report::Rep::lefts`, the raw
records), `impl/src/main.rs`, `impl/cobc/src/main.rs`; `impl/model/src/{machine,explore,main}.rs`;
`impl/tools/certify_model.py`, `impl/tools/assurance.py` (new); `impl/battery.sh`, `impl/tools/suites.sh`;
`impl/tests/cli.rs`, `impl/cobc/tests/driver.rs`, `impl/model/src/explore.rs` (tests); `impl/selfhost/front/typecheck.cb`,
`impl/selfhost/cbc/{driver,main,certificate}.cb` in the same Update (D-0296); `README.md`, `impl/STATUS.md`,
`impl/model/README.md`, `htmlguide/generator/content_perf.py`

## What changed

1. **`--certificate FILE`** in `coby` (before `--check FILE` or a run) and `cobc` (anywhere on the line, `--check`
   and `--emit-c` included). The static pass appends a raw record for each body it checks when
   `COBALTC_CERTIFICATE` names the file (`tier_report::end`): the body's key, every conflict it leaves to the
   run (`Rep::lefts`, new: `left` kept the first reason only), its events, its error. `certificate::finish`
   rewrites the file: `CobaltC certificate 1`, the tool, the program, the verdict, then each body sorted
   (`main`, the program's own, `std`'s) as `fn KEY: decided | left | rejected` with its `left`, `dynamic`,
   `point`, `tier-d`, `origin` and `static-data` lines, every line mapped to its file and line. The verdict is
   `decided` or `paths kept: WHY` from the lowering's `pathless` result; `rejected: DIAG` for a program the
   static pass refuses (`finish_rejected`); for `--check`, or a `coby` run, how many bodies the pass decided
   (`certificate::check_one` checks one program and writes the file; two paths or a directory are refused,
   exit status 2). Nothing else changes: the static pass's verdicts and the tier report are as they were.
2. **`cobmodel certify [--witness] CERT FILE`.** `machine::compile_certified` marks the certificate's decided
   bodies (`Code::decided`, closures with their function; `Code::all_decided` for `verdict: decided`);
   `explore::run_certified` explores; a fault at a step of a decided body with a diagnostic in
   `explore::UNRECORDED`, not a value-state check (`Fault::value`: `[Fn-Value-Empty-Call]`, a handle already
   taken), through a frame-held path (`State::frame_held`: the access's token, up through projections, is a
   binding's own path in the current call or the reference a reference-typed binding holds; never in a
   thread's entry call for its parameters) — or any path when the whole program is decided — is reported as
   `model.decided-but-false`. `Fault` carries the access's token (`fault_at`) where the fault is an access's
   (`clash` at `borrow`, `read`, `write`, the affinity and lend checks; temporal validity at `borrow`, `read`,
   `write`). The static verdicts are compared first. `impl/tools/certify_model.py DIR [--check]` runs
   `cobc --certificate … --emit-c -o /dev/null` and `cobmodel certify` over a directory and tallies.
3. **`spec/ASSURANCE.md`** and **`impl/tools/assurance.py --check`**, as D-0299 describes them; `spec/02` §5
   lists the page; `spec/README.md` points to it beside `STABILITY.md`.
4. **Battery and suites.** Stage 1: `certify_model.py impl/model/sample --check` after `--as-licensed`, and
   `assurance.py --check` beside `proofcover`; `suites.sh`: `model_certified`.
5. **Documentation.** `README.md` (the certificate paragraph, the spec list), `impl/STATUS.md` (both tools'
   modes, the tools table, the testing table), `impl/model/README.md` (the command, the battery), the guide's
   chapter P (one paragraph after `--report-tracking`), `cobc --help`.
6. **The toolchain in CobaltC.** `cbc --certificate` as `cobc`'s, the raw records written by
   `front/typecheck.cb`'s `tier_flush`, finished by `cbc/certificate.cb`.

## Verification

- `cargo test --release -p cobmodel` (the three new tests and the two refinements); `-p coby --test cli`
  (`certificate_is_written_by_check_and_by_a_run`); `-p cobc --test driver`
  (`certificate_names_decided_bodies_and_the_verdict`).
- `certify_model.py impl/model/sample --check`: every program certified or rejected by both (D-0299, Results:
  the three first-run findings and the refinement they led to).
- A forged certificate refuted (`model.decided-but-false` at the conflict's line).
- `assurance.py --check`, `speccheck.py`; `cargo check --workspace --tests` for Linux and
  `x86_64-pc-windows-msvc`; `cargo +1.77.2 check`; the guide regenerated; `cobfmt --check` on the new `.cb`
  files; `impl/selfhost/compare_c.sh` on the showcases after the port.
