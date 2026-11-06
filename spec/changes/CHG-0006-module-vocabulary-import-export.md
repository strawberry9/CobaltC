# CHG-0006 — Module Vocabulary: `use`/`pub` → `import`/`export`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`CHG-0001` renamed `mod` to `module`, matching C++20's own module
keyword, but left `use` and `pub` as Rust's own words — C++20 modules
use `import` and `export` for exactly those two roles. The result was
a module system mixing two languages' vocabulary rather than
consistently mirroring one. The human owner directed finishing the
rename: `use` → `import`, `pub` → `export`.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed — `rule.module.resolve`,
`rule.module.visibility`, and `rule.module.use`'s resolution order,
visibility rule, and import behavior are unaffected; only the surface
keywords naming them changed. `rule.module.use`'s own id is
deliberately unchanged, matching the precedent `CHG-0001` set for
`rule.init.let` when `let` was removed — an entity id is stable
independent of the surface keyword it currently governs
(`spec/02-schema.md` §1).

Directly touched: `spec/22-surface-syntax.md` (2.5.0: keyword list,
`vis`, `use-decl` renamed `import-decl`, §5's deliberate-absences
note); `spec/17-modules.md` (1.3.0: §1's resolution order, §2's
visibility rule, §3's heading and body, §4's prose reworded to avoid
"export" colliding with its own new keyword meaning);
`spec/21-standard-library-semantics.md` (2.3.0: 21 sites — every
`pub` on the prelude types, `Vec`, `String`, `Rc`, and their
functions); `spec/registry/diagnostics.md` (1.4.0:
`diag.unbound-name`, `diag.ambiguous-name`, `diag.name-not-visible`).

Not touched: `spec/examples.md`, `spec/conformance.md` — verified
before making this change that neither file has ever exercised
`module`/`use`/`pub` directly (no `ex.module-*`/`conf.module-*` case
exists), so there was nothing to cascade there.

## Previous semantics (concrete syntax; no rule semantics involved)

`use-decl ::= 'use' path ';'`; `vis ::= 'pub'?`.

## New semantics (concrete syntax; no rule semantics involved)

`import-decl ::= 'import' path ';'`; `vis ::= 'export'?`. Combined
with `CHG-0001`'s `module`, the module system now consistently spells
`module`/`import`/`export` — one real precedent (C++20 modules)
throughout, rather than `module` from one language and `use`/`pub`
from another.

## Affected invariants

None. Modules introduce no invariant of their own (`spec/17`'s own
Purpose section); visibility and import resolution are purely lexical
facts, unaffected by which keyword spells them.

## Dependency impact

None. No `Depends on`/`Affects` line changes anywhere.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`–
`CHG-0005`: every `use` and `pub` in existing CobaltC source must be
renamed; no program's meaning changes.

## Migration implications

Mechanical and total: replace every `use` with `import` and every
`pub` with `export`. No new fact is introduced — a pure keyword
substitution, sound for every program accepted under 2.4.0's grammar.

## Example changes

None required (see "Affected entities" — never used in either file).

## Conformance changes

None required, for the same reason.

## Future implementation implications

None beyond an ordinary keyword-table update; the resolution and
visibility algorithms themselves (`rule.module.resolve`,
`rule.module.visibility`) are untouched.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`–`CHG-0005`'s
redesign, and completing `CHG-0001`'s own module-vocabulary work
specifically.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
