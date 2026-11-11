# D-0148 — `min`, `max`, `abs`, `pow` belong to `std::math`

Status: ACCEPTED (2026-10-04, the owner: "move min, max, abs, pow to std::math", after asking why the guide placed them in `std::core`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0091 (the four functions), D-0136 (`std`'s submodules, `std::math`)
Affects: `spec/21` §0 (4.13.0), the guide §21 and its intro table, `impl/std/core.cb`, `impl/std/math.cb`; `CHG-0176`

## Problem

D-0136 divided `std` into submodules and created `std::math` for the
fourteen floating-point functions that had been language intrinsics
(`sqrt` … `powf`): they take only `f32` or `f64`, which no bound can
say, so both tools realize them natively. The four generic numeric
helpers of D-0091, `min`, `max`, `abs` and `pow`, written in CobaltC
over D-0090's bounds, stayed where they already were, in `std::core`
beside `Option`, `swap` and `assert`.

A reader takes a submodule's name as its subject. `pow` in `core` while
`powf` is in `math` says that the split follows how a function is
realized, which is an implementation detail no module name should
leak; nothing technical keeps the four in `core`, since a submodule may
mix CobaltC-written and natively realized items (`std::random` does).

## Candidate mechanisms

1. **Leave them.** Costs a question from every reader of the guide's
   module table. Rejected.
2. **Move the four to `std::math`.** The module is then "numbers": the
   generic helpers and the floating-point functions. Selected.
3. **Move the floating-point functions to `std::core` instead.** Would
   dissolve `std::math` and put fourteen natively realized functions
   beside `Option`. Rejected.

## Selected design

`min`, `max`, `abs` and `pow` are items of `std::math`, their code and
meaning unchanged (`spec/21` §0's `std::math` table and submodule row;
the `std::core` row loses them). `std` re-exports every submodule, so
`import std;`, `min(a, b)`, `pow(x, 3)` and `std::min` mean what they
meant. The guide's module table and intro table name them under
`std::math`.

## Compatibility impact

Additive for every program that names them through `std` or
unqualified. Breaking (source) for a program that wrote the full path
`std::core::min` (or `max`, `abs`, `pow`): it now writes
`std::math::min`. No checked-in program, case, showcase or guide example
does.

## Revisit conditions

- Further numeric helpers (`clamp`, `sign`, `gcd`), if programs ask;
  they would join `std::math`.
