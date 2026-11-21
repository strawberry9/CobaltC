# CHG-0246 — Seven conformance cases from the review's recommended tests

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0208)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0208
Affects: `spec/conformance.md` 3.202.0; `impl/conformance/06-arithmetic/shift_width_edges.cb`,
`signed_zero_ok.cb`, `07-resource-authority/early_return_destructured_ok.cb`,
`16-aggregates/zero_sized_types_ok.cb`, `19-concurrency/try_join_resource_result_ok.cb`;
`impl/src/interp.rs` (`spawn`'s handle type, `unify_type_shape`); `spec/02` 1.0.70

## What changed

- The rows `conf.shift-width-edges`, `conf.signed-zero`, `conf.if-branch-type-mismatch` (§1),
  `conf.early-return-destructured` (§3), `conf.zero-sized-types` (§6), `conf.reclaim-twice-same-object`
  (§7) and `conf.try-join-resource-result` (§12), with their derivations; five case files, two fragments.
- `coby`: a `handle<R>` value carries the spawned function's declared result type (it carried `void`);
  call-site inference binds only names that are not declared structs or enums.
- `spec/02` §5: the "in use" ranges reach D-0208 and CHG-0246.

## What did not change

No rule, diagnostic or existing case. `cobc` needed no change; every new case had its expected outcome
from `cobc` and `coby` agreeing after the fix.
