# CHG-0015 — Cross-Thread Destroy-Authority Transfer for Joined/Discarded Resource Results

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: inv.resource-authority, state.authority, rule.conc.join, rule.resauth.transfer, rule.value-object.object-establish
Affects: rule.conc.join, conf.spawn-join-resource-result

## Problem / motivation

Checking `[Handle-Destructor]`'s `if …/if …` shape
for well-definedness. Deriving it against a spawned thread
whose body returns a *resource* (untested by any existing case — every
`spec/19` conformance row returns `i32`) exposed that neither `[Join]`
nor `[Handle-Destructor]` ever re-keys the result's destroy authority
from the worker thread (which established it) to the thread now
receiving it. `state.authority` is keyed by `Performer` (`spec/04`
§1), granted by `rule.value-object.object-establish` to whichever
thread executes `establish` — the worker, for an object created inside
a spawned body and returned out of it. Without a transfer, the first
attempt to `drop` a joined resource, or to let an unjoined handle's
resource result be swept automatically, would find
`authority(joiner, destroy, o, Σ)` undefined and fault
`[Destroy-No-Authority]` — meaning a spawn/join pair could never
actually clean up a resource-typed result at all.

## What changed

`spec/19` 1.5.0: `[Join]` re-keys `authority(ℓ, destroy, o)` from
`ℓ'` (the worker) to `ℓ` (the joiner) for `o ∈ objs-in(r_b)`, mirroring
exactly the mechanism `rule.resauth.transfer`'s `ℓ2 ≠ ℓ` case already
uses for `rule.conc.spawn`'s *argument*-passing direction (into the
worker). `[Handle-Destructor]`'s discard branch does the same re-key,
to whichever thread is running the destructor, before discarding.
Composite sub-obligations `(o, path)` need no corresponding change:
`rule.resauth.destroy-composite`'s `[Destroy-Composite]` does not gate
on per-thread authority for sub-resources at all (it unconditionally
sets `consumed := true` for each), so the cross-thread mismatch is
specific to the *top-level* object's authority, which `objs-in(r_b)`
names directly (it is always a singleton set or empty).

`conf.spawn-join-resource-result` (`spec/examples.md`
`ex.e2e-spawn-join-resource-result`) derives both paths: an explicit
`drop` of a joined `Vec<i32>`, and an unjoined handle's automatic
discard of a second one, showing each would fault
`diag.no-destroy-authority` without the fix and succeeds with it.

## Rule changes

`spec/19` 1.4.0 → 1.5.0: `[Join]`, `[Handle-Destructor]`
(`rule.conc.join`). Semantics-completing: `objs-in(r_b)` is empty for every plain
(non-resource) result, so both added clauses are vacuous for every
existing conformance case (all return `i32`); no existing outcome
changes.

## Affected invariants

`inv.resource-authority`'s single-holder discipline (D-0003) is
restated to hold across the one place it previously did not:
authority following a value across a thread boundary via `join` or an
automatic handle sweep, exactly as it already did for `spawn`'s
argument-passing direction.

## Dependency impact

None on existing entities beyond `rule.conc.join`'s own rule bodies.

## Compatibility classification

Semantics-completing, not source-breaking. No previously well-formed
program's observable behavior changes; a program that previously would
have faulted `diag.no-destroy-authority` on a joined/discarded
resource-typed thread result now completes normally instead (no such
program appears in the existing conformance corpus).

## Migration implications

None.

## Example changes

`ex.e2e-spawn-join-resource-result` added (`spec/examples.md` 3.8.0).

## Conformance changes

`conf.spawn-join-resource-result` added (`spec/conformance.md`
3.10.0); `conf.destroy-composite-multi-field-order` also added in the
same pass (a `spec/07` §4 ordering confirmation, unrelated rule, no
change needed there — see `spec/AUDIT-STATUS.md`'s eighth pass). No
existing row changed.

## Future implementation implications

An implementation whose authority bookkeeping is per-thread (e.g., a
thread-local ownership tracker used only for debug-mode double-destroy
detection) must propagate a joined or swept resource's ownership record
to the receiving thread, not merely hand over the bytes.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

None outstanding from this record. See
`spec/AUDIT-STATUS.md`'s eighth pass for the other three
prose-stated-algorithm confirmations from the same pass (no rule
change needed for those).
