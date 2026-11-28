# CHG-0261 — `Vec::index_exclusive` and `Box::get_mut` take their container exclusively; `cobc`'s map bodies check their borrows

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-12; D-0222)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0222, D-0219, D-0218
Affects: `spec/21`, `spec/conformance.md`; `impl/std/collections.cb`, `impl/std/memory.cb`; `cobc`
(`impl/cobc/src/lower.rs`); `impl/model`; five new cases under `impl/conformance/`

## What changed

- `spec/21` and `impl/std`: `Vec::index_exclusive` gains `_ = &mut (*v)[i .. i + 1];` after its bounds check;
  `Box::get_mut` gains `_ = &mut *b;`.
- `cobc`: the native bodies of `Vec::index_exclusive` and `Box::get_mut` make those checks, as does the inline
  form of `Vec::index_exclusive` for a plain element type (`native_elem_access`); the native
  `HashMap::get_mut`, `entry` and `insert` check each exclusive borrow of the map's vectors through `m` where
  the text makes it.
- `impl/model`: an explicit `Vec::index_exclusive(v, k)` call.
- `spec/conformance.md`: `conf.vec-index-exclusive-while-container-shared`, `conf.box-get-mut-while-box-shared`,
  `conf.hashmap-get-mut-while-map-shared`, `conf.hashmap-get-mut-local-while-shared-path-kept`, `conf.hashmap-entry-while-map-shared`,
  `conf.hashmap-insert-while-map-shared`.

## What changed for programs

A soundness repair (`spec/STABILITY.md` §2). A call of `Vec::index_exclusive` or `Box::get_mut` whose reference
has a live shared reborrow faults `diag.aliasing-conflict` under both implementations, where before the
element changed under the shared reference; under `cobc`, `HashMap::get_mut`, `entry` and `insert` fault in the
same case, as they always did under `coby`. Nothing else: a call whose argument is a borrow formed for it made
the same check already.
