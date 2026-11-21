# CHG-0032 — `[Item-Duplicate]` and `diag.duplicate-item`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-23, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: rule.module.resolve, rule.stdlib.prelude, `spec/04` `Σ.items`, D-0023
Affects: rule.module.resolve, diag.duplicate-item, conf.duplicate-fn-rejected, conf.duplicate-struct-rejected, conf.duplicate-prelude-fn-rejected, conf.duplicate-variant-fn-rejected, conf.duplicate-module-rejected, conf.prelude-type-new-assoc-fn-ok, conf.same-name-other-module-ok

## Problem / motivation

`rule.module.resolve` says `Σ.items` "maps a qualified name … to its
item" and that each declaration "adds" its name. Nothing said what a
second declaration of the same name does. Both implementations let
the later one silently replace the earlier: two `fn f()` ran the
second, two `struct P` were accepted, and a program's own
`fn Vec::drop<T>` or `fn print(str s)` replaced the prelude's. The last
two are not only confusing. A replaced `Vec::drop` never destroys a
`Vec<Vec<i32>>`'s elements, which leaves their destroy obligations
silently undischarged (`inv.resource-authority`). D-0023 noted the gap
("Observed in passing") while making `cobc`'s `Vec` access native.

## Decision

A second declaration of any qualified name is rejected statically,
the prelude's declarations included. The owner chose this scope
(2026-09-23) over rejecting only collisions with the prelude, and over
also closing prelude types to new associated functions.

## What changed

**`spec/17` 1.6.0** (§1): `rule.module.resolve` gains `[Item-Duplicate]`
and a paragraph saying which names it counts.
**`spec/registry/diagnostics.md` 1.7.0**: `diag.duplicate-item`.
**`spec/conformance.md` 3.17.0** (§14): seven cases, listed under
"Conformance changes" below.

## Affected entities

`rule.module.resolve` (one new labelled rejection); `diag.duplicate-item`
(new).

## Previous semantics

Unspecified. Both implementations kept the last declaration.

## New semantics

    [Item-Duplicate]   disposition: rejected   two declarations add the same qualified name   ill-formed; diag.duplicate-item

The names counted, in one namespace, are the ones `spec/17` §1 already
has declarations add:
- `M::name` for a `fn`, `extern fn`, `struct`, `enum` or `module`;
- `M::T::name` for an associated function;
- `M::E::V` for an enum variant.

The prelude's declarations are root-module items like any others. So:
- a program cannot redeclare a prelude item;
- a program may add a new associated function to a prelude type from
  the root module, since that name is new;
- `f` and `m::f` are different names.

Body-less prelude intrinsics (`sizeof`, `spawn`, …) are not `Σ.items`
entries (`spec/04`, `CHG-0023`) and are not covered. Location: the later
declaration, which for a prelude collision is always the program's.

## Affected invariants

None restated. The change closes a way for a program to bypass the
prelude's `Vec::drop`, which is what discharges the elements'
obligations under `inv.resource-authority`.

## Dependency impact

None; `rule.module.resolve`'s `Depends on` is unchanged.

## Compatibility classification

A new static rejection. It rejects only programs where one declaration
was already being silently ignored. No program that declared each
name once changes. Every existing case (spec rows, file suite,
examples, 35 showcases, 90 guide examples) is unaffected.

## Migration implications

Rename one of the two declarations, or move it into a module. A program
that meant to replace a prelude function calls its own function under
a new name.

## Example changes

None.

## Conformance changes

`conf.duplicate-fn-rejected`, `conf.duplicate-struct-rejected`,
`conf.duplicate-prelude-fn-rejected`, `conf.duplicate-variant-fn-rejected`,
`conf.duplicate-module-rejected`, `conf.prelude-type-new-assoc-fn-ok`,
`conf.same-name-other-module-ok` — file-based, under
`impl/conformance/17-modules/`.

## Future implementation implications

The check runs over the parsed program, prelude included, before name
resolution (`coby`: `lib.rs` `check_duplicate_items`, ahead of
`modres::resolve_program`), so no later pass ever sees two items under
one key. Declarations now carry their line for this location. `cobc`
shares the front end.

## Prior-art status

C (within one translation unit), Rust (`E0428`), Go and Java all
reject a name defined twice in one scope. Rust also rejects an inherent
method defined twice (`E0592`) but allows new inherent methods only in
the type's own crate. That mirrors this record: a new associated
function is allowed only in the prelude type's own module, the root.

## Revisit conditions

- A library or separate-compilation mode in which the prelude is not
  part of the program's item set.
- A decision to let programs deliberately replace a prelude function,
  which would need a named mechanism and an argument for
  `inv.resource-authority`, not silent last-wins.
