# D-0133 — A local holding references ends after its last use

Status: ACCEPTED (2026-10-02, owner-delegated: "I leave all these matters to you to solve")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0111 (a local reference binding ends after its last use), D-0018, D-0019
Revisits: D-0111's revisit condition ("Bindings of other types that hold references … could follow if programs ask for it")
Affects: `spec/14` §1 `[Ref-Binding-Last-Use]` (1.21.0), `spec/conformance.md`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## Problem

D-0111 ended a local `ref`, `slice` or `StringView` binding after its
last use. A local of another type that holds a reference still held it
to its block's end, and the commonest such local is a map lookup's
result:

    Option<ref<u32, shared>> hit = HashMap::get(&m, &k);
    printf("%v\n", Option::is_some(&hit));
    *HashMap::entry(&mut m, k2, 0) += 1;     // diag.aliasing-conflict: `hit` still borrows `m`

was rejected although `hit` was never used again; the fix was a `{ }`
around the lookup. The friction appeared as soon as `HashMap` code was
written against the native map operations (2026-10-02).

## Candidate mechanisms

1. **Keep D-0111's three types** and teach the `{ }` idiom.
2. **Extend `[Ref-Binding-Last-Use]` to every local whose type holds a
   reference and nothing that must be destroyed**: `Option<ref<…>>`,
   a struct or enum of references, an array of them. Selected.

## Decision

`[Ref-Binding-Last-Use]` applies to a local of any type that holds a
reference (`ref`, `slice`, `StringView`, or an aggregate, option or
array containing one) and holds no resource and no `fn` value, under
D-0111's conditions unchanged (declared once by a `let` of the block,
not in the trailing expression, not captured, nothing borrowed through
it, no reference-holding binding sharing a statement with it used
later). Its object ends after its last use, and with it the paths it
holds.

Nothing is destroyed earlier: a type that owns a resource or a `fn`
value keeps lexical holding, as before. Ending a plain value that holds
references has no effect a program can see except the paths it no
longer holds.

## Compatibility

Additive: programs rejected only because such a local lived to its
block's end are accepted; every accepted program keeps its output.
