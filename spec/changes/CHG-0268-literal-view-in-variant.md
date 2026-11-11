# CHG-0268 — `[Str-Literal-View]` in a variant constructor's payload

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; D-0225)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0225
Affects: `spec/21` §2a (4.59.0), `spec/conformance.md`; `impl/src/typecheck.rs`, `impl/src/views.rs`

## What changed

- `spec/21` `[Str-Literal-View]`: the payload of a variant constructor whose payload type is `StringView`.
- The checker records such a literal (`Rewrite::LiteralView`); the front end makes it `StringView::of(L)`
  before the second check.
- Cases: `conf.str-literal-view-in-variant`, `conf.str-binding-not-view-in-variant`.

## What changed for programs

An extension: `Some("v")` where an `Option<StringView>` is expected, and the like.
