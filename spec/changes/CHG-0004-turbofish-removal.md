# CHG-0004 — Turbofish Removal (Uniform Bare Generic-Instantiation Syntax)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.3.0's predecessor required Rust's `::<...>` ("turbofish")
for explicit generic instantiation of anything that wasn't a prelude
intrinsic, while intrinsics got a bare `<...>` exception
(`widen<u64>(x)` but `apply::<i32, i32>(f, x)`). The `::` prefix is
specifically a Rust invention — C++ templates never need it
(`foo<int>(x)`, `std::vector<int>`) — and the split between intrinsics
and everything else was an arbitrary carve-out, not a semantic
distinction. The human owner directed removing `::` entirely, making
the bare form uniform.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed — `rule.fn.generic-call`'s inference rules
(`[Generic-Call-Explicit]`, `[Generic-Call-Inferred]`, `[Generic-Call-
Expected-Type]`, `[Generic-Call-Uninferable]`) are unaffected; this is
purely which token sequence names an explicit instantiation. Touched:
`spec/22-surface-syntax.md` (2.3.0, `type-args` grammar and
disambiguation (1)); `spec/17-modules.md` (1.2.0, one illustrative
`p::<σ1,…>`); `spec/registry/diagnostics.md` (1.3.0,
`diag.cannot-infer-type-parameter`'s repair text, which also had a
stray reference to the retired `let` keyword that `CHG-0001` missed —
fixed in the same pass since it was in the line being edited anyway).
`spec/decisions/D-0013-expected-type-generic-inference.md` and
`D-0014-expected-type-field-propagation.md` both illustrate this area
with `Vec::<i32>::new()` and are unaffected — immutable once
`ACCEPTED` per `spec/02-schema.md` §6.

## Previous semantics (concrete syntax; no rule semantics involved)

`type-args ::= '::' '<' type (',' type)* '>'`, with a stated exception
letting an intrinsic name drop the `::`. Ordinary generic functions,
methods, and types required it: `apply::<i32, i32>(f, x)`,
`Vec::<i32>::new()`.

## New semantics (concrete syntax; no rule semantics involved)

`type-args ::= '<' type (',' type)* '>'` — no `::`, ever, for anyone.
`apply<i32, i32>(f, x)`, `Vec<i32>::new()`. Disambiguation (1) is
restated to justify this unconditionally: comparison's non-
associativity (`compare` admits at most one comparison operator) means
`identifier '<' type (',' type)* '>'` can never be mistaken for a
comparison regardless of what follows the closing `>` — a call, a
further path segment, or nothing at all. This was already true for
the intrinsic case in 2.2.0; 2.3.0's wording generalizes the stated
reasoning rather than changing it.

## Affected invariants

None. Generic instantiation's soundness (`rule.type.kind`,
`rule.fn.generic-call`) never depended on which token sequence spelled
an explicit instantiation.

## Dependency impact

None. No `Depends on`/`Affects` line changes anywhere.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`–
`CHG-0003`: every `::<...>` in existing CobaltC source must drop the
`::`; no program's meaning changes.

## Migration implications

Mechanical and total: delete every `::` immediately preceding a `<`
that opens a type-argument list. No new fact is introduced (unlike
`CHG-0002`'s capture-list check) — this is a pure token deletion, sound
for every program accepted under 2.2.0's grammar.

## Example changes

None required. `spec/examples.md` and `spec/conformance.md` never used
the `::<...>` form — `conf.generic-fn-explicit`'s `id<i32>(5)` was
already written bare, for a non-intrinsic, ahead of this record
formalizing that as the only form. Verified by corpus-wide search
before making this change.

## Conformance changes

None required, for the same reason.

## Future implementation implications

A parser needing to resolve `identifier '<' ... '>'` no longer has a
`::`-prefix cue for "this is definitely a generic instantiation" — it
never actually needed one, since disambiguation (1)'s reasoning
(non-associative comparison) is what did the real work even in 2.2.0.
Removing `::` removes a redundant signal, not the mechanism that made
the grammar unambiguous.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`–`CHG-0003`'s
redesign.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
