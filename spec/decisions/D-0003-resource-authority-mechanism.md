# D-0003 — Resource Authority and Destruction Mechanism

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §5, §7, §11, §17 (Resource
Authority), §12
Depends on: inv.resource-authority, inv.temporal-validity, inv.identity,
term.resource, term.authority

## Problem

`inv.resource-authority` (`spec/03-invariants.md`) requires every
operation on a resource to be backed by held, unconsumed authority, and
`rule.value-object.object-end` (`spec/05-value-object-semantics.md`) was
left an explicitly `trusted-unchecked` skeleton pending this decision.
CobaltC needs an actual mechanism for: who may create, hold, transfer,
and destroy a resource, and how the language prevents double-destruction
and leaks — without presupposing ownership, borrowing, GC, reference
counting, or regions (Master Instructions §7).

## Constraints

- §5: CobaltC must support "explicit resource management" and
  "deterministic low-level control," and must extend to operating-system
  interaction, raw storage, and hardware-facing programming, not memory
  alone.
- §8 priority order: invariant preservation and safety of valid safe
  programs outrank performance (11th); but predictable representation
  and deterministic control are stated §5 *requirements*, not
  performance nice-to-haves, so a mechanism that satisfies safety by
  imposing unpredictable representation or nondeterministic destruction
  timing is not automatically acceptable either.
- §9: prefer few orthogonal mechanisms; this decision should not
  introduce a mechanism that still leaves a *second* mechanism
  necessary for the closely related `inv.alias-validity` (next artifact)
  to reuse none of it.
- §17: function interfaces must "carry enough semantic information to
  preserve interprocedural guarantees without undocumented caller/callee
  agreements" — whatever mechanism is chosen must be expressible in a
  function signature.
- §12: every operation must land in the closed disposition set; a
  mechanism that can only ever be static (no escape) or only ever
  dynamic (no static prevention) is a worse fit than one that supports
  both, since real programs need both.

## Candidate mechanisms

1. **Tracing garbage collection.** Authority is implicit; destruction
   timing is determined by runtime reachability. Prevents
   use-after-destroy and double-destroy by construction for memory, but:
   destruction timing is not deterministic (conflicts with §5's
   "deterministic low-level control"); does not generalize to
   non-memory resources (file handles, locks, hardware buffers) without
   a separate finalization mechanism that reintroduces the same
   authority-tracking problem this decision exists to solve; requires a
   runtime capable of reachability analysis, in tension with "efficient
   native execution" and "predictable data representation." Rejected as
   the primary mechanism.

2. **Reference counting** (with or without cycle collection). Authority
   modeled as a holder count; destruction on count reaching zero is
   deterministic, unlike (1), and generalizes better to arbitrary
   resources. But: cycles are a genuine soundness gap for the
   "eventually destroyed exactly once" side of `inv.resource-authority`
   unless a cycle collector is added, which reintroduces (1)'s
   unpredictability for exactly the cyclic case; imposes representation
   overhead (a count field) on every resource regardless of whether the
   program wants it, in tension with "predictable data representation
   where requested"; is an *automatic* mechanism, not the "explicit
   resource management" §5 asks for. Rejected as the primary mechanism;
   not excluded as a possible opt-in library-level pattern once
   function interfaces and aggregates exist, since nothing here forbids
   building a counted-handle type out of the primary mechanism.

3. **Region-based allocation** (bulk deallocation at region end). Fast,
   deterministic, but coarse-grained: does not support an individual
   resource's authority/lifetime independent of its enclosing region,
   and does not naturally extend to a specific non-memory resource
   needing its own destroy point (e.g. one file handle closed earlier
   than its allocating region ends) without per-resource authority
   tracking anyway. Rejected as the primary mechanism; not excluded as
   a future allocation-strategy *representation* choice layered on top
   of the primary mechanism (a region can be modeled as one kind of
   resource whose destroy-authority, once consumed, ends every resource
   it was the origin-determinant for — compatible with, not competing
   with, option 4).

4. **Static authority tracking**: each resource's destroy-capable
   authority is tracked as a semantic fact
   (`state.authority`/`state.destruction-obligations`,
   `spec/04-abstract-state.md`) attached to a single current holder at a
   time; the static semantics tracks its flow through bindings and
   function calls and rejects a program that would use a resource after
   its authority was transferred away, destroy it twice, or leave a
   required destroy-obligation undischarged — wherever that flow is
   statically determinable (the common case). Where it is not (e.g. an
   authority decision made by a runtime branch), the same fact is
   checked dynamically (`discharge: dynamic`) rather than being
   unsupported. Zero mandatory per-resource representation overhead;
   destruction timing is exactly where the program says it is
   (deterministic); generalizes uniformly to any resource kind because
   `state.authority`/`state.destruction-obligations` are already
   resource-kind-agnostic (`ResourceId ⊇ Identity`, per
   `spec/04-abstract-state.md` §1); a function signature can state
   authority flow directly (taking authority, returning it, or not
   touching it), which is exactly the interprocedural information §17
   requires. **Selected.**

