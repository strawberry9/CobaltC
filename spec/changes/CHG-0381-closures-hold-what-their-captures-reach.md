# CHG-0381 — A closure holds what its captures reach, and a program with closures may be decided

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0288, D-0281, D-0284, D-0111, D-0089
Affects: `spec/14-control-flow.md` 1.35.0 (`[Ref-Binding-Last-Use]`, the `deriv` table), `spec/conformance.md`
3.278.0; `impl/src/typecheck.rs`, `impl/cobc/src/lower/program.rs`, `impl/cobc/src/lower/stmts.rs`,
`impl/cobc/src/lower/exprs.rs`, `impl/model/src/statics.rs`

## What changed

In the static pass (`typecheck.rs`):

1. `closure_facts_of` gives a closure binding, `move` or not, `deriv(c, *r, m)` for each captured reference or
   slice `r` and the facts of each other captured binding that may hold a reference (a view, an `Option` of one, a
   `fn` value); a closure without `move` keeps its borrow of each captured owner. The facts are set at every
   closure literal bound or assigned whole (one that captures nothing clears them).
2. `fn_value_facts`: a `fn` binding made from a binding holds its facts; from a call, the facts of each argument
   that may hold a reference, as `?` (the run decides).
3. `early_end_plan` no longer keeps alive what a `move` closure bound whole by a `let` or an assignment captures
   (`closure_captures_stmt`); a closure anywhere else keeps its captures alive, as before.
4. `ref_end_blocked`: a binding that shared a statement with `x` stops counting for `x` once a later statement
   assigns it whole from a value that does not read it, with no `break`, `continue`, `return` or `?` between
   (`stmt_overwrites`, `stmt_may_leave`).
5. `end_ref_bindings` returns the bindings it could not end because another binding's facts are rooted in them;
   they wait for a later statement.
6. `inherited_facts_in`: a variant built of a payload (`Some(v)`, `Ok(r)`) holds what the payload holds; it was
   taken for static data. `Option::unwrap`/`expect` and `Result::unwrap`/`expect` hold what their argument holds,
   as `contained_ref_facts` already had it, so `Ok(Option::unwrap(HashMap::get_mut(…)))` stays decided.
7. A closure without `move` holds `deriv(c, r, m)` for a captured reference binding `r`, which it borrows itself.
8. A shared borrow of a `fn` binding (`&f`) leaves the function to the run (D-0218: whether the closure writes
   what it captured is not known before it).

In `cobc`:

9. `pathless_blocker` no longer keeps a program's paths for a closure that borrows or captures a reference; a
   type that keeps references in data, `fn` values among them, still does.
10. An assignment to a place whose value holds `fn` values and owns no resource ends the old value
   (`cb_drop_value`), as a discarded value is ended (D-0089): the closure it held is destroyed with what it holds.
11. An `auto` binding of a closure that the body assigns whole is a boxed `fn` value, as one the body borrows is.

In the model (`cobmodel`, D-0219):

12. The closure row, the `fn` value row and the three rules of `[Ref-Binding-Last-Use]` as written in spec/14
   1.35.0. The model sample found the tools outside the old rules' set on three programs: a binding rooted in
   another now ends once that one has (`three-p000238`), and reassigning a reference a closure captured is
   rejected before the run (`hand-k02`, `disagreed-r5c-p004329`).

## What changed for programs

A `move` closure that copied a view no longer keeps the view's binding alive: the closure holds what the view
reached, for as long as the closure lives, and assigning the closure binding a new closure ends that. Programs
that read through a view after the viewed string changed, by way of a variant, a copied `fn` value or a returned
closure, are rejected or keep their tracking; `coby` faulted on each. A program whose only tracking was a closure
is decided when its conflicts are.

## Conformance

`conf.option-of-view-blocks-owner-write` (static), `conf.move-closure-view-capture-blocks-owner-write` (static),
`conf.move-closure-reassigned-lets-old-capture-end` (positive).
