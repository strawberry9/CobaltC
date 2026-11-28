# CHG-0485 — Three faults the CIR review found

Status: implemented (2026-11-27); Update 213
Affects: `spec/conformance.md` 3.329.0 (2 rows); `impl/front/typecheck.cb`, `impl/front/ast.cb`, `impl/back/passes.cb`,
`impl/back/gen.cb`; `impl/tests/cases/cobc/reports_update213.case`;
`impl/conformance/12-type-system/empty_array_literal_uninferable_rejected.cb`,
`impl/conformance/08-alias-validity/implied_check_not_carried_into_loop_dynamic.cb`; `impl/STATUS.md`.

## Why

Phase 0 of the CobaltC IR (CIR) plan (the owner's ruling of 2026-11-27: `cobc` frozen at Update 212, fixes only, while the
new lowering is built beside it) read the whole lowering. It found one fault by reading and confirmed it by a run,
and two it could only suspect. The owner asked for them to be fixed before the new lowering copies anything, so the
compiler it is checked against is sound.

- **`auto x = [];` faulted the compiler.** An empty array literal is typed by its declaration (`[Array-Construct]`
  with `N = 0`, `spec/16`; `conf.zero-sized-types`). With `auto` nothing types it. The checker gave it a placeholder
  `array<i32, 0>`, so `[Generic-Call-Uninferable]`'s refusal (CHG-0438), which looks for an untyped value, let it
  through, and the lowering then stopped on `Option::expect` with `diag.unwrap-failed: cobc: empty array literal
  without an expected type` (`gen.cb`, `lower_array_lit`). The program was accepted by the check and the compiler
  crashed on it. Testing the fix found two more forms that reached the same `expect`: `Vec::from_slice(&[])`, a
  generic call the checker leaves to the lowering, and `Vec::from_slice<i32>(&[])`, a correct program, since a
  borrowed `[]` did not take the element type of the slice parameter it is given to.
- **A check before a loop removed the same check at the top of the loop's body.** `elide_implied_checks`, the pass
  over the generated C that drops a check an earlier one implies, forgets what it has seen at a join (a `}`, an
  `else`, a label, a `case`), but not at a loop's head, which is a join: the back edge arrives there from the end of
  the body. In `implied_check_not_carried_into_loop_dynamic.cb` the read of `p.x` before the loop dropped the read at
  the top of the body. On the second pass `keep` holds `&mut p.x`, and `[Read-Conflict]` requires the read to fault
  there; the program printed its line a second time and faulted only at the next borrow. `--all-checks` did not
  help, since the pass still ran; the bootstrap form, which runs no pass, faulted where it should. A check the
  specification requires was not made, and the outcome differed.
- **A call shortcut could lower a call without its point checks.** `lower_user_call_access` emits a call's point
  checks (D-0283) after its general path; the stage-3 shortcuts (`lower_call_shortcut`) return before it. The one
  shortcut that applies to user functions, `lower_dual_bare_call`, takes only visible borrows rooted at distinct
  owned locals and plain parameters that hold no reference (`dual_eligible`), which leaves no conflict for a point
  check to find. The others apply to named `std` functions, which have access summaries too, so a point check can be
  recorded for such a call. No program was found that reaches one: in every variant tried the conflict was found
  first where the borrow was formed. The gap is closed by construction rather than left to that search.

## What changes

- **`[]` with nothing to type it is refused** (`uninferred`, `empty_array_lit`): `diag.cannot-infer-type-parameter`
  (static), the message `auto v = Vec::new();` gives, wherever `uninferred` looks (an `auto` binding, an
  expression statement, a `match` scrutinee, a formatted argument). An empty array literal with a declared or
  expected type is unchanged.
- **`&[]` given where a slice is taken is an empty array of the slice's element type** when the call's types give it:
  the checker, which passes that type in, records it for the lowering (`note_settled`), and
  `Vec::from_slice<i32>(&[])` and `Vec<i32> v = Vec::from_slice(&[]);` are accepted. A literal with elements is
  typed by them, as before.
- **The lowering refuses `[]` it cannot type** with `diag.type-annotation-needed` (status 1), the error D-0339 gave
  the type arguments it cannot infer, in place of the `expect` that faulted the compiler. With the two changes above
  no program is known to reach it.
- **A loop's head is a join for `elide_implied_checks`** (`loop_head`: a line `for (…) {`, `while (…) {` or
  `do {`): the facts seen before it are forgotten, as at a `}`. A check at the top of a loop's body is kept; the
  checks inside one pass of the body are elided as before.
- **A call with point checks takes the general path** (`lower_user_call_access`): no shortcut lowers it, and the
  general path emits its checks. A body with point checks keeps its paths (`tracking_needed`), so no decided program
  loses a shortcut.
- `impl/STATUS.md`'s line on type arguments the lowering cannot solve said `unsupported:` (status 3); since D-0339
  it is `diag.type-annotation-needed` (status 1).

## What is left

- Inside a generic body `Vec::from_slice<T>(&[])` is refused by the checker, as before, with `diag.type-mismatch`
  and the repair `array<T, 0> none = [];` and `&none`, though it is correct: there the literal is typed by its
  elements alone. The CIR lowering types every instance on its own.
- The same reading found that the lowering's type probes (`probe_raw`, `probe_block_type`) do not roll back the
  request queue, the type descriptors, the closure table or the generated key functions, so a discarded probe can
  leave a function, a descriptor or a helper behind. No wrong output was traced to it; the CIR lowering will not
  have probes.
