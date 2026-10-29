# CHG-0020 — Minimal I/O `extern` Surface Added to the Prelude

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §18, §21
Depends on: rule.trust.extern-call, rule.stdlib.prelude, inv.trust-transition, term.conformance
Affects: rule.stdlib.prelude, feat.minimal-io-extern-surface

## Problem / motivation

`rule.fn.program`'s "observable behavior" is defined as the
termination outcome plus the sequence of `extern` calls a program
performs (`spec/15` §7), but `spec/21` declared no `extern` function
at all. No CobaltC program could actually produce an observable
`extern` call, so every conformance case's "outcome" was a value or
diagnostic an implementer had to trust the formalism for, never
something a real compiled/run program could print or a test harness
could capture from outside the process. `CHG-0017` prepared
`feat.minimal-io-extern-surface` as an open design item — a genuine
project-scope question (Master Instructions §1), not resolvable from
the invariants alone — with three candidate directions and left it
`UNDER_INVESTIGATION` for the human owner to decide.

The human owner directed a decision: option (a).

## Decision

Of the three candidates `spec/registry/features.md` recorded:
declare one minimal `extern fn` in the prelude (selected); leave
`extern` entirely user-declared per program, with no prelude instance
(rejected — leaves the corpus itself unable to demonstrate an
observable call uniformly); or stay out of scope permanently, verifying
conformance by hand-derivation only (rejected — the smaller-cost option
was available and directly serves Master Instructions §26's success
criterion). Option (a) is additive, requires no new rule
(`rule.trust.extern-call`'s `[Extern-Call]` already governs any
`FfiType` signature), and costs nothing beyond one prelude table row.

## What changed

**`spec/21` 2.8.0** (`rule.stdlib.prelude`, §0): the prelude now
declares

    extern fn write(rawptr<u8> buf, usize len) : isize;

as a named intrinsic instance, resolved exactly like any other
`extern fn` by `rule.trust.extern-call`'s `[Extern-Call]` (`spec/20`
§3). `isize`, `usize`, and `rawptr<u8>` are all members of `FfiType`,
so no grammar or typing rule needed to change. `write` carries the
same `disposition: trusted-unchecked` as every other `extern`
call — the callee honoring the declared signature remains a trusted
condition, discharged nowhere by this specification, exactly as
`rule.trust.extern-call` already states for any `extern fn`.

**`spec/registry/features.md` 1.2.0**: `feat.minimal-io-extern-surface`
moved `UNDER_INVESTIGATION → ACCEPTED`, citing this record.

## Rule changes

None. `rule.trust.extern-call` (`spec/20`) is unchanged; `spec/21`
2.7.0 → 2.8.0 adds one prelude declaration, no new or modified rule.

## Affected invariants

None restated. `inv.trust-transition` already governs any `extern`
call's result as an unchecked claim (`spec/AUDIT-STATUS.md`); `write`
is a new *instance* of that boundary, not a new invariant or a change
to how the existing one is stated or enforced.

## Dependency impact

`feat.minimal-io-extern-surface` gains a `Depends on` citation of this
record; `rule.stdlib.prelude` gains a `Depends on` citation of
`rule.trust.extern-call`. No existing entity's meaning changes.

## Compatibility classification

Purely additive. No existing conformance case, example, or program's
behavior changes — `write` is a new prelude name that did not exist
before; nothing previously in scope could have collided with it
(`extern fn write` was not reserved).

## Migration implications

None. A program that already declared its own `extern fn write` with
a different signature in the same unqualified scope would now
conflict with the prelude name — the same shadowing/redeclaration
rule that already applies to any other prelude intrinsic
(`rule.module.resolve`, `spec/17`) governs this identically; no new
rule was needed to cover it.

## Example changes

None added by this record. A future example or conformance case may
now route an observable value through `write` where before it could
only assert a value or diagnostic by hand-derivation; none is required
by this decision itself.

## Conformance changes

None added by this record.

## Future implementation implications

A reference interpreter must provide a real OS binding for `write`
(e.g. a `write(2)`-shaped syscall or equivalent) — the calling
convention and linking mechanism remain outside this specification's
scope, exactly as `rule.trust.extern-call` already disclaims for any
`extern fn`. This is the first prelude entry that requires such a
binding to be useful; `allocate`/`deallocate` and the other
intrinsics did not.

## Prior-art status

Every systems language provides some minimal write-to-descriptor
primitive; this brings CobaltC's prelude in line with that norm rather
than leaving the mechanism (`extern`) present with no standard
instance.

## Revisit conditions

None outstanding. `feat.minimal-io-extern-surface` is closed; the
signature chosen (`write(rawptr<u8>, usize) : isize`) may be revisited
if a future need demonstrates it is insufficient (e.g. a second I/O
primitive), but no such need is recorded.
