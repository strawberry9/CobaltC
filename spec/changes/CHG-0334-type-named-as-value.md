# CHG-0334 — A type name in value position gives way to a variant of that name; a type that is no variant is rejected

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, D-0266: a tool defect met while writing the book's chapter 15)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0266; `spec/17` `[Resolve-Unqualified]` (clauses 1–4b, `CHG-0033`, `CHG-0041`)
Affects: `spec/17` (2.17.0); `impl/src/modres.rs`; `spec/conformance.md` (3.244.0); `spec/02` §5 (1.0.115)

## What changed

- **The rule.** `[Resolve-Unqualified]` says that in value position a struct or enum that the item clauses
  (1)–(3) yield is passed over, and the variant clauses (4), (4b) are tried for a variant of that name. A
  type that is no variant in value position is `diag.type-mismatch`, and so is a qualified path to a type,
  unless it is `m::V` with `m` a module declaring an enum with a variant `V`: that variant, as before
  (`conf.qualified-variant-clash`).
- **The defect.** `std` exports the struct `File` and its own `fs.cb` writes `Ok(File(len))` for
  `PathKind::File`; a program's `enum E { File(i32), Other }` beside `import std;` writes `File(3)`. The
  item clauses take both to the struct `std::File`. The resolver rewrote the path to it, and the type
  checker, asked for the enum of a path whose prefix is a module, looked the last segment up among that
  module's enums' variants: `PathKind::File`. Right by luck in `std`, wrong in the program ("`make`
  returns `E`, but its body gives `PathKind`", or a `PathKind` value where nothing fixed the type).
- **The repair.** `modres::walk_expr_untagged`: a single-segment path that resolves to a struct or an enum
  type (a variant also resolves as `Kind::Enum`, under its own key, so the enum-key test tells them
  apart) is resolved again by `resolve_variant`, the factored clauses (4), (4b); what it yields is the
  path's meaning. A type that no variant replaces, or a qualified path to a type, is rejected:
  "`Vec` names the struct `std::Vec` here, which is a type, not a value", with "; for the variant, write
  `E::V`" when some enum has a variant of that name.

## What changed for programs

A program whose own enum has a variant named as an exported type of an imported module (`File`,
`Json`, `Command`, …) can now write the variant bare, as the rule's prose already promised for a program's
own names. A program that named a type where a value is expected was rejected before with a message about
another enum, or ran with another enum's value; it is rejected now naming the type. No program the old
lookup accepted changes meaning. `std` is unchanged.
