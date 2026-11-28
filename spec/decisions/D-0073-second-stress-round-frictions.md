# D-0073 — Frictions from the second stress round

Status: ACCEPTED (2026-09-27, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (items 7, 10), §9
Depends on: D-0019, D-0072, rule.ref.form, rule.stdlib.vec
Affects: `spec/09` `[Ref-Form-Temporary]`; `spec/21` §0 and §1;
`spec/22` (the variant message)

## Problem

A second set of programs (a JSON parser, a glob matcher, a binary search
tree, a module-split shop, an event bus, linear algebra) found:

1. **No insert or remove at an index in a `Vec`.** `Vec::remove_at` and
   `Vec::replace` exist but are internal to `std`.
2. **No `is_some`, `is_none`, `is_ok` or `is_err`.** Asking which variant
   a value is took a whole `match`.
3. **No way to compare two `String`s.** `a == b` was
   `diag.read-of-resource`, and comparing `String::as_bytes` results
   compares references.
4. **A temporary could not be borrowed even as an argument.**
   `printf("%v", &describe(e))` and `fold(&map_all(…), …)` had to bind
   the value first. That's three separate programs hit this.
5. **A variant with two payloads** (`Expected(usize, str)`) was told only
   "expected `)`, found `,`".

## Constraints

No new tokens, no new syntax. The library is preferred over the
language (Master Instructions §9).

## Selected design

1. **`Vec::insert(&mut v, i, x)`** (`0 ≤ i ≤ len`) and
   **`Vec::remove(&mut v, i) : T`**. The elements after `i` move by
   `Vec::swap`, so no element is copied and element identities stay
   correct. `diag.index-out-of-bounds` otherwise.
2. **`Option::is_some(&o)`, `Option::is_none(&o)`, `Result::is_ok(&r)`,
   `Result::is_err(&r)`**. They take a shared reference, so nothing is
   consumed.
3. **`String::eq(&a, &b)` and `String::eq_str(&a, "text")`**, defined by
   the views' `[View-Eq]`. `a == b` on a `String` stays
   `diag.read-of-resource`, but its message now names these.
4. **`&e` / `&mut e` of a temporary is allowed as a call's argument**,
   when the call's result is not a reference. The temporary ends at the
   statement's end (D-0019), as before, and the call ends before the
   statement does, so the reference cannot outlive it. Everywhere else
   (`auto r = &make();`, and an argument of a call that returns a
   reference, which would hand the reference straight back),
   `[Ref-Form-Temporary]` still rejects.

   A result that only *contains* a reference, such as `Option<ref<V>>` from
   `HashMap::get_mut(&mut m, &key(…))`, is allowed. The language has no
   lifetimes to say where that reference came from. If it did come from
   the temporary, its use after the statement is `diag.stale-binding`
   through the dynamic temporal check (`inv.temporal-validity`), as for
   any reference whose referent has ended. So this is safe, though
   caught late.
5. **The variant message**: "a variant carries one payload; to carry
   several values, make the payload a struct".

## Rejected alternatives

- **`==` on `String` values:** this would make `==` read a resource,
  which D-0006 and `[Read-Resource-Rejected]` exclude for every other
  resource.
- **Borrowing any temporary, with the lifetime extended to the
  enclosing block (C++'s rule):** this is a lifetime rule the language
  does not otherwise have. The statement-scoped argument case covers
  every instance met.
- **Tuple payloads:** a new form. A struct already says it, with names.

## Compatibility impact

Extension. Programs that were rejected are accepted: new library names,
and `&temp` as an argument. No accepted program changes meaning.
