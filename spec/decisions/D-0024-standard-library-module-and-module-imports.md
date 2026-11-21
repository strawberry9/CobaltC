# D-0024 — The Standard Library Is the Module `std`, Reached by `import std;`

Status: ACCEPTED (2026-09-23, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0017, D-0021, rule.module.resolve, rule.module.use, rule.module.visibility, rule.stdlib.prelude, CHG-0027, CHG-0032
Affects: rule.stdlib.prelude, rule.module.resolve, rule.module.use, CHG-0032 (its prelude clause), every program that uses the standard library

## Problem

The prelude's declarations were items of the root module. Private
items and fields are visible in their own module and every module
nested in it (`rule.module.visibility`), and every module is nested in
the root. So `Vec`'s private fields were writable from any program, in
safe code:

    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    v.len = 100000;                         // no unsafe
    i32 x = *Vec::index_shared(&v, 99999);  // reads past the buffer

Both implementations ran this to completion: a breach of
`inv.spatial-validity` without `unsafe`. `String`'s bytes (its UTF-8
invariant) and `Rc`'s count were exposed the same way. The same fact let
the root module add functions to prelude types (`fn Vec::first_or`),
and `CHG-0032` had to count prelude names as reserved to stop a program
from redeclaring `print` or `Vec::drop`.

## Constraints

- No new tokens (the owner's standing preference).
- `spec/21` §0: the intrinsics are language operations, not items
  (`CHG-0023`).
- Programs should be able to name their own `print` or `Vec` without
  colliding with the library.

## Candidate mechanisms

1. **`std` as its own module, its exports in scope everywhere.** Closes
   the hole. It keeps the "in scope in every module" exception and needs
   a rule reserving the library's simple names.
2. **`std` as its own module, reached by explicit imports only**
   (`import std::print;` …). No exceptions, but `import` names one item,
   so a program needs one line per name.
3. **`std` as its own module, and `import m;` of any module brings its
   exports.** A program writes `import std;` once. **Selected** by the
   owner, who asked for exactly `import std;`, as a general module
   import, with a module's own declarations taking precedence.

## Selected design

Candidate 3:
- **`std`.** The program's item set contains, at the root,
  `export module std { … }`, whose body is `spec/21`'s declarations.
  It uses no new syntax: it is an ordinary module.
- **Module import.** `import p;` naming a module keeps its existing
  meaning (`p` resolves) and also makes `p`'s exported items, and its
  exported enums' variants, resolve unqualified in the importing
  module (`[Resolve-Unqualified]` clauses 2b and 4).
- **Order:**
  1. the module's own declarations;
  2. single-name imports;
  3. module imports;
  4. enclosing modules.

  A clash between two imported modules is `diag.ambiguous-name`, but
  only where the name is used.
- **Per module.** An import acts in the module that declares it.
- **Qualified paths.** `std::…` works anywhere, with no import.
- **Associated functions.** `fn T::name` outside `T`'s module is now
  rejected (`[Assoc-Fn-Foreign-Type]`), which `spec/17` §1's wording
  implied but no rule stated.
- **Exports.** `write` and `Utf8Error`'s `offset` are exported: both
  are documented for programs' use, and until now were reachable only
  through the hole.

## Rejected alternatives

- **1:** keeps a special case (library names in scope everywhere) and
  needs reserved names. The owner preferred explicit use.
- **2:** one import line per name, since there is no glob form.
- **Clash with a library name as an error**, instead of the module's own
  declaration winning: every later addition to `std` could then break
  programs that already use that name.

## Semantic rationale

Moving the library into a module makes its privacy real, using only the
visibility rule that already exists. No special visibility rule for the
library is needed.

## Usability

Every program that uses the library gains one line, `import std;`, and
so does each nested or file-backed module that uses it unqualified.
`Some`, `None`, `Ok` and `Err` come with it. Programs that use nothing
from `std` are unchanged. A missing import is `diag.unbound-name`, whose
repair text now names `import std;`.

## Explainability

Every unqualified name in a module resolves to a declaration in that
module, an import in that module, or an enclosing module. The library
has no special status.

## Implementation-feasibility

Done in both implementations (`CHG-0033`):
- the prelude source is wrapped in `export module std { … }`;
- the resolver resolves every import once and consults the targets for
  clauses 2, 2b and 4;
- the few names the implementations constructed by spelling (`Option`,
  `Result`, `Vec::…`) are now `std::`-qualified.

This also fixed a latent bug in both: a resource struct declared inside
a module could not have a destructor at all. The destructor-signature
check compared the parameter's qualified type with the bare name after
`fn`, and rejected every such destructor as
`diag.bad-destructor-signature`. `coby` also looked destructors up by the
bare struct name. `conf.module-struct-destructor-runs` now covers it.

## Compatibility impact

Breaking. Every program using the library needs `import std;` in each
module that uses it unqualified. Programs that touched library-private
fields, or declared `fn T::name` for another module's `T`, are now
rejected; those were the hole. All in-repository programs are
migrated (`CHG-0033`).

## Prior-art status

- **Rust:** the `std` prelude is imported implicitly, `use m::*` imports
  a module's public items, and local items shadow glob imports.
- **Python:** `from m import *` is the same kind of import, though
  there the later binding wins, whichever it is.
- **C++23:** `import std;` imports the standard library module.

This decision takes C++'s spelling (CobaltC's module vocabulary is
C++20's, `CHG-0006`) with Rust's precedence.

## Invariant traceability

`inv.spatial-validity` and `inv.initialization-validity` regain their
guarantee for library types in safe code: a `Vec`'s `len` and `cap` can
be changed only by `std`'s own functions.

## Revisit conditions

- A demonstrated need for an implicit import, for example a scripting
  mode.
- A glob-free, per-name form becoming preferable, for example if large
  libraries make ambiguity at use sites common.
