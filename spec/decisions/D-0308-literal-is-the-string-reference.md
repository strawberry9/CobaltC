# D-0308 — A text literal is the `String` reference where one is declared; the last second spellings of text retire

Status: ACCEPTED (2026-11-02, the owner: "implement the proposal in full, autonomously, after the push, all
decisions as recommended"); deprecated by `CHG-0408`, removed by `CHG-0409`
Kind: Decision (`spec/02-schema.md`); a typing rule, and a `std` retirement (`spec/STABILITY.md` §3a)
Governed by: `CobaltC_Master_Instructions.md` §8 (priorities 3, 6, 9, 10, 12), §9 (the helper admission test;
"one way of writing one thing", the owner's principle of 2026-11-01)
Depends on: D-0006, D-0108 (`[Cmp-Text]`), D-0113 (the `_str` lookups), D-0134, D-0149 and D-0304
(`[Str-Literal-String]`), D-0162 (a literal fixes a type parameter), D-0164 (`[Text-Mixed-Eq]`), D-0200, D-0258
(`[Ref-Form-Const]`: an object of storage-kind static that never ends), D-0302 (`[Ref-String-View]`,
`[Str-Value-View]`), D-0303 (the cycle), D-0305 (a private function the front ends name, `String::copy_str`),
D-0306 (its deferred finding)
Affects: `spec/21` §2a (`[Str-Literal-String-Ref]`), §1 (the suffix table), §2g (`[Lookup-Str]` goes), §2h (the
retired rows; `[Text-Mixed-Eq]`, `[View-Eq]`, `[View-Form]` name the private functions), `type.str`; `spec/09`
(`[Ref-Form-Const]`'s cross-reference); `spec/conformance.md`; `impl/std/text.cb`, `collections.cb`; both
toolchains' checkers, interpreters/back ends and runtime (`cb_str_string`); every tracked tree that called a
retired name; the guide, the book and the showcases

## Problem

`HashMap::get<K, V>` takes `ref<K, shared> k`. With `K := String` the argument must be a reference to a `String`
*place*, and a literal is not one: `HashMap::get(&m, &"apple")` and `HashMap::contains(&m, &"apple")` were
`diag.borrow-of-non-place` under both toolchains, while `HashMap::insert(&mut m, "apple", 3)` was accepted
(`insert`'s parameter is `K` by value, so `[Str-Literal-String]` applies). D-0113 added six twins for exactly that
gap: `HashMap::get_str`, `get_mut_str`, `contains_str`, `remove_str`, `HashSet::contains_str`, `remove_str`, "lookups
of a String-keyed map by a `str`, without making a String". Measured on 2026-11-02 (Update 163's source), every
call of them outside `std` passed a literal.

D-0306 recorded, for the owner, that `String::char_count`, `chars`, `code_point`, `eq_str` and `len` have
`StringView::` twins of the same shape. Asking its question of every exported `String::`/`StringView::` name found
`String::parse<T>` (a sixth twin) and five exported functions that the rules give an operator or a form as the
spelling of: `String::eq_str`, `String::eq_view`, `StringView::eq_str` (`[Text-Mixed-Eq]`, `[View-Eq]`: `==`) and
`String::view`, `StringView::sub` (`[View-Form]`: `&s[lo..hi]`).

## Candidates

1. A checker rewrite of `HashMap::get(&m, "k")` to a hidden `get_str`: puts `std`'s `HashMap` into both checkers
   by name (D-0271 removed such lists) and keeps a twin per lookup. Declined.
2. **A literal (or a `str` value) is the `String` reference where a `ref<String, shared>` is declared.** Selected.
3. A key-view type per key type (Rust's `Borrow<Q>`): associated types in the generics, for six functions.
   Declined.
4. Keep the twins: against the principle.

`&"apple"` was considered and declined: the language types a literal by its position without `&` for its two
other text forms, `&L` would be the one borrow of a non-place, and `&` on a literal says nothing (a literal is
never moved or consumed) where `&s` on a place says "borrowed, not moved".

## Decision

- `[Str-Literal-String-Ref]` (`spec/21` §2a): a `str` literal, a use of a constant whose value is one, or a value
  of type `str`, where the declared type is `ref<String, shared>` (the positions of `[Str-Literal-String]`, and a
  `ref<K, shared>` once the other arguments and the expected type have fixed `K` as exactly `String`, as D-0162),
  is a shared reference to the `String` of its bytes, an object of storage-kind static that is never written and
  never ends. Not where a `ref<String, exclusive>` is declared, not for a type parameter nothing fixes, not `&L`.
- Retired by D-0303's cycle, each with a message naming its one spelling:
  - the six lookups, for the plain `HashMap::get`, `get_mut`, `contains`, `remove`, `HashSet::contains`, `remove`;
  - `String::len`, `chars`, `char_count`, `code_point`, `parse<T>`, for their `StringView::` functions on `&s`;
  - `String::eq_str`, `String::eq_view`, `StringView::eq_str`, `String::view`, `StringView::sub`, for `==` and
    `&s[lo..hi]`. The rules and the front ends' rewrites name `std`'s unexported `String::same_str`,
    `String::same_view`, `StringView::same_str`, `String::part`, `StringView::part`, which no program can name
    (`diag.name-not-visible`), as D-0305 did with `String::copy_str`.
- The literal's reference is the call of `std`'s unexported `String::static_ref`, which the front ends write.

Kept, each a different operation: `String::eq` and `String::less` (what the key-type bounds call, D-0271),
`String::join`, `String::as_bytes`, `String::from_utf8`, `StringView::of` (D-0306's reasons), and every
`StringView::` function.

## Migration

`stress/retire/migrate_c.py` over every tracked tree, checked by each tree's verdict before and after (the
pattern of D-0306's `migrate_b.py`), in `CHG-0408`'s Update; `CHG-0409` turns the attributes into tombstones.

## Compatibility impact

Breaking (source): a program calling a retired name is rejected unless built with `--allow-deprecated` until
`CHG-0409`, and always after it. `std` is *moving* (D-0220), so no window is owed. Additive for every other
program: a literal is now accepted where a `ref<String, shared>` is declared.

## Revisit conditions

None of its own; D-0303's apply.
