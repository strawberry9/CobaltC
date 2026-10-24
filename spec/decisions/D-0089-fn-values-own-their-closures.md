# D-0089 — A `fn` value owns its closure: copied, or moved when it owns a resource

Status: ACCEPTED (2026-09-28, the owner's decision on the fourth stress round's findings)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0018, D-0073, rule.fn.closure, rule.type.fn
Affects: `spec/15` §6, `spec/registry/diagnostics.md`, the implementations

## Problem

A closure used where a `fn(…) : R` is expected becomes a `fn` value
(`[Callable-Closure]`). A `fn` type is plain: nothing in it says whether
the closure behind it owns a resource (`move [s]`, `s` a `String`) or
holds references. The tools disagreed on what copying and ending such
a value does, and neither did what a struct holding the captures would:

- coby let `g = f` share one closure between two `fn` values (a closure
  counting in a captured value counted for both); cobc copied it, and
  faulted `diag.read-of-resource` when it owned a resource;
- a closure held in a struct's `fn` field was never destroyed, in either
  tool, nor what it owned;
- coby destroyed a closure pushed into a `Vec<fn…>` at the push, cobc
  faulted there;
- coby's copy in `auto b = a;` of a borrowing closure had no owner: its
  references never ended, and a later read of the captured variable
  faulted `diag.aliasing-conflict`.

## Candidate mechanisms

1. **Copy is deep; a closure that owns a resource is not copied**
   (`diag.read-of-resource`). The first draft. It made such a value
   unusable in generic code: `Vec::push` copies its `T x` parameter into
   the buffer, and `T` is a plain `fn` type, so a callback that owned a
   `String` could not be put in a `Vec`.
2. **Every `fn` value is move-only.** Static and uniform, but a `fn`
   item or a capture-free closure could no longer be passed twice
   (`spawn(worker, a, f); spawn(worker, b, f)`).
3. **A closure that owns a resource cannot become a `fn` value.** Static,
   but callbacks owning their data would have no nameable type to be
   stored under.
4. **Copy when the closure owns nothing; move when it owns a resource.**
   A read by value copies the closure, or moves it and leaves the source
   `fn` value empty; a call of an empty `fn` value is
   `diag.stale-binding`. Every holder ends the closure it holds.
   Selected.

## Selected design

- `[Fn-Value-Read]`: a place holding a `fn` value (directly, or in a
  field, element or payload) read by value gives a copy of the closure
  (captures copied as a struct is), unless the closure owns a resource:
  then the closure moves to the result and the place's `fn` value is
  left empty. A read through a reference moves nothing out: a call
  through `*r`, `t.f` or `v[i]` calls the value in place.
- `[Fn-Value-Empty-Call]`: calling an empty `fn` value is
  `diag.stale-binding` (dynamic).
- A `fn` value owns its closure: the closure, captures included, is
  destroyed once, when the value ends (its binding's block exit, the
  destruction of what holds it, `overwrite`, `drop`, a discarded
  temporary).

Found with it (coby): a `move` closure's body changed a per-call copy of
its captures, so a closure counting in a captured value started over at
every call; the body's writes now reach the closure's own fields
(spec/15 `[Closure-Call]`'s `self.f_i`).

## Compatibility impact

coby: two `fn` values no longer share a closure; a copied value's state
is its own. Both tools: closures in fields, elements and discarded
temporaries are destroyed; a read of a resource-owning `fn` value moves
it. No program in the repository relied on the old behavior.

## Revisit conditions

If `fn` types ever say what their closures own, the move could be found
statically.
