# D-0254 — Portable vector types: `simd<τ, N>`

Status: ACCEPTED (2026-10-16; the owner asked for a proposal on vector intrinsics or portable vector types
"with your recommendations, and then proceed to implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §9
Depends on: D-0006, D-0086, D-0122, D-0123, D-0250, D-0251; `cobc -march=CPU`
Affects: `spec/16` §1, §3b (1.31.0), `spec/21` §0 (4.66.0), `spec/22` (2.53.0), `spec/registry/diagnostics.md`
(1.57.0), `spec/conformance.md`; `impl/src/{ast,lexer,parser,typecheck,value}.rs`, `impl/src/interp/*`
(`intrinsics_simd.rs`), `impl/cobc/src/lower/*` (`simd.rs`); `CHG-0312`

## Problem

Data-parallel loops could not ask for vector instructions: there was no vector type, checked arithmetic kept
the C compiler from vectorising on its own, and platform intrinsics would tie a program to one processor.

## Options

A platform intrinsics; B a portable built-in `simd<τ, N>` with lane-wise operators and a few intrinsics; C
`std` types with functions only; D auto-vectorisation. Selected: B (`private/D-0254-simd-proposal.md`), with
its five recommended sub-choices: a reserved `simd`; integer overflow checked as for scalars, with the
`wrapping_`/`saturating_` intrinsics as the fast path; masks as signed integer lanes; float reductions left to
right; no integer `/`.

## Decision

`spec/16` §3b: `type.simd` (shapes of 8 to 64 bytes, lanes of a non-128-bit integer or a float),
`[Layout-Simd]` (aligned to its size), `rule.agg.simd`: construction from an array literal or `[x; N]` where a
`simd` is expected, lanes as places that are never borrowed, operators lane by lane with the scalar's checks
(the lowest faulting lane's diagnostic). `spec/21` §0: `simd_load`, `simd_store`, the six comparisons giving
masks, `simd_select`, `simd_any`, `simd_all`, `simd_min`, `simd_max`, `simd_sum`, `simd_reduce_min`,
`simd_reduce_max`, and the wrapping and saturating intrinsics on vectors. One departure from the proposal:
`simd_splat` is not provided, because `[x; N]` where a `simd` is expected already says it.

## Results

`cobc` emits a GCC/Clang vector `typedef` per shape (and an unsigned twin for integer arithmetic, so a wrap is
defined in C); overflow is tested on the whole vector; loads and stores check every lane's access and copy with
`memcpy`. Two performance traps were found and fixed on the way: the typedef names first began `cb_`, which
the scope-elision pass reads as runtime calls (a frame and a statement scope per loop iteration), and a slice
parameter passed to `simd_load` was copied and registered as an object at every call; the load now reaches it
in place, and the per-lane checks of a direct slice test its flag once. `coby` runs every lane through the
scalar code. On this VM (SSE4.2, no AVX), a float dot product plus an integer sum of squares over 16 Ki
elements, 5000 times: scalar 277 ms; `simd<·, 8>` 144 ms (GCC), 129 ms (GCC, `-march=native`), 84 ms (Clang,
`-march=native`). Over 4 Mi elements both forms are bound by memory bandwidth.

## Revisit conditions

Shuffles, lane conversions, gather and scatter, fused multiply-add: when a program needs one. A tree-order
float reduction: if the fixed order costs a measured program.
