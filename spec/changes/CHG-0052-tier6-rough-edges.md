# CHG-0052 — `String::clone`, `push_ascii`, `clear`; `Vec::clear`; full destructuring; a map loop's position

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-26, delegated by the owner)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0044
Affects: rule.stdlib.text, rule.stdlib.hashmap, rule.init.let, rule.control.foreach, spec/22 §2, diag.not-ascii (new), the cases listed below

## Problem / motivation

D-0044: five gaps met while writing Tier 6.

## What changed

- **`spec/21` 3.14.0:** `String::clone`, `String::push_ascii`
  (`[Push-Ascii-Not-Ascii]`), `String::clear` (§2d); `Vec::clear` (§2g's
  code).
- **`spec/12` 1.11.0, `spec/17` 2.2.3:** `[T-Let-Destructure]` and the
  field-visibility note over every field.
- **`spec/11` 1.3.0:** `[Let-Destructure]` over every field;
  `[Let-Destructure-Fields]`, `[Let-Destructure-Destructor]`.
- **`spec/14` 1.7.0:** `foreach (i, k, v in m)`.
- **`spec/22` 2.16.0:** the destructuring declaration and
  `foreach-expr` grammar.
- **`spec/registry/diagnostics.md` 1.20.0:** `diag.not-ascii`; the two
  new rejections named under `diag.type-mismatch` and
  `diag.move-out-of-field`.
- **`spec/conformance.md` 3.39.0:** the cases below.
- **`spec/02-schema.md` 1.0.28:** §5's "in use" ranges.
- **Implementations:** `std`; `Stmt::Destructure` holds a field list
  (parser, `modres`, typecheck, `coby`, `cobc`); `coby` ends a
  destructured container without destroying its moved-out fields (it
  had destroyed the one field of a single-field struct twice when that
  field had a destructor of its own); the parser's `foreach` takes a
  third name; `src/each.rs` refuses three names except over a map.
- **Showcases:** Tier 6 uses the new functions and destructuring.

## Compatibility classification

Extension. One program shape the implementations had accepted outside
the specification is now rejected: destructuring that names only some
fields of a multi-field struct (`[Let-Destructure]` always required a
single-field struct, but neither tool checked, and `coby` bound field 0
whatever its name). `conf.field-export-ok`'s program did this; it now
destructures a struct whose fields it names.

## Conformance changes

**Added:** `conf.string-clone`, `conf.string-push-ascii`,
`conf.string-push-ascii-rejects-non-ascii`, `conf.vec-clear`,
`conf.destructure-all-fields`, `conf.destructure-missing-field-rejected`,
`conf.destructure-destructor-rejected`, `conf.foreach-map-position`,
`conf.foreach-three-names-rejected`.

## Prior-art status

See D-0044.

## Revisit conditions

See D-0044.
