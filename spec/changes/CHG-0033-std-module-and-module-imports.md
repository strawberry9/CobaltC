# CHG-0033 — The Module `std`, Module Imports, and `[Assoc-Fn-Foreign-Type]`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-23, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0024, rule.module.resolve, rule.module.use, rule.module.visibility, rule.stdlib.prelude, CHG-0032
Affects: rule.stdlib.prelude, rule.module.resolve, rule.module.use, state.items, diag.foreign-associated-fn, diag.duplicate-item, diag.ambiguous-name, diag.unbound-name, conf.string-into-bytes-roundtrip, conf.duplicate-prelude-fn-rejected (removed), conf.prelude-type-new-assoc-fn-ok (removed), the §14 cases listed below, ex.e2e-* programs

## Problem / motivation

D-0024 has the details. In short, the prelude's items shared the root
module, so their private fields were writable from safe code, the root
could add functions to library types, and `CHG-0032` had to reserve
library names.

## Decision

D-0024: the standard library is the root-level module `std`. `import m;`
of any module brings that module's exported items and exported enums'
variants, and a module's own declarations take precedence. `fn T::name`
outside `T`'s module is rejected.

## What changed

- **`spec/21` 3.0.0:** the module `std` paragraph (§0 purpose);
  "Prelude types" becomes "Types in `std`"; `write` and `Utf8Error`'s
  `offset` are `export`; `map_err` is an exported function of `std`.
- **`spec/17` 2.0.0:**
  - `[Resolve-Unqualified]` clause (2b), and clause (4) extended to
    imported enums;
  - `[Resolve-Ambiguous]` over clauses 2, 2b and 4;
  - `[Assoc-Fn-Foreign-Type]`;
  - `rule.module.use` for module imports;
  - `[Item-Duplicate]`'s paragraph no longer treats the library as part
    of the root.
- **`spec/04` 1.3.2:** `state.items` note (non-normative).
- **`spec/registry/diagnostics.md` 1.8.0:** `diag.foreign-associated-fn`,
  plus text changes to three entries.
- **`spec/conformance.md` 3.18.0:**
  - fragments are assembled after `import std;`;
  - two `CHG-0032` rows removed, eleven added;
  - `conf.string-into-bytes-roundtrip`'s fragment no longer builds a
    `String` literal.
- **`spec/examples.md`:** the nine `ex.e2e-*` programs that use the
  library begin with `import std;` (non-normative text of normative
  examples; no derivation changes).

## Affected entities

`rule.stdlib.prelude`, `rule.module.resolve`, `rule.module.use` (meaning
extended); `diag.foreign-associated-fn` (new).

## Previous semantics

- The prelude's declarations were root-module items, in scope
  unqualified everywhere, and their private fields were visible to
  every module.
- `import p;` of a module made only `p` resolve.
- `fn T::name` for a `T` declared elsewhere was not addressed by any
  rule.

## New semantics

    [Resolve-Unqualified] (2b)  M''::name, exported, for a module M'' that an `import p;` in M names
    [Resolve-Unqualified] (4)   E::name for exactly one enum E declared in M or an enclosing module,
                                named by an import in M, or exported by a module an import in M names
    [Assoc-Fn-Foreign-Type]     disposition: rejected   `fn T::name` in M, T not a struct/enum declared in M   ill-formed; diag.foreign-associated-fn

The standard library is `export module std { … }` at the root.

## Affected invariants

`inv.spatial-validity`, `inv.initialization-validity`: restored for the
library types in safe code (D-0024).

## Dependency impact

`rule.stdlib.prelude` now depends on `rule.module.resolve` (its names
are `std::`-qualified) and `rule.module.use` (how programs reach them).

## Compatibility classification

Breaking.
- **Needs `import std;`:** every program that uses the library
  unqualified, in each module that does. Qualified `std::…` paths need
  no import.
- **Now rejected:** programs that wrote library-private fields or
  declared another module's associated functions.
- **Newly accepted:** a resource struct declared inside a module can now
  have a destructor. Before, both implementations rejected it
  (`conf.module-struct-destructor-runs`).

## Migration implications

Add `import std;` to each module scope that uses a library name
unqualified. In this repository that means:

| Files | Count |
|---|---|
| conformance, example and showcase files | 106 |
| guide examples | 39 |
| programs in `spec/examples.md` | 9 |

The test harnesses begin each inline program and table fragment with
`import std;`. A program that built a `String` literal
(`String { .bytes = … }`) uses `String::from_str` or
`String::from_utf8` instead.

## Example changes

The nine `ex.e2e-*` programs above.

## Conformance changes

**Removed:**
- `conf.duplicate-prelude-fn-rejected` (now `[Assoc-Fn-Foreign-Type]`,
  covered by `conf.foreign-assoc-fn-rejected`);
- `conf.prelude-type-new-assoc-fn-ok` (now rejected).

**Added:**
- `conf.foreign-assoc-fn-rejected`
- `conf.std-private-field-rejected`
- `conf.std-not-imported-unbound`
- `conf.std-qualified-ok`
- `conf.std-own-declaration-wins-ok`
- `conf.std-import-per-module`
- `conf.module-import-exports-ok`
- `conf.module-import-private-unbound`
- `conf.module-import-ambiguous-rejected`
- `conf.module-import-unused-clash-ok`
- `conf.module-struct-destructor-runs`

## Future implementation implications

- **`std` has no special status:** it is an ordinary module, and name
  resolution must not treat it specially.
- **`map_err` is native.** Both implementations keep a native
  `std::map_err` (`spec/21` §0's latitude). Their resolver enters it as
  `std`'s exported item, since it has no parsed body.
- **Resolve imports up front.** Imports should be resolved once, before
  names are looked up through them. Resolving an import through the
  module's imports would find the import itself; a single-segment
  `import std;` in a nested module did exactly that and overflowed the
  stack before this change fixed it.

## Prior-art status

See D-0024.

## Revisit conditions

See D-0024.
