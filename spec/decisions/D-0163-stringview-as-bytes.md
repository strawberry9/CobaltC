# D-0163 — `StringView::as_bytes`: text's bytes as a slice

Status: ACCEPTED (2026-10-04, the owner: "accept both as D-0163 and D-0164 and implement", after the frictions found writing showcase Tier 11)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0053 (`StringView`), D-0134 (`[Str-Literal-View]`, the view-returning elision), D-0151 (crypto functions take `slice<u8, shared>`)
Affects: `spec/21` §0, §2h (4.28.0), `spec/conformance.md` (3.153.0), the guide §21, `impl/std/text.cb`; `CHG-0191`

## Problem

Every hashing, MAC, cipher and socket function takes `slice<u8,
shared>`. A program holding text had no way to pass it without copying:
`StringView`'s `bytes` field is private, `str_slice` is reserved to
`std`, and the routes left were `String::into_bytes(String::from_view(v))`
(a copy) or `&b"…"[0..$]` (literals only). Showcase Tier 11 met it in
every program.

## Candidate mechanisms

1. **`StringView::as_bytes(v) : slice<u8, shared>`**: the same bytes,
   borrowed for as long as the view. A literal in that position is a
   view already, and `String::as_view(&s)` a `String`'s, so one function
   serves every text type. Selected.
2. **Export the field `bytes`.** Commits the representation of a type
   whose invariant is UTF-8; a function does not. Rejected.
3. **`str` overloads of the crypto functions.** Spreads one need over
   many signatures. Rejected.

## Selected design

`spec/21` §2h `[View-As-Bytes]`: `StringView::as_bytes(StringView v) :
slice<u8, shared>`, written in CobaltC (`v.bytes`), the result's
lifetime the view's (D-0134's elision for one borrowed parameter).
`hmac_sha256(StringView::as_bytes("key"), StringView::as_bytes(String::as_view(&body)))`
needs no copy.

## Compatibility impact

Additive.

## Revisit conditions

None foreseen.
