# CHG-0044 — Radix literals, compound assignment, `for`, `const`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-directed; details delegated)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0035, D-0036
Affects: spec/22 §1–§3, rule.expr.compound-assign (new), rule.control.for (new), rule.module.const (new), diag.const-not-constant (new), the cases listed below

## Problem / motivation

D-0035 and D-0036: four things C programs rely on that CobaltC lacked.

## What changed

- **`spec/22` 2.13.0:** `int-literal` in bases 16, 8, 2; keywords
  `for`, `const`; `assign-op`; `for-expr`; `const-decl`.
- **`spec/13` 1.6.0:** `rule.expr.compound-assign`.
- **`spec/14` 1.5.0:** `rule.control.for`.
- **`spec/17` 2.2.0:** §1a `rule.module.const`.
- **`spec/registry/diagnostics.md` 1.14.0:** `diag.const-not-constant`.
- **`spec/conformance.md` 3.30.0:** the cases below.
- **`spec/02-schema.md` 1.0.20:** §5's "in use" ranges.
- **Implementations** (shared front end unless named):
  - lexer: `0x`/`0o`/`0b` literals, compound-assignment tokens, `for`,
    `const`;
  - parser: `p op= e` as `p = p op e`, or through a hidden exclusive
    reference when `p` contains a call; `for` as a block holding the
    initializer and a `while` with a step (`ExprKind::While`'s third
    field); `const` as a function item marked `is_const`;
  - `modres`: a use of a constant becomes a call of it; `consts.rs`
    rejects cycles and folds integer, float, `bool` and `str`
    constants to literals, reporting checked failures at the constant;
  - typecheck: `[Const-Not-Constant]`; a `for` step in the flow
    analysis; an assignment whose left side is not a place is rejected
    statically (it was a run-time type mismatch);
  - `coby`: runs the step after the body and after `continue`;
  - `cobc`: a `for`'s `continue` jumps to a label before its step.

## Compatibility classification

Extension, except that assigning to something that is not a place
(`5 = 6;`) is now rejected statically instead of faulting when run.

## Conformance changes

**Added:**
- `conf.radix-literals`, `conf.compound-assign-ops`,
  `conf.compound-assign-overflow`, `conf.compound-assign-call-target`
- `conf.for-sum-continue`, `conf.for-empty-parts`,
  `conf.for-variable-scoped`
- `conf.const-folded`, `conf.const-struct`, `conf.const-exported`,
  `conf.const-shadowed-by-local`, `conf.const-cycle-rejected`,
  `conf.const-not-constant-rejected`, `conf.const-resource-rejected`,
  `conf.const-overflow-static`, `conf.const-assign-rejected`

File cases: `22-surface-syntax/radix_literal_no_digits_parse_error.cb`,
`radix_literal_bad_digit_parse_error.cb`.

## Prior-art status

See D-0035, D-0036.

## Revisit conditions

See D-0035, D-0036.
