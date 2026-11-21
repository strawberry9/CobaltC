# CHG-0030 — Re-attaching a Reclaimed Object Takes Its Destroy Authority

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-23, owner-directed)
Governed by: `CobaltC_Master_Instructions.md` §17, §20, §21
Depends on: inv.resource-authority, state.authority, rule.trust.rawptr, rule.resauth.transfer, rule.resauth.destroy, rule.conc.spawn, rule.conc.join, CHG-0015
Affects: rule.trust.rawptr (`[Reclaim]`, `[Rawptr-Move-Out]`), inv.resource-authority, conf.thread-vec-of-resources-dropped, conf.thread-vec-of-resources-returned

## Problem / motivation

A collection of resources could not change threads. `Vec::push` stores
a resource element through `[Rawptr-Move-In]`, which makes the element
a top-level reclaimed object whose destroy authority stays keyed to the
pushing thread. `[Authority-Transfer]` on `spawn`, and `[Join]`'s
re-keying (`CHG-0015`), move authority only for the object passed or
returned — the `Vec` itself; nothing in `Σ` links the element objects
to it, since its buffer is a raw pointer. When the receiving thread
later destroys the `Vec`, `Vec::drop` runs
`drop(reclaim<T>(self.ptr + i))`; `[Reclaim]` re-attached the existing
element object *without* granting authority (only a fresh reclaim did),
so the `drop` was `[Destroy-No-Authority]`:

    fn consume(Vec<Vec<i32>> vv) : usize { Vec::len(&vv) }
    Vec<Vec<i32>> vv = Vec::new(); Vec<i32> inner = Vec::new();
    Vec::push(&mut inner, 1); Vec::push(&mut vv, inner);
    auto h = spawn(consume, vv); join(h)          -- diag.no-destroy-authority in the spawned thread

Returning a `Vec<Vec<i32>>` or `Vec<String>` through `join` faulted the
same way in the joiner. Neither implementation tracked per-thread
authority, so both ran these programs to `ok`; no conformance case
covered the shape. Found while closing `cobc` Stage 2
(`impl/STATUS.md`).

## Decision

Option A of three put to the owner: re-attachment takes the authority.
`[Reclaim]` and `[Rawptr-Move-Out]`, when they re-attach an existing
reclaimed object, re-key its destroy authority to the performing thread,
keeping its `consumed` flag. Rejected: (B) authority follows the
container through `spawn`/`join` — needs an ownership link through raw
pointers, i.e. new `Σ` state; (C) drop the thread dimension of
authority — revisits D-0003's performer model for a gain this record
already obtains.

## What changed

**`spec/20` 1.7.0** (§2): `[Reclaim]`'s re-attachment branch and
`[Rawptr-Move-Out]`'s re-keying state the performer; the "Reclaimed
objects" paragraph says so. **`spec/03`** `inv.resource-authority`:
the re-attachment is listed among the transformations (the proposition
already named "the `unsafe` author who reclaimed it" as the owner).
**`spec/conformance.md` 3.15.0** (§12): two rows.

## Affected entities

`rule.trust.rawptr` (two rules' effect on `authority`; no premise or
disposition changes), `inv.resource-authority` (transformation list).

## Previous semantics

    [Reclaim]   o = reclaimed-at(addrs(p,τ), τ, Σ) if it exists, else a fresh identity with …
                and (if is-resource(τ)) authority(ℓ, destroy, o) := {consumed: false}   -- fresh case only
    [Rawptr-Move-Out]  … obligations/authority/held-by re-keyed from o' to o_new        -- performer unchanged

## New semantics

    [Reclaim]   a fresh identity: as before (authority(ℓ, destroy, o) := {consumed: false});
                a re-attached o, is-resource(τ): authority(ℓ, destroy, o) := authority(ℓ0, destroy, o)
                    where ℓ0 is the thread holding it, and the ℓ0 entry is removed
    [Rawptr-Move-Out]  authority re-keyed from (ℓ0, destroy, o') to (ℓ, destroy, o_new)

`consumed` is carried over, so a destroyed-then-reclaimed object is
still a double destroy (in practice `diag.stale-binding` first: ending
the object invalidated its paths, and a later reclaim of the same
cells is a fresh identity under the trusted side-condition).

## Affected invariants

`inv.resource-authority`: preserved — authority is re-keyed, never
duplicated or dropped, exactly one thread holds it; the reclaiming
author is the owner the proposition already names.

## Dependency impact

None beyond the entities above.

## Compatibility classification

Relaxing: programs that `[Destroy-No-Authority]` rejected when a
reclaimed resource was destroyed by a thread other than the one that
created it are now `ok`. No accepted program changes outcome. Neither
implementation checked per-thread authority, so both already behaved
as the new semantics require.

## Migration implications

None.

## Example changes

None.

## Conformance changes

`conf.thread-vec-of-resources-dropped` and
`conf.thread-vec-of-resources-returned` (`spec/conformance.md` §12),
with file cases in `impl/conformance/19-concurrency/`.

## Future implementation implications

After this record `[Destroy-No-Authority]` has no reachable instance
that an earlier check does not report first: a binding moved to
another thread is stale before any destroy; a destroy through a
reference fails `solitary` while the owner's path is live; a reclaimed
object is destroyed by the thread that re-attached it, which now holds
its authority; a double destroy is `diag.stale-binding`. An
implementation need not track the performer of authority to conform.

## Prior-art status

Rust moves a `Vec<T>`'s elements with the `Vec` because ownership is
structural through the type; here the structure is invisible below a
raw pointer, so the trust transition (`reclaim`) is where ownership is
re-established — the same place the invariant already located it.

## Revisit conditions

- A demonstrated program where a thread reclaims and destroys an
  object another thread still expects to own: that is a violation of
  `[Reclaim]`'s trusted side-condition ("whose destroy obligation is
  not tracked anywhere else"), not a case for re-introducing the check.
- Option B becomes attractive if `Σ` ever gains an ownership link
  through raw pointers for another reason.
