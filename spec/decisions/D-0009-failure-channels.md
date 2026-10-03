# D-0009 — Two Failure Channels: Checked Faults vs. Fallible Results

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §11, §12, §17
Depends on: D-0008

## Problem

`disposition: checked` rules (`↛`, `diag.*`) and `disposition: fallible`
rules (`Result<T,E>`, `spec/16-aggregates.md` §3) are both already
`ACCEPTED`, but their relationship was never fixed: can a `↛` fault be
caught and recovered from (exception-like unwinding), or is recovery
only ever available through `fallible`'s explicit `Result`?

## Candidate mechanisms

1. **Catchable checked faults** (exception-like): a `↛` can be caught by
   an enclosing construct and converted into ordinary control flow.
   Rejected: this would make `checked` a *second* recoverable-failure
   mechanism alongside `fallible`, violating §9's preference for one
   orthogonal mechanism per concern — every recoverable-failure call
   site would need to consider two different propagation shapes for
   what is semantically the same concern.
2. **Fail-fast: `↛` is never caught; it always terminates the program**,
   after running `Block-Exit`'s sweep (D-0008) in every enclosing frame
   as it unwinds (best-effort external cleanup — closing files,
   releasing OS-visible locks — since those have consequences beyond
   the terminating process, unlike plain memory). Recoverable failure is
   available *only* via `fallible`'s `Result<T,E>`, which a caller must
   explicitly construct and propagate. **Selected.**

## Selected design

- `disposition: checked` faults are fatal: unwind (running `Block-Exit`
  for every enclosing frame, reusing D-0008's mechanism rather than
  inventing a second one) then terminate the program, reporting the
  triggering `diag.*`.
- `disposition: fallible` operations are the *only* recoverable-failure
  channel; a caller receives a `Result<T,E>` and must explicitly `match`
  it (`spec/16-aggregates.md` §3) or propagate it (§2 of this artifact's
  realization).

## Rejected alternatives

Catchable/exception-like checked faults (1).

## Semantic rationale

A `checked` fault means a required invariant could not be established —
exactly the category Master Instructions §6 says must never have a false
claim "become trusted." Allowing it to be caught and program execution
to continue would reintroduce exactly that risk one layer up (the
catching code now must reason about a program state where an invariant
check already failed). Keeping it fatal preserves the guarantee
absolutely; `fallible` remains the fully general, explicit answer for
every case where failure is an expected, recoverable outcome rather than
an invariant violation.

## Usability / Explainability / Implementation-feasibility

Usability: one recoverable-failure idiom (`Result`), not two competing
ones. Explainability: a `↛` diagnostic is unambiguous — the program is
ending, not "maybe continuing under a handler." Implementation-
feasibility: unwinding via the already-defined `Block-Exit` needs no new
mechanism.

## Compatibility / Prior-art / Invariant traceability

None yet impacted. Reuses D-0008's unwinding mechanism rather than
introducing exception tables or similar; not adopted for familiarity —
independently the minimal extension of already-accepted mechanisms.
Traces to every invariant with a `checked` disposition entry.

## Revisit conditions

Revisit only if a demonstrated need arises for partial recovery from a
specific, bounded class of checked fault (e.g. arithmetic overflow
specifically) without full program termination — the expected resolution
would be moving that specific operation to the `fallible` family
(already available, e.g. `checked_add`), not making `↛` catchable.

## Candidate-set review (`CHG-0024`)

`spec/AUDIT-2.md` B-26 flagged this decision's candidate set as thin
(one alternative). Reviewed, not expanded: catchable checked faults
(1) was eliminated by direct appeal to an already-`ACCEPTED` doctrine
section (§9's one-orthogonal-mechanism-per-concern), not by a shallow
search — introducing a second recoverable-failure channel is exactly
what §9 already rules out, leaving no live third candidate between
"catchable" and "fatal" once that constraint applies. Five
verification passes since have found no case where fail-fast checked
faults produced a wrong or awkward outcome. The human owner reviewed
this finding and closed it without directing new candidates be added.
