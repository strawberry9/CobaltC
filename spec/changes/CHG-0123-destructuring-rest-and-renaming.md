# CHG-0123 — Destructuring: `..` and renaming

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0104
Affects: spec/11 `[Let-Destructure]`, `[Let-Destructure-Fields]`; spec/22 `decl-stmt`, `dfield`

## What changed

- **spec/11, spec/22:** entries `field` / `field : name`, and `..` last.
- **Implementations:** the parser records each entry's field and binder
  and the `..`; the checker, `coby` and `cobc` bind the fields `..`
  covers to hidden names (`ast::destructure_pairs`).
- **Rows:** `conf.destructure-rest`, `conf.destructure-rename`,
  `conf.destructure-two-of-one-type`, `conf.destructure-missing-without-rest-rejected`,
  `conf.destructure-rest-destroys-at-block-end`.

## Compatibility classification

Additive.
