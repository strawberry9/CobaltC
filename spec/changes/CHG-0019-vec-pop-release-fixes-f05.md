# CHG-0019 — `Vec::pop` Releases Its Slot; Closes Finding F-05

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §12, §20, §21
Depends on: inv.alias-validity, inv.temporal-validity, rule.trust.rawptr, rule.stdlib.vec, D-0017
Affects: rule.stdlib.vec, rule.trust.rawptr, ex.e2e-vec-pop-push-reuse-stale-ref, conf.e2e-vec-pop-push-reuse-stale-ref

## Problem / motivation

`spec/AUDIT-STATUS.md`'s seventh pass recorded finding F-05, escalated
rather than patched at the time: a live `Vec::index_shared` reference
into a *plain*-typed element survives `Vec::pop` (its `[Rawptr-Read]`
correctly leaves the reclaimed object alone, since reading owns
nothing) and is then silently overwritten by a `Vec::push` reusing the
same slot, whose `[Rawptr-Write]` trusted condition explicitly
exempted the cells' own `reclaimed-at(addrs)` object from its
"no live derived path reaches them" requirement. No diagnostic, static
or dynamic, caught it — unlike a resource-typed element, where
`[Rawptr-Move-Out]` unconditionally ends the source, so the same reuse
is already caught as `diag.stale-binding`.

The human owner directed that this be decided rather than left open
further.

## Decision

Of the three candidate directions `spec/AUDIT-STATUS.md` recorded:
tighten the rule only (insufficient by itself — a `trusted` condition
being false has no runtime effect, so it would not stop the silent
corruption); widen the documented hazard to accept permanently silent
corruption on this one path (rejected — it is the only
`diag.stale-binding` case in the entire specification that would never
be dynamically caught, an inconsistency with every other instance of
the same diagnostic); or make `Vec::pop`'s plain-element path release
its slot, unifying its behavior with the resource-element path
(selected). The third is a minimal, mechanical use of an already-
existing primitive (`release`, already exposed unsafe in the prelude
and already used by `Vec::grow`) — not a new rule, not a weakened
guarantee, and it makes the diagnostic behavior on this hazard
identical to the reallocation case `CHG-0012` already closed.

## What changed

**`spec/21` 2.7.0** (`rule.stdlib.vec`): `Vec::pop` now binds the
popped value to a local (`auto x = *p;`) and calls
`release(reinterpret_ptr<u8>(p), sizeof<T>())` before returning
`Some(x)`. For a resource `T`, this is a no-op — `[Rawptr-Move-Out]`
already ended the reclaimed object as part of the move. For a plain
`T`, it ends any reclaimed object `index_shared`/`index_exclusive` may
have left there, so a live reference into a popped-and-reused slot is
now caught by the ordinary `[Object-End]` → `[Read-Stale]` path —
dynamically, at the reference's next use — exactly like the
reallocation case.

**`spec/20` 1.6.0** (`rule.trust.rawptr`): `[Rawptr-Read]`/
`[Rawptr-Write]`'s trusted conditions no longer exempt
`reclaimed-at(addrs)` from the "no live derived path reaches them"
requirement — the exemption was never actually needed by any correct
call site (verified: every existing `[Rawptr-Write]` site targets
either never-reclaimed cells, or, after the `Vec::pop` fix above,
cells whose reclaimed object has always already been released) and its
presence is exactly what let F-05's hazard through undetected. Now
symmetric with `[Rawptr-Move-In]`'s already-strict condition.

`ex.e2e-vec-pop-push-reuse-stale-ref`/
`conf.e2e-vec-pop-push-reuse-stale-ref` derive the fixed behavior end to end: the same hazard
`spec/AUDIT-STATUS.md` used to demonstrate F-05, now
`diag.stale-binding` (dynamic) instead of a silent, undiagnosed read of the wrong
value.

## Rule changes

`spec/21` 2.6.0 → 2.7.0 (`Vec::pop`'s body). `spec/20` 1.5.0 → 1.6.0
(`[Rawptr-Read]`, `[Rawptr-Write]`).

## Affected invariants

`inv.alias-validity`'s guarantee — no access observes a value changed
by a conflicting concurrent or intervening access — now holds across
the one raw-boundary path it previously did not (a `pop`-then-`push`
slot reuse); restated nowhere (no invariant text named the exemption
explicitly), closed by rule completion instead.

## Dependency impact

None on existing entities beyond the two rules changed.

## Compatibility classification

Semantics-*narrowing* for `unsafe` code only: a raw write that used to
be `trusted` (silently) at an address with a live reclaimed object is
no longer covered by `[Rawptr-Write]`'s stated trusted condition. No
existing conformance case relied on the removed exemption (verified by
re-deriving every existing `Vec::push`/`Rc::new` call site in the
corpus); the only *behavioral* change for any program is that
`hazard()`-shaped code now faults `diag.stale-binding` instead of
silently reading a corrupted value — strictly safer, not source-
breaking (no previously *correct* program's meaning changes; a
previously *silently wrong* program now visibly faults).

## Migration implications

None for safe code. `unsafe` library authors who wrote raw-pointer
code assuming an address could be written to while its own reclaimed
object was still alive (relying on the now-removed exemption) must
release or otherwise end that object first — `spec/21`'s own `Vec::pop`
is the reference example.

## Example changes

`ex.e2e-vec-pop-push-reuse-stale-ref` added (`spec/examples.md`
3.9.0).

## Conformance changes

`conf.e2e-vec-pop-push-reuse-stale-ref` added (`spec/conformance.md`
3.11.0); no existing row's outcome changed (verified: no existing row
exercises a write to a still-reclaimed address).

## Future implementation implications

A reference interpreter's `Vec::pop` must release the popped slot
regardless of element type, not merely for resources — the previous
"only resources need cleanup on pop" shortcut is unsound. No other
implementation obligation changes.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

None outstanding. F-05 is closed; `spec/IMPLEMENTATION-NOTES.md`'s
recommendation to resolve it before authorizing a reference
implementation is satisfied.
