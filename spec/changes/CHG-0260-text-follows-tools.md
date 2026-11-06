# CHG-0260 — Closure calls borrow shared; the deferred last-use end; `Vec` elements are not flow places

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-12; D-0221)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0221, D-0219
Affects: `spec/15` 1.14.0, `spec/14` 1.26.0, `spec/16` 1.25.0, `spec/conformance.md`; three new cases under
`impl/conformance/`; the guide's §15 and its data-parallelism section; `impl/model`

## What changed

- `spec/15` `[Closure-Call]`: `borrow(a_c, shared)`; §6's prose says what that permits and what it does not.
- `spec/14` `[Ref-Binding-Last-Use]`: the end waits to `s_j`, the first statement after which no binding that
  shares a statement with `x` is mentioned, and happens there unless the flow facts root another binding's
  reference in `x`; an example and the history. The flow table's row names `s_j`.
- `spec/14` §6: a literal index is a place only in an array; a borrow of a `Vec` element forms a fact on the
  whole container, and element accesses are decided at run time.
- `spec/16` `[Index-Vec]`: the element check is dynamic; statically an element borrow is a container borrow.
- `spec/conformance.md`: `conf.closure-called-while-shared-borrow-lives`, `conf.ref-holding-binding-end-deferred`,
  `conf.vec-element-borrows-compared-at-run`; two derivations corrected.
- `impl/model`: the deferred end is the text's; `--literal-last-use` removed.

## What changed for programs

Nothing: a clarification. Every program either implementation accepted, it still accepts, with the same
outcome; the three new cases passed under both before the text changed.
