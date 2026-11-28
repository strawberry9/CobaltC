# D-0053 — `StringView`: borrowed views of a `String`'s text

Status: ACCEPTED (2026-09-26, owner: "StringView", split's result left to the assistant)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0020, D-0047, D-0049, rule.stdlib.string, rule.agg.slice, rule.temporal.elision
Affects: rule.stdlib.stringview (new), rule.agg.slice, `spec/21` §0

## Problem

Text a program processes lives in a `String`, but part of one could be
reached only as bytes (`&(*String::as_bytes(&s))[a .. b]`, a
`slice<u8, shared>`), which is not known to be text: it cannot be
printed with `%s`, compared with `==`, or passed where text is expected
without copying it into a new `String`. Tokenizing, trimming and
splitting input fell back to byte loops and copies (the owner ran into
this; so did the integration programs).

## Candidate mechanisms

A. Byte views only (`&s[i..j]` a `slice<u8, shared>`) and helpers over
byte slices. B. A borrowed text-view type made by `&s[i..j]`. C. Make
`str` itself a borrowed view, as Rust's `&str` is.

## Selected design

B, named `StringView` (a `std` type beside `String`, capitalized as
`std`'s types are; the owner's choice: it views a `String`, not a
`str`). `&s[lo .. hi]` on a `String` place, with `$` its length in
bytes, is a shared view; `&v[lo .. hi]` re-slices a view. Bounds are
byte positions that must fall on character boundaries
(`diag.not-char-boundary`). A view prints with `%s`/`%v`, appends with
`String::append`, and compares with `==`/`!=` to another view or a
`str`. `std` gives `len`, `find`, `starts_with`, `ends_with`, `trim`
(ASCII white space), `split` (returning `Vec<StringView>`), `parse<T>`
and `String::from_view`. A view is a borrowed type for elision and for
the flow analysis, as a slice is.

## Rejected alternatives

- **A:** a byte slice cannot promise text, so every print or compare
  would recheck or copy it.
- **C:** changes what `str` is (D-0020): every `str` would acquire a
  lifetime and borrow tracking, and existing code would change meaning.
- **An exclusive view:** writing through a view could break UTF-8.
- **A lazy `split`** usable only in `foreach`: new machinery; the
  `Vec<StringView>` it returns allocates only the list of views, not the
  text.

## Semantic rationale

A view is a `slice<u8, shared>` of the `String`'s bytes with the added
guarantee that both ends are character boundaries, so it inherits the
slice's borrow rules unchanged.

## Usability

    StringView word = &line[2..7];
    if (word == "hello") { … }
    foreach (part in StringView::split(StringView::trim(&line[0..$]), ",")) { printf("<%s>\n", part); }

## Explainability

"`&s[i..j]` on a `String` is a view of its text: no copy, and the
`String` can't change while you hold it."

## Implementation-feasibility

`std` code in CobaltC (a struct over `slice<u8, shared>`). The static
pass types the three forms and records them (`Items::view_ops`); `coby`
and `cobc` evaluate each form's own operands and call `std`. Printing
reads the view's bytes through its slice's reference.

## Compatibility impact

Extension; `&s[i..j]` on a `String` was ill-typed before.

## Prior-art status

Rust `&str` of a `String`, C++ `std::string_view`, Go substrings, Swift
`Substring`.

## Invariant traceability

`inv.string.utf8-validity` (boundaries), `inv.temporal-validity` and
`inv.alias-validity` (as for slices).

## Revisit conditions

Views into `str` values, `foreach` over a view's characters, and a lazy
`split`, if programs need them.
