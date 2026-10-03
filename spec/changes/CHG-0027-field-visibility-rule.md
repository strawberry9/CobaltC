# CHG-0027 — `[Field-Not-Visible]`: Field Visibility Becomes a Rule

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-22)
Governed by: `CobaltC_Master_Instructions.md` §1, §21
Depends on: rule.module.visibility, rule.agg.struct-construct, rule.init.let, term.module, CHG-0026
Affects: rule.module.visibility, diag.name-not-visible, conf.field-private-read-rejected, conf.field-private-literal-rejected, conf.field-private-destructure-rejected, conf.field-export-ok

## Problem / motivation

`spec/22` §3 has carried `field ::= vis type identifier -- field
visibility as item visibility (spec/17 §2)` since 2.0.0, and the
language guide teaches per-field `export`. But `rule.module.visibility`
spoke only of *items*, no rule in `spec/13`/`spec/16` mentioned a
field's visibility, and `diag.name-not-visible` cited only
`[Resolve-Not-Visible]`. The consistency audit that followed
`CHG-0026` found the consequence: the reference interpreter accepted a
private field read and a struct literal naming a private field from
another module, and one new conformance fixture depended on that.

## Decision

The grammar's promise becomes a rule. A field is an item of its struct
for visibility, and every construct that names a field — access,
struct literal, destructuring — is well-formed only where the field is
visible.

## What changed

**`spec/17` 1.5.0** (§2): `rule.module.visibility` gains a paragraph
and `[Field-Not-Visible]` (`disposition: rejected`; `diag.name-not-
visible`), citing `[Field-Access]`, `[Struct-Construct]`, and
`[Let-Destructure]` as the naming sites. **`spec/registry/diagnostics.md`
1.6.0**: `diag.name-not-visible` cites the new label. **`spec/
conformance.md` 3.14.0** (§14): four `conf.field-*` rows.

## Affected entities

`rule.module.visibility` (extended; its item clause is unchanged),
`diag.name-not-visible` (one more emitting rule, same id). Not
touched: `rule.agg.struct-construct`, `[Field-Access]`,
`rule.init.let` — the new rule is a well-formedness premise layered on
them, stated once in `spec/17` rather than three times.

## Previous semantics

A private field was, by the rule text, accessible from anywhere its
struct was visible; only the grammar comment said otherwise.

## New semantics

    [Field-Not-Visible]   disposition: rejected   a field f of S named in M;  ¬visible(M', f, M)   ill-formed; diag.name-not-visible

with `visible` exactly as for items: `export`, or `M` is `S`'s
declaring module or nested inside it.

## Affected invariants

None. Visibility is lexical (`spec/17`'s Purpose, D-0017).

## Dependency impact

`rule.module.visibility` now depends on `rule.agg.struct-construct`
and `rule.init.let` (the naming sites). Nothing else changes.

## Compatibility classification

Source-breaking for programs that named a private field from outside
its module — which the grammar comment and the guide said was not
allowed. Semantics-preserving for every program the rule text and the
grammar comment already agreed on.

## Migration implications

Mark the field `export`, or move the access into the struct's module
(a function there is the usual shape).

## Example changes

`ex.module-file` (`CHG-0029`) writes `export f64 w;` on the fields the
root constructs. D-0021's Usability example does not, and is not
edited (`spec/02` §3); `CHG-0026`'s Example section notes this.

## Conformance changes

`conf.field-private-read-rejected`, `conf.field-private-literal-
rejected`, `conf.field-private-destructure-rejected`,
`conf.field-export-ok` (`spec/conformance.md` §14, file-based). The
`CHG-0026` fixture `fixtures/geometry.cb` now exports its fields.

## Future implementation implications

For `coby`: `typecheck.rs` checks the field's `export` against the
module of the function being checked at the three naming sites (it
already resolves the struct declaration at each); no resolver change.
A compiler does the same wherever it resolves a field.

## Prior-art status

Rust's per-field `pub` has the selected shape; C++'s `private` is
per-class-member with the same "the declaring scope may" rule. No
derivation record: this realizes a promise the grammar already made.

## Revisit conditions

A demonstrated need for a struct literal to be usable outside its
module while some field stays private (a constructor function covers
it today).
