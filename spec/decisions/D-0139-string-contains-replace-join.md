# D-0139 — `String::contains`, `replace` and `join`

Status: ACCEPTED (2026-10-04, the owner: "add replace, join and contains to String")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0053 (`StringView`), D-0113 (the `String::` searching functions), D-0134 (`StringView` parameters)
Affects: `spec/21` §0 and §2h `rule.stdlib.stringview` (4.5.0), `spec/conformance.md` (3.129.0), the guide §21, `impl/std/text.cb`

## Problem

`String` could be searched (`find`), tested at its ends (`starts_with`,
`ends_with`), split and trimmed, but three of the commonest things a
program does with text had no name:

- **Is this in that?** Written `String::find(&s, t) != None`, which says
  "where" to ask "whether".
- **Replace every occurrence.** Written as a `find` loop that copies the
  gaps and the replacement into a new `String`: twelve lines, with the
  same off-by-one at every site.
- **Join parts with a separator.** Written as a `foreach` with an
  "except before the first" test; the inverse of `split`, which `std`
  already has.

Each names one very common intent (§9's admission test), and each has
one obvious meaning.

## Candidate mechanisms

1. **Leave them to programs.** The `find` loop is where the mistakes
   live; `join`'s absence makes `split` one-directional.
2. **`contains`, `replace` and `join` on both `StringView` and `String`,
   as the other searching functions are (D-0113).** Selected.
3. **In-place `String::replace(&mut s, from, to)`.** Saves a copy when
   the result replaces the source, but the common use keeps the source
   (a template, a line read from a file), and an in-place form would
   still build the new text in a buffer. The new-`String` form is the
   simpler contract; an in-place form is a later decision if programs
   measure the copy.
4. **`replace_first`, `replace_n`, `split_n`.** Not asked for; each is
   `find` plus two views.

## Selected design

`spec/21` §2h, `[Contains]`, `[Replace]`, `[Join]`:

- `StringView::contains(v, t) : bool` is whether `StringView::find(v, t)`
  is `Some`; `String::contains(&s, t)` is the same of `&s[0..$]`. The
  empty text is contained in every text, as `find` finds it at 0.
- `StringView::replace(v, from, to) : String` is a new `String`: `v` with
  every occurrence of `from` replaced by `to`. Occurrences are found left
  to right and do not overlap (`"aaa"` with `"aa"` replaced by `"b"` is
  `"ba"`). An **empty `from` occurs nowhere**, so the result is a copy of
  `v`; this differs from Rust, whose empty pattern matches between every
  pair of characters, a behaviour no program asks for and one that
  surprises. The matched bytes are whole characters because `from` is
  valid UTF-8, so the result is too. `String::replace(&s, from, to)` is
  the same of `&s[0..$]`.
- `StringView::join(slice<StringView, shared> parts, StringView sep) : String`
  is the texts of `parts`, in order, with `sep` between each two; no parts
  give an empty `String`. `String::join(slice<String, shared> parts, StringView sep)`
  is the same for `String`s. The two forms exist because `split` gives
  views and programs build `Vec<String>`; without a text bound there is no
  one signature for both. `join` follows the file's convention that the
  prefix names the argument's type (`String::find` takes a `String`,
  `StringView::find` a view), not the result's.
- Every text sought or inserted is a `StringView` (D-0134): a literal, a
  `String`'s `&s[0..$]`, or another view. `replace` and `join` return a
  new `String`, not views, so they may take any number of borrowed
  parameters (the one-borrow rule binds only functions returning views).

## Compatibility impact

Additive: six new associated functions of `std`'s `String` and
`StringView`; a program cannot declare `String::` or `StringView::`
functions of its own (`[Assoc-Fn-Foreign-Type]`).

## Revisit conditions

- An in-place `replace`, if programs measure the copy.
- `replace_first` or a count-limited `replace`, if programs ask.
- A `join` over `slice<str, shared>`, if literal lists prove common.
