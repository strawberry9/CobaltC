# D-0164 — `==` and `!=` between a `String` and a `str` or a `StringView`

Status: ACCEPTED (2026-10-04, the owner: "accept both as D-0163 and D-0164 and implement")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0053 (`str == StringView`), D-0108 (`[Cmp-Text]`: ordering within one text type), D-0006 (no implicit conversion)
Affects: `spec/06` `rule.arith.cmp` (1.17.0), `spec/21` §0, §2h (4.29.0), `spec/conformance.md` (3.154.0), the guide §12 and §21, `impl/std/text.cb`, `impl/src/typecheck.rs`, `impl/src/views.rs`, `impl/src/lib.rs`; `CHG-0192`

## Problem

D-0108 let `==` and the orderings compare two values of one text type,
and D-0053 let a `str` and a `StringView` be compared for equality.
`String` stood apart: `name == "sha256"` was accepted for a `StringView`
`name` and refused for a `String` one, although a program's text usually
arrives as a `String` (`arg`, `read_line`, `read_file`). Showcase Tier
11's `hashsum` met it.

## Candidate mechanisms

1. **Equality between a `String` and a `str` or a `StringView`, either
   way round**, comparing bytes, the `String` borrowed shared, as the
   existing `str`–view pair. Selected.
2. **Mixed ordering as well.** Rare, and D-0108 kept orderings within one
   type; not adopted.
3. **Typing the literal as a `String`** (D-0149 style) on the other side
   of `==`. An allocation for every comparison. Rejected.

## Selected design

`spec/21` §2h `[Text-Mixed-Eq]`, `spec/06` `rule.arith.cmp`: `s == b`
and `s != b`, `s` a `String` and `b` a `str` or a `StringView` (either
side), is `String::eq_str(&s, b)` or `String::eq_view(&s, b)` (new,
written in CobaltC), negated for `!=`. Nothing is moved or copied; a
temporary `String` lives to the end of its statement. `<`, `<=`, `>`,
`>=` between them stay `diag.type-mismatch`, with a message saying so.

**Realization:** the checker records each such comparison; the front end
rewrites it to the call (`impl/src/views.rs`, the D-0162 mechanism,
generalized to a table of rewrites) and checks again, so both tools see
an ordinary call.

## Compatibility impact

Additive.

## Revisit conditions

- Mixed ordering, if programs ask.
