# CHG-0247 — `[Standing-Local]`: the check-elision property as a lemma, and two cases for its new hypothesis

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0209)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0188, D-0205, D-0209
Affects: `spec/08` 1.3.0, `spec/conformance.md` 3.203.0, `spec/02` 1.0.71; `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`, `impl/cobc/src/main.rs`, `impl/cobc/src/cache.rs`,
`impl/battery.sh`; `impl/conformance/19-concurrency/direct_param_element_grown_by_other_thread_rejected.cb`,
`frozen_param_element_grown_by_other_thread_rejected.cb`

## What changed

- `spec/08` §4: the D-0188 paragraph is the lemma `[Standing-Local]`, hypotheses H1–H6 and a case per
  transition kind of another thread; H5 (fresh storage, or the thread alone) excludes `[Release]`.
- `spec/conformance.md`: the two rows.
- `cbrt`: `cb_live_threads` exported; `cb_direct_ok`; `cb_frozen_ok` and `cb_paths_disjoint` refuse a
  reclaimed object unless the thread is alone.
- `cobc`: `lower_fn_direct` (the checked twin and its dispatcher); a frozen function's checked twin fully
  checked, its dispatcher testing direct parameters; `--all-checks` / `COBALTC_ALL_CHECKS`, in the cache key.
- `cobc`: `direct_local_call` — the direct twin called itself where every direct argument is a borrow rooted
  at a local the caller owns; a program function's dispatcher inlined at every call (D-0209 "Results").
  Two more rows, `conf.direct-call-owned-locals` and `conf.direct-call-lent-vec-element`
  (`spec/conformance.md` 3.206.0).
- `cbrt`: `cb_frozen_ok` builds the normalised path only when an exclusive path is there to compare with.
- `impl/battery.sh` stage 3: the file suite again under `COBALTC_ALL_CHECKS=1` with GCC.

## What did not change

No rule, diagnostic or accepted program. `coby` reported the stale reference before and after. What D-0188
permits is unchanged; the lemma says under which hypotheses, and the implementation now tests the one it had
assumed.
