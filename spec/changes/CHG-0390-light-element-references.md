# CHG-0390 — Light element references: a decided function's element references carry no record in any program

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-29)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0295, D-0292, D-0281
Affects: `impl/cbrt/src/lib.rs`, `spec/14-control-flow.md` §6, `spec/decisions/D-0281-reference-cost-model.md`
§24, `htmlguide/generator/content_perf.py`

## What changed

In `cbrt` (`impl/cbrt/src/lib.rs`):

1. `elem_borrow` takes `light`: in a frame that forms light references (`!frame_rec()`), a borrow of an element
   whose object persists (an existing object that is not ephemeral, or one established now for an element type
   that holds references or is a `fn` value) returns `LIGHT | obj` after `check_root_access` against the element
   object's holders, with no path minted, nothing stamped and nothing in flight to retire. `cb_elem_borrow` (the
   native `Vec::index_*` bodies, a reference result in flight) and `cb_elem_borrow_here` (the `…_pre` bodies, the
   result in the caller's statement) pass true; `cb_elem_access_slow` (it checks through the path and retires it)
   and `cb_elem_borrow_datum` (the path travels as a datum's reference, stored by `cb_recv_datum`, which takes
   no light token) pass false. `COBALTC_RECORD=full` keeps every element borrow recorded, as before.
2. The in-flight queue (`inflight`) is read and written through `ts_raw()`: pushing or popping a flight no longer
   makes the statement's pending scopes real (`ts()`), so a statement whose only runtime event is a reference's
   flight is popped by `pend.pop()` alone.

In the specification: `spec/14` §6 "The cost contract", the tier-D paragraph: element references are light in a
decided function (recorded for a plain element with no object, and everywhere under the oracle); the provisional
lines of D-0281 §14 are replaced by the promises of this edition (D-0295 item 4). D-0281 §24: phase 3 closed, the
single port next. The guide's performance chapter says the same.

## Verification

- `stress/d0281/phase3/gate3.sh` over the 82 dynamic-tracking conformance cases (`u150/gdyn.list`), GCC and Clang:
  78 SAME, 4 DECIDED (no light form to compare), 0 DIFF.
- `gate3.sh` over `kept146.list` (996 programs), GCC; repeated runs for the thread races; `cobmodel compare` over
  the modelled DIFFs.
- `stress/d0281/phase3/bench.sh elem` → `runs/elem`, `BENCH.md`.
- `cargo +1.77.2 check --workspace --tests`; `cargo +1.77.2 build --release -p cbrt --target x86_64-pc-windows-gnu`.
- The release suites (`stress/suites/runs/u150`).
