# CHG-0391 — The self-hosted toolchain ported to the current edition; an arm's nested `match` scopes its arms

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-30)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0296, D-0278, D-0281, D-0282–D-0295
Affects: `impl/selfhost/front/*`, `impl/selfhost/back/*`, `impl/selfhost/cbc/*`, `impl/selfhost/compare_c.sh`,
`impl/cobc/src/lower/operators.rs`, `impl/conformance/14-control-flow/nested_match_arm_temporaries_end_ok.cb`,
`spec/conformance.md`, `impl/battery.sh`, `impl/STATUS.md`, `README.md`

## What changed

1. **The port (D-0296).** In `impl/selfhost/front/`: the static pass of Updates 136–150 (`typecheck.cb`, and
   `tier.cb`, new, the port of `tier.rs`'s access summaries): the frame-reference rules and their per-body report
   (`COBALTC_TIER_REPORT`), point checks, closure and `fn`-value facts, scoped lending, holders, `?` facts, lent
   slices, wrapped handles; the facts the code generator reads kept on `Items` (`tracking_needed`,
   `point_check_fns`, `point_checks`) and the type questions it asks (`keeps_refs_in_data`,
   `keeps_refs_in_data_by`, `frame_holdable`); D-0287's `$each_lazy` (`parser.cb`, `modres.cb`, `each.cb`).
   `front/prelude.cb` regenerated. In `impl/selfhost/back/` and `cbc/`: `cobc`'s lowering of the same Updates —
   the two-pass decided lowering and its `-v` notes, `--report-tracking`'s texts, `pathless_blocker`
   (`back/program.cb`, new), datum flights (`back/flights.cb`, new), point checks (`back/points.cb`, new), the
   text passes (`elide_record_chains`, `resolve_scopes_in`, the `G`/`R` frame markers, `cb_lf`), the objectless
   analysis and its decided forms, `cb_hold_ref`, `cb_rec_ref`, `cb_frame_push_fn`/`_rec`; `cbc --emit-c
   [--c-name NAME] [-o OUT] FILE`. `back/header.cb` and `back/std_values.cb` regenerated. `compare_c.sh` takes
   `ROOT` and `COBC` from the environment and exits non-zero on a difference; the battery's selfhost stage runs it
   on the showcases (`selfhost compilers compared`).

2. **A `match` that is an arm's whole value scopes its own arms** (`cobc`, `impl/cobc/src/lower/operators.rs`
   `lower_branch`, and `cbc`'s `gen.cb`). An arm of a statement `match` gets its own statement scope, and so does
   a block's tail `match`'s; a `match` that was itself the whole value of such an arm did not pass the property on,
   so its arms' temporaries went to the outer arm's scope, which outlives the inner arm's bindings. In a compiled
   program that keeps its paths, a view of a binder made for a call (`Some(hh) : left(b, sprintf("%s",
   String::as_view(&hh.name)))`) then outlived `hh`, and the frame's end faulted `diag.destroy-while-aliased` — a
   fault `coby` does not raise and the static pass does not predict. Found by the port: `cbparse` faulted on it
   when an interim build kept paths. Present since the statement scopes of D-0103/D-0135; a decided program runs
   no such check. New case `conf.nested-match-arm-temporaries-end`.

## Verification

All with the ported tools built by `impl/selfhost/build.sh` from the final source, 0 differences each:

- `impl/selfhost/compare_parser.sh -m dump,resolved,folded,checked,syntax,mutants` over every tracked file: 21,157
  same. The tier report (`COBALTC_TIER_REPORT`, `coby` against `cbparse`) over the corpus: 1,249 bodies same.
- `impl/selfhost/compare_c.sh` over every tracked `.cb`: 1,162 same, 603 skipped (rejected by `cobc`); over the
  conformance suite with `FLAGS=--all-checks`: 753 same; over the showcases and `stress/perf_oct9` with `-g`,
  `--no-line-directives` and `LEVEL=10`: 92 same each.
- `impl/selfhost/compare.sh -m plain,narrow,messy`: 5,304 same.
- `impl/selfhost/fixpoint.sh` (level 0, GCC): stage 1 = `cobc`'s C, stage 2 = stage 1 (129,708,500 bytes).
- `COBC=impl/target/selfhost/cbc cargo test -p cobc --release` (the compiled conformance suite and the
  showcases under GCC and Clang, with `cbc` as the compiler under test): 52 tests in 12 targets passed.
- `nested_match_arm_temporaries_end_ok.cb` under `cobc` (GCC and Clang) and `coby`; `cargo +1.77.2 check
  --workspace --tests`.
- `stress/d0281/phase3/bench.sh port` → `runs/port` against `runs/elem`: `cbc` (`--emit-c anagrams.cb`) 3.44 G → 5.21 G instructions, the
  static pass and lowering of D-0282–D-0295 it now runs; every other row (the micro programs, the showcases,
  `cbparse --check-syntax`, `cobfmt-cb`, the controls) within 0.3%. Wall clock: `cbc` 1.35 s against `cobc` 0.54 s
  on `anagrams.cb`, about 11 minutes against 79 s on its own source.
- The release suites (`stress/suites/runs/u151`): every stage exit 0.
- The mechanised fragment, checked against the current edition: `lake build` with no `sorry`, `proofcover.py
  --check` (53 rows, 0 errors), `replay_model.py impl/model/sample --check` (315 programs: 199 whole, 116 outside
  the fragment, none refused); `proofs/README.md`'s replay figures restated and its scope note on the static
  rules (D-0282's among them) added.
