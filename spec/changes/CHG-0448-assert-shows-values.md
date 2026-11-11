# CHG-0448 — A failed assertion shows the values it compared (D-0322)

Status: implemented (2026-11-14), D-0322, D-0296; Update 188
Affects: `impl/src/modres.rs` (`assert`'s rewrite), `impl/src/typecheck.rs` (`Rewrite::AssertValues`,
`ASSERT_CMPS`, `assert_comparison`), `impl/src/views.rs` (`assert_values`); their twins
`impl/selfhost/front/modres.cb` (`assert_terms`), `impl/selfhost/front/typecheck.cb` (`AssertValuesRewrite`,
`Facts::assert_cmps`, `assert_comparison`, `assert_values`); `spec/18` §1a (1.11.0);
`spec/registry/diagnostics.md` (1.68.0); `spec/conformance.md` (3.311.0)

## What changes

- `modres` writes `assert(p && q, …)` as one `if (!(p)) { $assert_fail(…); }` per term, in order.
- The checker, at an `if` of that shape whose condition is a comparison, marks the comparison
  (`ASSERT_CMPS`); `check_binary`, once both operands are typed, records `Rewrite::AssertValues { l, r }`, the
  operands' types (`Scalar`), when both are integers, floats or `bool` and the body is not generic (`subst`
  empty).
- The rewrite (`views::assert_values`, `assert_values` in the selfhost's `rw_expr`) makes the `if` a block:
  `τl $assert_l = l; τr $assert_r = r; if ($assert_l ⊙ $assert_r) { } else { $assert_fail($fmt("assertion
  failed: %v ⊙ %v", $assert_l, $assert_r)); }`, or the program's format followed by ` (%v ⊙ %v)`. Each binding
  is declared at its operand's type, so a literal keeps the type the comparison gave it (bound without a type,
  `1000000000980` compared with a `u64` was an `i32` in the message, and six conformance cases were rejected).
- `conf.contained-fault-message-kept` prints a contained fault's text, which now ends `(7 < 0)`. The new shape
  is not an assertion's, so the next round of checking records nothing more. The interpreter and the two
  compilers see ordinary code; cbc and cobc write the same C.

## Checked

Rows `conf.assert-operands-once`, `conf.assert-and-terms-in-order`, `conf.assert-shows-values`; the decided
sweep, perfguide and bench on the final source (`stress/ease188/LOG.md`).
