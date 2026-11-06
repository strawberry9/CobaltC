# D-0266 — A type name in value position gives way to a variant of that name

Status: ACCEPTED (2026-10-18, under the owner's standing delegation of defect repair; a tool defect met while writing the book)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20
Depends on: D-0024 (`import std;`), D-0032 (`CHG-0041`, variant lookup), D-0136
Affects: `spec/17` `[Resolve-Unqualified]` (2.17.0); `impl/src/modres.rs`; `CHG-0334`

## Problem

`[Resolve-Unqualified]` tries the item clauses (1)–(3) before the variant clauses (4), (4b). `std` exports
a struct `File` and an enum `PathKind` with a variant `File`, and its own `fs.cb` writes `Ok(File(len))`:
by clause 1 that `File` is the struct `std::File`, so the rule as written made `std`'s own body ill-formed.
Both tools accepted it only because the type checker, handed a path to a struct in value position, looked
the last segment up among every enum's variants and found `PathKind::File`. The same lookup took a
program's `enum E { File(i32), … }` with `import std;` and `File(3)` to `std::File` (clause 2b) and then,
silently, to `PathKind::File`: "`make` returns `E`, but its body gives `PathKind`", or a `PathKind` value
where nothing fixed the type. Found by the book's chapter 15, whose error enum wanted a variant `File`.

## Options

1. **Reject a type in value position outright.** True to the rule's letter; makes `std`'s `File(len)` and
   every program variant that shares a name with an exported type (`File`, `Json`, `Command`, `Rng`,
   `Args`, …) unwritable bare. `std` would be edited to `PathKind::File`; programs would qualify.
2. **In value position, pass over a type and try the variant clauses.** A type is not a value, so
   finding one answers nothing; the lookup goes on to (4), (4b). `std` is unchanged, a program's own
   variant stays reachable bare beside `import std;`, and `File` in type position is still `std::File`.
   A type with no variant of its name, or a qualified path to a type, in value position is rejected with
   a message naming the type.
3. **Reorder the clauses: variants before imported items.** Changes what `Empty`, `None` and every
   other bare name means for programs that have an item of that name; too wide for the defect.

## Decision

Option 2. The rule gains one sentence: in value position a struct or enum that clauses (1)–(3) yield is
passed over and (4), (4b) are tried; a type they yield with no such variant is `diag.type-mismatch`, and
so is a qualified path to a type in value position, unless it is `m::V` with `m` a module declaring an enum
with a variant `V`, which is that variant as before (`conf.qualified-variant-clash`). The resolver does this in its expression walk
(`CHG-0334`); the type checker's last-segment lookup is no longer what decides.

## Results

`std` unchanged. `conf.variant-beside-imported-type` (a program's `File(3)` is `E::File` beside
`import std;`), `conf.variant-qualified-beside-imported-type` (`E::File(3)` with `std::File` in use as a
type), `conf.type-named-as-value` (`Vec(3)`: "`Vec` names the struct `std::Vec` here, which is a type,
not a value"). No accepted program changes meaning: every program the old lookup accepted resolved its
bare variant to the one enum of that variant's name, which is what (4), (4b) yield.

## Revisit conditions

If a second enum of the program and an imported one share a variant name that a type also has, (4b)'s
ambiguity reporting applies as before; if that proves common, the message could list the enums.
