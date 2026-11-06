# D-0045 — `Box<T>`, and Recursive Types Without Indirection

Status: ACCEPTED (2026-09-26, owner-approved)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17, §23
Depends on: D-0010, rule.stdlib.rc, rule.agg.layout
Affects: rule.stdlib.box (new), rule.agg.layout (`[Type-Recursive]`), diag.recursive-type (new)

## Problem

1. A struct or enum that contains itself by value
   (`struct Node { i32 v; Option<Node> next; }`) has no finite size.
   Nothing rejected it: both implementations overflowed their own
   stacks computing its layout.
2. A recursive type needs indirection, and `std` offered it only
   shared and read-only (`Rc<T>`), as a whole collection (`Vec<T>`), or
   unsafe (`rawptr<T>`). A list or tree whose nodes each own the next —
   a single, mutable owner on the heap — had no safe spelling; the
   showcases used arenas indexed by `usize` instead.

## Constraints

No new tokens, no new language mechanism; `std` written in CobaltC.

## Selected design

- **`[Type-Recursive]`** (`spec/16` §1): a struct or enum that contains
  itself by value — through fields, payloads, array elements or a
  `mutex`, with type arguments substituted — is ill-formed,
  `diag.recursive-type` (static), whose repair names `Box`, `Rc` and
  `Vec`. A `ref`, `rawptr`, `fn`, `handle` or `guard` holds nothing
  inline and ends the search; a generic chain that only grows is
  refused at 64 levels.
- **`Box<T>`** in `std` (`spec/21` §3a): `Box::new`, `get`, `get_mut`,
  `into_inner`, and a destructor; one owner, on the heap, built on
  `rawptr<T>` like `Rc`.

## Rejected alternatives

- **Unsized types or `sizeof` of a recursive type as a fault:** a type
  with no size is a static fact; the program can be rejected.
- **No `Box`, arenas only:** safe, but an index is not typed as "a node
  that exists", and every tree needs its own arena.

## Semantic rationale

`Box<T>` is ordinary CobaltC over `allocate`, `reclaim` and
`deallocate`, as `Rc` is; its `full` flag lets `into_inner` move the
value out and leave the destructor only the memory to free.

## Usability

    struct Node
    {
        i32 v;
        Option<Box<Node>> next;
    }

## Explainability

"A type cannot contain itself; put the recursive part in a `Box`."

## Implementation-feasibility

`typecheck::recursive_type`, run first in `check_program`, before any
layout is computed; `Box` in the prelude.

## Compatibility impact

Extension; the programs now rejected crashed before.

## Prior-art status

Rust `Box<T>` and E0072 ("recursive type has infinite size", which
suggests `Box`); C++ `std::unique_ptr<T>`.

## Invariant traceability

`inv.resource-authority`: a `Box` is a resource with one owner; its
value is destroyed exactly once, by the destructor or by whoever
`into_inner` gave it to.

## Revisit conditions

- Matching through a reference (`match (*b)` on an enum with a
  resource payload is `diag.move-out-of-field`): what makes `Box`
  trees pleasant to walk; a separate decision.
- Deep recursive structures destroyed recursively (a very long `Box`
  list) could exhaust the stack.
