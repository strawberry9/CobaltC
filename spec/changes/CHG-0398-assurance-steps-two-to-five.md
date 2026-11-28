# CHG-0398 — The certificate read again, the static rules proved for a core, the runtime's runs replayed, the decided lowering validated (D-0300; D-0299 steps 2–5)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0300, D-0299, D-0270, D-0292
Affects: `spec/ASSURANCE.md` (1.1.0); `impl/tools/factcheck.py`, `impl/tools/validate_c.py`,
`impl/tools/replay_runtime.py` (new); `impl/cbrt/src/replay.rs` (new), `impl/cbrt/src/lib.rs`; `proofs/Cobalt/Statics.lean`
(new), `proofs/Cobalt/Step.lean`, `Preservation.lean`, `Elision.lean`, `Exec.lean`, `proofs/Replay.lean`,
`proofs/lakefile.lean`, `proofs/COVERAGE.md`, `proofs/README.md`; `impl/battery.sh`, `impl/tools/suites.sh`;
`impl/STATUS.md`, `impl/CHANGELOG.md`

## What changed

Evidence only: no rule, no lowering, no runtime behaviour changes unless `COBALTC_REPLAY_TRACE` is set.

1. **`impl/tools/factcheck.py`** — D-0300 step 2, as its docstring states.
2. **`proofs/Cobalt/Statics.lean`** and the step `dead` — step 3. `proofcover`: `[Object-End]` gains `dead`.
3. **`COBALTC_REPLAY_TRACE`** (`impl/cbrt/src/replay.rs`: the trace; `lib.rs`: one hook per entry point the
   trace follows, one relaxed load when off) and **`impl/tools/replay_runtime.py`** — step 4. `proofs/Replay.lean`
   reads `flt DIAG` (`agreed`, `MISSED`) and takes `inv` as the `dead` step where its premises hold.
4. **`impl/tools/validate_c.py`** — step 5.
5. **Battery stage 1** and **`suites.sh`**: `factcheck.py` over the model sample, the showcases and
   `impl/conformance/14-control-flow`; `validate_c.py` over the sample and the showcases; `replay_runtime.py`
   over the sample under both C compilers. All with `--check`.

## Verification

On the final source (2026-11-01), each with `--check`:

| Check | Result |
|---|---|
| `factcheck.py` over the model sample, the showcases and `impl/conformance/14-control-flow` | 903 bodies read; 372 checked, all agreeing with the certificate (363 decided by both, none decided by the certificate and left by this reading); 531 unchecked, chiefly `match` (147), values that keep references in data (`Hold`, a parameter `h`, `Vec`, `Channel`), closures and threads |
| `factcheck.py` on the earlier `cobc`'s certificates of the two holes | `factcheck.decided-but-left` at the write after the loop, both |
| `validate_c.py` over the model sample and the showcases | 411 programs: 206 decided, 118 undecided, 86 rejected, 1 library; no finding |
| `validate_c.py` over `impl/conformance`, the showcases and the sample (before the repairs) | 1,568 programs: 825 decided, 254 undecided; one finding, the validator's own (a pathless program lowered by the first lowering has no `CB_DECIDED`; the datum flights it keeps carry token 0), repaired |
| `validate_c.py` on forged inputs | a path entry named in decided C, a `left` body made light, a verdict rewritten to `decided`: each refused |
| `replay_runtime.py` over the model sample, GCC and Clang | 315 programs: 52 replay whole, 2 faulted as the rules say, 180 stop outside the fragment (threads 78, raw storage 36, an uncompared fault 21, mutexes 10, …), 81 rejected statically; none refused, none missed, the two compilers' traces the same; 1,598 actions admitted in the runs replayed to their end |
| `replay_model.py` over the model sample | 199 whole, 116 outside, none refused; 6,883 actions admitted, 64 gaps (219 before `dead`) |
| `certify_model.py` over the model sample | 234 certified, 81 rejected by both, none not certified |
| `lake build` | no `sorry`; `Statics.lean` and the `dead` cases checked |
| `proofcover.py`, `assurance.py`, `speccheck.py`, `runtime_contract.py` | clean (`[Object-End]` gains `dead`) |
| `cargo test --release -p cbrt`; `-p cobc` `compiled_spec_rows`, `compiled_suite`, `driver`, `tracking` | pass (the Windows runtime rebuilt with Rust 1.77.2 for the Wine test) |
| `cargo check --workspace --tests --target x86_64-pc-windows-msvc`; `cargo +1.77.2 check --workspace` | pass |
