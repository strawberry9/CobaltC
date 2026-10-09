# D-0069 — Function-local types

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0024, D-0066
Affects: rule.module.local-type (new), `spec/22` `statement`

## Problem

Every `struct` and `enum` had to be declared at a module's top level,
even a helper used by one function (a sort key, a parser's state). It
was then visible to the whole module and read far from its use. No
decision had excluded local types; the grammar simply had no place for
them.

## Constraints

- No new tokens; one set of rules for a type wherever it is declared.
- A type has one definition.

## Candidate mechanisms

1. **`struct` and `enum` as statements, with `fn T::name` for them in
   the same block**, scoped like a local constant. **Selected.**
2. Local types without functions of their own.
3. Local functions as well.

## Selected design

Candidate 1. A local type is a module type in all but the scope of its
name. Its declaration and functions see module items and the local
types and constants in scope, not the enclosing function's variables or
type parameters. It cannot be exported.

## Rejected alternatives

- **2:** a resource type needs its destructor, and a type its
  functions, where the type is.
- **3:** closures already are CobaltC's local functions; a second form
  with different capture rules would be a second concept.

## Semantic rationale

As for local constants (D-0066): only the scope of the name differs
from a module item; everything else is the module item's.

## Usability

    fn shortest(ref<Vec<String>, shared> words) : usize
    {
        struct Entry
        {
            usize len;
            usize at;
        }
        …
    }

## Implementation-feasibility

The parser, as for local constants: each local type becomes a module
item `T$k`, its uses (types, struct literals, `T::f`, `T::V`, patterns,
destructuring) renamed within its scope; a function of it is hoisted
with it, parsed with the enclosing function's variables out of scope.

## Compatibility impact

Extension.

## Prior-art status

- **Rust:** items in function bodies, including `impl` blocks; they do
  not see the function's locals or generics.
- **C:** a `struct` declared in a block.
- **C++:** local classes.

## Revisit conditions

- A local type that depends on the function's type parameters.
