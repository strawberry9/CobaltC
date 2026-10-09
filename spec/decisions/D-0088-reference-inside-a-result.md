# D-0088 — A result that holds a reference borrows its first reference argument

Status: ACCEPTED (2026-09-28, the owner's decision on the fourth stress round's findings)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0011, D-0071, D-0073
Affects: `spec/14` §6 (the `deriv` facts)

## Problem

D-0011's elision says where a returned `ref<T, m>` comes from. A result
that only *holds* a reference, such as `HashMap::get`'s
`Option<ref<V, shared>>`, had no stated origin (D-0073), so holding one
across a write to the container was left to the run-time check:

    ref<String, shared> first = Option::unwrap(HashMap::get(&m, &1));
    HashMap::insert(&mut m, 2, String::from_str("two"));
    printf("%s\n", first);

That faulted only when the insert grew the table, so whether a test
caught it depended on the map's capacity. The same gap let a function
with two reference parameters return `Option<ref<T>>` where returning
`ref<T>` is `[Call-Multi-Ref-Return-Rejected]`.

## Candidate mechanisms

1. **Keep it dynamic.** Sound; capacity-dependent to observe.
2. **Borrow from every reference argument.** Also refutes writing the
   key while the value is held, which is valid and natural (look up the
   current room, then move to another): a text adventure in the stress
   set was rejected by it.
3. **Borrow from the first reference argument**, the container, as a
   method's receiver would be. Selected.

## Selected design

For `auto o = e;`, `o = e;` and a `match (e)` arm's binder, where `e` is
a call (or `Option::unwrap`/`expect`, `Result::unwrap`/`expect` of one)
whose result holds a reference without being one, `o` holds
`deriv(o, x.π, m)` for the first reference-typed parameter's argument
when it is a visible borrow or slice of `x.π` in mode `m`. A result that
in fact comes from a later argument is still checked at run time.

## Compatibility impact

A tightening: holding such a reference across a conflicting access to
the first argument's place is rejected. No program in the repository or
the stress set is affected.

## Revisit conditions

If a function whose contained reference comes from a later parameter
proves common, a way to say so (a parameter marker) would be the
narrow extension.
