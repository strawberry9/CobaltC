# D-0322 — A failed assertion shows the values it compared

Status: ACCEPTED (2026-11-14, the owner: "proceed use your recommendations", on the plan
`private/EASE-PLAN-2026-11-14.md`, item 5 and its decision Q3, wording `assertion failed: 3 == 6`, the program's
message first when it gives one); implemented by `CHG-0448`
Kind: Decision (`spec/02-schema.md`); `rule.fail.assert` (`spec/18` §1a)
Governed by: `CobaltC_Master_Instructions.md` §9, §19
Depends on: D-0065 (`assert`), D-0162 (the checker's rewrites), D-0296 (both toolchains in step)
Affects: `spec/18` §1a (1.11.0); `spec/registry/diagnostics.md` `diag.assert-failed`; `spec/conformance.md`
(3.311.0); `impl/src/modres.rs`, `impl/src/typecheck.rs`, `impl/src/views.rs`; `impl/selfhost/front/modres.cb`,
`impl/selfhost/front/typecheck.cb`

## 1. Problem

`assert(a + 1 == b * 2)` failing said `assertion failed` and the line. To learn why, the program had to be
changed to print the values and run again. Rust's `assert_eq!` and most test frameworks show both sides.

## 2. Options

1. **A new form, `assert_eq(a, b)`.** One more name, and every existing assertion stays silent. Rejected.
2. **Show the operands of any comparison, by evaluating them again in the failing branch.** An operand with an
   effect (`Vec::pop(&mut v) == Some(3)`) would run twice. Rejected.
3. **Bind each operand once, before the comparison, when its type is one `%v` writes and binding it cannot
   change what the program does: an integer, a float or a `bool`, outside a generic body.** Any other condition
   reports what it reported. **Chosen**, with `&&` split into its terms, each its own assertion.

## 3. Decision

`spec/18` §1a: a condition `p && q` is `assert(p, …)` then `assert(q, …)` (q evaluated only when p holds, as
`&&` evaluates it). A comparison `l ⊙ r` of two such scalars is evaluated as `{ τl a = l; τr b = r; if (!(a ⊙ b))
{ assert_fail(format("assertion failed: %v ⊙ %v", a, b)); } }`, or with the program's message
`format(f ++ " (%v ⊙ %v)", a1, …, an, a, b)`, `τl` and `τr` the operands' types (a literal's from the other
operand, as `⊙` gives it). Each operand is evaluated once, left then right, as before; the identifier, phase and location of the fault do not change.

## 4. Consequences

- The checker records the rewrite (`Rewrite::AssertValues`) on an assertion's comparison when both operand types
  are scalars and the body is not generic (an instantiation could disagree); the program is then rewritten and
  checked again, as D-0162's literals are, so the interpreter and both compilers see ordinary code.
- A passing assertion costs what it cost: the operands are locals the C compiler sees through, and the
  formatting is on the failing path, which ends in the fault. The length assertion of CHG-0441
  (`assert(slice_len(s) >= n)`) still tells the C compiler the length.
