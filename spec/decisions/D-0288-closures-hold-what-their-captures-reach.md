# D-0288 — A closure holds what its captures reach, and a program with closures may be decided

Status: ACCEPTED (2026-10-27, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 1, closure captures as borrows.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0018 (closures), D-0071, D-0089 (`fn` values), D-0111 (references end at their last use), D-0243,
D-0275, D-0281, D-0282, D-0284 (decided programs)
Affects: `spec/14-control-flow.md` 1.35.0 (`[Ref-Binding-Last-Use]`, the `deriv` table), `spec/conformance.md`
3.278.0; `impl/src/typecheck.rs`, `impl/cobc/src/lower/program.rs`, `impl/cobc/src/lower/stmts.rs`,
`impl/cobc/src/lower/exprs.rs`, `impl/model/src/statics.rs`; `CHG-0381`

## Problem

Every program with a closure that borrows what it captures, or captures a reference or a view, kept its record
of references for its whole run (`cobc`'s `pathless_blocker`). The static pass held a closure without `move` as a
borrow of each captured owner (CHG-0100) but not what a captured reference or view reached, and held a `move`
closure as holding nothing. It kept the program safe by never ending a captured binding before its block's end,
so a `move` closure over a view kept the view's borrow alive after its last use. That rejected correct programs
(`stress/d0281/captures/probes/p8.cb`: a closure binding assigned a second closure, after which the string the
first one's view reached is written).

## Options

- **A. Keep the blocker.** Closures stay tier D. Programs with callbacks, sort keys and `retain` predicates pay
  for tracking everywhere.
- **B. Closures carry facts.** A closure binding holds the `deriv` facts of what it captured: a borrow of each
  captured owner (no `move`, as now), and for any closure, the referent of each captured reference and the facts
  of each captured view, `Option` of a reference or `fn` value. A `fn` value copied from a binding holds that
  binding's facts; one a call returns may hold what any argument reached, which the run decides (`?`). Then a
  `move` closure's capture may end at its last use, and the blocker goes.
- **C. Closure types.** Give each capturing closure its own type in the static pass, as spec/15 §6 gives it in
  the semantics, and follow captures through it. A larger change, for no program B does not already decide.

## Decision

B, with three rules for when a binding ends at its last use (`[Ref-Binding-Last-Use]`):

1. A `move` closure bound whole (`auto c = move […]…`, `c = move […]…`) does not keep what it captures alive:
   it holds a copy, and `c` the facts. A closure anywhere else (an argument, `spawn`'s first) still does, since
   no binding would hold its facts.
2. A binding that may hold a reference derived from `x` stops counting for `x` once a later statement assigns it
   whole from a value that does not read it, with no `break`, `continue`, `return` or `?` on the way: what it got
   ends there (`f = move [vb]() …` after `f = move [va]() …`).
3. A binding whose facts another binding's are still rooted in waits, and ends after the first statement after
   which they no longer are. Before, it gave up and lived to its block's end.

`cobc` no longer keeps a program's paths for a closure. A `fn` value kept in data still makes the program tier D,
as any type that keeps references in data does.

## Repairs found with it

Ending captures early and deciding closure programs exposed these faults, each repaired in `CHG-0381`:

1. **A variant built in place.** `Some(String::as_view(&s))` was taken for static data: an `Option<StringView>`
   made so held no fact, and both `cobc` builds read the view after `String::append(&mut s, …)`, where `coby`
   faults. A variant now holds what its payload holds.
2. **A `fn` value overwritten.** Assigning a `fn` binding (or a value holding `fn` values) did not end the closure
   it held: the old closure's captures stayed borrowed in `cobc`'s tracked build until the binding's end, and its
   box leaked. The overwritten value now ends, as a discarded one does (D-0089).
3. **An `auto` closure binding assigned.** `auto f = move [a]() …; f = move [b]() …;` gave C that did not compile:
   `cobc` typed `f` as the first closure's own struct. A binding the body assigns whole is now a boxed `fn` value,
   as one the body borrows already was.
4. **A `fn` value's source.** A `fn` binding copied from another (`auto g = f;`) or returned by a call
   (`auto f = pick(v);`) held no fact. It now holds the source's facts; a call's result holds the arguments'
   facts as `?`, so the run decides (`stress/d0281/captures/probes/p11.cb`).

Two more were the blocker's to catch, and the gate over the closure programs found them once it was gone:

5. **A captured reference reassigned.** A closure without `move` borrows a captured reference binding itself
   (D-0243 marked it escaped, which no longer kept the program tracked); it now holds `deriv(c, r, m)`, so
   `r = &mut y` or a conflicting `*r` while the closure lives is rejected before the run.
6. **A closure through a shared reference.** Whether a `fn` value reached through `&f` writes what it captured
   (D-0218, `diag.borrow-exceeds-source`) only the run knows: a shared borrow of a `fn` binding keeps the
   program's tracking.

## Consequences

- No correct program is rejected that was accepted; some that were rejected now run (p8's shape). Programs that
  read a view through a variant, a `fn` copy or a returned closure after the viewed string changed are rejected,
  or keep their tracking, where `coby` faulted on each at run time.
- Programs with closures over references and views are decided when every conflict is: the closure over a view,
  the comparator capturing a key, the counter capturing `&mut n`.
- `cbc` (frozen, D-0278) keeps the blocker until it is ported.

## Verification

See `CHG-0381`.
