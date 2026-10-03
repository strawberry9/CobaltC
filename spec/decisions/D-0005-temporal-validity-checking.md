# D-0005 — Temporal Validity Checking Strategy

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §11, §17 (Temporal
Validity), §12
Depends on: inv.temporal-validity, term.access-path

## Problem

`inv.temporal-validity` already has a **complete, sound dynamic
enforcement mechanism**: `rule.value-object.read`/`write`
(`spec/05-value-object-semantics.md`) check `alive(o, Σ)` on every
access, and `rule.value-object.object-end` invalidates every access path
targeting an ended object. No soundness gap exists. The open question is
purely one of §12's preference for static rejection where possible: how
much of this can be moved to compile time without first designing a full
static lifetime-tracking system (a large, independently-hard design
problem — cf. lifetime polymorphism in prior art, which took years to
mature elsewhere).

## Constraints

- §12: prefer static rejection wherever the fact is knowable without
  execution — but a dynamic `checked` disposition is an equally valid,
  closed-set outcome, not a fallback of last resort to be apologized
  for.
- §9: do not accept the first familiar solution, and do not spend
  complexity budget building a general mechanism (full lifetime
  polymorphism) before a concrete need forces it (§24).
- §18: a feature not yet derivable to `ACCEPTED` should be recorded
  `UNDER_INVESTIGATION`, not silently deferred with no record.

## Candidate mechanisms

1. **Dynamic-only.** Already fully specified by existing rules; sound;
   zero additional design cost. Leaves every reference-escape pattern as
   a runtime check rather than a compile-time diagnostic, which is
   correct but strictly worse than necessary for the common case (a
   reference to a local that provably cannot outlive it).
2. **Full static lifetime polymorphism** (parametrize reference types by
   a lifetime, check subtyping/outlives relations through function
   signatures and structures). Would eliminate the dynamic check for
   nearly all cases, but is a large, independently-difficult mechanism
   requiring its own multi-stage derivation, and depends on Function
   Semantics and Aggregates (not yet designed) to even state signatures
   over. Not derivable to `ACCEPTED` at this point in the design order
   without either forward-referencing undesigned artifacts or
   under-deriving a mechanism this consequential. Recorded as
   `feat.static-lifetime-tracking`, status `UNDER_INVESTIGATION`
   (`spec/registry/features.md`) — explicitly not rejected, not
   accepted, revisited once Function Semantics and Aggregates exist.
3. **Static lexical-scope containment (structural, non-polymorphic)**: a
   reference formed from an object `o` is statically rejected if any of
   its *lexically visible* uses occur outside `o`'s enclosing lexical
   scope (its establishing block). This requires no lifetime parameters
   and no cross-function propagation — it is a purely structural check
   over program text nesting, decidable without any new type-system
   machinery, and catches the dominant real-world case (returning a
   reference to a local, storing one past its block). It does **not**
   attempt to resolve cases requiring interprocedural reasoning (a
   reference passed into a function and stored somewhere the caller
   can't see) — those fall through to option 1's dynamic check.
   **Selected, layered on top of option 1.**

## Selected design

`inv.temporal-validity`'s enforcement is **two-layered**:

- **Baseline (already `ACCEPTED`, unchanged by this decision):** the
  dynamic `alive(o, Σ)` check in `Read`/`Write`
  (`spec/05-value-object-semantics.md`), `disposition: checked`. This
  alone is sound and complete for every case.
- **Static refinement (this decision):** lexical-scope containment,
  `disposition: rejected`, applied wherever a reference's uses are all
  lexically visible at the point of formation. Stated as a partial
  well-formedness obligation here (mirroring
  `rule.resauth.leak`'s treatment in `spec/07-resource-authority.md`),
  since "lexical scope"/"block" as a concrete structural entity is not
  yet defined — that belongs to Control Flow. This decision fixes the
  *rule*; Control Flow must instantiate it against real block structure.
- **Full lifetime polymorphism** is recorded, not designed:
  `feat.static-lifetime-tracking`, `UNDER_INVESTIGATION`.

## Rejected alternatives

None rejected outright — option 1 is retained as the baseline, option 2
is deferred (not rejected) pending prerequisite artifacts, option 3 is
added on top of option 1.

## Semantic rationale

Layering avoids the trap of either (a) shipping only a dynamic check
when a large, easy-to-state static case is available for free, or (b)
blocking on a full lifetime system before any static rejection exists.
The lexical rule is a strict, structurally-decidable subset of what a
full lifetime system would eventually catch statically, so adopting it
now does not foreclose option 2 later — it can only ever *shrink* the
set of cases lifetime tracking would need to additionally cover.

## Usability/Explainability/Implementation-feasibility

Usability: local, non-escaping reference use (the common case) gets a
compile-time diagnostic; escaping/stored references fall back to a
runtime check rather than being rejected outright, which is more
permissive than a design that required full lifetime annotations
everywhere. Explainability: a lexical-containment violation names the
exact enclosing scope and escape point. Implementation-feasibility:
lexical nesting is directly available from program structure; no new
analysis machinery is required for the static layer.

## Compatibility impact

None yet.

## Prior-art status

The dynamic baseline and the lexical special case are both derived
directly from already-`ACCEPTED` mechanisms; full lifetime polymorphism
is prior art acknowledged as a strong existing solution but deliberately
not adopted yet, since adopting it now would mean designing it out of
sequence, before its prerequisites exist (§7: do not force novelty, but
do not adopt a mechanism before its own dependencies are derived either).

## Invariant traceability

Realizes `inv.temporal-validity`'s static-refinement layer;
`feat.static-lifetime-tracking` is traced to the same invariant as
future work.

## Revisit conditions

Revisit `feat.static-lifetime-tracking` once Function Semantics and
Aggregates exist; revisit the lexical rule if Control Flow's concrete
block structure does not admit a clean containment check.
