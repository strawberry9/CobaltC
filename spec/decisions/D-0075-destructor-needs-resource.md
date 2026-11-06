# D-0075 — A type with a destructor is a resource

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §4, §8 (items 1, 10), §12
Depends on: D-0003, D-0008, D-0049
Affects: `spec/07` §1, `spec/registry/diagnostics.md`

## Problem

`struct Noisy { u32 id; }` with `fn Noisy::drop(ref<Noisy, exclusive>
self)` was accepted, and its destructor never ran.

A struct without the `resource` marker, and without resource fields, is
plain (`spec/12` §3). A plain value is copied and never destroyed, so
`[Run-Destructor]` never reaches it. A program that relied on the
destructor (to print, count, or release something) silently lost that
behaviour.

## Candidate mechanisms

1. **Declaring a destructor makes the type a resource.** This is
   implicit: the marker would mean less, and whether `auto b = a;`
   copies or moves would depend on a function declared elsewhere.
2. **A destructor on a type that is not a resource in every
   instantiation is a static error** that says to add `resource`.
   Selected.
3. **Leave it.** The silent loss stays.

## Selected design

Candidate 2: `diag.destructor-on-plain-type` (static), at the
destructor's declaration.

A generic type counts only when it is a resource whatever its type
arguments. A destructor that would run for `Wrap<String>` but not for
`Wrap<i32>` is refused too.

## Compatibility impact

Tightening. Programs with a destructor on a plain type are rejected.
Their destructor never ran, so none of them behaved as written.

## Revisit conditions

None.
