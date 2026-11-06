# CHG-0177 — `[Str-Literal-String]`: a literal where a `String` is declared

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0149)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0149
Affects: `spec/21` (4.14.0) §2a; `spec/12` (1.20.0) `rule.type.expected`; `spec/conformance.md` (3.139.0); `spec/examples.md` (3.17.0); the guide §21; `impl/src/views.rs`; the conformance cases, showcases and guide examples, whose `String::from_str("…")` calls in declared-`String` positions are written short

## What changed

- **`spec/21` §2a:** `[Str-Literal-String]`: a `str` literal (or a `str` constant's use) in a position whose declared type is `String` — a parameter, a local's type, a struct literal's field, or the result of the function returning it — is `String::from_str(L)`.
- **`spec/12` `rule.type.expected`:** the second exception for a `str-literal`.
- **Both tools:** `impl/src/views.rs` rewrites such literals, as it does view literals.
- **Rows:** `conf.str-literal-as-string`, `conf.string-of-str-binding-rejected`.
- **Existing programs:** at the owner's request, every `String::from_str("…")` in a position the rule covers (a `String` local's initializer, a struct literal's field declared `String`, a `: String` function's tail and match arms) is written as the literal: 118 calls in 62 conformance cases, showcases and guide examples, plus `spec/examples.md`. The 145 in other positions (generic arguments such as `Vec::push`, `Some(…)`, borrowed arguments) stay.

## Compatibility classification

Additive: only programs that were type errors change, to accepted.
