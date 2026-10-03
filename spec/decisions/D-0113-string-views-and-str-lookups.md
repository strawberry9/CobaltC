# D-0113 — Views straight from a `String`, and map lookups by a `str`

Status: ACCEPTED (2026-09-30, the owner: "proceed with items 1 and 3 as D-0113" — the findings report's F6, `private/findings.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9
Depends on: D-0041 (hash tables, `[Key-Bytes]`), D-0053 (`StringView`, `[View-Form]`), D-0020 (`str`), rule.stdlib.hashmap, rule.stdlib.stringview
Affects: `spec/21` §0, `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## Problem

The three text types are each right on their own; the corpus pays at
their seams (288 programs, about 47 000 lines):

| Shape | Sites |
|---|---|
| `String::view(&s, 0, String::len(&s))` — a whole-string view, to reach `StringView::split`, `find`, `trim`, … | 80 |
| `HashMap::get(&m, &String::from_str("k"))` and kin — a `String` built only to be compared and thrown away | 23 |

`&s[0..$]` (`[View-Form]`) already is the whole-string view, but no
program found it: the searching functions live under `StringView::`,
so a `String` in hand reads as needing `String::view` first. And a map
keyed by `String` can be asked only with a `String`, though `[Key-Bytes]`
hashes a `str` and a `String` of the same text to the same bytes.

## Candidate mechanisms

1. **Leave it.** `&s[0..$]` and `String::from_str("k")` do the job.
2. **Wrappers in `std`, written in CobaltC.** The `StringView` searching
   and splitting functions also under `String::`, over `&s[0..$]`; and
   `_str` lookups for a `String`-keyed map or set, hashing the `str`'s
   bytes and comparing with `String::eq_str`, so no `String` is made.
   Selected.
3. **A rule that the key parameter of `get`/`contains`/`remove` takes
   any text type when `K = String`.** Less visible (an overload decided
   by the key type), and a rule where a library function does.
4. **A `str` literal typed as `String` by its context** (the report's
   F6.4). Not adopted: it would be the one place a literal allocates
   and an implicit conversion between two types, against `spec/22` §5.

## Selected design

    String::as_view(ref<String, shared> s) : StringView            // ≡ &s[0..$]
    String::find(ref<String, shared> s, str t) : Option<usize>     // ≡ StringView::find(&s[0..$], t)
    String::starts_with(ref<String, shared> s, str t) : bool       // likewise for each:
    String::ends_with(ref<String, shared> s, str t) : bool
    String::trim(ref<String, shared> s) : StringView
    String::trim_start(ref<String, shared> s) : StringView
    String::trim_end(ref<String, shared> s) : StringView
    String::split(ref<String, shared> s, str sep) : Vec<StringView>

    HashMap::get_str<V>(ref<HashMap<String, V>, shared> m, str k) : Option<ref<V, shared>>
    HashMap::get_mut_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<ref<V, exclusive>>
    HashMap::contains_str<V>(ref<HashMap<String, V>, shared> m, str k) : bool
    HashMap::remove_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<V>
    HashSet::contains_str(ref<HashSet<String>, shared> s, str k) : bool
    HashSet::remove_str(ref<HashSet<String>, exclusive> s, str k) : bool

- Each `String::` function is the `StringView::` one applied to
  `&s[0..$]`; the result borrows `s` as the view does. `as_view` is
  named only so the equivalence has a name to look up; `&s[0..$]` stays
  the form the guide teaches.
- Each `_str` lookup is the same lookup as with a `String` key `k'`
  whose bytes are `k`'s (`[Lookup-Str]`, `spec/21` §2g): the same slot
  (`[Key-Hash]` over `bytes(k)`, which `[Key-Bytes]` makes equal for a
  `str` and a `String` of one text), the same entry, and for `remove_str`
  the same reordering as `remove` (the last entry moves into the hole).
  A map whose key type is not `String` cannot call them
  (`diag.type-mismatch` at the call, an ordinary type error on the
  first argument). `remove_ordered` has no `_str` twin: no program
  needed it, and the order-keeping removal is the rare one.
- Not adopted, by the owner's acceptance: `StringView::of(str)` (item
  2 of the report, deferred earlier) and a `str` literal typed as
  `String` (item 4).

## Compatibility impact

Additive. New names in `std` are shadowed by a program's own (D-0024).
No rule, grammar, diagnostic or native code changes.

## Revisit conditions

If `StringView::of(str)` is lifted from its deferral, the `String::`
wrappers gain nothing and lose nothing: a literal's search is then
`StringView::find(StringView::of("…"), t)`.
