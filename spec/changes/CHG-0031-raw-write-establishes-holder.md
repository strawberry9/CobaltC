# CHG-0031 — A Raw Write of Reference-Bearing Data Establishes Its Holder

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-23, owner-directed)
Governed by: `CobaltC_Master_Instructions.md` §17, §20, §21
Depends on: rule.trust.rawptr, rule.value-object.write, rule.control.stmt, D-0018, D-0019, CHG-0030
Affects: rule.trust.rawptr (`[Rawptr-Write]`), conf.vec-of-refs-usable, conf.vec-holds-exclusive-ref-conflict, conf.vec-pop-releases-ref

## Problem / motivation

A `Vec` whose element type holds a reference could not be used. For a
plain `T`, `Vec::push` stores the element with `[Rawptr-Write]`, which
records the written references as held "with `reclaimed-at(addrs(p,τ))`
as the holder if it exists" — and on a fresh `push` no reclaimed object
covers the element's cells. After the call the references were
therefore held by nothing; `[Stmt-Exit]` (D-0019) invalidated them as
`Unheld`, and any later use of a stored reference was
`diag.stale-binding`:

    i32 a = 10;
    Vec<ref<i32, shared>> v = Vec::new();
    Vec::push(&mut v, &a);
    **Vec::index_shared(&v, 0)          -- diag.stale-binding: the element's reference died with the push

The resource case already worked: `[Rawptr-Move-In]` establishes a
reclaimed object `o'` over the cells and re-keys held-by to it. Found
while fixing references in raw storage in both implementations
(`impl/STATUS.md`), where no case exercised a `Vec` of references; the
implementations had diverged (`cobc` counted the element's references as
held, `coby` neither held nor invalidated them).

## Decision

`[Rawptr-Write]` of a value that holds references re-attaches the
reclaimed object over the written cells, or establishes one there (as
`[Reclaim]` would: `storage-kind = reclaimed`, `init = valid`, no
authority or obligation for a plain `τ`), and that object is the holder
of the written references — the same shape `[Rawptr-Move-In]` already
has. A value holding no reference is unchanged: no object is needed.

## What changed

**`spec/20` 1.8.0** (§2): `[Rawptr-Write]`'s effect. **`spec/conformance.md`
3.16.0** (§7): three rows.

## Affected entities

`rule.trust.rawptr` `[Rawptr-Write]` (its effect on `objects` and
`held-by`; no premise, side-condition or disposition changes).

## Previous semantics

    held-by updated for refs-in(v) … with reclaimed-at(addrs(p,τ)) as the holder if it exists

## New semantics

    if refs-in(v) ≠ ∅: o' = reclaimed-at(addrs(p,τ), τ, Σ) if it exists, else a fresh identity with
        objects(o') := { extent: addrs(p,τ), type: τ, storage-kind: reclaimed, temp-scope: current-scope },
        init(o') := valid
    held-by updated for refs-in(v) and the old contents as in rule.value-object.write, with o' the holder

Consequences, all by existing rules: a reference stored in a `Vec`
element stays valid while the element's object lives; it takes part in
`clash` meanwhile (writing `x` while a `Vec` holds `&mut x` is
`diag.aliasing-conflict`, dynamic); `Vec::pop`'s `release` and
`Vec::drop`'s `deallocate` end the object (`[Release]` → `[Object-End]`),
after which the reference is no longer held.

## Affected invariants

`inv.alias-validity`: strengthened in effect — a stored reference is
now visible to `clash` instead of being invalidated. `inv.temporal-
validity`: unchanged in form; fewer references go stale.

## Dependency impact

`spec/21`'s `Vec` is unchanged; its `push`/`index_*`/`pop` now behave
for a reference-bearing `T` as they always did for a resource `T`.

## Compatibility classification

Relaxing for uses (a later use of a stored reference was
`diag.stale-binding`, now valid), tightening for conflicts (an access
conflicting with a stored reference was admitted, now
`diag.aliasing-conflict`, dynamic). No existing conformance case
changes outcome; none stored a reference in raw storage.

## Migration implications

None in the corpus.

## Example changes

None.

## Conformance changes

`conf.vec-of-refs-usable`, `conf.vec-holds-exclusive-ref-conflict`,
`conf.vec-pop-releases-ref` (`spec/conformance.md` §7), with file cases
in `impl/conformance/20-trust-boundaries/`.

## Future implementation implications

An implementation records a written reference as held by the object
over its cells and ends that holding when the object ends (`release`,
`deallocate`, a move out, a destroy). An implementation that tracks
references by cell address (`cobc`'s slot table) must forget a
released range's slots even where no object was ever reclaimed.

## Prior-art status

As for `[Rawptr-Move-In]`: raw storage that holds a value is an object
for the purposes of the rules the value's contents need.

## Revisit conditions

A demonstrated need to store a reference in raw storage *without* it
being held (e.g. an FFI buffer whose bytes merely contain an address) —
that is `rawptr`, not `ref`, and needs no change.
