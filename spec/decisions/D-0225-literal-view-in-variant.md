# D-0225 — A text literal is a view in a variant's `StringView` payload

Status: ACCEPTED (2026-10-13; the owner's answer to Q6 of the 2026-10-12 plan, "accept", confirmed 2026-10-13)
Kind: Decision (`spec/02-schema.md`); a typing extension
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0134 (`[Str-Literal-View]`), D-0162 (a literal typed through a generic parameter), D-0079
Affects: `spec/21` §2a (4.59.0), the checker and the front end's rewrites (`impl/src/typecheck.rs`,
`impl/src/views.rs`); two cases (CHG-0268)

## Problem

`[Str-Literal-View]` made `"key"` a `StringView` where a parameter, a local or a struct field is declared one,
but not inside a constructor: `Option<StringView> o = Some("v");` was `diag.type-mismatch`, and
`Some(StringView::of("v"))` the form.

## Options

1. **The variant constructor's payload is a position too**, once the constructor's expected type has fixed the
   enum's type arguments (`Some("v")` where `Option<StringView>` is expected, nested through `Ok(Some("v"))`, a
   program's own enum with a `StringView` payload). Selected (Q6).
2. Decline: `Some(StringView::of("v"))` stays the form.

## Decision

Option 1. As in D-0134, a literal only: a `str` binding stays a `str`. The checker records the literal and the
front end rewrites it to `StringView::of(L)` before the program is checked again (D-0162's mechanism), so both
tools see an ordinary call.

## Results

Cases: `conf.str-literal-view-in-variant`, `conf.str-binding-not-view-in-variant`; both tools, GCC and Clang.
