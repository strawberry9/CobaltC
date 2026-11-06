# D-0306 — The `String::` text wrappers retired: `&s` is the view, the `StringView::` functions take it

Status: ACCEPTED (2026-11-01, with D-0303 to D-0305, as the proposal's second retirement, under the one principle the
owner set: "my dislike of being able to do the same thing two different ways"); deprecated by `CHG-0404` (Update B),
removed by `CHG-0405` (Update C)
Kind: Decision (`spec/02-schema.md`); a `std` retirement (`spec/STABILITY.md` §3a)
Governed by: `CobaltC_Master_Instructions.md` §9 (a helper that is only shorter, not clearer, does not earn its
place)
Depends on: D-0113, D-0139 (the wrappers), D-0134 (the `StringView::` functions take a view), D-0302 (`&s` is a view
where one is declared), D-0303 (the attribute)
Affects: `impl/std/text.cb`; `spec/21` §2h and the rule blocks `[Contains]`, `[Replace]`; every tracked tree that
called a wrapper; the guide, the book and the showcases

## Problem

D-0113 (2026-09-30) added `String::as_view`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`
and `split` "because the searching functions live under `StringView::`, so a `String` in hand reads as needing
`String::view` first"; D-0139 added `contains` and `replace` "each on `StringView` and on `String`"; `strip_prefix`,
`strip_suffix`, `lines` and `split_whitespace` followed the pattern. D-0113 said of `as_view` that it was "named
only so the equivalence has a name to look up". Since D-0302, `&s` is the view of `s` wherever a `StringView` is
declared, so each wrapper is its `StringView::` twin applied to the argument as written: fourteen second spellings.

## Candidates

1. Keep both spellings: against the principle.
2. Retire the `StringView::` forms instead: they take every text form (a view, a part, a `str`, `&s`); the
   wrappers take a `String` only.
3. Retire the fourteen wrappers. Selected.

## Selected design

`StringView::NAME(&s, …)` for the thirteen searching and splitting functions; `&s` where a `StringView` is
declared, `&s[0..$]` elsewhere (D-0113's own definition of `as_view`), for `as_view`. Each wrapper is
`[[deprecated("…")]]` in Update B and a tombstone in Update C, not before the edition that publishes Update B.

Not retired, each a different operation rather than a second spelling: `String::join` (it takes a
`slice<String, shared>`, which `StringView::join` cannot); the `HashMap`/`HashSet` `_str` lookups (a different key
type); `StringView::of` (the one form where no position types the literal or the `str` value: `auto`, a type
parameter not yet fixed); `String::as_bytes` (its result is the `Vec<u8>`'s reference, not a slice) and
`String::from_utf8` (it owns the bytes); `String::eq` and `String::less` (what the key-type bounds call, D-0271).

**Found while implementing, for the owner** (not retired here; each would be its own record): `String::char_count`,
`String::chars`, `String::code_point`, `String::eq_str` and `String::len` have `StringView::` twins of the same
shape and would be second spellings by this record's argument.

## Migration

Every tracked tree, by `stress/retire/migrate_b.py`: `String::NAME(` becomes `StringView::NAME(` (where the first
argument is a reference that is not a place, a call's result, it is written `&(*e)[0..$]`, since D-0302 views
places only); `String::as_view(E)` becomes `E` where the checker gives the same verdict, otherwise `&X[0..$]`
(`E = &X`) or `&(*E)[0..$]`; `StringView::of(L)` becomes `L` where the checker gives the same verdict. The proposal
asked that in `std` and the self-hosted toolchain such a rewrite be kept only where `cobc` writes the same C; that
cannot be met while `cobc` lowers D-0302's view forms apart from `String::as_view` in two places (`CHG-0404`, "Not as
the proposal wrote it"), and the final perf gate stands instead. The git-ignored `stress/` corpus takes the renames and the fallback form by `sed`.

## Compatibility impact

Breaking (source): a program calling a wrapper is rejected unless built with `--allow-deprecated` until Update C,
and always after it. `std` is *moving* (D-0220), so no window is owed.

## Revisit conditions

None of its own; D-0303's apply.
