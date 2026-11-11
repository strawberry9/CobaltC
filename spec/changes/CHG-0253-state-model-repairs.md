# CHG-0253 — `[Guard-Drop]` keeps a poison; `state.sync` and `state.threads` complete; the UTF-8 invariant's scope; `[Write]` counts every copy

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0215)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0215
Affects: `spec/19` 1.13.0, `spec/04` 1.5.0, `spec/03` 1.6.0, `spec/05` 1.6.0, `spec/conformance.md` 3.207.0;
`impl/conformance/08-alias-validity/reference_held_twice_in_one_object_ok.cb`; `impl/tests/conformance.rs` (covers line)

## What changed

- `spec/19` §2 `[Guard-Drop]`: `sync(o) := unlocked` only if `Σ.sync(o) = locked-by(ℓ')` for some `ℓ'`; a paragraph after the
  lock rules says that a guard destroyed by a contained fault's unwind leaves the mutex poisoned.
- `spec/04` `state.sync`: `SyncState ::= unlocked | locked-by(Thread) | poisoned(Thread)`; `state.threads`:
  `value: Result | failed(Diagnostic) | taken | None`; `state.access-paths`: `held-by`'s meaning stated as the
  set of live objects whose contents hold the path, every copy counting.
- `spec/03` `inv.string.utf8-validity`: `∧ init-state(o,Σ) = valid`. The others were read for the same
  question: `inv.identity` and `inv.origin-stability` hold from establishment, which is right for an
  uninitialized object; `inv.spatial-validity`, `inv.temporal-validity`, `inv.initialization-validity`,
  `inv.resource-authority`, `inv.alias-validity`, `inv.trust-transition`, `inv.concurrency-validity` and
  `inv.arith.range-validity` quantify over accesses, operations or values, not over live objects' contents;
  `inv.str.utf8-validity` is over `str` values.
- `spec/05` `[Write]`: `S'`, `kept`, `gone = old \ kept`; holders released and references invalidated over
  `gone`; the prose after the rule says so.
- `spec/conformance.md` §4: `conf.reference-held-twice-in-one-object`; §12/§15: the three poisoned-mutex
  rows cite the corrected `[Guard-Drop]`.

## What did not change

No accepted program, outcome or diagnostic; both tools already behaved so (D-0215 "Results").
