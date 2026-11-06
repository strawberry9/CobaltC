# D-0305 — `String::from_str` retired

Status: ACCEPTED (2026-11-01, with D-0303 and D-0304, as the proposal's first retirement: the owner, "One of the
first ones I want to make obsolete is String::from_str so that the literal is always used instead"); deprecated by
`CHG-0403` (Update A), removed by `CHG-0405` (Update C)
Kind: Decision (`spec/02-schema.md`); a `std` retirement (`spec/STABILITY.md` §3a)
Governed by: `CobaltC_Master_Instructions.md` §9 (a helper that is only shorter, not clearer, does not earn its
place)
Depends on: D-0149, D-0155, D-0162, D-0302, D-0303 (the attribute), D-0304 (the positions)
Affects: `impl/std/text.cb`; `spec/21` §2, §2a, §2f (`[Sprintf]`), §2h; `spec/12` `rule.type.expected`; the
diagnostics that name it; every tracked tree that called it; the guide, the book and the showcases

## Problem

Owned text from a literal had two spellings, `String s = "x";` and `String s = String::from_str("x");`, and both
were taught (the guide's "Written as" table). After D-0304 the call form has no position left that needs it. For a
`str` value that is not a literal, `String::from_view` has taken every argument shape `from_str` was given since
D-0302 (checked on 2026-11-01 under `coby` and `cobc --run`: a `str` parameter, a `str` constant, a call giving a
`str`, an `if` of literals, a literal, inside `Some(…)`, an assigned value), and it takes a view, `&s` and a part of
a text too. `from_str` was a second name for a subset of `from_view`.

## Candidates

1. Keep both: against the principle the owner set.
2. Retire `from_view` instead: `from_str` takes a `str` only; `from_view` takes every text form.
3. Retire `from_str`. Selected.

## Selected design

A literal is a `String` where one is declared (`[Str-Literal-String]`, with D-0304's positions); a `str` value, a
view or a part is copied by `String::from_view`; a whole `String` by `String::clone(&s)` (the derived `clone`
every `clone`-bounded type answers to, D-0116, which no rule can remove; `String::from_view(&s)` is legal and never
written for a whole `String` in a tracked tree). `String::from_str` is `[[deprecated("…")]]` in Update A and a
tombstone in Update C, not before the edition that publishes Update B.

The rules that defined the literal forms by the call keep their meaning and name it no more:
`[Str-Literal-String]`'s and `[Sprintf]`'s conclusions are the `String` of the literal's or the text's bytes,
`String::from_view(StringView::of(L))`.

## Migration

Every tracked tree, by `stress/retire/migrate.py`: a call with a literal (or a constant's name) argument becomes the
literal where the checker gives the same verdict on the program; every other call becomes `String::from_view(X)`;
`String::from_view(&p)`, `p` a name or a field path, becomes `String::clone(&p)`. The git-ignored `stress/` corpus
takes the last two by `sed`.

## Compatibility impact

Breaking (source): a program calling `String::from_str` is rejected unless built with `--allow-deprecated` until
Update C, and always after it. `std` is *moving* (D-0220), so no window is owed; §3a's rule for a moving part sets
the pace.
