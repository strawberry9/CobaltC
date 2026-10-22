# CHG-0337 — `cobc`: a fused `retain`/`position`/`binary_search_by` helper is named uniquely across a function's bodies

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, the overnight run of 2026-10-18, owner's instruction: a tool defect found by round 10)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0181 (`[Retain]`), D-0188 (dual bodies), D-0196 (`__raw` forms), CHG-0237 (frozen and checked bodies), D-0199 (fused literal calls)
Affects: `impl/cobc/src/lower/literal_calls.rs`; `spec/conformance.md` (3.247.0); `spec/02` §5 (1.0.118)

## What changed

- **The defect.** `cobc` fuses a call of `Vec::retain`, `Vec::position` or `Vec::binary_search_by` whose
  predicate is a closure literal into a C helper of its own (D-0199), named `retain_N` by the lowering's
  name counter `g.uid`. That counter restarts for each of a function's bodies — the frozen and the checked
  body of CHG-0237, the fast and slow bodies of D-0188, the `__raw` form of D-0196 — while the closure's
  type does not: its id carries the body's `closure_base`. A function with frozen shared parameters and
  such a call was emitted as two bodies whose helpers shared one name with two closure types, and the C
  compiler refused the program ("conflicting types for `retain_65`"). Found by round 10's `pubsub.cb`
  (a broker with a `ref<Vec<Channel<Delivery>>, shared>` parameter and a `Vec::retain` over a captured
  key); reduced to `conf.retain-in-frozen-body`.
- **The repair.** The helper is named by `g.closure_id()`, as `sort_by`'s helper already was: unique in
  the program whatever else it contains. One line in `literal_calls.rs`.
- **Verification.** `conf.retain-in-frozen-body` (`impl/conformance/21-standard-library-semantics/
  retain_in_frozen_body_ok.cb`): refused by the C compiler before, prints `5` under both tools now;
  `pubsub.cb` compiles and matches `coby` under GCC and Clang.

## What changed for programs

A program that was refused by the C compiler with "conflicting types for `retain_N`" (or `position_N`,
`search_by_N`) compiles. No accepted program changes meaning: the helpers' bodies are unchanged, only
their names.
