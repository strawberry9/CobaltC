# CHG-0425 — `HashMap::get` names its source; `cbc` reads a binding where it lies

Status: implemented (2026-11-10), D-0316; the allocation plan's last item (`CHG-0423`'s "Not done")
Affects: `impl/std/collections.cb` (`HashMap::get`, `get_mut`: `from m`), `spec/21` 4.82.0, `impl/src/typecheck.rs` and
`impl/selfhost/front/typecheck.cb` (`can_receive`), `impl/selfhost/back/state.cb` (`lookup_ref`), `gen`, `bodies`,
`flights`, `vec3` (the callers that only read), `impl/selfhost/front/prelude.cb` (regenerated), `spec/conformance.md`
3.304.0 (two rows). No program's outcome, output or fault changes; no C changes.

## What changes

1. **`HashMap::get` and `HashMap::get_mut` are declared `from m`** (D-0316 §2.1). `entry` is not: the checker refuses
   the clause there, since `entry` stores `fresh`.
2. **`can_receive`, in both checkers**, takes a type it does not open (`probe<K>`, a `Vec<T>` by value) to receive a
   reference when one of its arguments can, not when one may hold a reference (D-0316 §2.2, `spec/10`'s text). Before,
   a lookup's `probe<K>` counted as receiving, so in a function with a `from` clause every argument of a `std` lookup
   was kept (`conf.result-source-key-probed-by-lookup` was refused). An exclusive container still receives
   (`conf.result-source-kept-pushed` is unchanged), and so does a key stored through `HashMap::insert`.
3. **`lookup_ref(x, name) : Option<ref<Binding, shared>> from x`** in `cbc`'s `state.cb`: the binding in its scope,
   not a copy. The 18 callers of `lookup` that only read the binding's fields use it; the 30 that keep the binding, or
   change the scope tables while they hold it, keep `lookup`.

## Measured

Median of five runs of `--emit-c`, against Update 176's `cbc` in the same run (the machine slower than when Update 176
was measured, so only this run's columns compare): the Lisp interpreter 3.30 s → 3.20 s (−3.1%), `large_200.cb`
5.40 s → 5.29 s (−2.0%), the speed test's ten programs 15.34 s → 15.09 s (−1.6%; 1.12 → 1.10 times `cobc`'s 13.71 s).
`cbc`'s C identical to `cobc`'s on the showcases and the speed test (100 files, 5 module files skipped); the fixed
point holds (stage 2 == stage 1, 144,927,332 bytes). The two new cases pass under `coby`, `cobc` and `cbc` with gcc
and clang, and Update 176's `coby` refuses both (`diag.result-source-kept`); every `result_source_*` refusal is
unchanged in `coby` and `cbc`.
