# D-0008 — Automatic Scope-End Destruction

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §5, §9, §11, §17
Depends on: inv.resource-authority, D-0003

## Problem

`rule.resauth.leak` (`spec/07-resource-authority.md`) was left a partial
well-formedness obligation ("reject if an obligation survives frame
end") pending a concrete frame/scope definition. Control Flow is about
to supply that definition (`spec/14-control-flow.md`) and must decide
what actually happens to an undischarged destruction obligation at scope
end: static rejection, or automatic destruction.

## Candidate mechanisms

1. **Static rejection only** (spec/07's original framing): a flow
   analysis must prove every path destroys or transfers every
   obligation, else the program is ill-formed. Requires genuine
   flow-sensitive analysis across every branch.
2. **Automatic destruction of every obligation still held at scope end**
   (reverse-declaration order, mirroring stack unwind order): no flow
   analysis needed — whatever is still outstanding when the scope's
   block ends is destroyed then, deterministically. Explicit `destroy`/
   `transfer` remain fully available and simply mean "nothing left for
   the automatic step to do." **Selected.**

## Selected design

At block exit, for every `o ∈ Σ.destruction-obligations` whose
originating `Object-Establish-Resource` occurred within the exiting
block and which has not been destroyed or transferred out, the block
automatically invokes `rule.resauth.destroy` on it, in reverse
declaration order (last-established, first-destroyed) — the concrete
`rule.control.block-exit` rule is in `spec/14-control-flow.md` §2.

## Rejected alternatives

Static-only rejection (1) — not rejected as *wrong*, but strictly more
mechanism for a strictly worse guarantee: option 2 makes leak-freedom a
**structural** fact (nothing can be forgotten, because forgetting just
means "destroyed automatically instead of manually"), rather than a
fact a separate analysis pass must prove. This mirrors the same
by-construction pattern D-0004/`spec/08-alias-validity.md` §2 used for
`inv.alias-validity` and `spec/04-abstract-state.md` §3 used for
`inv.identity`.

## Semantic rationale

§5 requires "explicit resource management," which this satisfies at the
level that matters — authority, transfer, and destruction are all
explicit, programmer-controlled facts (D-0003) — while making the
*trigger point* for leftover obligations deterministic and automatic,
exactly analogous to how a stack frame's storage is deterministically
reclaimed at function return without that being considered "implicit"
in the GC sense. §9 favors this because it needs no separate
flow-sensitive leak analysis on top of the block structure Control Flow
already must define.

## Usability / Explainability / Implementation-feasibility

Usability: resources are never silently leaked by omission; a caller
who wants early destruction still calls `destroy` explicitly.
Explainability: the exact destruction point for any un-transferred
resource is its enclosing block's exit, always. Implementation-
feasibility: reverse-declaration-order destruction at a known program
point is directly implementable, no analysis required.

## Compatibility impact

Supersedes `rule.resauth.leak`'s static-rejection framing
(`spec/07-resource-authority.md`) — that artifact is still `PROVISIONAL`,
so this is an in-place revision, not a Change record.

## Prior-art status

Scope-based automatic destruction (RAII-shaped) is well-established
elsewhere; retained here because independently the strongest fit for
turning "leak-freedom" into a structural rather than analyzed guarantee,
not adopted for familiarity.

## Invariant traceability

Strengthens `inv.resource-authority`'s "leak" failure mode from
"statically checked" to "structurally impossible."

## Revisit conditions

Revisit if Function Semantics finds returning a resource from a function
(transferring its obligation to the caller) does not compose cleanly
with block-exit auto-destruction — the expected resolution is that a
`return` expression counts as an explicit transfer out of the block
before block-exit's automatic step runs, not a reversal of this
decision.

## Candidate-set review (`CHG-0024`)

`spec/AUDIT-2.md` B-26 flagged this decision's candidate set as thin
(one alternative). Reviewed, not expanded: static-only rejection (1)
was eliminated because it is strictly more mechanism (a full
flow-sensitive prover) for a strictly worse guarantee than automatic
destruction, per this record's own §"Rejected alternatives" — a
principled elimination, not an unconsidered gap. Five verification
passes since have found rule-completeness defects (e.g. `CHG-0019`'s
F-05) but never a case where automatic scope-end destruction itself
was the wrong mechanism. The human owner reviewed this finding and
closed it without directing new candidates be added.
