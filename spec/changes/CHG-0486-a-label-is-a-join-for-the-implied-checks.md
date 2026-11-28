# CHG-0486 — A label is a join for the implied checks

Status: implemented (2026-11-27); Update 214
Affects: `spec/conformance.md` 3.330.0 (1 row); `impl/back/passes.cb`;
`impl/tests/cases/cobc/reports_update214.case`;
`impl/conformance/08-alias-validity/implied_check_not_carried_past_label_dynamic.cb`; `impl/STATUS.md`.

## Why

`cobc` is frozen at Update 212 for fixes only while the CobaltC IR (CIR) lowering is built beside it. Moving the
passes over the generated C onto the new lowering's statement list meant reading each of them line by line against
the C the lowering writes. That reading found a sibling of the fault CHG-0485 closed in the same pass.

- **A check inside a skipped operand removed the same check after it.** `elide_implied_checks` drops a check that
  an earlier one implies, and forgets what it has seen at a join. It knew a label only as a line that ends in `:`,
  but the lowering writes its labels as `L:;` and `L: ;` (a label needs a statement after it), so no label was a
  join. The right operand of `&&` and `||` is lowered between a jump and its label (`if (!t) goto sc;` … `sc:;`),
  and a `continue` in a loop with a step jumps to a label before the step. A check among the lines such a jump
  passes over was taken as made, and the same check after the label was dropped. In
  `implied_check_not_carried_past_label_dynamic.cb` the left operand of `k > 5 && p.x > 3` is false, so the read of
  `p.x` in the right one is not made; the read `i32 b = p.x;` on the next line was dropped all the same. On the
  second pass of the loop `keep` holds `&mut p.x`, and `[Read-Conflict]` requires that read to fault; the program
  printed its line a second time and faulted only at the next borrow. A check the specification requires was not
  made, and the outcome differed. Only a program that keeps its paths has such checks at all; a decided program
  has none, and the bootstrap form, which runs no pass, faulted where it should.

## What changes

- **A label is a join for `elide_implied_checks`** (`label_of`, the reading of `L:;` and `L: ;` the loop passes
  already use): the facts seen before it are forgotten, as at a `}`. A check after `a && b`, `a || b` or a loop's
  `continue` label is kept; the checks along one straight run are elided as before.
- `impl/STATUS.md` lists one gap found by the same work (below).

## What is left

- `dead_locations_straight`, the rule `drop_dead_locations` falls back to where its flow analysis gives up, drops a
  location store when the next one comes before any line that could read the location and before any brace or
  jump, and its test for a jump (`boundary`) does not know a label as the lowering writes it, a `goto`, or a jump
  under a one-line `if` (`if (c) break;`). On the path such a jump takes, a line that reads the location before the
  next store would report the store before the dropped one. No program was found whose fault then names the wrong
  line, and making the rule stop at those lines keeps a store at the head of every such `while` loop (one perf-guide
  row 4% slower). Left as it is; the CIR lowering's passes read the statements' structure, where the flow analysis
  need not give up.
- A generic function given two closure-typed locals for one type parameter, with no type expected
  (`auto f = pick(true, dbl, inc);` with `fn pick<T>(bool c, T a, T b) : T`), is accepted by the check, which types
  both as `fn(i64) : i64`, and then fails in the C compiler: the lowering fixes `T` as the first closure's own type.
  With the binding's type written (`fn(i64) : i64 f = pick(true, dbl, inc);`) or closure literals as the arguments
  it compiles. The CIR lowering types the call once, from the checker's types.
