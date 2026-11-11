# CHG-0402 — A text literal typed by its position in five more positions (D-0304)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0304, D-0149, D-0155, D-0162, D-0225, D-0302
Affects: `spec/21-standard-library-semantics.md` §2a, §2h (4.74.0), `spec/12-type-system.md` (1.27.0),
`spec/conformance.md`; `impl/src/typecheck.rs`, `impl/src/views.rs`; `impl/selfhost/front/typecheck.cb`,
`impl/selfhost/front/views.cb`; the new cases under `impl/conformance/21-standard-library-semantics/`

## What changed

1. A text literal is a `String` (or a `StringView`) where it is the value assigned to a place of that type, an element
   of an array literal whose element type is declared so, a struct literal's field whose type the expected type
   fixes, a variant constructor's payload the expected type fixes, and (for `String`) what a closure declared
   `: String` gives; D-0155's tails apply. `&s` and a `str` value assigned to a `StringView` place are its view.
2. The closure's result is rewritten by the literal pass as a function's is. The other positions are found by the
   checker, which records each literal for D-0162's and D-0225's rewrites and checks the program again.
3. Conformance: `conf.str-literal-string-assigned`, `conf.str-literal-string-variant-payload`,
   `conf.str-literal-string-generic-field`, `conf.str-literal-string-array`, `conf.str-literal-string-closure`,
   `conf.str-literal-string-auto-is-str`, `conf.str-binding-assigned-rejected`.

## Verification

- Every new case under `coby` and `cobc --run` with GCC and Clang, and under `cbc`; as `CHG-0401` for the rest.
- A gap left open: a struct literal given as the argument of a generic call is not typed by the call's expected
  type, so `make(Pair { .a = "x" })` for a generic `make` still needs the struct's type written (`Pair<String> {…}`);
  the book's chapter 11 writes it.
