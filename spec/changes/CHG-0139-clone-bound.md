# CHG-0139 — The `clone` bound and `clone(&x)`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0116)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0116
Affects: `spec/12` (1.18.0), `spec/16` (1.18.0), `spec/21` (3.45.0), `spec/22` (2.40.0), `spec/conformance.md` (3.112.0), the guide, `impl/src/derive.rs` (new), `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`, `impl/src/prelude.rs`

## What changed

- **`spec/12`:** `clone` in the bound table; `clone-holds`; `[T-Clone]`,
  `[Clone-Unsatisfied]`; `clone` implied by every other bound.
- **`spec/16` §4a:** `rule.agg.derived-clone`, the function the front
  end derives for a struct or enum.
- **`spec/21`:** `Vec::clone`, `from_slice`, `filled`, `extend_from`
  at `T: clone`; `Box::clone`, `HashMap::clone`, `HashSet::clone`.
- **`spec/22`:** `bound` takes `clone`; §5's restriction 5.
- **Front end:** `derive.rs` adds the derived functions before name
  resolution; the checker's `clone_problem` decides `clone-holds` and
  names the failing field; the `clone` intrinsic is typed; the
  `HashMap` key check finds `K` by name (the `_str` lookups have none).
- **Tools:** `coby` and `cobc` reduce `clone(&x)` to a read or to
  `N::clone`.
- **Rows:** `conf.clone-plain`, `conf.clone-derived-struct`,
  `conf.clone-derived-enum`, `conf.clone-generic-struct`,
  `conf.clone-std-containers`, `conf.clone-declared-called`,
  `conf.clone-bound-generic-fn`, `conf.clone-resource-without-clone-rejected`,
  `conf.clone-field-not-clone-rejected`, `conf.clone-unbounded-parameter-rejected`,
  `conf.clone-mutex-rejected`; the four `*-resource-rejected` rows now use
  a `resource` type without a `clone`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; programs rejected only because a resource element type could
not be copied are now accepted where the type is `clone`.
