# CHG-0011 — Fixes from Deriving Four End-to-End Programs

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: inv.resource-authority, inv.temporal-validity, inv.trust-transition, inv.arith.range-validity
Affects: inv.resource-authority, state.temp-scope-stack, rule.arith.convert, rule.type.expected, rule.agg.match, rule.trust.rawptr, ex.e2e-threads-mutex, ex.e2e-vec-nested-realloc, ex.e2e-rc-resource-payload, ex.e2e-propagate-chain, conf.e2e-threads-mutex-total, conf.e2e-vec-nested-realloc, conf.e2e-rc-resource-payload, conf.e2e-propagate-chain

## Problem / motivation

Master Instructions §20 asks for implementation-feasibility analysis:
can an implementer determine every semantic decision from the
normative artifacts alone? After `CHG-0010` every conformance row
derived, but each row is a few statements. Four whole programs were
written to exercise the interactions the rows do not — two threads
sharing a mutex, a `Vec<Vec<i32>>` that reallocates with resource
elements inside, an `Rc` whose payload is a resource, and a `?` chain
returning out of a `match` arm inside a loop — and derived from
`[Program]` to `[Terminate-Ok]`. Seven decisions could not be made
from the rules as written. Each is closed here; the programs are
recorded as examples with their derivations as conformance rows so
the analysis is repeatable.

## Affected entities and what changed

### E-01 · `state.temp-scope-stack` (`spec/04` 1.2.0)
**Previous:** `current-scope(ℓ)` was `head` of the stack, undefined
when empty. A spawned thread binds its parameters (`[Spawn]` →
`[Store-Binding-Value]` → establishment) before its body opens any
statement scope, so every parameter object's `temp-scope` was
undefined. **New:** `⊥` on an empty stack; `⊥` matches no scope.

### E-02 · `rule.trust.rawptr` `[Rawptr-Move-Out]`, `[Release]` (`spec/20` 1.5.0)
**Previous:** `[Rawptr-Move-Out]` required an existing reclaimed
object at the cells. `Vec::grow` copies the buffer with `copy_raw` and
`deallocate`s the old one, whose `[Release]` ends the reclaimed
element objects; a later `Vec::pop` of a resource element therefore
had no rule. `[Release]`'s trusted condition ("no obligation the
author still needs") was false for `grow`, which needs every one of
them. **New:** `[Rawptr-Move-Out]` re-attaches lazily exactly as
`[Reclaim]` does; `[Release]`'s condition states the hand-over
(obligations discharged, or their cells copied to where a later
`[Reclaim]`/`[Rawptr-Move-Out]` re-establishes them).

### E-03 · `inv.resource-authority` (`spec/03` 1.1.0)
**Previous:** the single-owner clause allowed an obligation's owner
to be an open frame, an open statement scope, or a live container.
A reclaimed resource object (every `Vec` element reached by
`index_shared`) has none of these, so the invariant was violated by
the library's own normal operation. **New:** a fourth alternative —
a reclaimed object, owned by the `unsafe` author under
`discharge: trusted`.

### E-04 · `rule.agg.match` (`spec/16` 1.3.0)
**Previous:** the binder was stored "in a fresh arm frame" and the
body was "a block whose frame is the arm frame", with no rule pushing
the frame; whether `[Block-Enter]` pushed a second one decided how
`rule.control.unwind` folds a `return` inside an arm. **New:**
`[Match]` pushes `f_arm` and reduces the arm to
`block'(f_arm, stmt-scoped(e_i, keep))`.

### E-05 · `rule.arith.convert` `[Widen]`/`[Narrow-*]` (`spec/06` 1.3.0)
**Previous:** `[Widen]` required equal signedness, so `u8 → i32`,
which loses nothing, had to be spelled `narrow`. **New:** `[Widen]`
whenever the source domain is contained in the target's; `[Narrow-*]`
otherwise. `CHG-0010` D-03's cross-signedness `narrow` is unchanged
where the domains are not nested.

### E-06 · `rule.type.expected` (`spec/12` 1.6.0)
**Previous:** an expected type did not propagate through unary
operators, so `i64 x = -1;` typed `1` as `i32` and was rejected.
**New:** propagation through `-e`, `~e`, `!e`.

### E-07 · Examples and conformance (`spec/examples.md` 3.5.0, `spec/conformance.md` 3.6.0)
Four `ex.e2e-*` programs and four `conf.e2e-*` rows added; the rows
carry the full derivations.

## Affected invariants

E-03 restates `inv.resource-authority`'s ownership clause to cover a
state the rules already produced. E-01 and E-04 remove undefined
behavior in `Σ` bookkeeping that `inv.temporal-validity`'s statement-
end invalidation relies on. E-02 concerns `discharge: trusted`
conditions under `inv.trust-transition`. E-05/E-06 complete static
rules beneath `inv.arith.range-validity`.

## Dependency impact

No `Depends on`/`Affects` line changes in existing entities; the new
examples and rows depend on the rules they derive.

## Compatibility classification

Semantics-completing, not source-breaking. E-05 admits `widen`
between signednesses where the domains nest; no previously
well-formed program changes meaning.

## Migration implications

None.

## Example changes

Four examples added (E-07).

## Conformance changes

Four rows added (E-07); no existing row changed.

## Future implementation implications

A runtime represents "no current statement scope" for a thread
between parameter binding and its first statement; a `match` pushes
one frame for binder and body; raw-pointer move-out re-attaches
lazily; `widen` is decided by domain inclusion.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

Repeat this analysis with programs that exercise `Vec::pop` on a
reallocated buffer under a live element reference (expected:
`diag.stale-binding`, dynamic), a mutex holding a resource, and a
closure that captures an `Rc`; none is covered yet.
