# CHG-0314 — `round` at run time on Windows

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0091, D-0255
Affects: `spec/conformance.md` (one row); `impl/src/interp/intrinsics.rs`, `impl/cbrt/include/cbrt.h`,
`impl/cobc/src/lower/intrinsics.rs`; `impl/conformance/run.py`

## What changed

`rule.arith.float-fns` is unchanged: `round` halves away from zero. Both tools now compute it from `trunc`
(the dropped fraction is exact, so the comparison with one half is too) instead of taking the C library's
`round`. On Windows, MinGW-w64's `round` over msvcrt gives 1 for 0.49999999999999994, the largest double
below one half; `coby` reached it through Rust's `f64::round` and `f32::round`, and `cobc` through C's
`round`. `numeric_functions_ok.cb` caught it only under `coby`: its arguments are literals, which the C
compiler folded at compile time. The new row `conf.round-run-time` rounds values known only at run time.

`conformance/run.py` also skips, off Linux, a case that `coby` refuses because it calls C functions only on
x86-64 Linux, as the Rust runners have done since 2026-09-25; on Linux that refusal is still a failure.

## What changed for programs

None on Linux. On Windows, `round` gives the rule's result for every value.

## Found by

The owner's first `coby` conformance run on Windows 7 (Update 112).
