# CHG-0483 — A certain fault is refused: the left items of D-0339

Status: implemented (2026-11-25); Update 211
Affects: `spec/06` 1.25.0, `spec/10` 1.10.0, `spec/12` 1.35.0, `spec/18` 1.14.0, `spec/19` 1.25.0,
`spec/registry/diagnostics.md` 1.77.0, `spec/conformance.md` 3.327.0, `spec/examples.md` 3.24.0 (both versions
recorded in Update 212); `impl/front` (`typecheck.cb`,
`parser.cb`, `prelude.cb` generated), `impl/back/gen.cb`, `impl/cobc/frontend.cb`, `impl/cbrt`
(`rt_threads.c`, `diag_table.h` generated); `impl/conformance` (the cases that failed on purpose with
`1 / (1 - 1)`, now `assert(false)`; nine cases whose fault is now refused at the check; `README.md`);
`impl/tests/cases/cobc/reports_update211.case`; `htmlguide/` (`content_b.py`, `content_c.py`), `book/src`
(`appendix-b-diagnostics.md`); D-0340.

## Why

The owner went through D-0339 §3's left items one by one and ruled on each (D-0340), first making a language rule
of the principle behind them: a certain result from a static check is not left as a run-time check.

## What changes

| Item | Was | Now |
|---|---|---|
| the rule | a certain fault could be left to the run-time record | `[Certain-Fault-Refused]` (`spec/18`): a fault certain once a path is taken is refused |
| 384 | `printf("%v", 100 * 99999 * 100000 / 2)` faulted at run time | `[Literal-Expression-Folded]`: nested known operands folded once per node (`folded`), `diag.arith-overflow` / `diag.div-by-zero` at the check |
| `1 / (1 - 1)` | the cases' way to fail on purpose | refused; those cases use `assert(false)` (`std::assert` without `import std;`) |
| 395, 414 | a local's view, reference or non-`move` closure returned through a binding faulted on every call | `diag.reference-escapes-scope` (`[Ref-Escape-Through-Facts]`); `move` closures unaffected; D-0284's `if` case refused |
| 382 | a returned `spawn(f, &local)` faulted by timing | `diag.reference-escapes-scope` |
| 394 | views of a loop-local text kept after the loop faulted on every run | a holder that may refer to the text at its block's end, used later: `diag.destroy-while-aliased` |
| 413 | `current = Some(Conn {…})` in a loop faulted on the second round | `diag.overwrite-of-live-resource` unless the binding is `None` on every path (`empty_mark`) |
| 380 | `{ auto h = spawn(save, 2); }` lost the thread's `Err` | `diag.result-discarded`; a handle in a container ending unjoined after `Err` faults with it (`handle_destructor`) |
| 381 | the same `&mut counts[0]` lent to threads each round | `diag.aliasing-conflict` (report 313's rule, element places) |
| 390 | a guard passed to `spawn` faulted with an aliasing message | `diag.thread-affine-moved` (a guard is thread-confined) |
| 409 | `[None, Some(1: u8)]`, `auto r = if … { Ok(true) } else { Err("…") }` made `cobc` stop | the array: the checker's settled type (`settled`) is the lowering's; the `auto` `if`/`match` whose branches each fix one argument: `diag.type-annotation-needed` at the check |
| 410, 411 | `--check` accepted what the lowering could not type | `--check` runs the lowering: `diag.type-annotation-needed` there |
| 412 | `if (as_bytes(&s)[0] == b'a') { push_ascii(&mut s, …) }` faulted under `--all-checks` | the condition (and an integer or `bool` scrutinee) is lowered in a statement scope of its own |
| 399 | `fn die(…) : never` was "no type named `never`" | `[T-Never-Fn]`; `diag.never-returns` for a `return` or a path reaching the end |

## What is left

- `--check` runs the lowering (D-0340 §9; its cost accepted by the owner), and the lowering does not yet scale on
  100,000-deep types or thousands of element references (`check_scales_with_nesting_and_references.case` times
  those three inputs through `--certificate FILE --check`, the static pass alone): the deep types are copied at
  each level, and `elem_ref_lets` copies the rest of a block for each `let`. `quiet_locals` was made linear here
  (a statement index, `body_stmt_names`). For CHG-0484.
- A fault in a branch whose condition is a known constant false (`if (!(1 <= 1)) { 1 / (1 - 1); }`) is refused,
  though the branch never runs: the checker does not yet treat such a branch as dead. For CHG-0484.
- 392 (and 394's `lines(&text)` then `append`): a `Vec<StringView>` that may hold views of a text when the text is
  written. The checker's "may hold" cannot tell a `Vec` that may be empty, or whose reference was popped, from one
  that holds the view: refusing it refused correct programs (`conf.popped-vec-of-refs-frees-referent`,
  `conf.vec-pop-releases-ref`), so the run decides, and both builds fault since D-0339 §2.5.
- 418 (a reassigned reference counted as live until its reassignment): the program faults at run time in both
  builds, safely. Ending the old loan at its last read needs liveness that follows branches; a position-only rule
  could let a real conflict through where no run-time record is kept. For a later change.
- 409's `auto r = if (…) { Ok(true) } else { Err("…") }` (and the same through `match`) is refused at the
  check with `diag.type-annotation-needed`, not compiled: the checker gives `Ok(true)` alone no type, so neither
  branch fixes the other's; typing a variant literal partly is for a later change.
- 411's message is `diag.type-annotation-needed` (from the lowering), not the type error `Option::unwrap(&a)` alone
  gets.
- Priorities 3–5 of reports 350–451 (programs refused that are correct, performance, tools, messages,
  documentation) for later changes.

## Verification

The gate's sweep (`impl/tools/gate.py`) no longer counts as lost a program whose header now expects a static
refusal. The cases of `impl/tests/programs` that failed on purpose with `1 /(1 - 1)` use `assert(false)`.

Each report's repro rerun on the new build; `reports_update211.case` under GCC and Clang; the conformance suite
(nine cases now refused at the check updated); one gate-profile run before the push.
