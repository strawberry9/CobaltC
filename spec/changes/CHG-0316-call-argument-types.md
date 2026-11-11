# CHG-0316 — Argument types of closure and `fn`-value calls; `*` of a `simd`; built-in names as values

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0006, D-0254
Affects: `spec/conformance.md` (four rows); `impl/src/typecheck.rs`

## What changed

No rule changed. `[T-Call]` (`spec/12`) types each argument at its parameter's type, for a call of a closure or
a `fn` value as for a named function, but the static pass compared the argument's type with the parameter's
only for named functions. Through a closure or a `fn` value it checked the number of arguments and gave each
argument its parameter's type as the expected type, nothing more, so a call took an `i32` for an `i64` (an
implicit conversion, against D-0006), a `bool` or a `String` for an integer, or an `array<i32, 4>` or
`simd<f32, 4>` for an `array<i32, 2>` or `simd<f32, 2>`. The tools then disagreed: `coby` printed `true`
where `cobc` printed `1`, or `cobc` emitted C that did not compile. Now each argument is compared as for a
named function (a bare literal takes the parameter's type; a reference may weaken to a shared one), with
the message "argument N is `τ`, but the function value's parameter is `σ`".

`[T-Deref]`'s premise, that the operand is a reference, a guard or a raw pointer, was tested statically
against a list of the types that are not; `simd` (D-0254), handles and mutexes were missing from it, so
`*v` of a `simd` reached `coby`'s evaluator (a dynamic fault) and `cobc`'s code generator (an internal
error). They are rejected statically now.

A built-in operation's name (`sizeof`, `widen`, `spawn`, `simd_sum`, …) where a value is wanted, as in
`v[sizeof]`, passed the static pass (`[Resolve-Unbound]` skipped built-in names): `coby` reported an unbound
name when the program ran and `cobc` stopped with an internal error. It is a static `diag.unbound-name` now,
saying the operation is only called.

New rows: `conf.closure-argument-type`, `conf.fn-value-argument-type`, `conf.deref-simd`,
`conf.builtin-name-as-value`.

## What changed for programs

A program that called a closure or a `fn` value with an argument of another type than the parameter's is
now rejected; the repository's programs (both conformance suites, the showcases, the examples, the
benchmarks, every unit of `std`) had none.

## Found by

Token-level mutation of the round-10 programs (`stress/round10/mutate.py`), 2026-10-17: two mutants of
`simd_structs.cb` that `coby` accepted and `cobc` could not compile, and a third that used `sizeof` as an
index.
