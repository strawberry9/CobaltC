# D-0136 — `std` in submodules, still reached by `import std;`

Status: ACCEPTED (2026-10-03, the owner: "proceed with implementing private/std-modules-proposal.md using your recommendations")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 3, semantic consistency), §23
Depends on: D-0017 (modules), D-0021 (file-backed modules), D-0024 (`std` as a module, `import std;`), D-0055 (intrinsic names), D-0091, D-0101 (the floating-point functions)
Affects: `spec/17` §1–§3, `spec/22` §3, `spec/21` §0 and every section head, §3d (new), `spec/06` (`rule.arith.float-fns` moved), `spec/conformance.md`, the guide, both implementations

## Problem

The owner's suggestion, after D's `import std;`: divide the standard
library into modules by subject, while a program still writes the one
line `import std;`. `std` was one flat module, its source one 3 357-line
string. Splitting it into `std::text`, `std::io`, … would have made
`import std;` bring in only the submodule names, breaking every
program: CobaltC had no way for a module to pass another's names on as
its own.

## Candidate mechanisms

1. **Re-export, `export import p;`**, with `std` divided by subject and
   its root re-exporting each submodule. One general rule, usable by any
   library; no new token (C++20's spelling; D's `public import`, Rust's
   `pub use`). Selected.
2. **Split only the source files**, one module still. No language
   change, but no submodules for programs or for user libraries.
3. **`import std;` alone reaching into submodules.** An exception for
   one module, which D-0024 removed once.
4. **Every module import recursive.** Changes every existing `import`
   of a user module, and a library could not keep a submodule to itself.

## Selected design

- **Re-export** (`spec/17` §3, `spec/22` §3): `import-decl ::= vis
  'import' path ';'`. `export import p;` in `M` does what `import p;`
  does and adds to `M`'s items an *alias* for each exported item of the
  module `p` names (its aliases included, so re-exports chain), or for
  the one item `p` names. An alias denotes the item itself and is
  exported. A name `M` already has for a different item is
  `[Item-Duplicate]` at the `export import`; the same item brought in
  twice, or by a cycle, is accepted (proposal Q4).
- **`std` by subject** (`spec/21` §0): `std::core`, `std::collections`,
  `std::text`, `std::io`, `std::sys`, `std::memory`, `std::sync`,
  `std::random`, `std::math`, each re-exported by `std`'s root, which
  keeps the primitives private to `std`. Divided by type, since an
  associated function lives with its type. `import std;`, `std::Vec`,
  `std::printf` and every other path mean what they meant.
- **The floating-point functions move to `std::math`** (proposal §6a,
  Q7 (a)): `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`,
  `log2`, `log10`, `sin`, `cos`, `tan`, `atan2`, `powf` stop being
  intrinsics. They are typed as before (`T` is `f32` or `f64`), which no
  bound can say, so they are realized natively as `printf` is.
  `rule.arith.float-fns` keeps its id and moves to `spec/21` §3d. The
  bit functions stay intrinsics (Q8 (a)).
- **Messages** name an item by its shortest path (`std::Vec`).

## Resolved during implementation

- **`std`'s submodules share one privacy.** `File::read` and the
  directory functions (`std::io`, `std::sys`) fill a `Vec<u8>`'s spare
  room and hand a `StringView`'s bytes to the system, which needs
  fields and functions `std::collections` and `std::text` keep private.
  Under `rule.module.visibility` sibling modules share nothing private,
  so these would have needed new exported functions. Instead `std`'s
  submodules share one privacy, as the one module did (`spec/21` §0). No
  program is inside `std`, so no program can tell; a general mechanism
  (a crate-wide visibility, Rust's `pub(crate)`) is not proposed.
- **A module import brings no submodule.** `[Resolve-Unqualified]`
  (2b) runs before (3), so once `std` had submodules, `import std;` in
  a nested module made `std::memory` hide a program's own root module
  `memory` (the showcase `tinyos`). Clause (2b) now brings an imported
  module's exported items other than its submodules, which are reached
  by path or by an import of their own, as D's packages are. A
  library's submodule names therefore never collide with a program's.
- **Internal names** (proposal Q3): the implementations key each `std`
  item by its short path (`std::Vec::push`), Q3's option (a). Option (b),
  the declared paths, would have changed about 400 names written in
  the implementations, and in the C that `cobc` emits, for no
  difference a program can see.

## Compatibility impact

Breaking (source) only for a program that called a floating-point
function without `import std;`: it now needs the import
(`diag.unbound-name` names the repair). None in the repository did.
A program's own item named `round`, `exp`, … is now an ordinary item.
Everything else is additive: `std::collections::Vec`, `import
std::text;`, and re-export for any library.

## Revisit conditions

A `float` bound (proposal Q7 (b)) if programs want float-generic code of
their own; the floating-point functions would then be written as
ordinary CobaltC signatures.
