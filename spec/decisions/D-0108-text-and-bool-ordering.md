# D-0108 — `bool`, `str` and `String` are ordered, and a `String` is compared in place

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 proposal P3, option A)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0053 (views), D-0073, D-0076 (text order), D-0090 (bounds)
Affects: spec/06 `rule.arith.cmp`, spec/12 `rule.type.bound`, `rule.type.eq`, `[T-Cmp]`, spec/21 `type.str`

## Problem

Text was compared by name: `String::eq(&a, &b)`, `String::less(&a, &b)`,
`StringView::less(a, b)`. `a == b` on two `String`s, `"a" < "b"` and
`false < true` were rejected, and `min`, `max` and every function bound
`eq` or `ordered` could not take text or `bool`. The round-6 programs
wrote the named calls dozens of times, mostly inside sorting and
searching code that was generic in all but this.

## Candidate mechanisms

- **A: the comparison operators are defined on `bool`, `str` and
  `String`, and `eq`/`ordered` admit them.** A `String` operand is not
  read whole: it is borrowed shared for the comparison, as `&a` would
  be (a temporary lives to the statement's end). Selected.
- **B: only `eq`** (no ordering of text or `bool`): keeps `String::less`
  as the one way to order, but `min`/`max` and sorting stay closed to
  text.
- **C: nothing** (the status quo).

## Selected design

- `==` `!=` `<` `<=` `>` `>=` on two `String`s, two `str`s or two
  `StringView`s compare their bytes in byte order (a prefix first; for
  UTF-8, code point order); on `bool`, `false < true`.
- A `String` operand that is a place is borrowed shared for the
  comparison: nothing is moved, and the usual conflicts apply (an
  exclusive borrow of it that is still live rejects the comparison).
  A temporary `String` operand lives to the end of its statement.
- `eq` and `ordered` are satisfied by `bool`, `str` and `String`
  (`number` and `integer` are unchanged).
- A `str` and a `StringView` still compare with `==`/`!=` only; mixed
  text types do not order.

## Compatibility impact

Additive: every program accepted before means the same. Four
conformance rows that pinned a rejection now pin the result.

## Revisit conditions

If `StringView` should satisfy `eq`/`ordered` too (it is a value type,
so nothing blocks it except that no program has asked).
