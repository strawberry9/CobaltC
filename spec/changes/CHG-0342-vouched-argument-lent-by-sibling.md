# CHG-0342 — `cobc`: a vouched argument whose local a sibling argument lends to a thread

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19, the proof work of D-0270: the hypothesis list of `[Standing-Local]` read against the compiler's elisions)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: D-0209 (`[Standing-Local]`, the arrival test), D-0188 (frozen parameters), D-0107 (pending arguments), D-0213 `[Thread-Lend]`, D-0022
Affects: `impl/cobc/src/lower/user_calls.rs` (`lower_direct_arg`'s vouched forms, `sibling_names_root`), `vec_calls.rs` (`direct_local_call`); `spec/08` §4 (1.7.2); `spec/conformance.md` (3.252.0); `spec/02` §5 (1.0.122)

## What changed

- **The defect.** `spec/08` §4 lets an implementation check an elided parameter's path once, where it
  arrives: at the function's entry, after every argument is evaluated. `cobc` vouched for a pure direct
  parameter's argument `&mut x` (a borrow of the caller's own local) at the borrow instead: the borrow's
  check was made (`cb_borrow_unminted`), no path was minted, and the address went in with token 0, which
  the dispatcher's entry test (`cb_direct_ok`) and the direct twin take as vouched. Between that borrow
  and the entry the call's other arguments are evaluated, and one of them can lend the same local to a
  thread: `g(&mut x, spawn(f, &x))`. The lend's borrow sees no exclusive path (none was minted; and the
  argument's path is pending, which `clash` does not count, D-0107), so it is made; `g` then writes
  through `p` with no check while `f` reads `x` through the lent path. `coby` faults at the write
  (`[Write-Conflict]`, D-0022: the conflict found at first use); `cobc` printed `f sees 1` and `r 1`.
  The shared twin, `h(&x, spawn(w, &mut x))` with `h`'s parameter frozen (D-0188), read `x` unchecked
  beside the writer: `r 5` or `r 10` by the schedule, where `coby` faults at the read. The model checker
  (D-0219) admits neither outcome. Found while listing `[Standing-Local]`'s hypotheses for the Lean
  development (D-0270): H2, "checked against the other arguments of its call", is exactly the window.
- **The repair.** An argument is vouched for (no path minted, token 0: `lower_direct_arg`'s `&p`,
  `&v[i]` and `Box::get(b)` forms for a pure direct parameter) only when no other argument of the call
  names the binding at its root, closures' bodies included (`sibling_names_root`); otherwise the borrow
  is minted as any other, and the dispatcher's entry test sees the lent path and runs the checked twin,
  which faults at the first use as `coby` does. `direct_local_call` (the direct twin called outright)
  requires the same of every argument. `lower_dual_bare_call` already did ("its other arguments naming
  none of them"), and a frozen parameter's own field (`CHG-0237`) needs nothing: the parameter's path is
  held for the call, and a lend of it would clash.
- **The text.** `spec/08` §4's closing paragraph says in one sentence where the arrival test is made:
  at the entry, after every argument, so that a lend made by a later argument is seen there.
- **Verification.** `conf.direct-argument-lent-by-sibling-argument` and
  `conf.frozen-argument-lent-by-sibling-argument` (`impl/conformance/08-alias-validity/`): the spawned
  thread waits on a static flag so it is alive at the access under every schedule; both tools fault at the
  access, and the compiler before this change printed `f sees 1`/`r 1` and `r 5`. The compiled suite
  and the compiler's hand-written tests unchanged.

## What changed for programs

A program that lends a local to a thread in one argument of a call whose other argument borrows the same
local faults under `cobc` at the callee's first access, as it does under `coby` and as the rules say.
Such a call's borrow is minted (one path more, one check at the entry); no other call changes.
