# CHG-0284 — `[Swap-Places]` and `[Lend-Places]` write by `[Write]`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0235)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0235
Affects: `spec/21` §3b (4.62.0), `spec/08` (1.6.0), `spec/OVERVIEW.md`; `impl/std/core.cb` (`swap`'s
documentation); `coby` (`interp/intrinsics_runtime.rs`); `cobc` (`lower/intrinsics.rs`); `spec/conformance.md`

## What changed

- `spec/21` §3b: `[Swap-Places]` with `[Write]`'s premises for each of two distinct places,
  `[Swap-Places-Stale]`, `[Swap-Places-Conflict]`; `[Lend-Places]` the same for its place.
- `spec/08`: `[Thread-Affine]`'s list of rules; §3 names the two intrinsics as reducing to `[Write]`.
- `coby`: `swap_places` checks `[Thread-Affine]` and the write conflict of each place, the same place twice
  excepted; `lend_places` the same for its place. `cobc`: `cb_write` on each token inside the distinct-places
  branch, and before `cb_lend`.
- Cases: `conf.swap-passed-ref-beside-shared-reborrow`, `conf.replace-passed-ref-beside-shared-reborrow`,
  `conf.swap-through-another-threads-ref`, `conf.swap-forms`.

## What changed for programs

A soundness repair (`spec/STABILITY.md` §2). `swap`, `replace` and `overwrite` through a reference with a live
conflicting path, or one that is another running thread's, fault `diag.aliasing-conflict` under both
implementations, where before the exchange happened. Nothing else.
