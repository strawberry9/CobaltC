# CHG-0423 — No copy to look, and a place's probe is kept as a value's is

Status: implemented (2026-11-10), D-0296; step 3 of the allocation plan (the agent's proposal of 2026-11-10 on the owner's speed test; the owner: "implement the proposal in full")
Affects: `impl/selfhost/back/` (`state` — `is_bound`, `lookup`; `gen` — `foreach_alias`, `lower_block_body`,
`lower_while`, `probe_place_type`; `fresh3`, `flights`, `literals`, `bodies`); `impl/cobc/src/lower/matching.rs`
(`probe_place_type`). No rule changes. No program's outcome or C changes.

## What changes

1. **`is_bound(x, name)`** answers the 31 tests that asked `Option::is_some(&lookup(x, name))`, which copied the
   binding to discard it. `lookup` no longer copies its key: `HashMap::get` takes the key's probe (D-0312).
2. **The next statement** `foreach_alias` looks at is passed by reference, not copied.
3. **`probe_place_type`** keeps its answer in the function's probe cache, keyed by the expression (`cobc`: its
   address) as `probe_type`'s is, a failed probe not kept. It had no cache in either toolchain, so a place probed
   from `lower_borrow`, `lower_match`, `elem_field_read` and `vec_index_call` was lowered again each time.

Not done: a `lookup_ref` returning the binding where it is. With `x` and the name both reference parameters it needs
a `from x` clause, and `HashMap::get`, which may keep its probe, has none (`diag.result-source-kept`); giving std's
lookups a `from m` clause is a change to `spec/21`'s signatures, outside this plan.
Done later the same day by `CHG-0425` (D-0316): `HashMap::get` and `get_mut` are declared `from m`, and `cbc` has
`lookup_ref`.

## Measured

A prototype of item 3 (on top of `CHG-0421`): `complex_lisp.cb` −4.5%, `large_200.cb` −4.3%; `probe_place_type` 4.8%
→ 0.4% of `cbc`'s cycles on the Lisp program, 6.7% → 3.4% on `large_200`. Item 1's key copy measured nothing; the
binding's copy is the cost. On the final source `probe_place_type` is 0.7% of `cbc`'s cycles on the Lisp
program and 3.4% on `large_200.cb` (the first probe of each `elem_field_read` expression).
