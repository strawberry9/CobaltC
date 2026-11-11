# D-0179 — "Pending" covers an argument's own borrow, not a path a value holds

Status: ACCEPTED (2026-10-06, the owner delegated the choice: "I leave these decisions to you, proceed with your recommendations")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0107 (argument borrows pending until bound), D-0053 (`StringView`)
Affects: `spec/15` `rule.fn.call` (1.12.1); `CHG-0207`

## Problem

D-0107 makes "a path an argument forms" pending until `[Call]` binds it,
so `put(&mut x, read(&x) + 1)` runs. It did not say whether a path *held by
a value* an argument computes is pending too: in
`f(String::as_view(&s), &mut s)` the first argument is a view whose path
`String::as_view` formed and returned; is it pending when `&mut s` is
formed for the second?

## Candidate mechanisms

1. **Not pending: only a borrow the argument itself forms (`&x`, `&mut
   x.f`, a slice `&x[a..b]`) is.** Selected. The value's path was formed,
   stored and returned by a call (or written by a literal) that has
   finished; it is held like any other, and `&mut s` meets it. Both
   implementations already behave this way (since Update 72 coby reads an
   argument into a temporary when it is evaluated).
2. **Pending too: every path reachable from an argument's value until
   binding.** More programs run, but the runtime must find and suspend the
   paths inside arbitrary values (views, structs, `Option`s of references)
   for the span of the call's argument list, and a value built earlier
   and passed by name (`f(v, &mut s)`) would behave differently from the
   same value built in place.

## Selected design

`spec/15`'s note on argument borrows says that a path held by a value an
argument computes is not pending. `f(String::as_view(&s), &mut s)` faults
`diag.aliasing-conflict` when `&mut s` is formed; `f(&s, &mut s)` still
runs to where a parameter is used (D-0107).

## Compatibility impact

None: the rule now states what both implementations do.

## Revisit conditions

- Real programs that need the view-and-mutate pattern in one call.
