# CHG-0370 — `drop(reclaim<T>(p))` destroys the place, in `cobc` too

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-24)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.resauth.destroy` (`spec/07` §3), `rule.trust.rawptr` (`[Reclaim]`), D-0023
Affects: `spec/conformance.md` (one row, 3.270.0); `impl/cobc/src/lower/intrinsics.rs` (`drop`),
`impl/cobc/src/lower/native_vec.rs` (`Vec::drop`'s native bodies); `impl/selfhost/back/gen.cb`,
`impl/selfhost/back/native_vec.cb` (the same in `cbc`); `impl/cbrt/src/lib.rs` (`cb_destroy`)

## What changed

No rule changed. `drop(reclaim<T>(p))` of a resource `T` is `[Destroy]` of the place `[Reclaim]` makes
(`spec/07` §3: `destroy(a)` is reachable from source as `drop(place)`), and a live path into that object is
`[Destroy-Not-Solitary]`'s `diag.destroy-while-aliased`, as `coby` reports it. `cobc` lowered the argument as a
value, a move out of the reclaimed object into a temporary, and so reported `diag.move-while-aliased` for the
same program: `drop(v)` of a `Vec` whose element a live reference pointed into (`Vec::drop`'s body is
`drop(reclaim<T>(…))` per element), and in the native `Vec<String>` drop, which took the same steps. Both
compilers now destroy the place through its root; `cb_destroy` keeps the caller's location when a `std` body
gives none. The new row `conf.destroy-vec-while-element-referenced` covers it.

## What changed for programs

Such a program faults with `diag.destroy-while-aliased` under `cobc`, as under `coby`, at the same place.

## Found by

P4 (`cbc`'s speed): checking the native `Vec::drop` for elements holding a resource against `coby`
(`stress/p4/micro/vd.cb`), 2026-10-24.
