# D-0055 — An item of an intrinsic's name hides it only where it is found directly

Status: ACCEPTED (2026-09-26, owner-delegated: "fix them both, using your best judgment")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: CHG-0036, D-0021, D-0024, rule.module.resolve
Affects: rule.module.resolve (`[Resolve-Unqualified]` clause (3))

## Problem

CHG-0036 lets a program declare an item named as an intrinsic (its own
`fn widen`), and the item wins wherever `[Resolve-Unqualified]` finds
it. Clause (3) finds it from every module nested in the one that
declares it, and `std` is a module at the program's root. So a program
declaring `fn reinterpret`, `fn sizeof`, `fn allocate`, … at its root
changed what `std`'s own code called: `Vec::push` then called the
program's function and the program failed with `diag.type-mismatch` at
no location. A file-backed library module (D-0021) had the same
exposure: what its `sizeof` meant depended on the program loading it.
D-0054 first met it (`conf.item-shadows-intrinsic` failed once `std`
used `widen`).

## Constraints

- CHG-0036 stands: a program may name its own items as it likes, and
  its own code calls them.
- D-0021 stands: a file-backed module means exactly its inline form.
- No new tokens.

## Candidate mechanisms

1. **Clause (3) does not apply to an intrinsic's name.** An item hides
   an intrinsic in its own module (clause 1) and where an `import`
   names it (clause 2); code elsewhere reaches the intrinsic.
   **Selected.**
2. **`std` has no enclosing module for resolution.** Fixes `std` only.
3. **A file-backed module has no enclosing module for resolution.**
   Fixes libraries, but gives the file form a meaning its inline form
   does not have (against D-0021), and cuts a library off from every
   enclosing item, not only intrinsics' names.
4. **Qualified intrinsics** (`core::sizeof`) that `std` writes.
5. **Reserve intrinsics' names** (reverting CHG-0036).
6. **Document only.**

## Selected design

Candidate 1. One clause of `[Resolve-Unqualified]` gains one
condition. It fixes `std` and every library, file-backed or inline,
alike, and 2 is then unnecessary: every other name `std`'s code uses
is its own and found by clause (1) or (4) before clause (3).

The implementation needed a second part: a root item's key was its
bare name, the same string as the intrinsic's, so `std`'s unresolved
`reinterpret` still looked the root item up. A root item named as an
intrinsic is now keyed `name$` (`modres::qualify`), which no source
name can spell; its own module's references resolve to that key.

## Rejected alternatives

- **2:** leaves libraries exposed.
- **3:** breaks D-0021's equivalence, and removes more than the hazard.
- **4:** a namespace and a spelling to learn for a problem the
  resolution rule causes.
- **5:** takes names from programs; CHG-0036 decided against it.
- **6:** a silent trap with an error at no location.

## Consequence

A nested module can no longer call a root-level item named as an
intrinsic unqualified (a root item cannot be imported or qualified):
it gets the intrinsic. A program that wants such a function shared
declares it in a module and imports it (`import util::sizeof;`).
`conf.item-intrinsic-nested-rejected` records the new outcome.

## Semantic rationale

Clause (3) exists so nested code sees its surroundings' items; an
intrinsic is visible everywhere already. Carrying an item that hides
one outward lets a declaration change the meaning of code it does not
contain, `std`'s included, which no other clause does.

## Usability

    fn sizeof(i32 a) : i32 { a + 2 }       // the program's own

    module m
    {
        export fn cells() : usize { sizeof<u64>() }   // the intrinsic: 8
    }

    fn main()
    {
        i32 x = sizeof(0);                  // the program's: 2
        Vec<i32> v = Vec::new();            // std's code: the intrinsic
    }

## Explainability

"Your own `sizeof` is yours in the module that declares it; everywhere
else `sizeof` is the language's."

## Implementation-feasibility

`modres.rs`: clause (3) skips an intrinsic's name (`is_intrinsic_name`,
over `typecheck::INTRINSIC_NAMES`), and `qualify` keys a root item of
such a name `name$`. No other file changes; `build_items`, the
duplicate check and constants all key through `qualify`.

## Compatibility impact

Source-breaking for a program whose nested module called a root-level
item named as an intrinsic; no such program is in the repository.
Every program that declared one at its root and used `std` was broken
before and now works.

## Prior-art status

- **Rust:** the prelude and built-in macros are shadowed by a module's
  own items, per module; a parent's item is not visible in a child
  module without `use super::…`.
- **C:** a program may not define a function named as a standard
  library function it also uses; the library is compiled separately,
  so the program's definition never reaches its code.
- **Zig:** builtins are `@`-prefixed and cannot be shadowed.

## Invariant traceability

None changed; this is name resolution.

## Revisit conditions

- Importing a root-level item (`import sizeof;` is unbound today), if
  sharing such a function becomes a need.
