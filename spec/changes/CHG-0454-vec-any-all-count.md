# CHG-0454 — `Vec::any`, `Vec::all`, `Vec::count` (D-0325)

Status: implemented (2026-11-14), D-0325; Update 189
Affects: `impl/std/collections.cb`; `spec/21-standard-library-semantics.md` (the row of D-0112's helpers);
`impl/conformance/21-standard-library-semantics/vec_any_all_count_ok.cb`, `vec_count_held_element_rejected.cb`
(new); `spec/conformance.md` (3.312.0); `impl/selfhost/front/prelude.cb` (regenerated)

## What changes

The three functions of D-0325, written as `Vec::position` is (a loop over `Vec::index_shared`), each with its
`///` documentation and one documented `unittest`. Rows `conf.vec-any`, `conf.vec-all`, `conf.vec-count`
(how many calls each makes, an empty `Vec`, a predicate capturing a value, `String` elements) and
`conf.vec-count-held-element` (the element borrowed for the predicate while an exclusive reference to it
lives).
