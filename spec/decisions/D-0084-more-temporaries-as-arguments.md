# D-0084 — Operator results, `?` payloads and literals as borrowed arguments

Status: ACCEPTED (2026-09-28, stress round under the owner's standing instruction to resolve frictions with the implementer's leanings; confirmed by the owner 2026-09-28)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (items 7, 10), §9
Depends on: D-0019, D-0073, rule.ref.form, rule.type.expected
Affects: `spec/09` `[Ref-Form-Temporary-Argument]`

## Problem

D-0073 let a call's result or an aggregate literal be borrowed as an
argument (`total(&make())`). Real programs borrow other values in the
same position and were rejected with `diag.borrow-of-non-place`:

- a map key computed on the spot: `HashMap::get(&table, &((len << 32) |
  code))` in a Huffman decoder;
- a key written as a literal: `Lru::get(&mut cache, &3)`,
  `HashSet::contains(&s, &-3)`;
- the value `?` gives: `Vec::len(&names()?)`.

Each needed a binding of its own, for a value that, like a call's
result, ends when the statement does, after the call is over.

## Candidate mechanisms

1. **Keep D-0073's list.** Every such argument is bound first.
2. **Admit every non-place expression** (`&if (c) { a } else { b }`,
   `&match …`). More than the programs asked for, and a block may end
   in a place.
3. **Admit operator expressions, `e?`, and number and `bool` literals.**
   Selected: the shapes the programs used. They are values, never places,
   and they end with the statement exactly as a call's result does.

## Selected design

- `[Ref-Form-Temporary-Argument]` (`spec/09`) admits, besides a call and
  an aggregate literal, a unary or binary operator expression, `e?`, and
  a number or `bool` literal, of non-reference type.
- A literal has no type of its own to take, so it is evaluated at the
  referent type τ when the argument's expected type is `ref<τ, _>`
  (`rule.type.expected`): `&3` is a `ref<u64, shared>` where a
  `ref<u64, shared>` is expected, and a literal that does not fit τ is
  `diag.literal-out-of-range`. With no expected type it is typed as any
  unsuffixed literal is.
- Outside an argument nothing changes: `ref<i32, shared> r = &5;` is
  `diag.borrow-of-non-place`, whose message now says that `&` borrows a
  place and a value only as such an argument.

## Compatibility impact

Loosening: programs rejected before are accepted; none changes meaning.

## Revisit conditions

If programs are found to need `&` of a block, `if` or `match` value as
an argument.