## Selected design

- **Authority is single-holder for the destroy-capable (exclusive)
  form**: at most one currently-valid holder can have unconsumed
  authority to destroy a given resource at a time. This is deliberately
  narrower than "all authority" — *weaker*, non-destroy-capable forms of
  authority (e.g. shared read access) are not required to be
  single-holder; their coexistence rules belong to the Alias-Validity
  and Concurrent Access artifact (next), which specializes
  `permitted(...)` using the same `state.access-paths`/`state.authority`
  substrate this decision fixes. This split is the concrete realization
  of the `inv.resource-authority` / `inv.alias-validity` separation
  already present in the Invariant Registry.
- **Transfer** moves destroy-capable authority from one holder to
  another, consuming it at the source and establishing it at the
  destination in one atomic step; a resource with transferred-away
  authority cannot be used through the source's access path
  afterward (this is `inv.temporal-validity` applied to the
  authority-losing access path, not a new invariant).
- **Destruction** consumes the destroy-capable authority (single-use,
  `state.authority`'s `consumed` field) and invokes
  `rule.value-object.object-end`'s bookkeeping — this decision replaces
  that rule's placeholder `trusted` side-condition (§7's next artifact,
  `spec/07-resource-authority.md`, does so concretely).
- **Leak detection** (destroy-obligation never discharged) and
  **double-destroy** (destroy attempted on already-consumed authority)
  are both checked against `state.destruction-obligations`/
  `state.authority` — `disposition: rejected` wherever statically
  determinable (the default target, per §12), `disposition: checked`
  otherwise.
- **Weakening** (destroy-capable → shared/weaker authority) and
  **explicit escapes** for cases static tracking cannot resolve
  (`disposition: trusted-unchecked`) are acknowledged as necessary but
  their concrete rules are deferred: weakening to the Alias-Validity
  artifact (it needs the `mode` vocabulary that artifact defines),
  explicit escapes to whichever construct needing one is designed later
  (Master Instructions §24: do not define a mechanism before a concrete
  need exists).

## Rejected alternatives

Tracing GC (1); reference counting as the primary mechanism (2, not
excluded as a derived library pattern); region-based bulk deallocation
as the primary mechanism (3, not excluded as a derived allocation
strategy).

## Semantic rationale

Static tracking is the only candidate that satisfies
`inv.resource-authority` in its strongest form — rejecting a violating
program before it runs — for the general case, while still admitting a
documented dynamic fallback for the residual cases §12 anticipates
(operations whose authority cannot be statically determined). It is also
the only candidate that shares its underlying substrate
(`state.authority`, `state.access-paths`) with the aliasing mechanism
the very next artifact must define, satisfying §9's preference for few
orthogonal mechanisms over introducing a second, unrelated bookkeeping
system for a closely related invariant.

## Usability implications

Programs must make destroy-authority transfer explicit at points where
it moves (assignment, function call/return, etc.); this is a real
authoring cost, paid in exchange for static leak/double-destroy
rejection. Concrete surface-syntax ergonomics are out of scope here
(§10: semantics before syntax) — this decision fixes the semantic
obligation, not how lightly it can be written.

## Explainability implications

A rejected program can be diagnosed precisely: `diag.*` entries (future,
Diagnostic Registry) for this area can name the exact resource, the
operation attempted, and which authority fact was missing or already
consumed, directly from `state.authority`/`state.destruction-obligations`.

## Implementation-feasibility implications

Tracking a single-holder authority fact through static control flow is a
standard, well-understood static-analysis problem class; an
implementation-feasibility analysis (§20) finds no gap requiring an
unresolved mechanism for the common case, and the dynamic fallback
covers the residual.

## Compatibility impact

None yet — first definition of this area; directly supersedes the
placeholder in `rule.value-object.object-end`.

## Prior-art status

The single-holder-with-explicit-transfer shape is independently derived
here from `inv.resource-authority` and the §5/§8/§9/§17 constraints
above; that this shape is also known elsewhere (affine/linear typing
disciplines) is retained per §7 as the strongest fit rediscovered, not
assumed as a starting point — no borrowing/lifetime-annotation mechanism
is adopted by this decision; that remains open for the next artifact to
derive on its own terms.

## Invariant traceability

Directly realizes `inv.resource-authority`; supersedes the placeholder
side-condition in `rule.value-object.object-end`
(`spec/05-value-object-semantics.md`); sets up but does not resolve
`inv.alias-validity` (weaker/shared authority, `permitted(...)`) or
`inv.concurrency-validity` (`synchronized(...)`).

## Revisit conditions

Revisit if the Alias-Validity artifact finds the single-holder/transfer
substrate insufficient to express shared authority without contradicting
this decision's shape, or if implementation-feasibility analysis later
finds a common, legitimate authority-flow pattern that neither static
tracking nor the dynamic fallback can express without
`disposition: unsupported`.
